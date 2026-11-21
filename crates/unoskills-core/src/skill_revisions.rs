use anyhow::{Context, Result, ensure};
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;
use walkdir::WalkDir;

use super::{UnoSkillsManager, safe_scoped_target};
use crate::domain::{
    DeployMode, DeploymentImpact, DeploymentImpactKind, Skill, SkillUpdateResult, SourceType,
};
use crate::parser::{hash_directory, is_valid_skill_dir, parse_skill_dir, sanitize_slug};
use crate::source::{ResolvedSource, fetch_git_skill, resolve_source_ref};
use crate::sync::{
    TargetState, classify_target, copy_dir_recursive, ensure_dst_not_overlapping_src,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillFileChangeKind {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillFileChange {
    pub path: PathBuf,
    pub kind: SkillFileChangeKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillRevisionPreview {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub rollback: bool,
    pub files: Vec<SkillFileChange>,
    pub deployments: Vec<DeploymentImpact>,
}

pub struct PreparedSkillRevision {
    preview: SkillRevisionPreview,
    staging: TempDir,
    central_path: PathBuf,
    base_hash: String,
    candidate_hash: String,
    source_ref: Option<String>,
    previous_source_branch: Option<String>,
    previous_source_subpath: Option<String>,
    source_branch: Option<String>,
    source_subpath: Option<String>,
    targets: Vec<PlannedTarget>,
}

impl PreparedSkillRevision {
    pub fn preview(&self) -> &SkillRevisionPreview {
        &self.preview
    }
}

#[derive(PartialEq, Eq)]
struct PlannedTarget {
    impact: DeploymentImpact,
    copy_baseline: Option<CopyBaseline>,
}

#[derive(PartialEq, Eq)]
struct CopyBaseline {
    hash: String,
    parent: PathBuf,
}

struct TargetRecord<'a> {
    agent: &'a str,
    mode: DeployMode,
    hash: &'a str,
    scope_valid: bool,
}

impl UnoSkillsManager {
    pub(crate) fn lock_skills(&self) -> Result<File> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root_dir.join(".skills.lock"))?;
        lock.lock().context("Could not lock the skill library")?;
        Ok(lock)
    }

    fn revisions_dir(&self) -> PathBuf {
        self.skills_dir.join(".skill-revisions")
    }

    pub(crate) fn previous_revision_path(&self, skill_id: &str) -> PathBuf {
        self.revisions_dir()
            .join(hex::encode(Sha256::digest(skill_id.as_bytes())))
    }

    pub fn has_skill_rollback(&self, skill_id: &str) -> bool {
        let path = self.previous_revision_path(skill_id);
        self.has_revisions_dir()
            && fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir())
            && is_valid_skill_dir(&path)
    }

    pub(crate) fn has_revisions_dir(&self) -> bool {
        fs::symlink_metadata(self.revisions_dir()).is_ok_and(|meta| meta.is_dir())
    }

    pub fn prepare_skill_update(
        &self,
        skill_id_or_slug: &str,
    ) -> Result<Option<PreparedSkillRevision>> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        ensure!(
            skill.source_type != SourceType::Adopted,
            "Skill '{}' has no upstream source to update from",
            skill.name
        );
        let source_ref = skill
            .source_ref
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .with_context(|| format!("Skill '{}' has no recorded source_ref", skill.name))?;
        let resolved = resolve_source_ref(source_ref)?;
        let (staging, branch, subpath) = match resolved {
            ResolvedSource::Local(path) => (
                self.stage_revision(&path)?,
                skill.source_branch.clone(),
                skill.source_subpath.clone(),
            ),
            ResolvedSource::Git {
                repo_url,
                branch,
                subpath,
                ..
            } => {
                let branch = branch.or_else(|| skill.source_branch.clone());
                let subpath = subpath.or_else(|| skill.source_subpath.clone());
                let proxy = self.http_proxy();
                let result = fetch_git_skill(
                    &repo_url,
                    branch.as_deref(),
                    subpath.as_deref(),
                    proxy.as_deref(),
                )?;
                (self.stage_revision(&result.skill_dir)?, branch, subpath)
            }
        };

        let _lock = self.lock_skills()?;
        let current = self
            .store
            .find_skill(&skill.id)?
            .context("Skill not found")?;
        ensure!(
            current.source_ref == skill.source_ref
                && current.source_subpath == skill.source_subpath
                && current.source_branch == skill.source_branch,
            "STALE_SKILL_REVISION: The skill source changed while checking for updates."
        );
        self.prepare_revision(current, staging, branch, subpath, false)
    }

    pub fn prepare_skill_rollback(
        &self,
        skill_id_or_slug: &str,
    ) -> Result<Option<PreparedSkillRevision>> {
        let _lock = self.lock_skills()?;
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        if !self.has_skill_rollback(&skill.id) {
            return Ok(None);
        }
        let staging = self.stage_revision(&self.previous_revision_path(&skill.id))?;
        let branch = skill.source_branch.clone();
        let subpath = skill.source_subpath.clone();
        self.prepare_revision(skill, staging, branch, subpath, true)
    }

    fn stage_revision(&self, source: &Path) -> Result<TempDir> {
        ensure!(
            is_valid_skill_dir(source),
            "Invalid skill directory: {}",
            source.display()
        );
        fs::create_dir_all(self.revisions_dir())?;
        ensure!(
            fs::symlink_metadata(self.revisions_dir())?.is_dir(),
            "Invalid skill revisions directory"
        );
        let staging = tempfile::Builder::new()
            .prefix(".prepared-")
            .tempdir_in(self.revisions_dir())?;
        copy_dir_recursive(source, &staging.path().join("candidate"))?;
        ensure!(
            is_valid_skill_dir(&staging.path().join("candidate")),
            "The staged skill has no SKILL.md"
        );
        Ok(staging)
    }

    fn prepare_revision(
        &self,
        skill: Skill,
        staging: TempDir,
        source_branch: Option<String>,
        source_subpath: Option<String>,
        rollback: bool,
    ) -> Result<Option<PreparedSkillRevision>> {
        self.check_library_path(&skill)?;
        let candidate = staging.path().join("candidate");
        let base_hash = revision_hash(&skill.central_path)?;
        let candidate_hash = revision_hash(&candidate)?;
        if base_hash == candidate_hash {
            return Ok(None);
        }
        let files = compare_files(&skill.central_path, &candidate)?;
        ensure!(
            revision_hash(&skill.central_path)? == base_hash
                && revision_hash(&candidate)? == candidate_hash,
            "STALE_SKILL_REVISION: The skill changed while preparing the file list."
        );
        let targets = planned_targets(&skill, &base_hash, &candidate_hash);
        Ok(Some(PreparedSkillRevision {
            preview: SkillRevisionPreview {
                id: Uuid::new_v4().to_string(),
                skill_id: skill.id,
                skill_name: skill.name,
                rollback,
                files,
                deployments: targets.iter().map(|target| target.impact.clone()).collect(),
            },
            staging,
            central_path: skill.central_path,
            base_hash,
            candidate_hash,
            source_ref: skill.source_ref,
            previous_source_branch: skill.source_branch,
            previous_source_subpath: skill.source_subpath,
            source_branch,
            source_subpath,
            targets,
        }))
    }

    fn check_library_path(&self, skill: &Skill) -> Result<()> {
        ensure!(
            sanitize_slug(&skill.slug).as_deref() == Some(&skill.slug)
                && self.skills_dir.join(&skill.slug).canonicalize()?
                    == skill.central_path.canonicalize()?,
            "STALE_SKILL_REVISION: The skill directory is outside its recorded library location."
        );
        Ok(())
    }

    pub fn apply_skill_revision(
        &self,
        mut prepared: PreparedSkillRevision,
    ) -> Result<SkillUpdateResult> {
        let _lock = self.lock_skills()?;
        let mut skill = self
            .store
            .find_skill(&prepared.preview.skill_id)?
            .context("Skill not found")?;
        self.check_library_path(&skill)?;
        let candidate = prepared.staging.path().join("candidate");
        ensure!(
            self.has_revisions_dir()
                && skill.central_path == prepared.central_path
                && skill.source_ref == prepared.source_ref
                && skill.source_branch == prepared.previous_source_branch
                && skill.source_subpath == prepared.previous_source_subpath
                && revision_hash(&skill.central_path)? == prepared.base_hash
                && revision_hash(&candidate)? == prepared.candidate_hash
                && planned_targets(&skill, &prepared.base_hash, &prepared.candidate_hash)
                    == prepared.targets
                && (!prepared.preview.rollback
                    || revision_hash(&self.previous_revision_path(&skill.id))?
                        == prepared.candidate_hash),
            "STALE_SKILL_REVISION: The skill or its deployments changed. Check again before applying."
        );

        let metadata = parse_skill_dir(&candidate);
        skill.name = metadata.name.unwrap_or_else(|| skill.slug.clone());
        skill.description = metadata.description;
        skill.estimated_tokens = metadata.estimated_tokens;
        if prepared.preview.rollback || !metadata.tags.is_empty() {
            skill.tags = metadata.tags;
        }
        skill.content_hash = Some(prepared.candidate_hash.clone());
        skill.updated_at = Utc::now().timestamp();
        skill.source_branch = prepared.source_branch;
        skill.source_subpath = prepared.source_subpath;

        let previous = self.previous_revision_path(&skill.id);
        let older = prepared.staging.path().join("older");
        let had_previous = move_if_present(&previous, &older)?;
        if let Err(error) = replace_directory(&skill.central_path, &candidate, &previous, || {
            ensure!(
                revision_hash(&previous)? == prepared.base_hash,
                "STALE_SKILL_REVISION: The installed files changed while applying the revision."
            );
            self.store.upsert_skill(&skill)
        }) {
            if had_previous && let Err(restore) = fs::rename(&older, &previous) {
                prepared.staging.disable_cleanup(true);
                return Err(error).context(format!(
                    "Previous snapshot preserved at {}: {restore}",
                    older.display()
                ));
            }
            return Err(error);
        }

        let mut refreshed_copies = 0;
        let mut deployment_issues = Vec::new();
        for target in prepared.targets {
            let mut impact = target.impact;
            match impact.kind {
                DeploymentImpactKind::RefreshCopy => {
                    let baseline = target
                        .copy_baseline
                        .expect("refreshable copy has a baseline");
                    if self
                        .refresh_revision_copy(&skill, &impact.path, &baseline)
                        .is_ok()
                    {
                        for dep in skill.project_deployments.iter_mut().filter(|dep| {
                            dep.mode == DeployMode::Copy && dep.target_path == impact.path
                        }) {
                            dep.copy_hash.clone_from(&skill.content_hash);
                        }
                        refreshed_copies += 1;
                    } else {
                        impact.kind = DeploymentImpactKind::RefreshFailed;
                        deployment_issues.push(impact);
                    }
                }
                DeploymentImpactKind::Missing
                | DeploymentImpactKind::Conflict
                | DeploymentImpactKind::RefreshFailed => deployment_issues.push(impact),
                DeploymentImpactKind::Current => {
                    if self
                        .store
                        .record_refreshed_project_copy(
                            &skill.id,
                            &impact.path,
                            skill.content_hash.as_deref(),
                        )
                        .is_ok()
                    {
                        for dep in skill.project_deployments.iter_mut().filter(|dep| {
                            dep.mode == DeployMode::Copy && dep.target_path == impact.path
                        }) {
                            dep.copy_hash.clone_from(&skill.content_hash);
                        }
                    } else {
                        impact.kind = DeploymentImpactKind::RefreshFailed;
                        deployment_issues.push(impact);
                    }
                }
                DeploymentImpactKind::Linked => {}
            }
        }
        Ok(SkillUpdateResult {
            skill,
            updated: true,
            refreshed_copies,
            deployment_issues,
        })
    }

    fn refresh_revision_copy(
        &self,
        skill: &Skill,
        path: &Path,
        baseline: &CopyBaseline,
    ) -> Result<()> {
        ensure_dst_not_overlapping_src(&skill.central_path, path)?;
        let parent = path.parent().context("Missing deployment directory")?;
        let target = baseline.parent.join(
            path.file_name()
                .context("Missing deployment directory name")?,
        );
        let mut staging = tempfile::Builder::new()
            .prefix(".unoskills-refresh-")
            .tempdir_in(&baseline.parent)?;
        let candidate = staging.path().join("candidate");
        copy_dir_recursive(&skill.central_path, &candidate)?;
        let previous = staging.path().join("previous");
        ensure!(
            parent.canonicalize()? == baseline.parent
                && classify_target(&skill.central_path, &target) == TargetState::RealDir
                && revision_hash(&target)? == baseline.hash
                && Some(revision_hash(&candidate)?.as_str()) == skill.content_hash.as_deref(),
            "TARGET_CONFLICT: The deployment changed: {}",
            path.display()
        );
        let result = replace_directory(&target, &candidate, &previous, || {
            ensure!(
                revision_hash(&previous)? == baseline.hash,
                "TARGET_CONFLICT: The deployment changed: {}",
                path.display()
            );
            self.store
                .record_refreshed_project_copy(&skill.id, path, skill.content_hash.as_deref())
        });
        if result.is_err() && previous.exists() {
            staging.disable_cleanup(true);
        }
        result
    }
}

