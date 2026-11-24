use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use super::UnoSkillsManager;
use super::project_versions::text_diff;
use super::skill_revisions::{SkillFileChange, SkillFileChangeKind, planned_targets};
use crate::domain::{AdoptCandidate, AgentInfo, DeployMode, Deployment, DeploymentImpact, Skill};
use crate::parser::{
    checked_directory_hash, find_skill_entry, managed_copy_fingerprint, parse_skill_dir,
    sanitize_slug, skill_tree,
};
use crate::sync::{
    RemovalBatch, TargetState, classify_target, copy_dir_recursive, create_dir_link,
    ensure_dst_not_overlapping_src, paths_overlap, physical_target_path, rename_without_replacing,
    verify_managed_target,
};

#[derive(Debug)]
pub struct AdoptionMergeConflict;

impl std::fmt::Display for AdoptionMergeConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .write_str("TARGET_CONFLICT: This agent already uses the library skill at another path")
    }
}

impl std::error::Error for AdoptionMergeConflict {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionMergeTarget {
    Existing,
    Detected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionMergeChoice {
    Library,
    Local,
}

#[derive(Debug, Deserialize)]
pub struct AdoptionMergeRequest {
    pub candidate: AdoptCandidate,
    pub token: String,
    pub target: AdoptionMergeTarget,
    pub content: AdoptionMergeChoice,
    #[serde(default)]
    pub allow_copy: bool,
}

#[derive(Debug, Serialize)]
pub struct AdoptionMergePreview {
    pub token: String,
    pub candidate: AdoptCandidate,
    pub library_path: PathBuf,
    pub existing_path: PathBuf,
    pub existing_present: bool,
    pub keep_existing_for_peers: bool,
    pub targets: Vec<AdoptionMergeTarget>,
    pub identical: bool,
    pub library_changes: Vec<SkillFileChange>,
    pub existing_changes: Vec<SkillFileChange>,
    pub skill_diff: Option<String>,
    pub local_version_impacts: Vec<DeploymentImpact>,
    pub shared_existing: Vec<String>,
    pub shared_detected: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AdoptionMergeResult {
    pub path: PathBuf,
    pub deployment: Deployment,
    pub backup: Option<PathBuf>,
    pub deployment_issues: Vec<DeploymentImpact>,
}

#[derive(Serialize)]
struct ReviewedDirectory {
    physical: PathBuf,
    source: PathBuf,
    mode: DeployMode,
    fingerprint: String,
}

impl ReviewedDirectory {
    fn read(path: &Path) -> Result<Option<Self>> {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            result => result?,
        };
        let physical = physical_target_path(path).context("Could not resolve the skill parent")?;
        let source = path.canonicalize()?;
        let mode = match classify_target(&source, path) {
            TargetState::LinkToSource => DeployMode::Symlink,
            TargetState::RealDir => DeployMode::Copy,
            _ => bail!(
                "ADOPTION_MERGE_PROTECTED: Unrecognized skill directory at {}",
                path.display()
            ),
        };
        ensure!(
            find_skill_entry(&source).is_some(),
            "ADOPTION_MERGE_PROTECTED: Missing SKILL.md at {}",
            path.display()
        );
        let fingerprint = managed_copy_fingerprint(&source)?;
        Ok(Some(Self {
            physical,
            source,
            mode,
            fingerprint,
        }))
    }

    fn stage(&self, changes: &mut RemovalBatch, path: &Path) -> Result<()> {
        ensure!(
            physical_target_path(path).as_ref() == Some(&self.physical),
            "STALE_ADOPTION_MERGE: The skill location changed"
        );
        let added = match self.mode {
            DeployMode::Copy => changes.add_directory(path, Some(self.fingerprint.clone()))?,
            DeployMode::Symlink => changes.add_deployment(&self.source, path, self.mode, None)?,
        };
        ensure!(
            added,
            "STALE_ADOPTION_MERGE: The skill directory disappeared"
        );
        Ok(())
    }
}

struct AdoptionPlan {
    preview: AdoptionMergePreview,
    skill: Skill,
    deployment: Deployment,
    library: ReviewedDirectory,
    detected: ReviewedDirectory,
    existing: Option<ReviewedDirectory>,
}

impl UnoSkillsManager {
    pub fn preview_adoption_merge(
        &self,
        candidate: &AdoptCandidate,
    ) -> Result<AdoptionMergePreview> {
        let _lock = self.lock_skills()?;
        Ok(self
            .plan_adoption_merge(candidate, &self.list_agents()?)?
            .preview)
    }

