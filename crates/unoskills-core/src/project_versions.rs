use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::project_deploy_preview::DirectoryIdentity;
use super::skill_revisions::SkillFileChangeKind;
use super::{UnoSkillsManager, project_scope_key, safe_scoped_target, scoped_project_skills_dir};
use crate::domain::{DeployMode, ProjectDeployment, Skill};
use crate::parser::{checked_directory_hash, checked_entries, is_valid_skill_dir, sanitize_slug};
use crate::sync::{
    ReplacePolicy, TargetState, copy_dir_recursive, deploy_skill_dir,
    ensure_dst_not_overlapping_src, verify_managed_target,
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
    Copy(DirectoryIdentity),
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
            VersionPlacement::Copy(identity) => {
                identity.matches_at(path)
                    && project_version_hash(path).is_ok_and(|hash| hash == self.version)
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

#[derive(Eq, PartialEq)]
enum TreeEntry {
    Directory,
    File { hash: Vec<u8>, executable: bool },
}

fn skill_tree(path: &Path) -> Result<BTreeMap<PathBuf, TreeEntry>> {
    ensure!(
        fs::symlink_metadata(path)?.is_dir() && is_valid_skill_dir(path),
        "Expected a complete skill directory at {}",
        path.display()
    );
    let mut tree = BTreeMap::new();
    for entry in checked_entries(path) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(path)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let item = if entry.file_type().is_dir() {
            TreeEntry::Directory
        } else {
            let file = File::open(entry.path())?;
            let mut hash = Sha256::new();
            std::io::copy(&mut &file, &mut hash)?;
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                file.metadata()?.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            TreeEntry::File {
                hash: hash.finalize().to_vec(),
                executable,
            }
        };
        tree.insert(relative.to_owned(), item);
    }
    Ok(tree)
}

pub(crate) fn project_version_hash(path: &Path) -> Result<String> {
    let mut digest = Sha256::new();
    for (relative, item) in skill_tree(path)? {
        let name = relative.as_os_str().as_encoded_bytes();
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name);
        match item {
            TreeEntry::Directory => digest.update([0]),
            TreeEntry::File { hash, executable } => {
                digest.update([1, u8::from(executable)]);
                digest.update(hash);
            }
        }
    }
    Ok(hex::encode(digest.finalize()))
}

impl UnoSkillsManager {
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
                self.store.count_deployments_at_path(target, &dep.id)? == shared.count() - 1,
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

    fn remove_unreferenced_snapshot(&self, hash: &str) -> Result<()> {
        if !self.store.project_snapshot_is_referenced(hash)? {
            fs::remove_dir_all(self.checked_project_snapshot(hash)?)?;
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
            VersionPlacement::Copy(_) => &first.path,
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
            let state = verify_managed_target(
                &skill.central_path,
                path,
                first.mode,
                first.copy_hash.as_deref().or(skill.content_hash.as_deref()),
            )?;
            let (placement, version) = match state {
                TargetState::RealDir => {
                    let version = project_version_hash(path)?;
                    ensure!(
                        entries.iter().all(|dep| dep.copy_hash == first.copy_hash)
                            && first
                                .locked_hash
                                .as_ref()
                                .is_none_or(|hash| hash == &version),
                        "TARGET_CONFLICT: The managed project copy has changed"
                    );
                    let identity =
                        DirectoryIdentity::read(path)?.context("Project skill disappeared")?;
                    (VersionPlacement::Copy(identity), version)
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
        let copy_hash = checked_directory_hash(snapshot)?;
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

pub(super) fn rename_without_replacing(from: &Path, to: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
        let from = from
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();
        let to = to.as_os_str().encode_wide().chain([0]).collect::<Vec<_>>();
        // Both paths are NUL-terminated and live for the duration of the call.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        use std::io::ErrorKind;
        ensure!(
            fs::symlink_metadata(to).is_err_and(|error| error.kind() == ErrorKind::NotFound),
            "TARGET_CONFLICT: An entry already exists: {}",
            to.display()
        );
        fs::rename(from, to)?;
    }
    Ok(())
}

pub(super) fn physical_target_path(path: &Path) -> Option<PathBuf> {
    Some(path.parent()?.canonicalize().ok()?.join(path.file_name()?))
}

pub(super) fn paths_overlap(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        let left = PathBuf::from(left.to_string_lossy().to_lowercase());
        let right = PathBuf::from(right.to_string_lossy().to_lowercase());
        left.starts_with(&right) || right.starts_with(&left)
    } else {
        left.starts_with(right) || right.starts_with(left)
    }
}

fn compare_versions(before: &Path, after: &Path) -> Result<Vec<ProjectVersionChange>> {
    let old = skill_tree(before)?;
    let new = skill_tree(after)?;
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
            let left = review_text(before, path, old_item);
            let right = review_text(after, path, new_item);
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
        Some(TreeEntry::File { .. }) => {
            let mut bytes = Vec::new();
            File::open(root.join(path))
                .ok()?
                .take((LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .ok()?;
            (bytes.len() <= LIMIT && !bytes.contains(&0))
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