fn move_if_present(path: &Path, destination: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            ensure!(
                meta.is_dir(),
                "Unexpected revision directory: {}",
                path.display()
            );
            fs::rename(path, destination)?;
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn replace_directory(
    current: &Path,
    next: &Path,
    previous: &Path,
    persist: impl FnOnce() -> Result<()>,
) -> Result<()> {
    fs::rename(current, previous)
        .with_context(|| format!("Could not preserve {}", current.display()))?;
    if let Err(error) = fs::rename(next, current) {
        fs::rename(previous, current)
            .with_context(|| format!("Original files preserved at {}", previous.display()))?;
        return Err(error.into());
    }
    if let Err(error) = persist() {
        fs::rename(current, next)
            .and_then(|()| fs::rename(previous, current))
            .with_context(|| format!("Original files preserved at {}", previous.display()))?;
        return Err(error);
    }
    Ok(())
}

fn checked_entries(directory: &Path) -> impl Iterator<Item = Result<walkdir::DirEntry>> {
    WalkDir::new(directory).into_iter().map(|entry| {
        let entry = entry?;
        ensure!(
            entry.file_name() != ".git"
                && (entry.file_type().is_dir() || entry.file_type().is_file()),
            "Unsupported file in skill directory: {}",
            entry.path().display()
        );
        Ok(entry)
    })
}

fn revision_hash(directory: &Path) -> Result<String> {
    ensure!(
        fs::symlink_metadata(directory)?.is_dir(),
        "Expected a skill directory: {}",
        directory.display()
    );
    for entry in checked_entries(directory) {
        entry?;
    }
    hash_directory(directory)
}

fn file_hashes(directory: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    checked_entries(directory)
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type().is_file() => Some((|| {
                let mut hash = Sha256::new();
                std::io::copy(&mut File::open(entry.path())?, &mut hash)?;
                Ok((
                    entry.path().strip_prefix(directory)?.to_owned(),
                    hash.finalize().to_vec(),
                ))
            })()),
            Err(error) => Some(Err(error)),
            _ => None,
        })
        .collect()
}