    fn plan_adoption_merge(
        &self,
        candidate: &AdoptCandidate,
        agents: &[AgentInfo],
    ) -> Result<AdoptionPlan> {
        let slug = candidate
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(sanitize_slug)
            .context("Missing local skill name")?;
        let agent = agents
            .iter()
            .find(|agent| agent.key == candidate.agent_key && agent.installed)
            .context("STALE_ADOPTION_MERGE: The agent is no longer available")?;
        ensure!(
            agent_scans(agent, &candidate.path),
            "STALE_ADOPTION_MERGE: The agent no longer scans this directory"
        );
        let skills = self.store.list_skills()?;
        let skill = skills
            .iter()
            .find(|skill| skill.slug.eq_ignore_ascii_case(&slug))
            .context("STALE_ADOPTION_MERGE: The library skill is no longer available")?;
        let deployment = skill
            .deployments
            .iter()
            .find(|dep| dep.agent_key == agent.key)
            .context("STALE_ADOPTION_MERGE: The agent's deployment is missing")?;
        let existing_path = &deployment.target_path;
        ensure!(
            !same_location(&candidate.path, existing_path),
            "STALE_ADOPTION_MERGE: This directory is already managed"
        );
        self.check_library_path(skill)?;
        for path in [&candidate.path, existing_path] {
            self.ensure_unlocked_global_target(path)?;
            ensure_dst_not_overlapping_src(&skill.central_path, path)?;
        }

        let library =
            ReviewedDirectory::read(&skill.central_path)?.context("Missing library directory")?;
        let detected =
            ReviewedDirectory::read(&candidate.path)?.context("Missing detected directory")?;
        let existing = ReviewedDirectory::read(existing_path)?;
        let existing_in_scan = agent_scans(agent, existing_path);
        ensure!(
            library.mode == DeployMode::Copy
                && !locations_overlap(&candidate.path, existing_path)
                && !locations_overlap(&candidate.path, &skill.central_path)
                && !locations_overlap(existing_path, &skill.central_path)
                && (existing.is_none()
                    || existing_in_scan
                    || verify_managed_target(
                        &skill.central_path,
                        existing_path,
                        deployment.mode,
                        deployment
                            .copy_hash
                            .as_deref()
                            .or(skill.content_hash.as_deref())
                    )
                    .is_ok()),
            "ADOPTION_MERGE_PROTECTED: The deployment overlaps a protected directory or has changed outside the agent's scan paths"
        );

        let shared = |path: &Path| -> Result<Vec<String>> {
            let mut references = BTreeSet::new();
            for item in &skills {
                for record in &item.project_deployments {
                    ensure!(
                        !locations_overlap(&record.target_path, path),
                        "ADOPTION_MERGE_PROTECTED: A project uses this path"
                    );
                }
                for record in item
                    .deployments
                    .iter()
                    .filter(|record| record.id != deployment.id)
                {
                    if locations_overlap(&record.target_path, path) {
                        ensure!(
                            item.id == skill.id && same_location(&record.target_path, path),
                            "ADOPTION_MERGE_PROTECTED: Another skill uses this path"
                        );
                        references.insert(agent_label(agents, &record.agent_key).to_owned());
                    }
                }
            }
            let physical = physical_target_path(path).unwrap_or_else(|| path.to_owned());
            for peer in agents.iter().filter(|peer| peer.installed) {
                for root in agent_roots(peer).filter_map(|root| root.canonicalize().ok()) {
                    ensure!(
                        !contains_path(&physical, &root),
                        "ADOPTION_MERGE_PROTECTED: A skill folder contains an agent's scan directory"
                    );
                }
                if peer.key != agent.key && reads_deployment(peer, &physical) {
                    references.insert(peer.display_name.clone());
                }
            }
            Ok(references.into_iter().collect())
        };
        let shared_existing = shared(existing_path)?;
        let shared_detected = shared(&candidate.path)?;
        let keep_existing_for_peers = !existing_in_scan && !shared_existing.is_empty();
        let mut targets = Vec::new();
        if existing.is_some() && existing_in_scan && shared_detected.is_empty() {
            targets.push(AdoptionMergeTarget::Existing);
        }
        if shared_existing.is_empty() || existing.is_none() || keep_existing_for_peers {
            targets.push(AdoptionMergeTarget::Detected);
        }
        let same_content = library.fingerprint == detected.fingerprint;
        let local_version_impacts = if same_content {
            Vec::new()
        } else {
            planned_targets(
                skill,
                &detected.fingerprint,
                &self.store.locked_project_target_paths()?,
            )
            .into_iter()
            .filter(|target| {
                (keep_existing_for_peers || !same_location(&target.impact.path, existing_path))
                    && !same_location(&target.impact.path, &candidate.path)
            })
            .map(|target| target.impact)
            .collect()
        };
        let mut preview = AdoptionMergePreview {
            token: String::new(),
            candidate: candidate.clone(),
            library_path: skill.central_path.clone(),
            existing_path: existing_path.clone(),
            existing_present: existing.is_some(),
            keep_existing_for_peers,
            targets,
            identical: same_content
                && existing
                    .as_ref()
                    .is_none_or(|dir| dir.fingerprint == library.fingerprint),
            library_changes: changes_between(&library.source, &detected.source)?,
            existing_changes: existing
                .as_ref()
                .map(|dir| changes_between(&library.source, &dir.source))
                .transpose()?
                .unwrap_or_default(),
            skill_diff: match (
                read_small_skill(&library.source),
                read_small_skill(&detected.source),
            ) {
                (Some(left), Some(right)) => text_diff(&left, &right),
                _ => None,
            },
            local_version_impacts,
            shared_existing,
            shared_detected,
        };
        let involved: Vec<_> = skill
            .deployments
            .iter()
            .filter(|dep| {
                same_location(&dep.target_path, existing_path)
                    || same_location(&dep.target_path, &candidate.path)
            })
            .collect();
        preview.token = hex::encode(Sha256::digest(serde_json::to_vec(&(
            &preview, &library, &detected, &existing, &skill.id, deployment, involved,
        ))?));
        Ok(AdoptionPlan {
            preview,
            skill: skill.clone(),
            deployment: deployment.clone(),
            library,
            detected,
            existing,
        })
    }

