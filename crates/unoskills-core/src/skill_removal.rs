use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::{UnoSkillsManager, project_scope_key, safe_scoped_target};
use crate::domain::{DeployMode, Deployment, ProjectDeployment, Skill};
use crate::parser::{
    COPY_FINGERPRINT_PREFIX, managed_copy_fingerprint, project_version_hash, sanitize_slug,
};
use crate::sync::{
    RemovalBatch, TargetState, classify_target, copy_baseline, paths_overlap, physical_target_path,
    verify_managed_target,
};

#[derive(Debug, Serialize)]
pub struct RemovalProblem {
    pub path: PathBuf,
    reason: String,
}

impl RemovalProblem {
    pub(crate) fn at(path: &Path, error: anyhow::Error) -> anyhow::Error {
        let reason = error.to_string();
        error.context(Self {
            path: path.to_owned(),
            reason,
        })
    }
}

impl std::fmt::Display for RemovalProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

#[derive(Debug, Deserialize)]
pub struct ReviewedDeployment {
    pub id: String,
    pub target_path: PathBuf,
}

struct DeploymentRemoval<'a> {
    skill: &'a Skill,
    id: &'a str,
    path: &'a Path,
    mode: DeployMode,
    copy_hash: Option<&'a str>,
    project: Option<&'a ProjectDeployment>,
}

impl<'a> DeploymentRemoval<'a> {
    fn global(skill: &'a Skill, dep: &'a Deployment) -> Self {
        Self {
            skill,
            id: &dep.id,
            path: &dep.target_path,
            mode: dep.mode,
            copy_hash: dep.copy_hash.as_deref(),
            project: None,
        }
    }

    fn project(skill: &'a Skill, dep: &'a ProjectDeployment) -> Self {
        Self {
            skill,
            id: &dep.id,
            path: &dep.target_path,
            mode: dep.mode,
            copy_hash: dep.copy_hash.as_deref(),
            project: Some(dep),
        }
    }
}

impl UnoSkillsManager {
    pub fn remove_skill(&self, skill_id_or_slug: &str) -> Result<()> {
        self.remove_skill_with_review(skill_id_or_slug, None)
    }

    pub fn remove_skill_with_review(
        &self,
        skill_id_or_slug: &str,
        reviewed: Option<&[ReviewedDeployment]>,
    ) -> Result<()> {
        let _lock = self.lock_skills()?;
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let library_directory = self.skills_dir.canonicalize()?;
        if !(sanitize_slug(&skill.slug).as_deref() == Some(&skill.slug)
            && physical_target_path(&skill.central_path)
                == Some(library_directory.join(&skill.slug)))
        {
            return Err(RemovalProblem::at(
                &skill.central_path,
                anyhow::anyhow!(
                    "TARGET_CONFLICT: The skill is outside its recorded library location"
                ),
            ));
        }
        let deployments = skill
            .deployments
            .iter()
            .map(|dep| DeploymentRemoval::global(&skill, dep))
            .chain(
                skill
                    .project_deployments
                    .iter()
                    .map(|dep| DeploymentRemoval::project(&skill, dep)),
            )
            .collect::<Vec<_>>();
        if let Some(reviewed) = reviewed {
            ensure!(
                deployments.len() == reviewed.len()
                    && deployments.iter().all(|dep| {
                        reviewed
                            .iter()
                            .any(|entry| entry.id == dep.id && entry.target_path == dep.path)
                    }),
                "STALE_REMOVAL: The list of deployments changed"
            );
        }
        let mut removal = RemovalBatch::default();
        self.stage_deployment_removals(&mut removal, &deployments, Some(&skill))?;
        removal
            .add_directory(&skill.central_path, None)
            .map_err(|error| RemovalProblem::at(&skill.central_path, error))?;
        if self.has_revisions_dir() {
            let previous = self.previous_revision_path(&skill.id);
            removal
                .add_directory(&previous, None)
                .map_err(|error| RemovalProblem::at(&previous, error))?;
        }
        removal.commit(|| self.store.delete_skill(&skill.id))?;
        self.cleanup_project_snapshots(skill.project_locks.iter().map(|lock| lock.hash.as_str()));
        Ok(())
    }

    pub fn undeploy_skill(&self, skill_id_or_slug: &str, agent_key: &str) -> Result<()> {
        let _lock = self.lock_skills()?;
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let Some(dep) = skill
            .deployments
            .iter()
            .find(|dep| dep.agent_key == agent_key)
        else {
            return Ok(());
        };
        let mut removal = RemovalBatch::default();
        self.stage_deployment_removals(
            &mut removal,
            &[DeploymentRemoval::global(&skill, dep)],
            None,
        )?;
        removal.commit(|| self.store.delete_deployment(&skill.id, agent_key))
    }

