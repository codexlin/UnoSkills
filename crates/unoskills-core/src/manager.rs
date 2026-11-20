use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::backup::{
    DEFAULT_BACKUP_BRANCH, GitBackupOpts, list_synced_artifacts, mirror_commit_and_push,
    normalize_branch, prepare_sync_repo,
};
use crate::domain::{
    AdoptCandidate, AgentInfo, DeployMode, Deployment, GitBackupPullResult, GitBackupPushResult,
    GitBackupStatus, MarketplacePage, MarketplaceSkill, ProjectDeployment, ProjectWorkspace, Skill,
    SkillLintReport, SkillUpdateResult, SourceType, StackInstallSummary, StackManifest,
    StackProjectDeployStatus, StackProjectDeploySummary, StackSkillRef, StackSource,
};
use crate::parser::{
    find_skill_entry, hash_directory, is_valid_skill_dir, lint_skill_dir, parse_skill_dir,
    sanitize_slug,
};
use crate::registry::{CustomAgentRecord, resolve_agents};
use crate::source::{
    ResolvedSource, fetch_git_skill, fetch_skillssh_leaderboard, fetch_skillssh_leaderboard_page,
    resolve_source_ref, scan_adoptable_skills, search_skillssh,
};
use crate::stacks::{
    DefaultStackRegistry, DeviceCodeRequest, StackRegistry, export_stack_to_toml,
    parse_community_index, parse_stack_from_toml, poll_github_device_token,
    request_github_device_code, share_stack_to_gist,
};
use crate::store::Store;
use crate::sync::{
    ReplacePolicy, TargetState, classify_target, copy_dir_recursive, deploy_skill_dir,
    ensure_dst_not_overlapping_src, remove_managed_target,
};
use crate::updater::{
    DEFAULT_RELEASES_API_URL, DownloadedUpdate, PlatformKind, ReleaseAsset, UpdateCheckOutcome,
    download_and_verify_update, fetch_latest_release, find_checksums_asset, is_newer_semver,
    select_platform_asset, should_run_startup_check,
};

pub struct UnoSkillsManager {
    root_dir: PathBuf,
    skills_dir: PathBuf,
    store: Store,
}

enum StackProjectTarget {
    Deployed,
    Missing(Option<DeployMode>),
    Register(DeployMode),
    Conflict,
    Unavailable,
}

fn project_skill_target(project: &ProjectWorkspace, agent: &AgentInfo, skill: &Skill) -> PathBuf {
    let relative_dir = agent.project_relative_skills_dir.trim();
    let relative_dir = if relative_dir.is_empty() {
        ".agents/skills"
    } else {
        relative_dir
    };
    project.path.join(relative_dir).join(&skill.slug)
}

fn inspect_stack_project_target(
    skill: &Skill,
    project: &ProjectWorkspace,
    agent: &AgentInfo,
    target: &Path,
) -> StackProjectTarget {
    let matching_records = || {
        skill
            .project_deployments
            .iter()
            .filter(|dep| dep.project_id == project.id && dep.target_path == target)
    };
    let mode = match classify_target(&skill.central_path, target) {
        TargetState::Absent if is_valid_skill_dir(&skill.central_path) => {
            let recorded_mode = matching_records()
                .find(|dep| dep.agent_key == agent.key)
                .or_else(|| matching_records().next())
                .map(|dep| dep.mode);
            return StackProjectTarget::Missing(recorded_mode);
        }
        TargetState::Absent => return StackProjectTarget::Unavailable,
        TargetState::LinkToSource if is_valid_skill_dir(&skill.central_path) => DeployMode::Symlink,
        TargetState::RealDir if matching_records().any(|dep| dep.mode == DeployMode::Copy) => {
            if !is_valid_skill_dir(target) {
                return StackProjectTarget::Conflict;
            }
            let Ok(actual_hash) = hash_directory(target) else {
                return StackProjectTarget::Conflict;
            };
            if !matching_records().any(|dep| {
                dep.mode == DeployMode::Copy
                    && dep.copy_hash.as_deref().or(skill.content_hash.as_deref())
                        == Some(actual_hash.as_str())
            }) {
                return StackProjectTarget::Conflict;
            }
            DeployMode::Copy
        }
        _ => return StackProjectTarget::Conflict,
    };
    if matching_records().any(|dep| dep.agent_key == agent.key && dep.mode == mode) {
        StackProjectTarget::Deployed
    } else {
        StackProjectTarget::Register(mode)
    }
}

impl UnoSkillsManager {
    pub fn open_default() -> Result<Self> {
        let home = dirs::home_dir().context("Cannot resolve home directory")?;
        Self::open_at(home.join(".unoskills"))
    }