    pub fn apply_adoption_merge(
        &self,
        request: AdoptionMergeRequest,
    ) -> Result<AdoptionMergeResult> {
        let _lock = self.lock_skills()?;
        self.apply_reviewed_adoption_merge(request, &self.list_agents()?)
    }

    fn apply_reviewed_adoption_merge(
        &self,
        request: AdoptionMergeRequest,
        agents: &[AgentInfo],
    ) -> Result<AdoptionMergeResult> {
        let AdoptionPlan {
            preview,
            mut skill,
            mut deployment,
            library,
            detected,
            existing,
        } = self.plan_adoption_merge(&request.candidate, agents)?;
        ensure!(
            preview.token == request.token,
            "STALE_ADOPTION_MERGE: Preview the current files and paths again"
        );
        ensure!(
            preview.targets.contains(&request.target),
            "ADOPTION_MERGE_PROTECTED: That location must be kept for another agent"
        );
        let update_library = request.content == AdoptionMergeChoice::Local
            && library.fingerprint != detected.fingerprint;
        let chosen = if update_library { &detected } else { &library };
        let destination = match request.target {
            AdoptionMergeTarget::Existing => &preview.existing_path,
            AdoptionMergeTarget::Detected => &preview.candidate.path,
        };
        let mut changes = RemovalBatch::default();
        let replace_existing = existing
            .as_ref()
            .filter(|_| !preview.keep_existing_for_peers);
        let backup = self.backup_adoption_merge(
            &preview,
            &library,
            &detected,
            replace_existing,
            &chosen.fingerprint,
            agents,
        )?;
        let other_targets = if update_library {
            planned_targets(
                &skill,
                &chosen.fingerprint,
                &self.store.locked_project_target_paths()?,
            )
            .into_iter()
            .filter(|target| {
                preview
                    .local_version_impacts
                    .iter()
                    .any(|impact| impact.path == target.impact.path)
            })
            .collect()
        } else {
            Vec::new()
        };
        if update_library {
            library.stage(&mut changes, &skill.central_path)?;
            changes.replace_last(&skill.central_path, |path| copy_reviewed(chosen, path))?;
            let metadata = parse_skill_dir(&chosen.source);
            skill.name = metadata.name.unwrap_or_else(|| skill.slug.clone());
            skill.description = metadata.description;
            skill.estimated_tokens = metadata.estimated_tokens;
            skill.tags = metadata.tags;
            skill.content_hash = Some(checked_directory_hash(&chosen.source)?);
            skill.updated_at = Utc::now().timestamp();
        }

        let mut mode = DeployMode::Symlink;
        for (path, review) in [
            (&preview.existing_path, replace_existing),
            (&preview.candidate.path, Some(&detected)),
        ] {
            let Some(review) = review else { continue };
            if path == destination
                && review.mode == DeployMode::Symlink
                && review.source == library.source
            {
                continue;
            }
            review.stage(&mut changes, path)?;
            if path == destination {
                mode = changes.replace_last(path, |staged| {
                    if try_stage_link(&library.source, staged)? {
                        return Ok(DeployMode::Symlink);
                    }
                    ensure!(
                        request.allow_copy,
                        "ADOPTION_LINK_UNAVAILABLE: This directory needs a managed copy"
                    );
                    copy_reviewed(chosen, staged)?;
                    Ok(DeployMode::Copy)
                })?;
            }
        }

        deployment.target_path = destination.clone();
        deployment.mode = mode;
        deployment.copy_hash = (mode == DeployMode::Copy).then(|| chosen.fingerprint.clone());
        deployment.synced_at = Utc::now().timestamp();
        let mut new_deployments = vec![deployment.clone()];
        new_deployments.extend(
            skill
                .deployments
                .iter()
                .filter(|dep| {
                    dep.id != deployment.id && same_location(&dep.target_path, destination)
                })
                .map(|dep| Deployment {
                    id: dep.id.clone(),
                    agent_key: dep.agent_key.clone(),
                    target_path: dep.target_path.clone(),
                    ..deployment.clone()
                }),
        );
        changes.commit(|| {
            let retired = match request.target {
                AdoptionMergeTarget::Existing => Some(&preview.candidate.path),
                AdoptionMergeTarget::Detected if !preview.keep_existing_for_peers => {
                    Some(&preview.existing_path)
                }
                AdoptionMergeTarget::Detected => None,
            };
            ensure!(
                managed_copy_fingerprint(&skill.central_path)? == chosen.fingerprint
                    && verify_managed_target(
                        &skill.central_path,
                        destination,
                        mode,
                        deployment.copy_hash.as_deref()
                    )? != TargetState::Absent
                    && retired.is_none_or(|path| fs::symlink_metadata(path)
                        .is_err_and(|error| error.kind() == ErrorKind::NotFound)),
                "STALE_ADOPTION_MERGE: The chosen library files or deployment changed"
            );
            self.store
                .save_adoption_merge(update_library.then_some(&skill), &new_deployments)
        })?;
        let backup = backup.map(TempDir::keep);
        let deployment_issues = if update_library {
            skill
                .deployments
                .retain(|dep| !new_deployments.iter().any(|next| next.id == dep.id));
            skill.deployments.extend(new_deployments);
            self.refresh_revision_targets(skill, other_targets, &chosen.fingerprint)
                .deployment_issues
        } else {
            Vec::new()
        };
        Ok(AdoptionMergeResult {
            path: preview.candidate.path,
            deployment,
            backup,
            deployment_issues,
        })
    }