    pub fn remove_project(&self, project_id_or_name: &str) -> Result<()> {
        let _lock = self.lock_skills()?;
        let project = self
            .store
            .find_project(project_id_or_name)?
            .context("Project workspace not found")?;
        let skills = self.list_skills()?;
        let deployments = skills
            .iter()
            .flat_map(|skill| {
                skill
                    .project_deployments
                    .iter()
                    .map(move |dep| (skill, dep))
            })
            .filter(|(_, dep)| dep.project_id == project.id)
            .map(|(skill, dep)| DeploymentRemoval::project(skill, dep))
            .collect::<Vec<_>>();
        let mut removal = RemovalBatch::default();
        self.stage_deployment_removals(&mut removal, &deployments, None)?;
        removal.commit(|| self.store.delete_project(&project.id))?;
        self.cleanup_project_snapshots(skills.iter().flat_map(|skill| {
            skill
                .project_locks
                .iter()
                .filter(|lock| lock.project_id == project.id)
                .map(|lock| lock.hash.as_str())
        }));
        Ok(())
    }

    pub fn undeploy_skill_from_project(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        agent_key: &str,
    ) -> Result<()> {
        self.undeploy_skill_from_project_at(
            skill_id_or_slug,
            project_id_or_name,
            Path::new(""),
            agent_key,
        )
    }

    pub fn undeploy_skill_from_project_at(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        scope: &Path,
        agent_key: &str,
    ) -> Result<()> {
        self.undeploy_project_with_review(
            skill_id_or_slug,
            project_id_or_name,
            scope,
            agent_key,
            None,
        )
    }

    pub fn undeploy_project_with_review(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        scope: &Path,
        agent_key: &str,
        reviewed: Option<&ReviewedDeployment>,
    ) -> Result<()> {
        let _lock = self.lock_skills()?;
        ensure!(
            scope
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
            "Invalid subproject directory"
        );
        let scope = project_scope_key(scope);
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let project = self
            .store
            .find_project(project_id_or_name)?
            .context("Project workspace not found")?;
        let Some(dep) = skill.project_deployments.iter().find(|dep| {
            dep.project_id == project.id && dep.agent_key == agent_key && dep.scope_path == scope
        }) else {
            ensure!(
                reviewed.is_none(),
                "STALE_REMOVAL: The project deployment changed"
            );
            return Ok(());
        };
        ensure!(
            reviewed.is_none_or(|entry| entry.id == dep.id && entry.target_path == dep.target_path),
            "STALE_REMOVAL: The project deployment changed"
        );
        let mut removal = RemovalBatch::default();
        self.stage_deployment_removals(
            &mut removal,
            &[DeploymentRemoval::project(&skill, dep)],
            None,
        )?;
        removal.commit(|| self.store.delete_project_deployment(&dep.id))
    }

