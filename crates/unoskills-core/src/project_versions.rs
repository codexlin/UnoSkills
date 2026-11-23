use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::skill_revisions::SkillFileChangeKind;
use super::{
    StackProjectTarget, UnoSkillsManager, inspect_stack_project_target, project_scope_directory,
    project_scope_key, project_skills_dir, safe_scoped_target, scoped_project_skills_dir,
};
use crate::domain::{DeployMode, ProjectDeployment, Skill};
pub(crate) use crate::parser::project_version_hash;
use crate::parser::{
    COPY_FINGERPRINT_PREFIX, TreeEntry, directory_tree, managed_copy_fingerprint, sanitize_slug,
};
use crate::sync::{
    DirectoryIdentity, RemovalBatch, ReplacePolicy, TargetState, classify_target, copy_baseline,
    copy_dir_recursive, deploy_skill_dir, ensure_dst_not_overlapping_src, paths_overlap,
    physical_target_path, rename_without_replacing, verify_managed_target,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectVersionIntent {
    Upgrade,
    Follow,
}

#[derive(Debug, Serialize)]
pub struct ProjectVersionChange {
    path: PathBuf,
    kind: SkillFileChangeKind,
    diff: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectVersionReview {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub intent: ProjectVersionIntent,
    pub current_hash: String,
    pub candidate_hash: String,
    pub files: Vec<ProjectVersionChange>,
}

#[derive(Debug, Serialize)]
pub struct ProjectConflictReview {
    pub path: PathBuf,
    pub kind: &'static str,
    pub open_folder: bool,
    pub pinned: bool,
    pub files: Option<Vec<ProjectVersionChange>>,
    pub detail: Option<String>,
    pub preview_id: Option<String>,
}

pub struct PreparedProjectConflict {
    pub id: String,
    project_id: String,
    skill_id: String,
    scope: PathBuf,
    agent_key: String,
    target: VersionTarget,
    source_fingerprint: String,
}

pub struct PreparedProjectVersion {
    review: ProjectVersionReview,
    project_id: String,
    agent_key: String,
    scope: PathBuf,
    targets: Vec<VersionTarget>,
    staging: TempDir,
}

impl PreparedProjectVersion {
    pub fn review(&self) -> &ProjectVersionReview {
        &self.review
    }
}

#[derive(Debug, PartialEq, Eq)]
enum VersionPlacement {
    Copy(DirectoryIdentity, String),
    Link(PathBuf),
}

#[derive(Debug, PartialEq, Eq)]
struct VersionTarget {
    path: PathBuf,
    parent: DirectoryIdentity,
    placement: VersionPlacement,
    version: String,
    ids: Vec<String>,
    locked: Option<String>,
    follow_mode: DeployMode,
}

impl VersionTarget {
    fn matches(&self, path: &Path) -> bool {
        match &self.placement {
            VersionPlacement::Copy(identity, fingerprint) => {
                identity.matches_at(path)
                    && managed_copy_fingerprint(path).is_ok_and(|actual| actual == *fingerprint)
            }
            VersionPlacement::Link(destination) => {
                fs::read_link(path).is_ok_and(|link| &link == destination)
                    && project_version_hash(&self.path.parent().unwrap().join(destination))
                        .is_ok_and(|hash| hash == self.version)
            }
        }
    }
}

struct StagedTarget<'a> {
    target: &'a VersionTarget,
    staging: TempDir,
    candidate: PathBuf,
    previous: PathBuf,
    mode: DeployMode,
    preserved: bool,
    placed: bool,
}

impl UnoSkillsManager {
    pub fn clear_retained_project_pin(
        &self,
        project_id: &str,
        scope: &Path,
        skill_id: &str,
        expected_hash: &str,
    ) -> Result<()> {
        let _guard = self.lock_skills()?;
        ensure!(
            scope
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
            "Invalid subproject directory"
        );
        let scope = project_scope_key(scope);
        self.store
            .clear_retained_project_pin(project_id, &scope, skill_id, expected_hash)?;
        self.cleanup_project_snapshots(std::iter::once(expected_hash));
        Ok(())
    }

    pub fn review_project_conflict(
        &self,
        project_id: &str,
        scope: &Path,
        agent_key: &str,
        skill_id: &str,
    ) -> Result<(ProjectConflictReview, Option<PreparedProjectConflict>)> {
        let _guard = self.lock_skills()?;
        self.inspect_project_conflict(project_id, scope, agent_key, skill_id)
    }

    fn inspect_project_conflict(
        &self,
        project_id: &str,
        scope: &Path,
        agent_key: &str,
        skill_id: &str,
    ) -> Result<(ProjectConflictReview, Option<PreparedProjectConflict>)> {
        let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
        let skill = self
            .find_skill(skill_id)?
            .filter(|skill| skill.id == skill_id)
            .context("Skill not found")?;
        ensure!(
            sanitize_slug(&skill.slug).as_deref() == Some(&skill.slug),
            "Invalid skill slug"
        );
        project_scope_directory(&project, scope)?;
        let scope = project_scope_key(scope);
        let record = skill.project_deployments.iter().find(|dep| {
            dep.project_id == project.id && dep.agent_key == agent.key && dep.scope_path == scope
        });
        let mut review = ProjectConflictReview {
            path: record.map_or_else(
                || {
                    project
                        .path
                        .join(&scope)
                        .join(project_skills_dir(&agent))
                        .join(&skill.slug)
                },
                |dep| dep.target_path.clone(),
            ),
            kind: "unsafe",
            open_folder: false,
            pinned: skill.project_lock(&project.id, &scope).is_some(),
            files: None,
            detail: None,
            preview_id: None,
        };
        let target = match scoped_project_skills_dir(&project, &scope, &agent) {
            Ok(directory) => directory.join(&skill.slug),
            Err(error) => {
                review.detail = Some(format!("{error:#}"));
                return Ok((review, None));
            }
        };
        if review.path != target
            || ensure_dst_not_overlapping_src(&skill.central_path, &target).is_err()
        {
            return Ok((review, None));
        }
        let state = classify_target(&skill.central_path, &target);
        if record.is_some_and(|dep| {
            matches!(
                (dep.mode, state),
                (
                    DeployMode::Copy,
                    TargetState::LinkToSource | TargetState::ForeignLink | TargetState::RealFile
                ) | (
                    DeployMode::Symlink,
                    TargetState::RealDir | TargetState::RealFile | TargetState::ForeignLink
                )
            )
        }) {
            return Ok((review, None));
        }
        if !matches!(
            inspect_stack_project_target(&skill, &project, &agent, &scope, &target),
            StackProjectTarget::Conflict | StackProjectTarget::Local
        ) {
            review.kind = "stale";
            return Ok((review, None));
        }
        if !matches!(state, TargetState::RealDir | TargetState::RealFile) {
            return Ok((review, None));
        }
        review.open_folder = state == TargetState::RealDir;
        let Some(record) = record else {
            if self.store.count_deployments_at_path(&target, "")? == 0 {
                review.kind = "unmanaged";
            }
            return Ok((review, None));
        };
        if state != TargetState::RealDir || record.mode != DeployMode::Copy {
            return Ok((review, None));
        }
        let identity = DirectoryIdentity::read(&target)?.context("Project copy disappeared")?;
        let pin = skill.project_lock(&project.id, &scope);
        let inspected = (|| -> Result<_> {
            self.check_library_path(&skill)?;
            let baseline = if let Some(hash) = pin {
                self.checked_project_snapshot(hash)?
            } else {
                skill.central_path.clone()
            };
            let source_fingerprint = managed_copy_fingerprint(&baseline)?;
            let original = if pin.is_some() {
                record
                    .copy_hash
                    .as_deref()
                    .filter(|hash| hash.starts_with(COPY_FINGERPRINT_PREFIX))
                    .unwrap_or(&source_fingerprint)
                    .to_owned()
            } else {
                record
                    .copy_hash
                    .as_deref()
                    .or(skill.content_hash.as_deref())
                    .and_then(|hash| {
                        copy_baseline(&baseline, hash).or_else(|| {
                            self.has_revisions_dir()
                                .then(|| {
                                    copy_baseline(&self.previous_revision_path(&skill.id), hash)
                                })
                                .flatten()
                        })
                    })
                    .context("No trustworthy managed copy baseline")?
            };
            let fingerprint = managed_copy_fingerprint(&target)?;
            let files = compare_versions(&target, &baseline)?;
            ensure!(
                identity.matches_at(&target)
                    && managed_copy_fingerprint(&target)? == fingerprint
                    && managed_copy_fingerprint(&baseline)? == source_fingerprint,
                "STALE_PROJECT_VERSION: The files changed during review"
            );
            Ok((original, fingerprint, source_fingerprint, files))
        })();
        let (original, fingerprint, source_fingerprint, files) = match inspected {
            Ok(value) => value,
            Err(error) => {
                review.kind = "unreadable";
                review.detail = Some(format!("{error:#}"));
                return Ok((review, None));
            }
        };
        if original == fingerprint {
            return Ok((review, None));
        }
        review.kind = "modified";
        review.files = Some(files);
        let parent = target.parent().context("Missing deployment parent")?;
        let peers = self.list_skills()?.into_iter().any(|skill| {
            skill
                .deployments
                .iter()
                .map(|dep| (&dep.id, &dep.target_path))
                .chain(
                    skill
                        .project_deployments
                        .iter()
                        .map(|dep| (&dep.id, &dep.target_path)),
                )
                .any(|(id, path)| {
                    id != &record.id
                        && (paths_overlap(path, &target)
                            || physical_target_path(path)
                                .is_some_and(|path| paths_overlap(&path, &target)))
                })
        });
        if pin.is_some() || peers || self.ensure_unlocked_global_target(&target).is_err() {
            return Ok((review, None));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let prepared = PreparedProjectConflict {
            id: id.clone(),
            project_id: project.id,
            skill_id: skill.id,
            scope,
            agent_key: agent.key,
            target: VersionTarget {
                parent: DirectoryIdentity::read(parent)?.context("Missing deployment parent")?,
                path: target,
                placement: VersionPlacement::Copy(identity, fingerprint),
                version: original,
                ids: vec![record.id.clone()],
                locked: None,
                follow_mode: DeployMode::Copy,
            },
            source_fingerprint,
        };
        review.preview_id = Some(id);
        Ok((review, Some(prepared)))
    }

    pub fn apply_project_conflict(&self, prepared: PreparedProjectConflict) -> Result<()> {
        let _guard = self.lock_skills()?;
        let (_, fresh) = self.inspect_project_conflict(
            &prepared.project_id,
            &prepared.scope,
            &prepared.agent_key,
            &prepared.skill_id,
        )?;
        ensure!(
            fresh.is_some_and(|current| current.target == prepared.target
                && current.source_fingerprint == prepared.source_fingerprint),
            "STALE_PROJECT_VERSION: The reviewed deployment or library changed"
        );
        let skill = self
            .find_skill(&prepared.skill_id)?
            .context("Skill not found")?;
        let version = project_version_hash(&skill.central_path)?;
        let staging = self.stage_project_version(&skill.central_path, &version)?;
        let candidate = staging.path().join("snapshot");
        ensure!(
            managed_copy_fingerprint(&candidate)? == prepared.source_fingerprint,
            "STALE_PROJECT_VERSION: The reviewed library changed"
        );
        self.commit_project_version(&skill, &[prepared.target], &candidate, &version, false)
    }

    pub(super) fn project_deploy_snapshot(
        &self,
        skill: &Skill,
        project_id: &str,
        scope: &Path,
        agent_key: &str,
        target: &Path,
    ) -> Result<Option<(PathBuf, String)>> {
        let owns =
            |dep: &ProjectDeployment| dep.project_id == project_id && dep.scope_path == scope;
        let shared = skill
            .project_deployments
            .iter()
            .filter(|dep| dep.target_path == target);
        ensure!(
            shared
                .clone()
                .all(|dep| dep.locked_hash.is_none() || owns(dep)),
            "TARGET_CONFLICT: This target is locked by another project scope"
        );
        ensure!(
            skill.project_deployments.iter().all(|dep| {
                dep.locked_hash.is_none()
                    || !owns(dep)
                    || dep.agent_key != agent_key
                    || dep.target_path == target
            }),
            "TARGET_CONFLICT: The locked deployment was recorded at another path"
        );
        let physical = physical_target_path(target);
        ensure!(
            self.store
                .locked_project_target_paths()?
                .iter()
                .all(|path| {
                    (!paths_overlap(path, target)
                        && physical
                            .as_deref()
                            .is_none_or(|actual| !paths_overlap(path, actual)))
                        || shared.clone().any(|dep| {
                            owns(dep) && dep.locked_hash.is_some() && &dep.target_path == path
                        })
                }),
            "TARGET_CONFLICT: This destination overlaps another locked deployment"
        );
        let Some(hash) = skill.project_lock(project_id, scope) else {
            return Ok(None);
        };
        if let Some(dep) = shared.clone().find(|dep| owns(dep)) {
            ensure!(
                self.store.count_deployments_at_path(target, &dep.id)?
                    == shared.clone().count() - 1,
                "TARGET_CONFLICT: A different deployment owns this target"
            );
        } else {
            ensure!(
                self.store.count_deployments_at_path(target, "")? == 0,
                "TARGET_CONFLICT: A different deployment owns this target"
            );
        }
        ensure!(
            shared_records_match(skill, target, project_id, scope, hash),
            "TARGET_CONFLICT: This target has inconsistent version records"
        );
        if classify_target(&skill.central_path, target) == TargetState::RealDir {
            for dep in shared {
                verify_managed_target(
                    &skill.central_path,
                    target,
                    dep.mode,
                    Some(&self.locked_copy_baseline(dep)?),
                )?;
            }
        }
        Ok(Some((self.checked_project_snapshot(hash)?, hash.into())))
    }

    pub(super) fn ensure_unlocked_global_target(&self, target: &Path) -> Result<()> {
        let physical = physical_target_path(target);
        ensure!(
            self.store
                .locked_project_target_paths()?
                .iter()
                .all(|path| {
                    !paths_overlap(path, target)
                        && physical
                            .as_deref()
                            .is_none_or(|actual| !paths_overlap(path, actual))
                }),
            "TARGET_CONFLICT: This target is a locked project deployment"
        );
        Ok(())
    }

    fn version_repository(&self, create: bool) -> Result<PathBuf> {
        let directory = self.root_dir.canonicalize()?.join(".project-versions");
        if create && !directory.exists() {
            match fs::create_dir(&directory) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error.into()),
            }
        }
        ensure!(
            DirectoryIdentity::read(&directory)?.is_some(),
            "TARGET_CONFLICT: Project version storage is unavailable"
        );
        Ok(directory)
    }

    pub(super) fn checked_project_snapshot(&self, hash: &str) -> Result<PathBuf> {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Invalid project version hash"
        );
        let path = self.version_repository(false)?.join(hash);
        ensure!(
            DirectoryIdentity::read(&path)?.is_some() && project_version_hash(&path)? == hash,
            "STALE_PROJECT_VERSION: The saved project snapshot changed"
        );
        Ok(path)
    }

    fn stage_project_version(&self, source: &Path, hash: &str) -> Result<TempDir> {
        let staged = tempfile::Builder::new()
            .prefix(".project-version-review-")
            .tempdir_in(self.root_dir.canonicalize()?)?;
        let candidate = staged.path().join("snapshot");
        copy_dir_recursive(source, &candidate)?;
        ensure!(
            project_version_hash(&candidate)? == hash && project_version_hash(source)? == hash,
            "STALE_PROJECT_VERSION: The skill changed while preparing its snapshot"
        );
        Ok(staged)
    }

    fn promote_project_snapshot(&self, candidate: &Path, hash: &str) -> Result<(PathBuf, bool)> {
        ensure!(
            project_version_hash(candidate)? == hash,
            "STALE_PROJECT_VERSION: The reviewed snapshot changed"
        );
        let root = self.version_repository(true)?;
        let snapshot = root.join(hash);
        match fs::symlink_metadata(&snapshot) {
            Ok(_) => return Ok((self.checked_project_snapshot(hash)?, false)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        rename_without_replacing(candidate, &snapshot)?;
        Ok((snapshot, true))
    }

    pub(super) fn remove_unreferenced_snapshot(&self, hash: &str) -> Result<()> {
        if !self.store.project_snapshot_is_referenced(hash)? {
            let snapshot = self.checked_project_snapshot(hash)?;
            let mut removal = RemovalBatch::default();
            removal.add_directory(&snapshot, Some(managed_copy_fingerprint(&snapshot)?))?;
            removal.commit(|| Ok(()))?;
        }
        Ok(())
    }

    fn install_project_version(
        &self,
        skill: &Skill,
        targets: &[VersionTarget],
        candidate: &Path,
        version: &str,
        keep_locked: bool,
    ) -> Result<()> {
        let (source, promoted) = if keep_locked {
            self.promote_project_snapshot(candidate, version)?
        } else {
            (candidate.to_owned(), false)
        };
        let committed = self.commit_project_version(skill, targets, &source, version, keep_locked);
        if committed.is_err() && promoted {
            let _ = self.remove_unreferenced_snapshot(version);
        } else if committed.is_ok()
            && let Some(previous) = targets[0].locked.as_deref()
            && (!keep_locked || previous != version)
        {
            let _ = self.remove_unreferenced_snapshot(previous);
        }
        committed
    }

    pub fn lock_project_skill(
        &self,
        project_id: &str,
        scope: &Path,
        agent_key: &str,
        skill_id: &str,
    ) -> Result<()> {
        let _guard = self.lock_skills()?;
        let (skill, targets) =
            self.project_version_targets(project_id, scope, agent_key, skill_id)?;
        let first = &targets[0];
        ensure!(
            first.locked.is_none(),
            "STALE_PROJECT_VERSION: This project skill is already locked"
        );
        let source = match first.placement {
            VersionPlacement::Copy(..) => &first.path,
            VersionPlacement::Link(_) => &skill.central_path,
        };
        let staging = self.stage_project_version(source, &first.version)?;
        self.install_project_version(
            &skill,
            &targets,
            &staging.path().join("snapshot"),
            &first.version,
            true,
        )
    }

    pub fn prepare_project_version(
        &self,
        project_id: &str,
        scope: &Path,
        agent_key: &str,
        skill_id: &str,
        intent: ProjectVersionIntent,
    ) -> Result<PreparedProjectVersion> {
        let _guard = self.lock_skills()?;
        let (skill, targets) =
            self.project_version_targets(project_id, scope, agent_key, skill_id)?;
        let current_hash = targets[0]
            .locked
            .as_deref()
            .context("Project skill is following the library")?;
        let current = self.checked_project_snapshot(current_hash)?;
        self.check_library_path(&skill)?;
        let candidate_hash = project_version_hash(&skill.central_path)?;
        ensure!(
            intent == ProjectVersionIntent::Follow || candidate_hash != current_hash,
            "STALE_PROJECT_VERSION: The project already uses this version"
        );
        let staging = self.stage_project_version(&skill.central_path, &candidate_hash)?;
        let candidate = staging.path().join("snapshot");
        let review = ProjectVersionReview {
            id: uuid::Uuid::new_v4().to_string(),
            skill_id: skill.id,
            skill_name: skill.name,
            intent,
            current_hash: current_hash.into(),
            candidate_hash,
            files: compare_versions(&current, &candidate)?,
        };
        Ok(PreparedProjectVersion {
            review,
            project_id: project_id.into(),
            agent_key: agent_key.into(),
            scope: project_scope_key(scope),
            targets,
            staging,
        })
    }

    pub fn apply_project_version(&self, prepared: PreparedProjectVersion) -> Result<()> {
        let _guard = self.lock_skills()?;
        let review = prepared.review;
        let (skill, targets) = self.project_version_targets(
            &prepared.project_id,
            &prepared.scope,
            &prepared.agent_key,
            &review.skill_id,
        )?;
        ensure!(
            targets == prepared.targets,
            "STALE_PROJECT_VERSION: A reviewed project deployment changed"
        );
        self.checked_project_snapshot(&review.current_hash)?;
        let candidate = prepared.staging.path().join("snapshot");
        ensure!(
            project_version_hash(&candidate)? == review.candidate_hash,
            "STALE_PROJECT_VERSION: The reviewed snapshot changed"
        );
        let keep_locked = review.intent == ProjectVersionIntent::Upgrade;
        if !keep_locked {
            self.check_library_path(&skill)?;
            ensure!(
                project_version_hash(&skill.central_path)? == review.candidate_hash,
                "STALE_PROJECT_VERSION: The library changed after review"
            );
        }
        self.install_project_version(
            &skill,
            &targets,
            &candidate,
            &review.candidate_hash,
            keep_locked,
        )
    }

    fn project_version_targets(
        &self,
        project_id: &str,
        scope: &Path,
        agent_key: &str,
        skill_id: &str,
    ) -> Result<(Skill, Vec<VersionTarget>)> {
        let (project, _) = self.stack_deploy_destination(project_id, agent_key)?;
        let scope = project_scope_key(scope);
        let skill = self
            .find_skill(skill_id)?
            .filter(|skill| skill.id == skill_id)
            .context("Skill not found")?;
        ensure!(
            sanitize_slug(&skill.slug).as_deref() == Some(&skill.slug),
            "Invalid skill slug"
        );
        let agents = self.list_agents()?;
        let mut records = BTreeMap::<&Path, Vec<&ProjectDeployment>>::new();
        let mut selected = false;
        for dep in skill
            .project_deployments
            .iter()
            .filter(|dep| dep.project_id == project.id && dep.scope_path == scope)
        {
            let agent = agents
                .iter()
                .find(|agent| agent.key == dep.agent_key)
                .context("TARGET_CONFLICT: A deployed agent is no longer configured")?;
            ensure!(
                dep.target_path
                    == scoped_project_skills_dir(&project, &scope, agent)?.join(&skill.slug)
                    && safe_scoped_target(dep)?,
                "TARGET_CONFLICT: The recorded project deployment moved"
            );
            selected |= agent_key == dep.agent_key;
            records.entry(&dep.target_path).or_default().push(dep);
        }
        ensure!(
            selected,
            "TARGET_CONFLICT: Deploy this skill to the selected project first"
        );
        let occupied = self
            .list_skills()?
            .into_iter()
            .flat_map(|skill| {
                skill
                    .deployments
                    .into_iter()
                    .map(|dep| (dep.target_path, dep.id))
                    .chain(
                        skill
                            .project_deployments
                            .into_iter()
                            .map(|dep| (dep.target_path, dep.id)),
                    )
            })
            .filter_map(|(path, id)| physical_target_path(&path).map(|physical| (physical, id)))
            .collect::<Vec<_>>();
        let mut targets = Vec::with_capacity(records.len());
        for (path, entries) in records {
            let first = entries[0];
            ensure!(
                self.store.count_deployments_at_path(path, &first.id)? == entries.len() - 1
                    && occupied
                        .iter()
                        .all(|(other, id)| !paths_overlap(other, path)
                            || entries.iter().any(|dep| &dep.id == id))
                    && entries.iter().all(|dep| dep.mode == first.mode
                        && dep.locked_hash == first.locked_hash
                        && dep.follow_mode == first.follow_mode),
                "TARGET_CONFLICT: The deployment is shared with another project or has inconsistent records"
            );
            let expected_copy_hash = |dep: &ProjectDeployment| -> Result<Option<String>> {
                if dep.locked_hash.is_some() {
                    self.locked_copy_baseline(dep).map(Some)
                } else {
                    Ok(dep.copy_hash.clone().or_else(|| skill.content_hash.clone()))
                }
            };
            let state = verify_managed_target(
                &skill.central_path,
                path,
                first.mode,
                expected_copy_hash(first)?.as_deref(),
            )?;
            let (placement, version) = match state {
                TargetState::RealDir => {
                    let version = project_version_hash(path)?;
                    for dep in &entries[1..] {
                        verify_managed_target(
                            &skill.central_path,
                            path,
                            dep.mode,
                            expected_copy_hash(dep)?.as_deref(),
                        )?;
                    }
                    let identity =
                        DirectoryIdentity::read(path)?.context("Project skill disappeared")?;
                    (
                        VersionPlacement::Copy(identity, managed_copy_fingerprint(path)?),
                        version,
                    )
                }
                TargetState::LinkToSource => {
                    self.check_library_path(&skill)?;
                    (
                        VersionPlacement::Link(fs::read_link(path)?),
                        project_version_hash(&skill.central_path)?,
                    )
                }
                _ => anyhow::bail!("TARGET_CONFLICT: A project deployment is missing"),
            };
            let mut ids = entries.iter().map(|dep| dep.id.clone()).collect::<Vec<_>>();
            ids.sort();
            targets.push(VersionTarget {
                path: path.into(),
                parent: DirectoryIdentity::read(
                    path.parent().context("Missing deployment parent")?,
                )?
                .context("Missing deployment parent")?,
                placement,
                version,
                ids,
                locked: first.locked_hash.clone(),
                follow_mode: first.follow_mode.unwrap_or(first.mode),
            });
        }
        ensure!(
            targets
                .iter()
                .all(|target| target.version == targets[0].version
                    && target.locked == targets[0].locked),
            "TARGET_CONFLICT: Project agents currently use different versions of this skill"
        );
        Ok((skill, targets))
    }

    fn commit_project_version(
        &self,
        skill: &Skill,
        targets: &[VersionTarget],
        snapshot: &Path,
        version: &str,
        keep_locked: bool,
    ) -> Result<()> {
        let copy_hash = managed_copy_fingerprint(snapshot)?;
        let mut staged = Vec::with_capacity(targets.len());
        let mut records = Vec::new();
        for target in targets {
            let parent = target.path.parent().context("Missing deployment parent")?;
            ensure!(
                target.parent.matches_at(parent),
                "TARGET_CONFLICT: Project skills directory changed"
            );
            ensure_dst_not_overlapping_src(snapshot, &target.path)?;
            ensure_dst_not_overlapping_src(&skill.central_path, &target.path)?;
            let staging = tempfile::Builder::new()
                .prefix(".unoskills-version-")
                .tempdir_in(parent)?;
            let candidate = staging.path().join("candidate");
            let previous = staging.path().join("previous");
            let (source, mode) = if keep_locked || target.follow_mode == DeployMode::Copy {
                (snapshot, DeployMode::Copy)
            } else {
                (skill.central_path.as_path(), DeployMode::Symlink)
            };
            let mode = deploy_skill_dir(source, &candidate, mode, ReplacePolicy::CreateOnly)?;
            check_candidate(skill, &candidate, mode, version)?;
            let check_copy = |path: &Path| -> Result<()> {
                ensure!(
                    mode != DeployMode::Copy || managed_copy_fingerprint(path)? == copy_hash,
                    "STALE_PROJECT_VERSION: The reviewed copy changed"
                );
                Ok(())
            };
            check_copy(&candidate)?;
            for mut dep in skill
                .project_deployments
                .iter()
                .filter(|dep| target.ids.contains(&dep.id))
                .cloned()
            {
                dep.mode = mode;
                dep.synced_at = Utc::now().timestamp();
                dep.copy_hash = (mode == DeployMode::Copy).then(|| copy_hash.clone());
                dep.follow_mode = keep_locked.then_some(target.follow_mode);
                records.push(dep);
            }
            staged.push(StagedTarget {
                target,
                staging,
                candidate,
                previous,
                mode,
                preserved: false,
                placed: false,
            });
        }
        let install = (|| -> Result<()> {
            for stage in &mut staged {
                let path = &stage.target.path;
                ensure!(
                    stage.target.parent.matches_at(path.parent().unwrap())
                        && stage.target.matches(path),
                    "STALE_PROJECT_VERSION: The project deployment changed: {}",
                    path.display()
                );
                check_candidate(skill, &stage.candidate, stage.mode, version)?;
                fs::rename(path, &stage.previous)?;
                stage.preserved = true;
                ensure!(
                    stage.target.matches(&stage.previous),
                    "STALE_PROJECT_VERSION: The project deployment changed: {}",
                    path.display()
                );
                rename_without_replacing(&stage.candidate, path)?;
                stage.placed = true;
            }
            ensure!(
                project_version_hash(snapshot)? == version,
                "STALE_PROJECT_VERSION: The project snapshot changed"
            );
            for stage in &staged {
                check_candidate(skill, &stage.target.path, stage.mode, version)?;
                ensure!(
                    stage.mode != DeployMode::Copy
                        || managed_copy_fingerprint(&stage.target.path)? == copy_hash,
                    "STALE_PROJECT_VERSION: The deployed copy changed"
                );
            }
            self.store
                .update_project_versions(&records, keep_locked.then_some(version))
        })();
        if let Err(error) = install {
            let mut preserved = Vec::new();
            for stage in staged.iter_mut().rev().filter(|stage| stage.preserved) {
                let path = &stage.target.path;
                let restore = (|| -> Result<()> {
                    ensure!(
                        stage.target.parent.matches_at(path.parent().unwrap()),
                        "Project skills directory changed during recovery"
                    );
                    if stage.placed {
                        check_candidate(skill, path, stage.mode, version)?;
                        rename_without_replacing(path, &stage.candidate)?;
                    }
                    rename_without_replacing(&stage.previous, path)
                })();
                if restore.is_err() {
                    stage.staging.disable_cleanup(true);
                    preserved.push(stage.previous.display().to_string());
                }
            }
            if !preserved.is_empty() {
                return Err(error).context(format!(
                    "Original deployments preserved at {}",
                    preserved.join(", ")
                ));
            }
            return Err(error);
        }
        Ok(())
    }
}

fn shared_records_match(
    skill: &Skill,
    target: &Path,
    project_id: &str,
    scope: &Path,
    hash: &str,
) -> bool {
    skill
        .project_deployments
        .iter()
        .filter(|dep| dep.target_path == target)
        .all(|dep| {
            dep.project_id == project_id
                && dep.scope_path == scope
                && dep.locked_hash.as_deref() == Some(hash)
        })
}

fn check_candidate(skill: &Skill, path: &Path, mode: DeployMode, version: &str) -> Result<()> {
    let source = if mode == DeployMode::Copy {
        path
    } else {
        ensure!(
            verify_managed_target(&skill.central_path, path, DeployMode::Symlink, None)?
                == TargetState::LinkToSource,
            "STALE_PROJECT_VERSION: The reviewed library link changed"
        );
        &skill.central_path
    };
    ensure!(
        project_version_hash(source)? == version,
        "STALE_PROJECT_VERSION: The reviewed files changed"
    );
    Ok(())
}

fn compare_versions(before: &Path, after: &Path) -> Result<Vec<ProjectVersionChange>> {
    ensure!(
        before.symlink_metadata()?.is_dir() && after.symlink_metadata()?.is_dir(),
        "Expected real directories for version comparison"
    );
    // Normalize platform aliases above the roots while keeping the per-file
    // canonical path check that rejects redirected entries within each tree.
    let before = before.canonicalize()?;
    let after = after.canonicalize()?;
    let old = directory_tree(&before)?;
    let new = directory_tree(&after)?;
    let paths: BTreeSet<_> = old.keys().chain(new.keys()).collect();
    Ok(paths
        .into_iter()
        .filter_map(|path| {
            let old_item = old.get(path);
            let new_item = new.get(path);
            if old_item == new_item {
                return None;
            }
            let kind = match (old_item, new_item) {
                (None, _) => SkillFileChangeKind::Added,
                (_, None) => SkillFileChangeKind::Removed,
                _ => SkillFileChangeKind::Modified,
            };
            let left = review_text(&before, path, old_item);
            let right = review_text(&after, path, new_item);
            let diff = left
                .zip(right)
                .and_then(|(left, right)| text_diff(&left, &right));
            Some(ProjectVersionChange {
                path: path.clone(),
                kind,
                diff,
            })
        })
        .collect())
}

fn review_text(root: &Path, path: &Path, entry: Option<&TreeEntry>) -> Option<String> {
    use std::io::Read;
    const LIMIT: usize = 16 * 1024;
    match entry {
        None => Some(String::new()),
        Some(TreeEntry::Directory) => None,
        Some(TreeEntry::File { hash, .. }) => {
            let path = root.join(path);
            if !path.symlink_metadata().ok()?.is_file() || path.canonicalize().ok()? != path {
                return None;
            }
            let mut bytes = Vec::new();
            File::open(path)
                .ok()?
                .take((LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .ok()?;
            (bytes.len() <= LIMIT
                && !bytes.contains(&0)
                && <[u8; 32]>::from(Sha256::digest(&bytes)) == *hash)
                .then(|| String::from_utf8(bytes).ok())
                .flatten()
        }
    }
}

fn text_diff(left: &str, right: &str) -> Option<String> {
    if left == right {
        return None;
    }
    let left = left.split_inclusive('\n').collect::<Vec<_>>();
    let right = right.split_inclusive('\n').collect::<Vec<_>>();
    if left.len() > 256 || right.len() > 256 {
        return None;
    }
    let mut lengths = vec![vec![0; right.len() + 1]; left.len() + 1];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            lengths[i][j] = if left[i] == right[j] {
                lengths[i + 1][j + 1] + 1
            } else {
                lengths[i + 1][j].max(lengths[i][j + 1])
            };
        }
    }
    let mut diff = String::new();
    let (mut i, mut j) = (0, 0);
    while i < left.len() || j < right.len() {
        let (prefix, line) = if i < left.len() && j < right.len() && left[i] == right[j] {
            i += 1;
            j += 1;
            ("  ", left[i - 1])
        } else if i < left.len() && (j == right.len() || lengths[i + 1][j] >= lengths[i][j + 1]) {
            i += 1;
            ("- ", left[i - 1])
        } else {
            j += 1;
            ("+ ", right[j - 1])
        };
        diff.push_str(prefix);
        diff.push_str(line.strip_suffix('\n').unwrap_or(line));
        if !line.ends_with('\n') {
            diff.push_str(" ⏎̸");
        }
        diff.push('\n');
    }
    Some(diff)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn text_diff_handles_a_path_alias_above_the_library() {
        let temp = tempfile::tempdir().unwrap();
        let library = temp.path().join("library");
        let alias = temp.path().join("library-alias");
        let project = temp.path().join("project");
        fs::create_dir_all(library.join("reviewer")).unwrap();
        fs::create_dir(&project).unwrap();
        std::os::unix::fs::symlink(&library, &alias).unwrap();
        fs::write(library.join("reviewer/SKILL.md"), "Library text\n").unwrap();
        fs::write(project.join("SKILL.md"), "Project text\n").unwrap();

        let changes = compare_versions(&project, &alias.join("reviewer")).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0].diff.as_deref(),
            Some("- Project text\n+ Library text\n")
        );
    }
}