    pub fn open_at(root_dir: PathBuf) -> Result<Self> {
        let skills_dir = root_dir.join("skills");
        std::fs::create_dir_all(&skills_dir)?;
        let store = Store::open(&root_dir.join("unoskills.db"))?;
        Ok(Self {
            root_dir,
            skills_dir,
            store,
        })
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    // ── Settings ──

    pub fn default_deploy_mode(&self) -> DeployMode {
        self.store
            .get_setting("deploy_mode")
            .ok()
            .flatten()
            .map(|s| DeployMode::from_str_lossy(&s))
            .unwrap_or(DeployMode::Symlink)
    }

    pub fn set_default_deploy_mode(&self, mode: DeployMode) -> Result<()> {
        self.store.set_setting("deploy_mode", mode.as_str())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.store.get_setting(key)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.store.set_setting(key, value)
    }

    pub fn http_proxy(&self) -> Option<String> {
        self.store
            .get_setting("http_proxy")
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn set_http_proxy(&self, proxy: Option<&str>) -> Result<()> {
        let val = proxy.map(str::trim).unwrap_or("");
        self.store.set_setting("http_proxy", val)
    }

    pub fn github_token(&self) -> Option<String> {
        self.store
            .get_setting("github_token")
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn set_github_token(&self, token: Option<&str>) -> Result<()> {
        let val = token.map(str::trim).unwrap_or("");
        self.store.set_setting("github_token", val)
    }

    // ── Agents ──

    pub fn list_agents(&self) -> Result<Vec<AgentInfo>> {
        let overrides = self.store.get_agent_overrides()?;
        let disabled = self.store.get_disabled_agents()?;
        let custom = self.store.list_custom_agents()?;
        resolve_agents(&overrides, &disabled, &custom)
    }

    pub fn set_agent_override(&self, agent_key: &str, path: Option<&str>) -> Result<()> {
        self.store.set_agent_override(agent_key, path)
    }

    pub fn set_agent_enabled(&self, agent_key: &str, enabled: bool) -> Result<()> {
        self.store.set_agent_enabled(agent_key, enabled)
    }

    pub fn add_custom_agent(&self, rec: &CustomAgentRecord) -> Result<()> {
        self.store.upsert_custom_agent(rec)
    }

    pub fn remove_custom_agent(&self, key: &str) -> Result<()> {
        self.store.delete_custom_agent(key)
    }

    // ── Skills Library ──

    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        self.store.list_skills()
    }

    pub fn find_skill(&self, id_or_slug_or_name: &str) -> Result<Option<Skill>> {
        self.store.find_skill(id_or_slug_or_name)
    }

    pub fn read_skill_markdown(&self, skill_id: &str) -> Result<(String, String)> {
        let skill = self
            .store
            .find_skill(skill_id)?
            .context("Skill not found")?;
        let entry = find_skill_entry(&skill.central_path).unwrap_or_else(|| "SKILL.md".to_string());
        let content = std::fs::read_to_string(skill.central_path.join(&entry)).unwrap_or_default();
        Ok((entry, content))
    }

    pub fn save_skill_markdown(&self, skill_id: &str, new_content: &str) -> Result<Skill> {
        let mut skill = self
            .store
            .find_skill(skill_id)?
            .context("Skill not found")?;
        let entry = find_skill_entry(&skill.central_path).unwrap_or_else(|| "SKILL.md".to_string());
        std::fs::write(skill.central_path.join(&entry), new_content)?;

        let meta = parse_skill_dir(&skill.central_path);
        if let Some(name) = meta.name {
            skill.name = name;
        }
        skill.description = meta.description;
        skill.tags = meta.tags;
        skill.estimated_tokens = meta.estimated_tokens;
        skill.content_hash = hash_directory(&skill.central_path).ok();
        skill.updated_at = Utc::now().timestamp();
        self.store.upsert_skill(&skill)?;
        Ok(skill)
    }

    pub fn lint_skill(&self, skill_id: &str) -> Result<SkillLintReport> {
        let skill = self
            .store
            .find_skill(skill_id)?
            .context("Skill not found")?;
        Ok(lint_skill_dir(&skill.central_path))
    }

    pub fn install_from_ref(&self, source_ref: &str) -> Result<Skill> {
        let resolved = resolve_source_ref(source_ref)?;
        match resolved {
            ResolvedSource::Local(path) => {
                if !is_valid_skill_dir(&path) {
                    bail!(
                        "Directory {} does not contain SKILL.md (or skill.md)",
                        path.display()
                    );
                }
                let slug_hint = path.file_name().map(|s| s.to_string_lossy().to_string());
                self.import_directory(
                    &path,
                    slug_hint.as_deref(),
                    SourceType::Local,
                    Some(path.to_string_lossy().to_string()),
                    None,
                    None,
                )
            }
            ResolvedSource::Git {
                repo_url,
                branch,
                subpath,
                display_ref,
                source_type,
            } => {
                let proxy = self.http_proxy();
                let fetched = fetch_git_skill(
                    &repo_url,
                    branch.as_deref(),
                    subpath.as_deref(),
                    proxy.as_deref(),
                )?;
                self.import_directory(
                    &fetched.skill_dir,
                    fetched.suggested_slug.as_deref(),
                    source_type,
                    Some(display_ref),
                    subpath,
                    branch,
                )
            }
        }
    }

    /// Re-fetches a skill from its recorded `source_ref` (Git / skills.sh / local
    /// directory), compares SHA-256 directory hashes, updates the central copy if
    /// upstream changed, and automatically refreshes any `Copy`-mode deployments.
    pub fn update_skill(&self, skill_id_or_slug: &str) -> Result<SkillUpdateResult> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;

        if skill.source_type == SourceType::Adopted {
            bail!(
                "Skill '{}' was adopted locally and has no upstream source URL to update from.",
                skill.name
            );
        }
        let source_ref = skill
            .source_ref
            .clone()
            .filter(|s| !s.trim().is_empty())
            .with_context(|| format!("Skill '{}' has no recorded source_ref", skill.name))?;

        let resolved = resolve_source_ref(&source_ref)?;
        let proxy = self.http_proxy();

        let (fetched_guard, src_dir, new_subpath, new_branch) = match resolved {
            ResolvedSource::Local(path) => {
                if !is_valid_skill_dir(&path) {
                    bail!(
                        "Original local source {} is no longer a valid skill directory",
                        path.display()
                    );
                }
                (None, path, None, None)
            }
            ResolvedSource::Git {
                repo_url,
                branch,
                subpath,
                ..
            } => {
                let eff_branch = branch.or_else(|| skill.source_branch.clone());
                let eff_subpath = subpath.or_else(|| skill.source_subpath.clone());
                let fetched = fetch_git_skill(
                    &repo_url,
                    eff_branch.as_deref(),
                    eff_subpath.as_deref(),
                    proxy.as_deref(),
                )?;
                let dir = fetched.skill_dir.clone();
                (Some(fetched), dir, eff_subpath, eff_branch)
            }
        };

        let new_hash = hash_directory(&src_dir)?;
        if skill.content_hash.as_deref() == Some(new_hash.as_str()) {
            drop(fetched_guard);
            return Ok(SkillUpdateResult {
                skill,
                updated: false,
                refreshed_copies: 0,
            });
        }

        // Preserve the existing slug so central_path & all symlinks remain valid.
        let mut updated_skill = self.import_directory(
            &src_dir,
            Some(&skill.slug),
            skill.source_type,
            Some(source_ref),
            new_subpath,
            new_branch,
        )?;
        drop(fetched_guard);

        // Symlink deployments reflect changes immediately; Copy deployments must be refreshed.
        let mut refreshed_copies = 0usize;
        for dep in &updated_skill.deployments {
            if dep.mode == DeployMode::Copy
                && deploy_skill_dir(
                    &updated_skill.central_path,
                    &dep.target_path,
                    DeployMode::Copy,
                    ReplacePolicy::Recorded(DeployMode::Copy),
                )
                .is_ok()
            {
                refreshed_copies += 1;
            }
        }
        refreshed_copies += self.refresh_project_copies(&mut updated_skill)?;

        Ok(SkillUpdateResult {
            skill: updated_skill,
            updated: true,
            refreshed_copies,
        })
    }

    pub fn import_directory(
        &self,
        source_dir: &Path,
        slug_hint: Option<&str>,
        source_type: SourceType,
        source_ref: Option<String>,
        source_subpath: Option<String>,
        source_branch: Option<String>,
    ) -> Result<Skill> {
        if !is_valid_skill_dir(source_dir) {
            bail!(
                "Invalid skill directory (missing SKILL.md): {}",
                source_dir.display()
            );
        }

        let meta = parse_skill_dir(source_dir);
        let raw_slug = slug_hint
            .or(meta.name.as_deref())
            .or_else(|| source_dir.file_name().and_then(|s| s.to_str()))
            .unwrap_or("skill");
        let slug =
            sanitize_slug(raw_slug).context("Could not derive a safe directory slug for skill")?;

        let central_path = self.skills_dir.join(&slug);
        ensure_dst_not_overlapping_src(source_dir, &central_path)?;

        // Stage into a sibling temporary directory first so a failed copy never
        // destroys an already-installed skill at `central_path`.
        let staging_path = self
            .skills_dir
            .join(format!(".staging-{}-{}", slug, Uuid::new_v4()));
        if let Err(err) = copy_dir_recursive(source_dir, &staging_path) {
            let _ = std::fs::remove_dir_all(&staging_path);
            return Err(err);
        }
        if central_path.exists() {
            std::fs::remove_dir_all(&central_path)?;
        }
        std::fs::rename(&staging_path, &central_path)?;

        let existing = self.store.find_skill(&slug)?;
        let now = Utc::now().timestamp();
        let skill = Skill {
            id: existing
                .as_ref()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            slug: slug.clone(),
            name: meta.name.unwrap_or(slug),
            description: meta.description,
            source_type,
            source_ref,
            source_subpath,
            source_branch,
            central_path: central_path.clone(),
            content_hash: hash_directory(&central_path).ok(),
            estimated_tokens: meta.estimated_tokens,
            tags: if !meta.tags.is_empty() {
                meta.tags
            } else {
                existing
                    .as_ref()
                    .map(|s| s.tags.clone())
                    .unwrap_or_default()
            },
            deployments: existing
                .as_ref()
                .map(|s| s.deployments.clone())
                .unwrap_or_default(),
            project_deployments: existing
                .as_ref()
                .map(|s| s.project_deployments.clone())
                .unwrap_or_default(),
            created_at: existing.as_ref().map(|s| s.created_at).unwrap_or(now),
            updated_at: now,
        };

        self.store.upsert_skill(&skill)?;
        Ok(skill)
    }

    pub fn remove_skill(&self, skill_id_or_slug: &str) -> Result<()> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;

        // Undeploy from every global agent first (respecting shared directories).
        let agent_keys: Vec<String> = skill
            .deployments
            .iter()
            .map(|d| d.agent_key.clone())
            .collect();
        for agent_key in agent_keys {
            let _ = self.undeploy_skill(&skill.id, &agent_key);
        }

        // Also undeploy from every attached project workspace.
        let proj_deps: Vec<(String, String)> = skill
            .project_deployments
            .iter()
            .map(|d| (d.project_id.clone(), d.agent_key.clone()))
            .collect();
        for (project_id, agent_key) in proj_deps {
            let _ = self.undeploy_skill_from_project(&skill.id, &project_id, &agent_key);
        }

        if skill.central_path.exists() {
            std::fs::remove_dir_all(&skill.central_path)?;
        }
        self.store.delete_skill(&skill.id)?;
        Ok(())
    }