    fn stage_deployment_removals(
        &self,
        batch: &mut RemovalBatch,
        deployments: &[DeploymentRemoval<'_>],
        removing_skill: Option<&Skill>,
    ) -> Result<()> {
        if deployments.is_empty() && removing_skill.is_none() {
            return Ok(());
        }
        let selected: HashSet<_> = deployments.iter().map(|dep| dep.id).collect();
        let mut selected_counts = HashMap::new();
        for dep in deployments {
            *selected_counts.entry(dep.path).or_insert(0) += 1;
        }
        let skills = self.list_skills()?;
        let surviving_paths = skills
            .iter()
            .flat_map(|skill| {
                skill
                    .deployments
                    .iter()
                    .map(|dep| (dep.id.as_str(), dep.target_path.as_path()))
                    .chain(
                        skill
                            .project_deployments
                            .iter()
                            .map(|dep| (dep.id.as_str(), dep.target_path.as_path())),
                    )
            })
            .filter(|(id, _)| !selected.contains(id))
            .filter_map(|(_, path)| physical_target_path(path))
            .collect::<Vec<_>>();
        if let Some(skill) = removing_skill
            && let Some(central) = physical_target_path(&skill.central_path)
            && let Some(path) = surviving_paths.iter().find(|path| {
                paths_overlap(path, &central)
                    || classify_target(&skill.central_path, path) == TargetState::LinkToSource
            })
        {
            return Err(RemovalProblem::at(
                path,
                anyhow::anyhow!("TARGET_CONFLICT: Another deployment depends on the library skill"),
            ));
        }
        let projects: HashMap<_, _> = self
            .store
            .list_projects()?
            .into_iter()
            .map(|project| (project.id, project.path))
            .collect();
        let mut staged = HashSet::<PathBuf>::new();
        deployments.iter().try_for_each(|dep| {
            (|| {
                let physical = physical_target_path(dep.path);
                let peer_count = self.store.count_deployments_at_path(dep.path, dep.id)?;
                if peer_count + 1 > selected_counts[dep.path]
                    || physical
                        .as_ref()
                        .is_some_and(|path| surviving_paths.contains(path))
                {
                    return Ok(());
                }
                if let Some(project) = dep.project {
                    let root = projects
                        .get(&project.project_id)
                        .context("Missing deployment project")?;
                    let scope = &project.scope_path;
                    ensure!(
                        scope
                            .components()
                            .all(|part| matches!(part, Component::Normal(_)))
                            && *scope == project_scope_key(scope)
                            && dep
                                .path
                                .parent()
                                .is_some_and(|parent| parent.starts_with(root.join(scope)))
                            && dep
                                .path
                                .file_name()
                                .is_some_and(|name| name == dep.skill.slug.as_str()),
                        "TARGET_CONFLICT: The deployment is outside its recorded project scope: {}",
                        dep.path.display()
                    );
                    if !safe_scoped_target(project)? {
                        return Ok(());
                    }
                } else if removing_skill.is_none() {
                    self.ensure_unlocked_global_target(dep.path)?;
                }
                if let Some(path) = &physical {
                    ensure!(
                        surviving_paths
                            .iter()
                            .all(|other| !paths_overlap(path, other)),
                        "TARGET_CONFLICT: Another deployment overlaps {}",
                        dep.path.display()
                    );
                }
                let baseline = self.removal_copy_baseline(dep)?;
                if physical.as_ref().is_some_and(|path| staged.contains(path)) {
                    verify_managed_target(
                        &dep.skill.central_path,
                        dep.path,
                        dep.mode,
                        baseline.as_deref(),
                    )?;
                } else if batch.add_deployment(
                    &dep.skill.central_path,
                    dep.path,
                    dep.mode,
                    baseline.as_deref(),
                )? {
                    staged.insert(physical.context("Missing deployment parent")?);
                }
                Ok(())
            })()
            .map_err(|error| RemovalProblem::at(dep.path, error))
        })
    }

    fn removal_copy_baseline(&self, dep: &DeploymentRemoval<'_>) -> Result<Option<String>> {
        if dep.mode != DeployMode::Copy
            || classify_target(&dep.skill.central_path, dep.path) == TargetState::Absent
        {
            return Ok(None);
        }
        if let Some(project) = dep.project
            && project.locked_hash.is_some()
        {
            return self.locked_copy_baseline(project).map(Some);
        }
        let Some(recorded) = dep.copy_hash.or(dep.skill.content_hash.as_deref()) else {
            return Ok(None);
        };
        Ok(
            copy_baseline(&dep.skill.central_path, recorded).or_else(|| {
                self.has_revisions_dir()
                    .then(|| copy_baseline(&self.previous_revision_path(&dep.skill.id), recorded))
                    .flatten()
            }),
        )
    }

    pub(super) fn locked_copy_baseline(&self, dep: &ProjectDeployment) -> Result<String> {
        let hash = dep
            .locked_hash
            .as_deref()
            .context("Missing project version")?;
        ensure!(
            project_version_hash(&dep.target_path).is_ok_and(|actual| actual == hash),
            "TARGET_CONFLICT: The locked project copy has changed"
        );
        if let Some(recorded) = dep.copy_hash.as_deref()
            && recorded.starts_with(COPY_FINGERPRINT_PREFIX)
        {
            return Ok(recorded.to_owned());
        }
        // Legacy project pins identify a trusted snapshot, even if the library
        // skill has since advanced to a different revision.
        let snapshot = self
            .checked_project_snapshot(hash)
            .context("TARGET_CONFLICT: Cannot verify the original locked project copy")?;
        managed_copy_fingerprint(&snapshot)
    }

    pub(super) fn cleanup_project_snapshots<'a>(&self, hashes: impl Iterator<Item = &'a str>) {
        for hash in hashes.collect::<HashSet<_>>() {
            // The filesystem and DB removal have already committed. A broken
            // or inaccessible snapshot must not prevent the user's removal.
            let _ = self.remove_unreferenced_snapshot(hash);
        }
    }
}
