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

use super::{UnoSkillsManager, safe_scoped_target};
use crate::domain::{
    DeployMode, DeploymentImpact, DeploymentImpactKind, Skill, SkillUpdateResult, SourceType,
};
use crate::parser::{
    checked_directory_hash as revision_hash, checked_entries, is_valid_skill_dir,
    managed_copy_fingerprint, parse_skill_dir, sanitize_slug,
};
use crate::source::{ResolvedSource, resolve_source_ref};
use crate::sync::{
    TargetState, classify_target, copy_baseline, copy_dir_recursive,
    ensure_dst_not_overlapping_src, paths_overlap, physical_target_path, rename_without_replacing,
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
    base_fingerprint: String,
    candidate_fingerprint: String,
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
pub(super) struct PlannedTarget {
    pub(super) impact: DeploymentImpact,
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
    hash: Option<&'a str>,
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
                let result =
                    self.fetch_git_skill(&repo_url, branch.as_deref(), subpath.as_deref())?;
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

    pub(super) fn apply_backup_revision(
        &self,
        skill: Skill,
        source: &Path,
        expected: &str,
    ) -> Result<SkillUpdateResult> {
        let branch = skill.source_branch.clone();
        let subpath = skill.source_subpath.clone();
        let staging = self.stage_revision(source)?;
        let prepared = self
            .prepare_revision(skill, staging, branch, subpath, false)?
            .context("The skill changed while reading the backup. Pull again.")?;
        ensure!(
            prepared.base_hash == expected,
            "STALE_SKILL_REVISION: The local skill changed while reading the backup"
        );
        self.apply_skill_revision(prepared)
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
        let candidate = staging.path().join("candidate");
        ensure_dst_not_overlapping_src(source, &candidate)?;
        copy_dir_recursive(source, &candidate)?;
        ensure!(
            is_valid_skill_dir(&candidate),
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
        let base_fingerprint = managed_copy_fingerprint(&skill.central_path)?;
        let candidate_fingerprint = managed_copy_fingerprint(&candidate)?;
        let files = compare_files(&skill.central_path, &candidate)?;
        ensure!(
            revision_hash(&skill.central_path)? == base_hash
                && revision_hash(&candidate)? == candidate_hash,
            "STALE_SKILL_REVISION: The skill changed while preparing the file list."
        );
        let targets = planned_targets(
            &skill,
            &candidate_fingerprint,
            &self.store.locked_project_target_paths()?,
        );
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
            base_fingerprint,
            candidate_fingerprint,
            source_ref: skill.source_ref,
            previous_source_branch: skill.source_branch,
            previous_source_subpath: skill.source_subpath,
            source_branch,
            source_subpath,
            targets,
        }))
    }

    pub(super) fn check_library_path(&self, skill: &Skill) -> Result<()> {
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
                && managed_copy_fingerprint(&skill.central_path)? == prepared.base_fingerprint
                && managed_copy_fingerprint(&candidate)? == prepared.candidate_fingerprint
                && planned_targets(
                    &skill,
                    &prepared.candidate_fingerprint,
                    &self.store.locked_project_target_paths()?
                ) == prepared.targets
                && (!prepared.preview.rollback
                    || managed_copy_fingerprint(&self.previous_revision_path(&skill.id))?
                        == prepared.candidate_fingerprint),
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
                managed_copy_fingerprint(&previous)? == prepared.base_fingerprint,
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

        Ok(self.refresh_revision_targets(skill, prepared.targets, &prepared.candidate_fingerprint))
    }

    pub(super) fn refresh_revision_targets(
        &self,
        mut skill: Skill,
        targets: Vec<PlannedTarget>,
        fingerprint: &str,
    ) -> SkillUpdateResult {
        let mut refreshed_copies = 0;
        let mut deployment_issues = Vec::new();
        for target in targets {
            let mut impact = target.impact;
            let recorded = match impact.kind {
                DeploymentImpactKind::RefreshCopy => {
                    let baseline = target
                        .copy_baseline
                        .expect("refreshable copy has a baseline");
                    let result =
                        self.refresh_revision_copy(&skill, &impact.path, &baseline, fingerprint);
                    if result.is_ok() {
                        refreshed_copies += 1;
                    }
                    result
                }
                DeploymentImpactKind::Current => (|| {
                    ensure!(
                        managed_copy_fingerprint(&impact.path)? == fingerprint,
                        "TARGET_CONFLICT: The deployment changed"
                    );
                    self.store
                        .record_refreshed_copy(&skill.id, &impact.path, Some(fingerprint))
                })(),
                DeploymentImpactKind::Missing
                | DeploymentImpactKind::Conflict
                | DeploymentImpactKind::RefreshFailed => {
                    deployment_issues.push(impact);
                    continue;
                }
                DeploymentImpactKind::Linked => continue,
            };
            if recorded.is_ok() {
                for dep in skill
                    .deployments
                    .iter_mut()
                    .filter(|dep| dep.mode == DeployMode::Copy && dep.target_path == impact.path)
                {
                    dep.copy_hash = Some(fingerprint.to_owned());
                }
                for dep in skill
                    .project_deployments
                    .iter_mut()
                    .filter(|dep| dep.mode == DeployMode::Copy && dep.target_path == impact.path)
                {
                    dep.copy_hash = Some(fingerprint.to_owned());
                }
            } else {
                impact.kind = DeploymentImpactKind::RefreshFailed;
                deployment_issues.push(impact);
            }
        }
        SkillUpdateResult {
            skill,
            updated: true,
            refreshed_copies,
            deployment_issues,
        }
    }

    fn refresh_revision_copy(
        &self,
        skill: &Skill,
        path: &Path,
        baseline: &CopyBaseline,
        fingerprint: &str,
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
                && managed_copy_fingerprint(&target)? == baseline.hash
                && managed_copy_fingerprint(&candidate)? == fingerprint,
            "TARGET_CONFLICT: The deployment changed: {}",
            path.display()
        );
        let result = replace_directory(&target, &candidate, &previous, || {
            ensure!(
                managed_copy_fingerprint(&previous)? == baseline.hash
                    && managed_copy_fingerprint(&target)? == fingerprint,
                "TARGET_CONFLICT: The deployment changed: {}",
                path.display()
            );
            self.store
                .record_refreshed_copy(&skill.id, path, Some(fingerprint))
        });
        if result.is_err() && previous.exists() {
            staging.disable_cleanup(true);
        }
        result
    }
}