    // ── Deployment (with shared-directory protection & ReplacePolicy) ──

    pub fn deploy_skill(
        &self,
        skill_id_or_slug: &str,
        agent_key: &str,
        mode_override: Option<DeployMode>,
        force: bool,
    ) -> Result<Deployment> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let agents = self.list_agents()?;
        let agent = agents
            .iter()
            .find(|a| a.key == agent_key)
            .with_context(|| format!("Unknown agent '{agent_key}'"))?;

        let target_path = agent.skills_dir.join(&skill.slug);
        let desired_mode = mode_override.unwrap_or_else(|| self.default_deploy_mode());
        let existing_dep = skill.deployments.iter().find(|d| d.agent_key == agent_key);

        let peer_mode =
            self.store
                .peer_deployment_mode_for_skill(&skill.id, &target_path, agent_key)?;
        let policy = if force {
            ReplacePolicy::UserConfirmed
        } else if let Some(dep) = existing_dep {
            ReplacePolicy::Recorded(dep.mode)
        } else if let Some(mode) = peer_mode {
            ReplacePolicy::Recorded(mode)
        } else {
            ReplacePolicy::NoClobber
        };

        let actual_mode =
            deploy_skill_dir(&skill.central_path, &target_path, desired_mode, policy)?;

        let dep = Deployment {
            id: existing_dep
                .map(|d| d.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            skill_id: skill.id.clone(),
            agent_key: agent.key.clone(),
            target_path,
            mode: actual_mode,
            synced_at: Utc::now().timestamp(),
        };
        self.store.upsert_deployment(&dep)?;
        Ok(dep)
    }

    pub fn undeploy_skill(&self, skill_id_or_slug: &str, agent_key: &str) -> Result<()> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let Some(dep) = skill.deployments.iter().find(|d| d.agent_key == agent_key) else {
            return Ok(());
        };