fn compare_files(current: &Path, candidate: &Path) -> Result<Vec<SkillFileChange>> {
    let before = file_hashes(current)?;
    let after = file_hashes(candidate)?;
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).collect();
    Ok(paths
        .into_iter()
        .filter_map(|path| {
            let kind = match (before.get(path), after.get(path)) {
                (None, Some(_)) => SkillFileChangeKind::Added,
                (Some(_), None) => SkillFileChangeKind::Removed,
                (Some(left), Some(right)) if left != right => SkillFileChangeKind::Modified,
                _ => return None,
            };
            Some(SkillFileChange {
                path: path.clone(),
                kind,
            })
        })
        .collect())
}

fn planned_targets(skill: &Skill, base_hash: &str, candidate_hash: &str) -> Vec<PlannedTarget> {
    let mut records: BTreeMap<&Path, Vec<TargetRecord<'_>>> = BTreeMap::new();
    for dep in &skill.deployments {
        records
            .entry(&dep.target_path)
            .or_default()
            .push(TargetRecord {
                agent: &dep.agent_key,
                mode: dep.mode,
                hash: base_hash,
                scope_valid: true,
            });
    }
    for dep in &skill.project_deployments {
        records
            .entry(&dep.target_path)
            .or_default()
            .push(TargetRecord {
                agent: &dep.agent_key,
                mode: dep.mode,
                hash: dep.copy_hash.as_deref().unwrap_or(base_hash),
                scope_valid: safe_scoped_target(dep).unwrap_or(false),
            });
    }
    records
        .into_iter()
        .map(|(path, entries)| {
            let (kind, copy_baseline) = if entries.iter().any(|record| !record.scope_valid) {
                (DeploymentImpactKind::Conflict, None)
            } else {
                match classify_target(&skill.central_path, path) {
                    TargetState::Absent => (DeploymentImpactKind::Missing, None),
                    TargetState::LinkToSource
                        if entries
                            .iter()
                            .all(|record| record.mode == DeployMode::Symlink) =>
                    {
                        (DeploymentImpactKind::Linked, None)
                    }
                    TargetState::RealDir
                        if entries.iter().all(|record| record.mode == DeployMode::Copy) =>
                    {
                        let candidate = revision_hash(path)
                            .ok()
                            .zip(path.parent().and_then(|parent| parent.canonicalize().ok()));
                        match candidate {
                            Some((hash, _)) if hash == candidate_hash => {
                                (DeploymentImpactKind::Current, None)
                            }
                            Some((hash, parent))
                                if entries.iter().all(|record| record.hash == hash) =>
                            {
                                (
                                    DeploymentImpactKind::RefreshCopy,
                                    Some(CopyBaseline { hash, parent }),
                                )
                            }
                            _ => (DeploymentImpactKind::Conflict, None),
                        }
                    }
                    _ => (DeploymentImpactKind::Conflict, None),
                }
            };
            let agents = entries
                .into_iter()
                .map(|record| record.agent.to_owned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            PlannedTarget {
                impact: DeploymentImpact {
                    path: path.to_owned(),
                    agents,
                    kind,
                },
                copy_baseline,
            }
        })
        .collect()
}
