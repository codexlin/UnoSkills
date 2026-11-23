use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::{
    StackProjectTarget, UnoSkillsManager, inspect_stack_project_target, is_globally_available,
    project_scope_directory, project_scope_key, project_skills_dir, scoped_project_skills_dir,
};
use crate::domain::{
    DeployMode, StackManifest, StackProjectDeployStatus, StackProjectDeploySummary,
};
use crate::parser::sanitize_slug;
use crate::parser::{managed_copy_fingerprint, project_version_hash};
use crate::sync::{
    DirectoryIdentity, ReplacePolicy, deploy_skill_dir, ensure_dst_not_overlapping_src,
    remove_managed_target, rename_without_replacing, verify_managed_target,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectDeployOutcome {
    KeepLocal,
    Occupied,
    UnsafePath,
    OtherSource,
    Duplicate,
    AlreadyPresent,
    Register,
    Ready,
    GlobalOnly,
    Unavailable,
}

impl ProjectDeployOutcome {
    fn eligible(self, include_global: bool) -> bool {
        matches!(self, Self::Ready | Self::Register) || (include_global && self == Self::GlobalOnly)
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectDeployRow {
    pub index: usize,
    pub name: String,
    pub target: Option<PathBuf>,
    pub outcome: ProjectDeployOutcome,
    source: String,
    skill_id: Option<String>,
    source_path: Option<PathBuf>,
    source_hash: Option<String>,
    mode: Option<DeployMode>,
    registration: Option<RegistrationTarget>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct RegistrationTarget {
    identity: DirectoryIdentity,
    link: Option<PathBuf>,
    hash: String,
    copy_hash: Option<String>,
    mode: DeployMode,
}

impl RegistrationTarget {
    fn read(target: &Path, source: &Path, mode: DeployMode) -> Result<Self> {
        let (identity, link, contents) = match mode {
            DeployMode::Copy => (
                DirectoryIdentity::read(target)?.context("Missing deployment target")?,
                None,
                target,
            ),
            DeployMode::Symlink => (
                DirectoryIdentity::from_metadata(target, &fs::symlink_metadata(target)?),
                Some(fs::read_link(target)?),
                source,
            ),
        };
        ensure_dst_not_overlapping_src(source, target)?;
        Ok(Self {
            identity,
            link,
            hash: project_version_hash(contents)?,
            copy_hash: (mode == DeployMode::Copy)
                .then(|| managed_copy_fingerprint(target))
                .transpose()?,
            mode,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectDeployPreview {
    stack_id: String,
    project_id: String,
    agent_key: String,
    scope: PathBuf,
    directories: Vec<DirectoryIdentity>,
    pub rows: Vec<ProjectDeployRow>,
    #[serde(flatten)]
    pub status: StackProjectDeployStatus,
}

impl UnoSkillsManager {
    pub fn preview_project_stack(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
        scope: &Path,
    ) -> Result<ProjectDeployPreview> {
        let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
        let scope_root = project_scope_directory(&project, scope)?;
        let scope = project_scope_key(scope);
        let directory = scoped_project_skills_dir(&project, &scope, &agent);
        let directories = match &directory {
            Ok(path) => path
                .ancestors()
                .take_while(|path| path.starts_with(&project.path))
                .filter_map(|path| DirectoryIdentity::read(path).transpose())
                .collect::<Result<Vec<_>>>()?,
            Err(_) => Vec::new(),
        };
        let skills = self.list_skills()?;
        let mut seen = HashSet::new();
        let mut status = StackProjectDeployStatus::default();
        let mut rows = Vec::with_capacity(stack.skills.len());
        for (index, member) in stack.skills.iter().enumerate() {
            use ProjectDeployOutcome::*;
            let installed = member.installed_in(&skills);
            let slug = installed.map_or_else(
                || sanitize_slug(&member.name),
                |skill| sanitize_slug(&skill.slug).filter(|slug| slug == &skill.slug),
            );
            let target = slug.map(|slug| scope_root.join(project_skills_dir(&agent)).join(slug));
            let skill = installed.filter(|skill| !member.has_source_conflict(skill));
            let mut source_hash = None;
            let mut mode = None;
            let mut source_path = None;
            let mut registration = None;
            let outcome = match (skill, target.as_ref()) {
                (_, None) => Unavailable,
                (_, Some(_)) if directory.is_err() => UnsafePath,
                (None, Some(_)) if installed.is_some() => OtherSource,
                (None, Some(_)) => Unavailable,
                (Some(skill), Some(target)) => {
                    let unique = if cfg!(windows) {
                        seen.insert(target.to_string_lossy().to_lowercase())
                    } else {
                        seen.insert(target.to_string_lossy().into_owned())
                    };
                    if !unique {
                        Duplicate
                    } else {
                        match inspect_stack_project_target(skill, &project, &agent, &scope, target)
                        {
                            StackProjectTarget::Deployed(_) => AlreadyPresent,
                            StackProjectTarget::Register(mode) => {
                                let existing = self
                                    .project_deploy_snapshot(
                                        skill,
                                        &project.id,
                                        &scope,
                                        agent_key,
                                        target,
                                    )
                                    .and_then(|locked| {
                                        let source = locked
                                            .as_ref()
                                            .map_or(&skill.central_path, |(path, _)| path);
                                        RegistrationTarget::read(target, source, mode)
                                    });
                                match existing {
                                    Ok(existing) => {
                                        registration = Some(existing);
                                        Register
                                    }
                                    Err(_) => UnsafePath,
                                }
                            }
                            StackProjectTarget::Local => KeepLocal,
                            StackProjectTarget::Conflict => Occupied,
                            StackProjectTarget::Unavailable => Unavailable,
                            StackProjectTarget::Missing(recorded_mode) => {
                                match self.project_deploy_snapshot(
                                    skill,
                                    &project.id,
                                    &scope,
                                    agent_key,
                                    target,
                                ) {
                                    Err(_) => UnsafePath,
                                    Ok(snapshot) => {
                                        let source = snapshot
                                            .as_ref()
                                            .map_or(&skill.central_path, |(path, _)| path);
                                        if ensure_dst_not_overlapping_src(source, target).is_err() {
                                            UnsafePath
                                        } else if let Ok(hash) = project_version_hash(source) {
                                            source_path = Some(source.clone());
                                            source_hash = Some(hash);
                                            mode = Some(if snapshot.is_some() {
                                                DeployMode::Copy
                                            } else {
                                                recorded_mode
                                                    .unwrap_or_else(|| self.default_deploy_mode())
                                            });
                                            if snapshot.is_none()
                                                && is_globally_available(skill, &agent)
                                            {
                                                GlobalOnly
                                            } else {
                                                Ready
                                            }
                                        } else {
                                            Unavailable
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
            match outcome {
                KeepLocal => status.project_local += 1,
                Occupied | UnsafePath | OtherSource | Duplicate => status.conflicts += 1,
                AlreadyPresent => status.deployed += 1,
                Ready | Register => status.pending += 1,
                GlobalOnly => status.global_available += 1,
                Unavailable => status.unavailable += 1,
            }
            rows.push(ProjectDeployRow {
                index,
                name: member.name.clone(),
                target: target
                    .and_then(|path| path.strip_prefix(&project.path).ok().map(Path::to_owned)),
                outcome,
                source: member.source.clone(),
                skill_id: skill.map(|skill| skill.id.clone()),
                source_path,
                source_hash,
                mode,
                registration,
            });
        }
        rows.sort_by_key(|row| match row.outcome {
            ProjectDeployOutcome::KeepLocal
            | ProjectDeployOutcome::Occupied
            | ProjectDeployOutcome::UnsafePath
            | ProjectDeployOutcome::OtherSource
            | ProjectDeployOutcome::Duplicate => 0,
            _ => 1,
        });
        Ok(ProjectDeployPreview {
            stack_id: stack.id.clone(),
            project_id: project.id,
            agent_key: agent.key,
            scope,
            directories,
            rows,
            status,
        })
    }

    pub fn apply_project_stack_preview(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
        scope: &Path,
        reviewed: ProjectDeployPreview,
        include_global: bool,
    ) -> Result<StackProjectDeploySummary> {
        let _lock = self.lock_skills()?;
        let scope = project_scope_key(scope);
        let scope = scope.as_path();
        ensure!(
            reviewed.stack_id == stack.id
                && reviewed.project_id == project_id
                && reviewed.agent_key == agent_key
                && reviewed.scope == scope,
            "STALE_PROJECT_DEPLOY: Review the selected stack and destination again."
        );
        let current = self.preview_project_stack(stack, project_id, agent_key, scope);
        let eligible = current.as_ref().ok().filter(|current| {
            reviewed
                .directories
                .iter()
                .all(|directory| current.directories.contains(directory))
        });
        let mut summary = StackProjectDeploySummary::default();
        let mut attempted = HashSet::new();
        for row in &reviewed.rows {
            use ProjectDeployOutcome::*;
            if !attempted.insert(row.index) {
                continue;
            }
            let unchanged = eligible.is_some_and(|preview| preview.rows.contains(row));
            if !unchanged {
                summary
                    .skipped
                    .push((row.name.clone(), "STALE_PROJECT_DEPLOY".into()));
                continue;
            }
            match row.outcome {
                AlreadyPresent => summary.already_deployed += 1,
                KeepLocal => summary.project_local += 1,
                Unavailable => summary.unavailable += 1,
                GlobalOnly if !include_global => summary.global_available += 1,
                _ => {}
            }
            if !row.outcome.eligible(include_global) {
                continue;
            }
            let placed = (|| -> Result<()> {
                let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
                let id = row.skill_id.as_deref().context("Missing reviewed skill")?;
                let skill = self
                    .find_skill(id)?
                    .filter(|skill| skill.id == id)
                    .context("Reviewed skill disappeared")?;
                let target = scoped_project_skills_dir(&project, scope, &agent)?.join(&skill.slug);
                let locked =
                    self.project_deploy_snapshot(&skill, &project.id, scope, agent_key, &target)?;
                let source = locked
                    .as_ref()
                    .map_or(skill.central_path.as_path(), |(path, _)| path);
                ensure!(
                    Some(target.strip_prefix(&project.path)?) == row.target.as_deref()
                        && reviewed
                            .directories
                            .iter()
                            .all(|directory| { directory.matches_at(&directory.path) }),
                    "STALE_PROJECT_DEPLOY: The source or destination changed."
                );
                if row.outcome == Register {
                    let Some(existing) = row.registration.as_ref() else {
                        anyhow::bail!("STALE_PROJECT_DEPLOY: Missing reviewed registration");
                    };
                    let deployment = self
                        .prepare_project_deployment(
                            &skill,
                            &project,
                            &agent,
                            scope,
                            target.clone(),
                            existing.mode,
                        )
                        .context("STALE_PROJECT_DEPLOY: The shared deployment changed.")?;
                    ensure!(
                        deployment.copy_hash == existing.copy_hash
                            && matches!(inspect_stack_project_target(&skill, &project, &agent, scope, &target),
                            StackProjectTarget::Register(mode) if mode == existing.mode)
                            && RegistrationTarget::read(&target, source, existing.mode)
                                .as_ref()
                                .is_ok_and(|current| current == existing),
                        "STALE_PROJECT_DEPLOY: The shared deployment changed."
                    );
                    self.store.upsert_project_deployment(&deployment)?;
                    return Ok(());
                }
                ensure!(
                    matches!(
                        inspect_stack_project_target(&skill, &project, &agent, scope, &target),
                        StackProjectTarget::Missing(_)
                    ) && Some(source) == row.source_path.as_deref(),
                    "STALE_PROJECT_DEPLOY: The source or destination changed."
                );
                let version = row
                    .source_hash
                    .as_deref()
                    .context("Missing reviewed source")?;
                ensure_dst_not_overlapping_src(source, &target)?;
                let parent = target.parent().context("Missing deployment parent")?;
                fs::create_dir_all(parent)?;
                let identity =
                    DirectoryIdentity::read(parent)?.context("Missing deployment parent")?;
                let staging = tempfile::Builder::new()
                    .prefix(".unoskills-deploy-")
                    .tempdir_in(parent)?;
                let candidate = staging.path().join("candidate");
                let mode = deploy_skill_dir(
                    source,
                    &candidate,
                    row.mode.context("Missing reviewed deployment mode")?,
                    ReplacePolicy::CreateOnly,
                )?;
                let candidate_source = if mode == DeployMode::Copy {
                    &candidate
                } else {
                    source
                };
                let copy_hash = (mode == DeployMode::Copy)
                    .then(|| managed_copy_fingerprint(&candidate))
                    .transpose()?;
                let copy_identity = if mode == DeployMode::Copy {
                    DirectoryIdentity::read(&candidate)?
                } else {
                    None
                };
                ensure!(
                    project_version_hash(candidate_source)? == version
                        && project_version_hash(source)? == version
                        && identity.matches_at(parent)
                        && scoped_project_skills_dir(&project, scope, &agent)? == parent,
                    "STALE_PROJECT_DEPLOY: The source or destination changed."
                );
                rename_without_replacing(&candidate, &target).with_context(|| {
                    if target.symlink_metadata().is_ok() {
                        "TARGET_CONFLICT: The deployment target appeared after review"
                    } else {
                        "Could not place the reviewed deployment"
                    }
                })?;
                let recorded = (|| -> Result<()> {
                    let deployment = self.prepare_project_deployment(
                        &skill,
                        &project,
                        &agent,
                        scope,
                        target.clone(),
                        mode,
                    )?;
                    ensure!(
                        deployment.copy_hash == copy_hash,
                        "STALE_PROJECT_DEPLOY: The deployed copy changed before recording"
                    );
                    self.store.upsert_project_deployment(&deployment)
                })();
                if let Err(error) = recorded {
                    // A failed write must not remove a target that changed after placement.
                    let current = if mode == DeployMode::Copy {
                        target.as_path()
                    } else {
                        source
                    };
                    if identity.matches_at(parent)
                        && copy_identity
                            .as_ref()
                            .is_none_or(|copy| copy.matches_at(&target))
                        && project_version_hash(current).is_ok_and(|hash| hash == version)
                        && verify_managed_target(source, &target, mode, copy_hash.as_deref())
                            .is_ok()
                    {
                        remove_managed_target(source, &target, mode, copy_hash.as_deref())?;
                    }
                    return Err(error);
                }
                Ok(())
            })();
            match placed {
                Ok(()) if row.outcome == Register => summary.registered += 1,
                Ok(()) => summary.deployed += 1,
                Err(error)
                    if format!("{error:#}").contains("STALE_PROJECT_DEPLOY")
                        || format!("{error:#}").contains("TARGET_CONFLICT") =>
                {
                    summary
                        .skipped
                        .push((row.name.clone(), "STALE_PROJECT_DEPLOY".into()))
                }
                Err(error) => summary
                    .failed
                    .push((row.name.clone(), format!("{error:#}"))),
            }
        }
        Ok(summary)
    }
}