        // Shared directory ref-count guard: if another agent is actively deployed
        // to the exact same path (e.g. amp & replit both using ~/.config/agents/skills),
        // only remove the DB record and keep the on-disk link intact for the other agent!
        let peer_count = self
            .store
            .count_deployments_at_path(&dep.target_path, agent_key)?;
        if peer_count == 0 {
            remove_managed_target(&skill.central_path, &dep.target_path, dep.mode)?;
        }
        self.store.delete_deployment(&skill.id, agent_key)?;
        Ok(())
    }

    // ── Project-Scope Workspaces & Deployments ──

    pub fn list_projects(&self) -> Result<Vec<ProjectWorkspace>> {
        self.store.list_projects()
    }

    pub fn add_project(
        &self,
        project_dir: &Path,
        name_override: Option<&str>,
    ) -> Result<ProjectWorkspace> {
        let raw = project_dir.to_string_lossy();
        let trimmed_path = if let Some(rest) = raw.trim().strip_prefix("~/") {
            dirs::home_dir()
                .map(|h| h.join(rest))
                .unwrap_or_else(|| PathBuf::from(raw.trim()))
        } else {
            PathBuf::from(raw.trim())
        };
        if !trimmed_path.is_dir() {
            bail!(
                "Project directory does not exist: {}",
                trimmed_path.display()
            );
        }
        let canonical = std::fs::canonicalize(&trimmed_path).unwrap_or(trimmed_path);

        let default_name = canonical
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("project")
            .to_string();
        let name = name_override
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or(default_name);

        if let Some(existing) = self.store.find_project(&canonical.to_string_lossy())? {
            let updated = ProjectWorkspace { name, ..existing };
            self.store.upsert_project(&updated)?;
            return Ok(updated);
        }

        let workspace = ProjectWorkspace {
            id: Uuid::new_v4().to_string(),
            name,
            path: canonical,
            created_at: Utc::now().timestamp(),
        };
        self.store.upsert_project(&workspace)?;
        Ok(workspace)
    }

    pub fn remove_project(&self, project_id_or_name: &str) -> Result<()> {
        let project = self
            .store
            .find_project(project_id_or_name)?
            .context("Project workspace not found")?;

        // Cleanly undeploy every managed skill inside this project before detaching.
        let deps = self
            .store
            .list_project_deployments_for_project(&project.id)?;
        for dep in deps {
            let _ = self.undeploy_skill_from_project(&dep.skill_id, &project.id, &dep.agent_key);
        }

        self.store.delete_project(&project.id)?;
        Ok(())
    }

    pub fn deploy_skill_to_project(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        agent_key: &str,
        mode_override: Option<DeployMode>,
        force: bool,
    ) -> Result<ProjectDeployment> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let project = self
            .store
            .find_project(project_id_or_name)?
            .context("Project workspace not found")?;
        if !project.path.is_dir() {
            bail!(
                "Project directory no longer exists: {}",
                project.path.display()
            );
        }

        let agents = self.list_agents()?;
        let agent = agents
            .iter()
            .find(|a| a.key == agent_key)
            .with_context(|| format!("Unknown agent '{agent_key}'"))?;

        let target_path = project_skill_target(&project, agent, &skill);
        let desired_mode = mode_override.unwrap_or_else(|| self.default_deploy_mode());
        let existing_dep = skill
            .project_deployments
            .iter()
            .find(|d| d.project_id == project.id && d.agent_key == agent_key);

        let peer_mode = self.store.peer_project_deployment_mode_for_skill(
            &skill.id,
            &project.id,
            &target_path,
            agent_key,
        )?;
        let policy = if force {
            ReplacePolicy::UserConfirmed
        } else if let Some(dep) = existing_dep {
            ReplacePolicy::Recorded(dep.mode)
        } else if let Some(mode) = peer_mode {
            ReplacePolicy::Recorded(mode)
        } else {
            ReplacePolicy::NoClobber
        };

        let actual_mode =
            deploy_skill_dir(&skill.central_path, &target_path, desired_mode, policy)?;

        self.record_project_skill_deployment(&skill, &project, agent, target_path, actual_mode)
    }

    fn record_project_skill_deployment(
        &self,
        skill: &Skill,
        project: &ProjectWorkspace,
        agent: &AgentInfo,
        target_path: PathBuf,
        actual_mode: DeployMode,
    ) -> Result<ProjectDeployment> {
        let copy_hash = if actual_mode == DeployMode::Copy {
            hash_directory(&target_path).ok()
        } else {
            None
        };
        let dep = ProjectDeployment {
            id: skill
                .project_deployments
                .iter()
                .find(|dep| dep.project_id == project.id && dep.agent_key == agent.key)
                .map(|d| d.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            skill_id: skill.id.clone(),
            project_id: project.id.clone(),
            agent_key: agent.key.clone(),
            target_path,
            mode: actual_mode,
            copy_hash,
            synced_at: Utc::now().timestamp(),
        };
        self.store.upsert_project_deployment(&dep)?;
        Ok(dep)
    }

    fn refresh_project_copies(&self, skill: &mut Skill) -> Result<usize> {
        let mut refreshed = 0;
        for dep in &mut skill.project_deployments {
            if dep.mode == DeployMode::Copy
                && deploy_skill_dir(
                    &skill.central_path,
                    &dep.target_path,
                    DeployMode::Copy,
                    ReplacePolicy::Recorded(DeployMode::Copy),
                )
                .is_ok()
            {
                self.store.record_refreshed_project_copy(
                    &skill.id,
                    &dep.target_path,
                    skill.content_hash.as_deref(),
                )?;
                dep.copy_hash = skill.content_hash.clone();
                refreshed += 1;
            }
        }
        Ok(refreshed)
    }

    pub fn undeploy_skill_from_project(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        agent_key: &str,
    ) -> Result<()> {
        let skill = self
            .store
            .find_skill(skill_id_or_slug)?
            .context("Skill not found")?;
        let project = self
            .store
            .find_project(project_id_or_name)?
            .context("Project workspace not found")?;
        let Some(dep) = skill
            .project_deployments
            .iter()
            .find(|d| d.project_id == project.id && d.agent_key == agent_key)
        else {
            return Ok(());
        };

        let peer_count = self.store.count_project_deployments_at_path(
            &project.id,
            &dep.target_path,
            agent_key,
        )?;
        if peer_count == 0 {
            remove_managed_target(&skill.central_path, &dep.target_path, dep.mode)?;
        }
        self.store
            .delete_project_deployment(&skill.id, &project.id, agent_key)?;
        Ok(())
    }

    // ── Adopt existing skills from installed agents ──

    pub fn scan_adoptable(&self) -> Result<Vec<AdoptCandidate>> {
        let agents = self.list_agents()?;
        let skills = self.list_skills()?;
        let managed_paths: HashSet<PathBuf> = skills
            .iter()
            .flat_map(|s| s.deployments.iter().map(|d| d.target_path.clone()))
            .collect();
        Ok(scan_adoptable_skills(
            &agents,
            &self.skills_dir,
            &managed_paths,
        ))
    }

    pub fn adopt_candidate(&self, candidate: &AdoptCandidate) -> Result<Skill> {
        let skill = self.import_directory(
            &candidate.path,
            candidate.path.file_name().and_then(|s| s.to_str()),
            SourceType::Adopted,
            Some(candidate.path.to_string_lossy().to_string()),
            None,
            None,
        )?;
        // Replace the exact discovered candidate path (whether inside the agent's
        // primary `skills_dir` or an `additional_scan_dirs` entry like `~/.agents/skills`)
        // with a managed link/copy to the central library so re-scans recognize it.
        let desired_mode = self.default_deploy_mode();
        let actual_mode = deploy_skill_dir(
            &skill.central_path,
            &candidate.path,
            desired_mode,
            ReplacePolicy::UserConfirmed,
        )?;
        let existing_dep = skill
            .deployments
            .iter()
            .find(|d| d.agent_key == candidate.agent_key);
        let dep = Deployment {
            id: existing_dep
                .map(|d| d.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            skill_id: skill.id.clone(),
            agent_key: candidate.agent_key.clone(),
            target_path: candidate.path.clone(),
            mode: actual_mode,
            synced_at: Utc::now().timestamp(),
        };
        self.store.upsert_deployment(&dep)?;
        self.store
            .find_skill(&skill.id)?
            .context("Failed to reload adopted skill")
    }

    // ── Stacks (Presets + Community Recipes) ──

    /// Default Stage-1 community index: a static `index.json` in the UnoSkills
    /// repo, so submitting a stack is just a PR against `community/index.json`.
    const DEFAULT_COMMUNITY_INDEX_URL: &str =
        "https://raw.githubusercontent.com/codexlin/UnoSkills/main/community/index.json";

    /// Built-in featured stacks shipped inside the app binary.
    pub fn featured_stacks(&self) -> Result<Vec<StackManifest>> {
        let registry = DefaultStackRegistry {
            remote_index_url: None,
            proxy_url: None,
        };
        registry.fetch_featured()
    }

    /// Community stacks from the remote `index.json`, with ETag caching and a
    /// cached-copy fallback for offline use. Never touches the network when the
    /// server answers `304 Not Modified`. Cache & ETag are scoped to the active
    /// index URL so switching `stacks_registry_url` never serves stale data.
    pub fn community_stacks(&self) -> Result<Vec<StackManifest>> {
        let configured = self.store.get_setting("stacks_registry_url")?;
        let url = configured
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Self::DEFAULT_COMMUNITY_INDEX_URL.to_string());
        let cached_url = self.store.get_setting("community_index_url")?;
        let cache_matches_url = cached_url.as_deref() == Some(url.as_str());
        let etag = if cache_matches_url {
            self.store.get_setting("community_index_etag")?
        } else {
            None
        };
        let registry = DefaultStackRegistry {
            remote_index_url: Some(url.clone()),
            proxy_url: self.http_proxy(),
        };

        let fetch = registry.fetch_community(etag.as_deref());
        match fetch {
            Ok(result) => {
                if result.not_modified {
                    // ETag matched: serve the previously cached body for this URL.
                    if cache_matches_url
                        && let Some(cached) = self.store.get_setting("community_index_cache")?
                    {
                        return parse_community_index(&cached);
                    }
                    // Stale ETag without a matching cached body: refetch fresh.
                    let fresh = DefaultStackRegistry {
                        remote_index_url: registry.remote_index_url.clone(),
                        proxy_url: registry.proxy_url.clone(),
                    }
                    .fetch_community(None)?;
                    self.save_community_cache(&url, fresh.etag.as_deref(), &fresh.stacks)?;
                    return Ok(fresh.stacks);
                }
                self.save_community_cache(&url, result.etag.as_deref(), &result.stacks)?;
                Ok(result.stacks)
            }
            Err(network_err) => {
                // Offline / unreachable: fall back to the cached copy only if it
                // belongs to the currently configured registry URL.
                if cache_matches_url
                    && let Some(cached) = self.store.get_setting("community_index_cache")?
                {
                    return parse_community_index(&cached);
                }
                Err(network_err)
            }
        }
    }

    fn save_community_cache(
        &self,
        url: &str,
        etag: Option<&str>,
        stacks: &[StackManifest],
    ) -> Result<()> {
        let body = serde_json::to_string(stacks)?;
        let _ = self.store.set_setting("community_index_url", url);
        let _ = self
            .store
            .set_setting("community_index_etag", etag.unwrap_or(""));
        let _ = self.store.set_setting("community_index_cache", &body);
        let _ = self.store.set_setting(
            "community_index_fetched_at",
            &Utc::now().timestamp().to_string(),
        );
        Ok(())
    }

    /// Resolve a stack by id or (case-insensitive) name across local, featured,
    /// and community sources — checking local and built-in stacks first so
    /// offline or local lookups never block on the network.
    pub fn find_any_stack(&self, key: &str) -> Result<Option<StackManifest>> {
        let trimmed = key.trim();
        let matches = |s: &StackManifest| s.id == trimmed || s.name.eq_ignore_ascii_case(trimmed);

        if let Some(found) = self.list_local_stacks()?.into_iter().find(matches) {
            return Ok(Some(found));
        }
        if let Some(found) = self.featured_stacks()?.into_iter().find(matches) {
            return Ok(Some(found));
        }
        let community = self.community_stacks().unwrap_or_default();
        Ok(community.into_iter().find(matches))
    }

    /// Publish a stack as a secret GitHub Gist using the locally stored PAT.
    /// Requires the `gist` scope. Returns the Gist URL.
    pub fn share_stack_to_gist(&self, stack: &StackManifest) -> Result<String> {
        let token = self
            .github_token()
            .filter(|s| !s.trim().is_empty())
            .context("No GitHub token configured (Settings → GitHub Token)")?;
        share_stack_to_gist(token.trim(), self.http_proxy().as_deref(), stack)
    }

    /// Run the GitHub OAuth Device Flow for Gist sharing. Calls `on_code` with
    /// the verification URI and user code before polling, then returns the Gist
    /// URL once approved. Requires `github_oauth_client_id` in settings.
    pub fn share_stack_via_device_flow(
        &self,
        stack: &StackManifest,
        on_code: impl FnOnce(&DeviceCodeRequest),
    ) -> Result<String> {
        let client_id = self
            .store
            .get_setting("github_oauth_client_id")?
            .filter(|s| !s.trim().is_empty())
            .context("No GitHub OAuth client id configured (setting 'github_oauth_client_id')")?;
        let proxy = self.http_proxy();
        let request = request_github_device_code(client_id.trim(), proxy.as_deref())?;
        on_code(&request);
        let token = poll_github_device_token(
            client_id.trim(),
            &request.device_code,
            request.interval,
            request.expires_in,
            proxy.as_deref(),
        )?;
        share_stack_to_gist(&token, proxy.as_deref(), stack)
    }

    pub fn list_local_stacks(&self) -> Result<Vec<StackManifest>> {
        let mut stacks = self.store.list_local_stacks()?;
        for stack in &mut stacks {
            stack.source = StackSource::Local;
        }
        Ok(stacks)
    }

    pub fn save_local_stack(&self, stack: &StackManifest) -> Result<()> {
        self.store.save_local_stack(stack, Utc::now().timestamp())
    }

    pub fn delete_local_stack(&self, stack_id: &str) -> Result<()> {
        self.store.delete_local_stack(stack_id)
    }

    pub fn create_stack_from_skills(
        &self,
        name: &str,
        author: &str,
        description: &str,
        tags: Vec<String>,
        skill_ids: &[String],
    ) -> Result<StackManifest> {
        let all_skills = self.list_skills()?;
        let mut refs = Vec::new();
        for sid in skill_ids {
            if let Some(sk) = all_skills.iter().find(|s| &s.id == sid || &s.slug == sid) {
                refs.push(StackSkillRef {
                    name: sk.name.clone(),
                    source: sk
                        .source_ref
                        .clone()
                        .unwrap_or_else(|| sk.central_path.to_string_lossy().to_string()),
                    note: sk.description.clone().unwrap_or_default(),
                });
            }
        }
        let id = sanitize_slug(name)
            .unwrap_or_else(|| Uuid::new_v4().to_string())
            .to_lowercase()
            .replace(' ', "-");
        let manifest = StackManifest {
            id,
            name: name.trim().to_string(),
            author: author.trim().to_string(),
            description: description.trim().to_string(),
            tags,
            recommended_agents: self
                .list_agents()?
                .into_iter()
                .filter(|a| a.installed && a.enabled)
                .map(|a| a.key)
                .collect(),
            verified: false,
            source: StackSource::Local,
            skills: refs,
        };
        self.save_local_stack(&manifest)?;
        Ok(manifest)
    }

    pub fn export_stack_toml(&self, stack: &StackManifest) -> Result<String> {
        export_stack_to_toml(stack)
    }

    pub fn import_stack_toml(&self, toml_text: &str) -> Result<StackManifest> {
        let mut manifest = parse_stack_from_toml(toml_text)?;
        manifest.source = StackSource::Local;
        self.save_local_stack(&manifest)?;
        Ok(manifest)
    }

    /// Installs every skill in a Stack (skipping ones already in the library),
    /// saves the stack locally, and optionally deploys all skills to `deploy_agents`.
    pub fn install_stack(
        &self,
        stack: &StackManifest,
        deploy_agents: &[String],
    ) -> Result<StackInstallSummary> {
        let mut installed = Vec::new();
        let mut skipped_existing = Vec::new();
        let mut failed = Vec::new();

        for item in &stack.skills {
            let existing = item.installed_in(&self.store.list_skills()?).cloned();
            let skill = if let Some(s) = existing {
                skipped_existing.push(s.name.clone());
                Some(s)
            } else {
                match self.install_from_ref(&item.source) {
                    Ok(s) => {
                        installed.push(s.name.clone());
                        Some(s)
                    }
                    Err(e) => {
                        failed.push((item.name.clone(), e.to_string()));
                        None
                    }
                }
            };

            if let Some(sk) = skill {
                for agent_key in deploy_agents {
                    let _ = self.deploy_skill(&sk.id, agent_key, None, false);
                }
            }
        }

        self.save_local_stack(stack)?;
        Ok(StackInstallSummary {
            stack_id: stack.id.clone(),
            installed,
            skipped_existing,
            failed,
            deployed_agents: deploy_agents.to_vec(),
        })
    }

    fn stack_deploy_destination(
        &self,
        project_id: &str,
        agent_key: &str,
    ) -> Result<(ProjectWorkspace, AgentInfo)> {
        let project = self
            .store
            .find_project(project_id)?
            .context("Project workspace not found")?;
        if !project.path.is_dir() {
            bail!(
                "Project directory no longer exists: {}",
                project.path.display()
            );
        }
        let agent = self
            .list_agents()?
            .into_iter()
            .find(|agent| agent.key == agent_key && agent.installed && agent.enabled)
            .with_context(|| format!("Agent '{agent_key}' is no longer available"))?;
        Ok((project, agent))
    }

    pub fn stack_project_deploy_status(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
    ) -> Result<StackProjectDeployStatus> {
        let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
        let skills = self.list_skills()?;
        let mut status = StackProjectDeployStatus::default();
        for item in &stack.skills {
            let Some(skill) = item.deployable_in(&skills) else {
                status.unavailable += 1;
                continue;
            };
            let target = project_skill_target(&project, &agent, skill);
            match inspect_stack_project_target(skill, &project, &agent, &target) {
                StackProjectTarget::Deployed => status.deployed += 1,
                StackProjectTarget::Missing(_) | StackProjectTarget::Register(_) => {
                    status.pending += 1
                }
                StackProjectTarget::Conflict => status.conflicts += 1,
                StackProjectTarget::Unavailable => status.unavailable += 1,
            }
        }
        Ok(status)
    }

    /// Recheck placement state and restore missing skills without replacing existing files.
    pub fn deploy_stack_to_project(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
    ) -> Result<StackProjectDeploySummary> {
        let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
        let skills = self.list_skills()?;
        let mut completed = HashSet::new();
        let mut summary = StackProjectDeploySummary::default();
        for item in &stack.skills {
            let Some(skill) = item.deployable_in(&skills) else {
                summary.unavailable += 1;
                continue;
            };
            if completed.contains(&skill.id) {
                summary.already_deployed += 1;
                continue;
            }
            let target = project_skill_target(&project, &agent, skill);
            let mode = match inspect_stack_project_target(skill, &project, &agent, &target) {
                StackProjectTarget::Deployed => {
                    summary.already_deployed += 1;
                    continue;
                }
                StackProjectTarget::Missing(recorded_mode) => deploy_skill_dir(
                    &skill.central_path,
                    &target,
                    recorded_mode.unwrap_or_else(|| self.default_deploy_mode()),
                    ReplacePolicy::CreateOnly,
                ),
                StackProjectTarget::Register(mode) => Ok(mode),
                StackProjectTarget::Conflict => Err(anyhow::anyhow!(
                    "TARGET_CONFLICT: Deployment target is occupied or has changed: {}",
                    target.display()
                )),
                StackProjectTarget::Unavailable => {
                    summary.unavailable += 1;
                    continue;
                }
            };
            let deployment = mode.and_then(|mode| {
                self.record_project_skill_deployment(skill, &project, &agent, target, mode)
            });
            match deployment {
                Ok(_) => {
                    completed.insert(&skill.id);
                    summary.deployed += 1;
                }
                Err(error) => summary
                    .failed
                    .push((item.name.clone(), format!("{error:#}"))),
            }
        }
        Ok(summary)
    }

    // ── Marketplace (skills.sh) ──

    pub fn marketplace_leaderboard(&self, board: &str) -> Result<Vec<MarketplaceSkill>> {
        let proxy = self.http_proxy();
        fetch_skillssh_leaderboard(board, proxy.as_deref())
    }

    pub fn marketplace_leaderboard_page(
        &self,
        board: &str,
        page: usize,
    ) -> Result<MarketplacePage> {
        let proxy = self.http_proxy();
        fetch_skillssh_leaderboard_page(board, page, proxy.as_deref())
    }

    pub fn marketplace_search(&self, query: &str, limit: usize) -> Result<Vec<MarketplaceSkill>> {
        let proxy = self.http_proxy();
        search_skillssh(query, limit, proxy.as_deref())
    }

    // ── Guided App Updater (v1) ──

    pub fn should_auto_check_updates(&self) -> bool {
        let last = self
            .store
            .get_setting("last_update_check_at")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i64>().ok());
        should_run_startup_check(last, Utc::now().timestamp())
    }

    pub fn check_app_update(
        &self,
        current_version: &str,
        record_timestamp: bool,
    ) -> Result<UpdateCheckOutcome> {
        let proxy = self.http_proxy();
        let token = self.github_token();
        let latest =
            fetch_latest_release(DEFAULT_RELEASES_API_URL, proxy.as_deref(), token.as_deref())?;
        if record_timestamp {
            let _ = self
                .store
                .set_setting("last_update_check_at", &Utc::now().timestamp().to_string());
        }
        let Some(release) = latest else {
            return Ok(UpdateCheckOutcome {
                current_version: current_version.to_string(),
                latest: None,
                update_available: false,
                recommended_asset: None,
                checksums_asset: None,
            });
        };
        let update_available = is_newer_semver(&release.tag_name, current_version);
        let recommended_asset = select_platform_asset(&release.assets, PlatformKind::current());
        let checksums_asset = find_checksums_asset(&release.assets);
        Ok(UpdateCheckOutcome {
            current_version: current_version.to_string(),
            latest: Some(release),
            update_available,
            recommended_asset,
            checksums_asset,
        })
    }

    pub fn download_app_update(
        &self,
        version: &str,
        asset: &ReleaseAsset,
        checksums_asset: Option<&ReleaseAsset>,
    ) -> Result<DownloadedUpdate> {
        let proxy = self.http_proxy();
        let token = self.github_token();
        let updates_dir = self.root_dir.join("updates");
        download_and_verify_update(
            version,
            asset,
            checksums_asset,
            &updates_dir,
            proxy.as_deref(),
            token.as_deref(),
        )
    }

    // ── Private Git Backup & Multi-Machine Sync ──

    pub fn backup_repo_url(&self) -> Option<String> {
        self.store
            .get_setting("git_backup_repo_url")
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn backup_branch(&self) -> String {
        self.store
            .get_setting("git_backup_branch")
            .ok()
            .flatten()
            .map(|s| normalize_branch(&s))
            .unwrap_or_else(|| DEFAULT_BACKUP_BRANCH.to_string())
    }

    pub fn configure_git_backup(&self, repo_url: &str, branch: Option<&str>) -> Result<()> {
        let trimmed = repo_url.trim();
        self.store.set_setting("git_backup_repo_url", trimmed)?;
        if let Some(b) = branch {
            let norm = normalize_branch(b);
            self.store.set_setting("git_backup_branch", &norm)?;
        }
        Ok(())
    }

    pub fn git_backup_status(&self) -> Result<GitBackupStatus> {
        let last_pushed_at = self
            .store
            .get_setting("git_backup_last_pushed_at")?
            .and_then(|s| s.parse::<i64>().ok());
        let last_pulled_at = self
            .store
            .get_setting("git_backup_last_pulled_at")?
            .and_then(|s| s.parse::<i64>().ok());
        let local_skill_count = self.list_skills()?.len();
        let local_stack_count = self.list_local_stacks()?.len();
        Ok(GitBackupStatus {
            repo_url: self.backup_repo_url(),
            branch: self.backup_branch(),
            last_pushed_at,
            last_pulled_at,
            local_skill_count,
            local_stack_count,
        })
    }

    /// Exports plain-text `skills/` and `stacks/*.toml` (never binary SQLite)
    /// into `<root_dir>/sync-repo`, commits any changes, and pushes to remote.
    pub fn git_backup_push(&self) -> Result<GitBackupPushResult> {
        let repo_url = self
            .backup_repo_url()
            .context("No Git backup repository URL configured (Settings → Git Backup)")?;
        let branch = self.backup_branch();
        let proxy = self.http_proxy();
        let token = self.github_token();
        let skills = self.list_skills()?;
        let local_stacks = self.list_local_stacks()?;
        let sync_dir = self.root_dir.join("sync-repo");

        let opts = GitBackupOpts {
            repo_url: &repo_url,
            branch: &branch,
            proxy_url: proxy.as_deref(),
            github_token: token.as_deref(),
        };
        let (committed, pushed, skills_exported, stacks_exported, commit_sha) =
            mirror_commit_and_push(&sync_dir, &opts, &skills, &local_stacks)?;

        if pushed || committed {
            let _ = self.store.set_setting(
                "git_backup_last_pushed_at",
                &Utc::now().timestamp().to_string(),
            );
        }

        Ok(GitBackupPushResult {
            committed,
            pushed,
            skills_exported,
            stacks_exported,
            commit_sha,
        })
    }

    /// Pulls plain-text `skills/` and `stacks/*.toml` from the bound backup
    /// repository into `<root_dir>/sync-repo`, rebuilds/updates the local
    /// SQLite index, and refreshes any `Copy`-mode deployments for updated skills.
    pub fn git_backup_pull(&self) -> Result<GitBackupPullResult> {
        let repo_url = self
            .backup_repo_url()
            .context("No Git backup repository URL configured (Settings → Git Backup)")?;
        let branch = self.backup_branch();
        let proxy = self.http_proxy();
        let token = self.github_token();
        let sync_dir = self.root_dir.join("sync-repo");

        let opts = GitBackupOpts {
            repo_url: &repo_url,
            branch: &branch,
            proxy_url: proxy.as_deref(),
            github_token: token.as_deref(),
        };
        let remote_has_branch = prepare_sync_repo(&sync_dir, &opts)?;
        if !remote_has_branch {
            return Ok(GitBackupPullResult {
                skills_imported: 0,
                skills_updated: 0,
                skills_unchanged: 0,
                stacks_imported: 0,
                refreshed_copies: 0,
            });
        }

        let (skill_dirs, stack_files) = list_synced_artifacts(&sync_dir);
        let mut skills_imported = 0usize;
        let mut skills_updated = 0usize;
        let mut skills_unchanged = 0usize;
        let mut refreshed_copies = 0usize;

        for (slug, dir) in skill_dirs {
            if !is_valid_skill_dir(&dir) {
                continue;
            }
            let incoming_hash = hash_directory(&dir).ok();
            let existing = self.store.find_skill(&slug)?;

            if let Some(prev) = &existing
                && prev.content_hash.is_some()
                && prev.content_hash == incoming_hash
            {
                skills_unchanged += 1;
                continue;
            }

            let source_type = existing
                .as_ref()
                .map(|s| s.source_type)
                .unwrap_or(SourceType::Local);
            // Preserve any existing real upstream source_ref, but leave newly
            // pulled skills with `None` instead of a synthetic non-resolvable ref.
            let source_ref = existing.as_ref().and_then(|s| s.source_ref.clone());
            let source_subpath = existing.as_ref().and_then(|s| s.source_subpath.clone());
            let source_branch = existing.as_ref().and_then(|s| s.source_branch.clone());

            let mut updated_skill = self.import_directory(
                &dir,
                Some(&slug),
                source_type,
                source_ref,
                source_subpath,
                source_branch,
            )?;

            if existing.is_some() {
                skills_updated += 1;
                for dep in &updated_skill.deployments {
                    if dep.mode == DeployMode::Copy
                        && deploy_skill_dir(
                            &updated_skill.central_path,
                            &dep.target_path,
                            DeployMode::Copy,
                            ReplacePolicy::Recorded(DeployMode::Copy),
                        )
                        .is_ok()
                    {
                        refreshed_copies += 1;
                    }
                }
                refreshed_copies += self.refresh_project_copies(&mut updated_skill)?;
            } else {
                skills_imported += 1;
            }
        }

        let mut stacks_imported = 0usize;
        for path in stack_files {
            if let Ok(toml_text) = std::fs::read_to_string(&path)
                && self.import_stack_toml(&toml_text).is_ok()
            {
                stacks_imported += 1;
            }
        }

        let _ = self.store.set_setting(
            "git_backup_last_pulled_at",
            &Utc::now().timestamp().to_string(),
        );

        Ok(GitBackupPullResult {
            skills_imported,
            skills_updated,
            skills_unchanged,
            stacks_imported,
            refreshed_copies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_install_skips_a_skill_previously_installed_by_source_under_another_name() {
        let tmp = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();
        let source = tmp.path().join("source-skill");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(
            source.join("SKILL.md"),
            "---\nname: Actual skill\n---\n# Skill",
        )
        .unwrap();
        let source_ref = source.to_string_lossy().into_owned();
        manager.install_from_ref(&source_ref).unwrap();
        std::fs::remove_dir_all(source).unwrap();
        let stack = StackManifest {
            id: "aliased-stack".into(),
            name: "Aliased stack".into(),
            author: String::new(),
            description: String::new(),
            tags: Vec::new(),
            recommended_agents: Vec::new(),
            verified: false,
            source: StackSource::Local,
            skills: vec![StackSkillRef {
                name: "Manifest alias".into(),
                source: source_ref,
                note: String::new(),
            }],
        };

        let summary = manager.install_stack(&stack, &[]).unwrap();

        assert_eq!(summary.skipped_existing, ["Actual skill"]);
        assert!(summary.installed.is_empty());
        assert!(summary.failed.is_empty());
    }

    #[test]
    fn full_lifecycle_install_deploy_shared_dir_and_stack() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = UnoSkillsManager::open_at(tmp.path().join("uno")).unwrap();

        // Create a sample local skill
        let local_skill = tmp.path().join("my-rust-skill");
        std::fs::create_dir_all(&local_skill).unwrap();
        std::fs::write(
            local_skill.join("SKILL.md"),
            "---\nname: rust-helper\ndescription: Helps write clean Rust\ntags:\n  - rust\n---\n# Rust Helper\nUse cargo clippy.",
        )
        .unwrap();

        // Install into central library
        let skill = mgr
            .install_from_ref(&local_skill.to_string_lossy())
            .unwrap();
        assert_eq!(skill.name, "rust-helper");
        assert!(skill.estimated_tokens > 0);

        // Configure two custom agents sharing the exact same skills directory!
        let shared_dir = tmp.path().join("shared-agent-skills");
        mgr.add_custom_agent(&CustomAgentRecord {
            key: "agent_a".into(),
            display_name: "Agent A".into(),
            skills_dir: shared_dir.to_string_lossy().into(),
            category: crate::domain::AgentCategory::Coding,
        })
        .unwrap();
        mgr.add_custom_agent(&CustomAgentRecord {
            key: "agent_b".into(),
            display_name: "Agent B".into(),
            skills_dir: shared_dir.to_string_lossy().into(),
            category: crate::domain::AgentCategory::Coding,
        })
        .unwrap();

        mgr.deploy_skill(&skill.id, "agent_a", Some(DeployMode::Symlink), false)
            .unwrap();
        mgr.deploy_skill(&skill.id, "agent_b", Some(DeployMode::Symlink), false)
            .unwrap();
        assert!(shared_dir.join(&skill.slug).exists());

        // Undeploying agent_a must NOT delete the shared symlink still used by agent_b!
        mgr.undeploy_skill(&skill.id, "agent_a").unwrap();
        assert!(shared_dir.join(&skill.slug).exists());

        // Undeploying the last agent removes the symlink cleanly.
        mgr.undeploy_skill(&skill.id, "agent_b").unwrap();
        assert!(!shared_dir.join(&skill.slug).exists());

        // Create & export a local Stack
        let stack = mgr
            .create_stack_from_skills(
                "My Rust Stack",
                "tester",
                "Daily Rust setup",
                vec!["rust".into()],
                std::slice::from_ref(&skill.id),
            )
            .unwrap();
        let toml = mgr.export_stack_toml(&stack).unwrap();
        assert!(toml.contains("My Rust Stack"));
        let imported = mgr.import_stack_toml(&toml).unwrap();
        assert_eq!(imported.name, "My Rust Stack");
    }
}