pub(super) fn backup_deployment_issues(
    skill: &Skill,
    locked_paths: &[PathBuf],
) -> Result<Vec<DeploymentImpact>> {
    let fingerprint = managed_copy_fingerprint(&skill.central_path)?;
    Ok(planned_targets(skill, &fingerprint, locked_paths)
        .into_iter()
        .map(|target| target.impact)
        .filter(|impact| {
            matches!(
                impact.kind,
                DeploymentImpactKind::Missing | DeploymentImpactKind::Conflict
            )
        })
        .collect())
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
    rename_without_replacing(current, previous)
        .with_context(|| format!("Could not preserve {}", current.display()))?;
    if let Err(error) = rename_without_replacing(next, current) {
        rename_without_replacing(previous, current)
            .with_context(|| format!("Original files preserved at {}", previous.display()))?;
        return Err(error);
    }
    if let Err(error) = persist() {
        rename_without_replacing(current, next)
            .and_then(|()| rename_without_replacing(previous, current))
            .with_context(|| format!("Original files preserved at {}", previous.display()))?;
        return Err(error);
    }
    Ok(())
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

pub(super) fn planned_targets(
    skill: &Skill,
    candidate_fingerprint: &str,
    locked_paths: &[PathBuf],
) -> Vec<PlannedTarget> {
    let locked = |path: &Path| {
        if locked_paths.is_empty() {
            return false;
        }
        let physical = physical_target_path(path);
        locked_paths.iter().any(|target| {
            paths_overlap(target, path)
                || physical
                    .as_deref()
                    .is_some_and(|path| paths_overlap(target, path))
        })
    };
    let mut records: BTreeMap<&Path, Vec<TargetRecord<'_>>> = BTreeMap::new();
    for dep in &skill.deployments {
        if locked(&dep.target_path) {
            continue;
        }
        records
            .entry(&dep.target_path)
            .or_default()
            .push(TargetRecord {
                agent: &dep.agent_key,
                mode: dep.mode,
                hash: dep.copy_hash.as_deref().or(skill.content_hash.as_deref()),
                scope_valid: true,
            });
    }
    for dep in &skill.project_deployments {
        if locked(&dep.target_path) {
            continue;
        }
        records
            .entry(&dep.target_path)
            .or_default()
            .push(TargetRecord {
                agent: &dep.agent_key,
                mode: dep.mode,
                hash: dep.copy_hash.as_deref().or(skill.content_hash.as_deref()),
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
                        let candidate = managed_copy_fingerprint(path)
                            .ok()
                            .zip(path.parent().and_then(|parent| parent.canonicalize().ok()));
                        match candidate {
                            Some((hash, _)) if hash == candidate_fingerprint => {
                                (DeploymentImpactKind::Current, None)
                            }
                            Some((hash, parent))
                                if entries.iter().all(|record| {
                                    record
                                        .hash
                                        .and_then(|recorded| {
                                            copy_baseline(&skill.central_path, recorded)
                                        })
                                        .is_some_and(|baseline| baseline == hash)
                                }) =>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_staging_rejects_a_source_containing_the_library() {
        let source = tempfile::tempdir().unwrap();
        fs::write(source.path().join("SKILL.md"), "# Local source").unwrap();
        let manager = UnoSkillsManager::open_at(source.path().join("library")).unwrap();

        let error = manager.stage_revision(source.path()).unwrap_err();
        assert!(error.to_string().contains("refusing to avoid recursion"));
    }
}