    fn backup_adoption_merge(
        &self,
        preview: &AdoptionMergePreview,
        library: &ReviewedDirectory,
        detected: &ReviewedDirectory,
        existing: Option<&ReviewedDirectory>,
        retained: &str,
        agents: &[AgentInfo],
    ) -> Result<Option<TempDir>> {
        let directories = [
            ("library", Some(library), &preview.library_path),
            ("detected", Some(detected), &preview.candidate.path),
            ("deployment", existing, &preview.existing_path),
        ];
        let differing: Vec<_> = directories
            .into_iter()
            .filter_map(|(label, directory, path)| {
                directory
                    .filter(|dir| dir.fingerprint != retained)
                    .map(|dir| (label, dir, path))
            })
            .collect();
        if differing.is_empty() {
            return Ok(None);
        }
        let root = self.root_dir.canonicalize()?.join(".adoption-backups");
        ensure!(
            !agents
                .iter()
                .any(|agent| agent_roots(agent).any(|path| locations_overlap(&root, path))),
            "ADOPTION_MERGE_PROTECTED: Recovery location overlaps an agent's skills directory"
        );
        fs::create_dir_all(&root)?;
        ensure!(
            fs::symlink_metadata(&root)?.is_dir() && root.canonicalize()? == root,
            "ADOPTION_MERGE_PROTECTED: Recovery directory is redirected"
        );
        let backup = tempfile::Builder::new()
            .prefix("merge-")
            .tempdir_in(&root)?;
        for (label, directory, _) in &differing {
            copy_reviewed(directory, &backup.path().join(label))?;
        }
        let paths: Vec<_> = differing
            .iter()
            .map(|(label, _, path)| (label, path))
            .collect();
        fs::write(
            backup.path().join("original-paths.json"),
            serde_json::to_vec_pretty(&paths)?,
        )?;
        Ok(Some(backup))
    }
}

fn copy_reviewed(directory: &ReviewedDirectory, target: &Path) -> Result<()> {
    ensure_dst_not_overlapping_src(&directory.source, target)?;
    copy_dir_recursive(&directory.source, target)?;
    ensure!(
        managed_copy_fingerprint(target)? == directory.fingerprint,
        "STALE_ADOPTION_MERGE: Files changed while preparing the merge"
    );
    Ok(())
}

fn try_stage_link(source: &Path, staged: &Path) -> Result<bool> {
    let trial = staged.with_file_name("link");
    if create_dir_link(source, &trial).is_ok()
        && classify_target(source, &trial) == TargetState::LinkToSource
    {
        rename_without_replacing(&trial, staged)?;
        return Ok(true);
    }
    #[cfg(windows)]
    {
        let trial = staged.with_file_name("junction");
        if junction::create(source, &trial).is_ok()
            && classify_target(source, &trial) == TargetState::LinkToSource
        {
            rename_without_replacing(&trial, staged)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn agent_roots(agent: &AgentInfo) -> impl Iterator<Item = &Path> {
    std::iter::once(agent.skills_dir.as_path())
        .chain(agent.additional_scan_dirs.iter().map(PathBuf::as_path))
}

fn agent_scans(agent: &AgentInfo, path: &Path) -> bool {
    physical_target_path(path)
        .and_then(|path| path.parent().map(Path::to_owned))
        .is_some_and(|parent| {
            agent_roots(agent).any(|root| {
                root.canonicalize()
                    .is_ok_and(|root| same_path(&parent, &root))
            })
        })
}

fn reads_deployment(agent: &AgentInfo, physical: &Path) -> bool {
    physical.parent().is_some_and(|parent| {
        agent_roots(agent)
            .filter_map(|root| root.canonicalize().ok())
            .any(|root| {
                same_path(&root, parent) || (agent.recursive_scan && contains_path(&root, parent))
            })
    })
}

fn contains_path(directory: &Path, path: &Path) -> bool {
    path.components().count() >= directory.components().count() && paths_overlap(directory, path)
}

fn agent_label<'a>(agents: &'a [AgentInfo], key: &'a str) -> &'a str {
    agents
        .iter()
        .find(|agent| agent.key == key)
        .map_or(key, |agent| agent.display_name.as_str())
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right || (cfg!(windows) && left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
}

fn same_location(left: &Path, right: &Path) -> bool {
    same_path(left, right)
        || physical_target_path(left)
            .zip(physical_target_path(right))
            .is_some_and(|(left, right)| same_path(&left, &right))
}

fn locations_overlap(left: &Path, right: &Path) -> bool {
    paths_overlap(left, right)
        || physical_target_path(left)
            .zip(physical_target_path(right))
            .is_some_and(|(left, right)| paths_overlap(&left, &right))
}

fn changes_between(base: &Path, candidate: &Path) -> Result<Vec<SkillFileChange>> {
    let before = skill_tree(base)?;
    let after = skill_tree(candidate)?;
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

fn read_small_skill(root: &Path) -> Option<String> {
    let path = root.join(find_skill_entry(root)?);
    (fs::metadata(&path).ok()?.len() <= 24 * 1024)
        .then(|| fs::read_to_string(path).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _directory: TempDir,
        manager: UnoSkillsManager,
        skill: Skill,
        existing: PathBuf,
        candidate: AdoptCandidate,
        agents: Vec<AgentInfo>,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let manager = UnoSkillsManager::open_at(directory.path().join("library")).unwrap();
            let source = directory.path().join("source/reviewer");
            let installed = directory.path().join("codex");
            let candidate_path = directory.path().join("other/reviewer");
            for path in [&source, &candidate_path] {
                fs::create_dir_all(path).unwrap();
                fs::write(
                    path.join("SKILL.md"),
                    "---\nname: reviewer\n---\nLibrary version\n",
                )
                .unwrap();
                fs::write(path.join("checks.txt"), "Original checks").unwrap();
            }
            fs::create_dir_all(&installed).unwrap();
            manager
                .set_agent_override("codex", installed.to_str())
                .unwrap();
            let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
            let existing = manager
                .deploy_skill(&skill.id, "codex", Some(DeployMode::Copy), false)
                .unwrap()
                .target_path;
            let mut agents = manager.list_agents().unwrap();
            agents.retain(|agent| agent.key == "codex");
            agents[0].additional_scan_dirs = vec![candidate_path.parent().unwrap().to_owned()];
            Self {
                _directory: directory,
                manager,
                skill,
                existing,
                candidate: AdoptCandidate {
                    agent_key: "codex".into(),
                    agent_name: "Codex".into(),
                    name: "reviewer".into(),
                    path: candidate_path,
                    description: None,
                    estimated_tokens: 0,
                },
                agents,
            }
        }

        fn preview(&self) -> AdoptionMergePreview {
            self.manager
                .plan_adoption_merge(&self.candidate, &self.agents)
                .unwrap()
                .preview
        }

        fn request(
            &self,
            preview: &AdoptionMergePreview,
            target: AdoptionMergeTarget,
            content: AdoptionMergeChoice,
        ) -> AdoptionMergeRequest {
            AdoptionMergeRequest {
                candidate: self.candidate.clone(),
                token: preview.token.clone(),
                target,
                content,
                allow_copy: true,
            }
        }

        fn apply(&self, request: AdoptionMergeRequest) -> Result<AdoptionMergeResult> {
            let _lock = self.manager.lock_skills()?;
            self.manager
                .apply_reviewed_adoption_merge(request, &self.agents)
        }
    }

    #[test]
    fn merges_identical_directories_into_the_existing_location() {
        let fixture = Fixture::new();
        let error = fixture
            .manager
            .adopt_candidate(&fixture.candidate)
            .unwrap_err();
        assert!(error.is::<AdoptionMergeConflict>());
        let review = fixture.preview();
        assert!(review.identical);
        assert_eq!(
            review.targets,
            [AdoptionMergeTarget::Existing, AdoptionMergeTarget::Detected]
        );
        let request = || {
            fixture.request(
                &review,
                AdoptionMergeTarget::Existing,
                AdoptionMergeChoice::Library,
            )
        };
        let mut link_only = request();
        link_only.allow_copy = false;
        let merged = match fixture.apply(link_only) {
            Ok(merged) => {
                assert_eq!(merged.deployment.mode, DeployMode::Symlink);
                merged
            }
            Err(error) => {
                assert!(
                    error.to_string().contains("ADOPTION_LINK_UNAVAILABLE"),
                    "{error:#}"
                );
                assert!(fixture.candidate.path.is_dir());
                assert!(fixture.existing.is_dir());
                let merged = fixture.apply(request()).unwrap();
                assert_eq!(merged.deployment.mode, DeployMode::Copy);
                merged
            }
        };
        assert!(merged.backup.is_none());
        assert_eq!(merged.deployment.target_path, fixture.existing);
        assert!(!fixture.candidate.path.exists());
        assert_eq!(
            fs::read_to_string(fixture.existing.join("checks.txt")).unwrap(),
            "Original checks"
        );
        #[cfg(unix)]
        assert_eq!(
            fixture.existing.canonicalize().unwrap(),
            fixture.skill.central_path.canonicalize().unwrap()
        );
        let skill = fixture
            .manager
            .find_skill(&fixture.skill.id)
            .unwrap()
            .unwrap();
        assert_eq!(skill.deployments.len(), 1);
        assert_eq!(skill.deployments[0].target_path, fixture.existing);
    }

    #[test]
    fn public_merge_rebinds_to_the_current_agent_folder_and_recovers_the_different_copy() {
        let fixture = Fixture::new();
        fixture
            .manager
            .set_agent_override("codex", fixture.candidate.path.parent().unwrap().to_str())
            .unwrap();
        fs::write(fixture.candidate.path.join("checks.txt"), "Detected edits").unwrap();
        let preview = fixture
            .manager
            .preview_adoption_merge(&fixture.candidate)
            .unwrap();
        assert_eq!(preview.targets, [AdoptionMergeTarget::Detected]);
        let result = fixture
            .manager
            .apply_adoption_merge(fixture.request(
                &preview,
                AdoptionMergeTarget::Detected,
                AdoptionMergeChoice::Library,
            ))
            .unwrap();
        assert!(!fixture.existing.exists());
        assert_eq!(result.deployment.target_path, fixture.candidate.path);
        assert_eq!(
            fs::read_to_string(fixture.candidate.path.join("checks.txt")).unwrap(),
            "Original checks"
        );
        assert_eq!(
            fs::read_to_string(result.backup.unwrap().join("detected/checks.txt")).unwrap(),
            "Detected edits"
        );
    }

    #[test]
    fn applies_selected_local_contents_and_keeps_last_version_in_recovery() {
        let fixture = Fixture::new();
        fs::write(
            fixture.candidate.path.join("SKILL.md"),
            "---\nname: reviewer\n---\nLocal version\n",
        )
        .unwrap();
        fs::remove_file(fixture.candidate.path.join("checks.txt")).unwrap();
        fs::write(fixture.existing.join("notes.txt"), "Edited deployed notes").unwrap();
        let review = fixture.preview();
        assert!(!review.identical);
        assert!(
            review
                .skill_diff
                .as_ref()
                .unwrap()
                .contains("+ Local version")
        );
        assert!(review.local_version_impacts.is_empty());
        let result = fixture
            .apply(fixture.request(
                &review,
                AdoptionMergeTarget::Detected,
                AdoptionMergeChoice::Local,
            ))
            .unwrap();
        assert_eq!(result.deployment.target_path, fixture.candidate.path);
        let backup = result.backup.unwrap();
        assert!(
            backup.starts_with(
                fixture
                    .manager
                    .root_dir()
                    .canonicalize()
                    .unwrap()
                    .join(".adoption-backups")
            )
        );
        assert_eq!(
            fs::read_to_string(backup.join("library/checks.txt")).unwrap(),
            "Original checks"
        );
        assert_eq!(
            fs::read_to_string(backup.join("deployment/notes.txt")).unwrap(),
            "Edited deployed notes"
        );
        assert!(!fixture.existing.exists());
        assert!(!fixture.skill.central_path.join("checks.txt").exists());
        assert!(
            fs::read_to_string(fixture.skill.central_path.join("SKILL.md"))
                .unwrap()
                .contains("Local version")
        );
    }

    #[test]
    fn adopting_local_version_skips_pinned_projects() {
        let fixture = Fixture::new();
        let project_path = fixture._directory.path().join("project");
        fs::create_dir(&project_path).unwrap();
        let project = fixture.manager.add_project(&project_path, None).unwrap();
        let pinned_path = fixture
            .manager
            .deploy_skill_to_project(
                &fixture.skill.id,
                &project.id,
                "codex",
                Some(DeployMode::Copy),
                false,
            )
            .unwrap()
            .target_path;
        fixture
            .manager
            .lock_project_skill(&project.id, Path::new(""), "codex", &fixture.skill.id)
            .unwrap();
        fs::write(
            fixture.candidate.path.join("SKILL.md"),
            "---\nname: reviewer\n---\nLocal version\n",
        )
        .unwrap();
        let review = fixture.preview();
        assert!(review.local_version_impacts.is_empty());
        fixture
            .apply(fixture.request(
                &review,
                AdoptionMergeTarget::Existing,
                AdoptionMergeChoice::Local,
            ))
            .unwrap();
        assert!(
            fs::read_to_string(pinned_path.join("SKILL.md"))
                .unwrap()
                .contains("Library version")
        );
    }

    #[test]
    fn shared_location_is_kept_and_stale_edits_block_the_merge() {
        let mut fixture = Fixture::new();
        let mut peer = fixture.agents[0].clone();
        peer.key = "other".into();
        peer.display_name = "Other Agent".into();
        peer.additional_scan_dirs.clear();
        fixture.agents.push(peer);
        let review = fixture.preview();
        assert_eq!(review.shared_existing, ["Other Agent"]);
        assert_eq!(review.targets, [AdoptionMergeTarget::Existing]);
        let request = fixture.request(
            &review,
            AdoptionMergeTarget::Detected,
            AdoptionMergeChoice::Library,
        );
        assert!(
            fixture
                .apply(request)
                .unwrap_err()
                .to_string()
                .contains("ADOPTION_MERGE_PROTECTED")
        );
        fs::write(
            fixture.candidate.path.join("checks.txt"),
            "Saved after preview",
        )
        .unwrap();
        let request = fixture.request(
            &review,
            AdoptionMergeTarget::Existing,
            AdoptionMergeChoice::Library,
        );
        assert!(
            fixture
                .apply(request)
                .unwrap_err()
                .to_string()
                .contains("STALE_ADOPTION_MERGE")
        );
        assert_eq!(
            fs::read_to_string(fixture.candidate.path.join("checks.txt")).unwrap(),
            "Saved after preview"
        );
        assert_eq!(
            fs::read_to_string(fixture.existing.join("checks.txt")).unwrap(),
            "Original checks"
        );
    }

    #[test]
    fn database_failure_restores_all_merge_paths() {
        let fixture = Fixture::new();
        fs::write(fixture.candidate.path.join("checks.txt"), "Local changes").unwrap();
        let review = fixture.preview();
        let connection =
            rusqlite::Connection::open(fixture.manager.root_dir().join("unoskills.db")).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_adoption BEFORE UPDATE ON deployments
             BEGIN SELECT RAISE(ABORT, 'test merge failure'); END;",
            )
            .unwrap();
        let result = fixture.apply(fixture.request(
            &review,
            AdoptionMergeTarget::Detected,
            AdoptionMergeChoice::Local,
        ));
        assert!(format!("{:#}", result.unwrap_err()).contains("test merge failure"));
        assert_eq!(
            fs::read_to_string(fixture.existing.join("checks.txt")).unwrap(),
            "Original checks"
        );
        assert_eq!(
            fs::read_to_string(fixture.skill.central_path.join("checks.txt")).unwrap(),
            "Original checks"
        );
        assert_eq!(
            fs::read_to_string(fixture.candidate.path.join("checks.txt")).unwrap(),
            "Local changes"
        );
        let skill = fixture
            .manager
            .find_skill(&fixture.skill.id)
            .unwrap()
            .unwrap();
        assert_eq!(skill.deployments[0].target_path, fixture.existing);
    }
}
