#[path = "backup_sync.rs"]
mod backup_sync;

#[path = "adoption_merge.rs"]
mod adoption_merge;
pub use adoption_merge::{
    AdoptionMergeChoice, AdoptionMergeConflict, AdoptionMergePreview, AdoptionMergeRequest,
    AdoptionMergeResult, AdoptionMergeTarget,
};

#[path = "skill_removal.rs"]
mod skill_removal;

#[path = "skill_revisions.rs"]
mod skill_revisions;
pub use skill_revisions::{PreparedSkillRevision, SkillRevisionPreview};

#[path = "project_deploy_preview.rs"]
mod project_deploy_preview;
pub use project_deploy_preview::{ProjectDeployOutcome, ProjectDeployPreview, ProjectDeployRow};

#[path = "project_versions.rs"]
pub(crate) mod project_versions;
pub use project_versions::{
    PreparedProjectConflict, PreparedProjectVersion, ProjectConflictReview, ProjectVersionIntent,
    ProjectVersionReview,
};
pub use skill_removal::{RemovalProblem, ReviewedDeployment};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

use crate::domain::{
    AdoptCandidate, AgentInfo, DeployMode, Deployment, MarketplacePage, MarketplaceSkill,
    ProjectDeployment, ProjectSkillPlacement, ProjectSkillStatus, ProjectWorkspace, Skill,
    SkillLintReport, SkillUpdateResult, SourceType, StackDeploymentFailure, StackInstallSummary,
    StackManifest, StackProjectDeployStatus, StackProjectDeploySummary, StackSkillRef, StackSource,
};
use crate::parser::{
    find_skill_entry, hash_directory, is_valid_skill_dir, lint_skill_dir, managed_copy_fingerprint,
    parse_skill_dir, project_version_hash, sanitize_slug,
};
use crate::registry::{CustomAgentRecord, resolve_agents};
use crate::source::{
    FetchedSkillDir, ResolvedSource, fetch_git_skill_with_console_option,
    fetch_skillssh_leaderboard, fetch_skillssh_leaderboard_page, resolve_source_ref, same_source,
    scan_adoptable_skills, search_skillssh,
};
use crate::stacks::{
    DefaultStackRegistry, DeviceCodeRequest, StackRegistry, export_stack_to_toml,
    parse_community_index, parse_stack_from_toml, poll_github_device_token,
    request_github_device_code, share_stack_to_gist,
};
use crate::store::Store;
use crate::sync::{
    ReplacePolicy, TargetState, classify_target, copy_dir_recursive, deploy_skill_dir,
    ensure_dst_not_overlapping_src, physical_target_path, verify_managed_target,
};
use crate::updater::{
    DEFAULT_RELEASES_API_URL, DownloadedUpdate, PlatformKind, ReleaseAsset, UpdateCheckOutcome,
    UpdateProgress, download_and_verify_update_with_progress, fetch_latest_release,
    find_checksums_asset, is_newer_semver, select_platform_asset,
};

pub struct UnoSkillsManager {
    root_dir: PathBuf,
    skills_dir: PathBuf,
    store: Store,
    hide_git_fetch_console: bool,
}

struct SkillImportSource {
    source_type: SourceType,
    source_ref: Option<String>,
    subpath: Option<String>,
    branch: Option<String>,
}

pub(crate) enum StackProjectTarget {
    Deployed(DeployMode),
    Missing(Option<DeployMode>),
    Register(DeployMode),
    Local,
    Conflict,
    Unavailable,
}

pub(crate) fn project_skills_dir(agent: &AgentInfo) -> &str {
    let relative_dir = agent.project_relative_skills_dir.trim();
    if relative_dir.is_empty() {
        ".agents/skills"
    } else {
        relative_dir
    }
}

pub(crate) fn project_scope_directory(project: &ProjectWorkspace, scope: &Path) -> Result<PathBuf> {
    let path = project.path.join(scope);
    if scope.as_os_str().is_empty() {
        ensure!(
            path.is_dir(),
            "Project directory no longer exists: {}",
            path.display()
        );
        ensure!(
            path.canonicalize()? == path,
            "TARGET_CONFLICT: Project directory now points to another location: {}",
            path.display()
        );
    } else {
        ensure!(
            scope
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
            "Invalid subproject directory: {}",
            scope.display()
        );
        ensure!(
            path.is_dir() && path.canonicalize()? == path,
            "Subproject directory has moved or points to another location: {}",
            path.display()
        );
    }
    Ok(path)
}

// Keep scope keys stable across the filesystem, SQLite and the desktop API.
pub(crate) fn project_scope_key(scope: &Path) -> PathBuf {
    let mut key = std::ffi::OsString::new();
    for component in scope.components() {
        if !key.is_empty() {
            key.push("/");
        }
        key.push(component.as_os_str());
    }
    key.into()
}

pub(crate) fn scoped_project_skills_dir(
    project: &ProjectWorkspace,
    scope: &Path,
    agent: &AgentInfo,
) -> Result<PathBuf> {
    let mut dir = project_scope_directory(project, scope)?;
    let relative_dir = Path::new(project_skills_dir(agent));
    for part in relative_dir.components() {
        let Component::Normal(part) = part else {
            bail!("Invalid Agent skills directory: {}", relative_dir.display());
        };
        dir.push(part);
        match std::fs::symlink_metadata(&dir) {
            Ok(entry) if entry.is_dir() && dir.canonicalize()? == dir => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => bail!(
                "TARGET_CONFLICT: Invalid project skills directory: {}",
                dir.display()
            ),
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot access {}", dir.display()));
            }
        }
    }
    Ok(dir)
}

fn safe_scoped_target(dep: &ProjectDeployment) -> Result<bool> {
    let parent = dep
        .target_path
        .parent()
        .context("Missing skills directory")?;
    for ancestor in parent.ancestors() {
        match ancestor.symlink_metadata() {
            Ok(metadata) => {
                ensure!(
                    metadata.is_dir() && ancestor.canonicalize()? == ancestor,
                    "TARGET_CONFLICT: Deployment parent now points to another location: {}",
                    ancestor.display()
                );
                return Ok(ancestor == parent);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(false)
}

pub(crate) fn inspect_stack_project_target(
    skill: &Skill,
    project: &ProjectWorkspace,
    agent: &AgentInfo,
    scope: &Path,
    target: &Path,
) -> StackProjectTarget {
    if skill.project_deployments.iter().any(|dep| {
        dep.project_id == project.id
            && dep.scope_path == scope
            && dep.agent_key == agent.key
            && dep.target_path != target
    }) {
        return StackProjectTarget::Conflict;
    }
    let matching_records = || {
        skill
            .project_deployments
            .iter()
            .filter(|dep| dep.target_path == target)
    };
    let in_scope =
        |dep: &ProjectDeployment| dep.project_id == project.id && dep.scope_path == scope;
    let is_current = |dep: &&ProjectDeployment| in_scope(dep) && dep.agent_key == agent.key;
    let scope_lock = skill.project_lock(&project.id, scope);
    if scope_lock.is_some() || matching_records().any(|dep| dep.locked_hash.is_some()) {
        if scope_lock.is_none()
            || !matching_records().all(|dep| {
                in_scope(dep)
                    && dep.mode == DeployMode::Copy
                    && dep.locked_hash.as_deref() == scope_lock
            })
        {
            return StackProjectTarget::Conflict;
        }
        return match classify_target(&skill.central_path, target) {
            TargetState::Absent => StackProjectTarget::Missing(Some(DeployMode::Copy)),
            TargetState::RealDir
                if matching_records().next().is_some()
                    && project_version_hash(target).ok().as_deref() == scope_lock =>
            {
                if matching_records().any(|dep| is_current(&dep)) {
                    StackProjectTarget::Deployed(DeployMode::Copy)
                } else {
                    StackProjectTarget::Register(DeployMode::Copy)
                }
            }
            _ => StackProjectTarget::Conflict,
        };
    }
    let mode = match classify_target(&skill.central_path, target) {
        TargetState::Absent if is_valid_skill_dir(&skill.central_path) => {
            let recorded_mode = matching_records()
                .find(is_current)
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
            if !matching_records().all(|dep| {
                verify_managed_target(
                    &skill.central_path,
                    target,
                    dep.mode,
                    dep.copy_hash.as_deref().or(skill.content_hash.as_deref()),
                )
                .is_ok_and(|state| state == TargetState::RealDir)
            }) {
                return StackProjectTarget::Conflict;
            }
            DeployMode::Copy
        }
        TargetState::RealDir
            if matching_records().next().is_none() && matches_skill_copy(skill, target) =>
        {
            return StackProjectTarget::Local;
        }
        _ => return StackProjectTarget::Conflict,
    };
    if matching_records().any(|dep| is_current(&dep) && dep.mode == mode) {
        StackProjectTarget::Deployed(mode)
    } else {
        StackProjectTarget::Register(mode)
    }
}

fn matches_skill_copy(skill: &Skill, target: &Path) -> bool {
    if !is_valid_skill_dir(target) {
        return false;
    }
    managed_copy_fingerprint(&skill.central_path)
        .ok()
        .zip(managed_copy_fingerprint(target).ok())
        .is_some_and(|(source, copy)| source == copy)
}

fn is_globally_available(skill: &Skill, agent: &AgentInfo) -> bool {
    std::iter::once(&agent.skills_dir)
        .chain(&agent.additional_scan_dirs)
        .any(|root| {
            let target = root.join(&skill.slug);
            match classify_target(&skill.central_path, &target) {
                TargetState::LinkToSource => is_valid_skill_dir(&target),
                TargetState::RealDir => matches_skill_copy(skill, &target),
                _ => false,
            }
        })
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
            hide_git_fetch_console: false,
        })
    }

    /// Suppresses console windows for Git skill fetches on Windows.
    #[must_use]
    pub fn with_background_git_fetches(mut self) -> Self {
        self.hide_git_fetch_console = true;
        self
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
        let _lock = self.lock_skills()?;
        self.store.set_agent_override(agent_key, path)
    }

    pub fn set_agent_enabled(&self, agent_key: &str, enabled: bool) -> Result<()> {
        let _lock = self.lock_skills()?;
        self.store.set_agent_enabled(agent_key, enabled)
    }

    pub fn add_custom_agent(&self, rec: &CustomAgentRecord) -> Result<()> {
        let _lock = self.lock_skills()?;
        self.store.upsert_custom_agent(rec)
    }

    pub fn remove_custom_agent(&self, key: &str) -> Result<()> {
        let _lock = self.lock_skills()?;
        self.store.delete_custom_agent(key)
    }

    // ── Skills Library ──

    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        self.store.list_skills()
    }

    /// Check availability for library skills at the project root without changing project or global files.
    pub fn project_skill_statuses(
        &self,
        project_id: &str,
    ) -> Result<HashMap<String, HashMap<String, ProjectSkillStatus>>> {
        self.project_skill_statuses_at(project_id, Path::new(""))
    }

    /// Check availability for library skills at a validated project scope (`""` for root or relative subproject path).
    pub fn project_skill_statuses_at(
        &self,
        project_id: &str,
        scope: &Path,
    ) -> Result<HashMap<String, HashMap<String, ProjectSkillStatus>>> {
        let project = self
            .store
            .find_project(project_id)?
            .context("Project workspace not found")?;
        let skills = self.list_skills()?;
        project_scope_directory(&project, scope)?;
        let scope = project_scope_key(scope);
        let scope = scope.as_path();
        let mut statuses = HashMap::new();
        for agent in self
            .list_agents()?
            .into_iter()
            .filter(|agent| agent.installed && agent.enabled)
        {
            let directory = scoped_project_skills_dir(&project, scope, &agent).ok();
            let by_skill = skills
                .iter()
                .map(|skill| {
                    let mut scoped_deployments = skill
                        .project_deployments
                        .iter()
                        .filter(|dep| dep.project_id == project.id && dep.scope_path == scope);
                    let has_deployment = scoped_deployments.clone().next().is_some();
                    let placement = match directory.as_ref() {
                        None if scoped_deployments.any(|dep| dep.agent_key == agent.key) => {
                            ProjectSkillPlacement::Conflict
                        }
                        None => ProjectSkillPlacement::Unavailable,
                        Some(directory) => match inspect_stack_project_target(
                            skill,
                            &project,
                            &agent,
                            scope,
                            &directory.join(&skill.slug),
                        ) {
                            StackProjectTarget::Deployed(_) => ProjectSkillPlacement::Managed,
                            StackProjectTarget::Missing(_) => ProjectSkillPlacement::Missing,
                            StackProjectTarget::Register(_) | StackProjectTarget::Local => {
                                ProjectSkillPlacement::Local
                            }
                            StackProjectTarget::Conflict => ProjectSkillPlacement::Conflict,
                            StackProjectTarget::Unavailable => ProjectSkillPlacement::Unavailable,
                        },
                    };
                    (
                        skill.id.clone(),
                        ProjectSkillStatus {
                            placement,
                            global_available: is_globally_available(skill, &agent),
                            retained_pin: skill.project_lock(&project.id, scope).is_some()
                                && !has_deployment,
                        },
                    )
                })
                .collect();
            statuses.insert(agent.key, by_skill);
        }
        Ok(statuses)
    }

    pub fn find_skill(&self, id_or_slug_or_name: &str) -> Result<Option<Skill>> {
        self.store.find_skill(id_or_slug_or_name)
    }

    pub fn read_skill_markdown(&self, skill_id: &str) -> Result<(String, String)> {
        let (entry, path) = self.skill_markdown_path(skill_id)?;
        Ok((entry, std::fs::read_to_string(path).unwrap_or_default()))
    }

    /// Reads the editable file and propagates failures to the caller.
    pub fn read_skill_markdown_checked(&self, skill_id: &str) -> Result<(String, String)> {
        let (entry, path) = self.skill_markdown_path(skill_id)?;
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        Ok((entry, content))
    }

    fn skill_markdown_path(&self, skill_id: &str) -> Result<(String, PathBuf)> {
        let skill = self
            .store
            .find_skill(skill_id)?
            .context("Skill not found")?;
        let entry = find_skill_entry(&skill.central_path).unwrap_or_else(|| "SKILL.md".to_string());
        let path = skill.central_path.join(&entry);
        Ok((entry, path))
    }

    pub fn save_skill_markdown(&self, skill_id: &str, new_content: &str) -> Result<Skill> {
        let _lock = self.lock_skills()?;
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
                    Some(std::path::absolute(&path)?.to_string_lossy().into_owned()),
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
                let fetched =
                    self.fetch_git_skill(&repo_url, branch.as_deref(), subpath.as_deref())?;
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

    fn fetch_git_skill(
        &self,
        repo_url: &str,
        branch: Option<&str>,
        subpath: Option<&str>,
    ) -> Result<FetchedSkillDir> {
        fetch_git_skill_with_console_option(
            repo_url,
            branch,
            subpath,
            self.http_proxy().as_deref(),
            self.hide_git_fetch_console,
        )
    }

    /// Update from the recorded source using the same checks as desktop revisions.
    pub fn update_skill(&self, skill_id_or_slug: &str) -> Result<SkillUpdateResult> {
        match self.prepare_skill_update(skill_id_or_slug)? {
            Some(prepared) => self.apply_skill_revision(prepared),
            None => Ok(SkillUpdateResult {
                skill: self
                    .store
                    .find_skill(skill_id_or_slug)?
                    .context("Skill not found")?,
                updated: false,
                refreshed_copies: 0,
                deployment_issues: Vec::new(),
            }),
        }
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
        let _lock = self.lock_skills()?;
        self.import_directory_locked(
            source_dir,
            slug_hint,
            SkillImportSource {
                source_type,
                source_ref,
                subpath: source_subpath,
                branch: source_branch,
            },
        )
    }

    fn import_directory_locked(
        &self,
        source_dir: &Path,
        slug_hint: Option<&str>,
        source: SkillImportSource,
    ) -> Result<Skill> {
        let SkillImportSource {
            source_type,
            source_ref,
            subpath: source_subpath,
            branch: source_branch,
        } = source;
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

        let existing = self
            .store
            .list_skills()?
            .into_iter()
            .find(|skill| skill.slug.eq_ignore_ascii_case(&slug));
        if let Some(skill) = &existing {
            ensure!(
                skill.central_path == central_path
                    && source_ref
                        .as_deref()
                        .zip(skill.source_ref.as_deref())
                        .is_some_and(|(incoming, saved)| same_source(incoming, saved)),
                "SKILL_SLUG_CONFLICT: A different skill already uses the library directory '{slug}'"
            );
        }
        match std::fs::symlink_metadata(&central_path) {
            Ok(entry) => {
                if let Some(skill) = existing
                    && entry.is_dir()
                    && classify_target(source_dir, &central_path) == TargetState::RealDir
                    && is_valid_skill_dir(&central_path)
                {
                    return Ok(skill);
                }
                bail!("SKILL_SLUG_CONFLICT: The library directory '{slug}' already exists");
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let staging = tempfile::Builder::new()
            .prefix(".staging-")
            .tempdir_in(&self.skills_dir)?;
        let candidate = staging.path().join("candidate");
        copy_dir_recursive(source_dir, &candidate)?;
        ensure!(
            is_valid_skill_dir(&candidate),
            "The staged skill has no SKILL.md"
        );
        let meta = parse_skill_dir(&candidate);
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
            content_hash: Some(hash_directory(&candidate)?),
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
            project_locks: existing
                .as_ref()
                .map(|s| s.project_locks.clone())
                .unwrap_or_default(),
            created_at: existing.as_ref().map(|s| s.created_at).unwrap_or(now),
            updated_at: now,
        };

        std::fs::rename(&candidate, &central_path)?;
        if let Err(error) = self.store.upsert_skill(&skill) {
            std::fs::rename(&central_path, &candidate).with_context(|| {
                format!(
                    "Could not undo the unindexed import at {}",
                    central_path.display()
                )
            })?;
            return Err(error);
        }
        Ok(skill)
    }

    // ── Deployment (with shared-directory protection & ReplacePolicy) ──

    pub fn deploy_skill(
        &self,
        skill_id_or_slug: &str,
        agent_key: &str,
        mode_override: Option<DeployMode>,
        force: bool,
    ) -> Result<Deployment> {
        let _lock = self.lock_skills()?;
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
        self.ensure_unlocked_global_target(&target_path)?;
        ensure_dst_not_overlapping_src(&skill.central_path, &target_path)?;
        let desired_mode = mode_override.unwrap_or_else(|| self.default_deploy_mode());
        let existing_dep = skill.deployments.iter().find(|d| d.agent_key == agent_key);

        let peer_mode =
            self.store
                .peer_deployment_mode_for_skill(&skill.id, &target_path, agent_key)?;
        let actual_mode = if force {
            deploy_skill_dir(
                &skill.central_path,
                &target_path,
                desired_mode,
                ReplacePolicy::UserConfirmed,
            )?
        } else {
            let recorded_mode = existing_dep
                .filter(|dep| dep.target_path == target_path)
                .map(|dep| dep.mode)
                .or(peer_mode);
            let current_mode = match classify_target(&skill.central_path, &target_path) {
                TargetState::Absent => None,
                TargetState::LinkToSource => Some(DeployMode::Symlink),
                TargetState::RealDir
                    if recorded_mode == Some(DeployMode::Copy)
                        && matches_skill_copy(&skill, &target_path)
                        && skill
                            .deployments
                            .iter()
                            .filter(|dep| dep.target_path == target_path)
                            .all(|dep| {
                                verify_managed_target(
                                    &skill.central_path,
                                    &target_path,
                                    dep.mode,
                                    dep.copy_hash.as_deref().or(skill.content_hash.as_deref()),
                                )
                                .is_ok()
                            }) =>
                {
                    Some(DeployMode::Copy)
                }
                _ => bail!(
                    "TARGET_CONFLICT: Deployment target is occupied or has changed: {}",
                    target_path.display()
                ),
            };
            match current_mode {
                Some(mode) if mode_override.is_none_or(|desired| desired == mode) => mode,
                Some(mode) => deploy_skill_dir(
                    &skill.central_path,
                    &target_path,
                    desired_mode,
                    ReplacePolicy::Recorded(mode),
                )?,
                None => deploy_skill_dir(
                    &skill.central_path,
                    &target_path,
                    desired_mode,
                    ReplacePolicy::CreateOnly,
                )?,
            }
        };

        let copy_hash = (actual_mode == DeployMode::Copy)
            .then(|| managed_copy_fingerprint(&skill.central_path))
            .transpose()?;
        let dep = Deployment {
            id: existing_dep
                .map(|d| d.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            skill_id: skill.id.clone(),
            agent_key: agent.key.clone(),
            target_path,
            mode: actual_mode,
            copy_hash,
            synced_at: Utc::now().timestamp(),
        };
        self.store.upsert_deployment(&dep)?;
        Ok(dep)
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

    pub fn deploy_skill_to_project(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        agent_key: &str,
        mode_override: Option<DeployMode>,
        force: bool,
    ) -> Result<ProjectDeployment> {
        self.deploy_skill_to_project_at(
            skill_id_or_slug,
            project_id_or_name,
            Path::new(""),
            agent_key,
            mode_override,
            force,
        )
    }

    pub fn deploy_skill_to_project_at(
        &self,
        skill_id_or_slug: &str,
        project_id_or_name: &str,
        scope: &Path,
        agent_key: &str,
        mode_override: Option<DeployMode>,
        force: bool,
    ) -> Result<ProjectDeployment> {
        let _lock = self.lock_skills()?;
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

        let target_path = scoped_project_skills_dir(&project, scope, agent)?.join(&skill.slug);
        let scope = project_scope_key(scope);
        let scope = scope.as_path();
        let locked =
            self.project_deploy_snapshot(&skill, &project.id, scope, agent_key, &target_path)?;
        let source = locked
            .as_ref()
            .map_or(skill.central_path.as_path(), |(path, _)| path);
        ensure_dst_not_overlapping_src(source, &target_path)?;
        let mode_override = if locked.is_some() {
            Some(DeployMode::Copy)
        } else {
            mode_override
        };
        let desired_mode = mode_override.unwrap_or_else(|| self.default_deploy_mode());
        let (actual_mode, registering) = if force && locked.is_none() {
            (
                deploy_skill_dir(
                    &skill.central_path,
                    &target_path,
                    desired_mode,
                    ReplacePolicy::UserConfirmed,
                )?,
                false,
            )
        } else {
            match inspect_stack_project_target(&skill, &project, agent, scope, &target_path) {
                StackProjectTarget::Missing(recorded_mode) => (
                    deploy_skill_dir(
                        source,
                        &target_path,
                        mode_override.or(recorded_mode).unwrap_or(desired_mode),
                        ReplacePolicy::CreateOnly,
                    )?,
                    false,
                ),
                StackProjectTarget::Deployed(mode)
                    if mode_override.is_none_or(|desired| desired == mode) =>
                {
                    return skill
                        .project_deployments
                        .into_iter()
                        .find(|dep| {
                            dep.project_id == project.id
                                && dep.agent_key == agent_key
                                && dep.scope_path == scope
                                && dep.target_path == target_path
                                && dep.mode == mode
                        })
                        .context("Project deployment record no longer matches the target");
                }
                StackProjectTarget::Register(mode)
                    if mode_override.is_none_or(|desired| desired == mode) =>
                {
                    (mode, true)
                }
                StackProjectTarget::Deployed(mode) | StackProjectTarget::Register(mode) => (
                    deploy_skill_dir(
                        &skill.central_path,
                        &target_path,
                        desired_mode,
                        ReplacePolicy::Recorded(mode),
                    )?,
                    false,
                ),
                StackProjectTarget::Unavailable => {
                    bail!("The skill directory is no longer available")
                }
                StackProjectTarget::Conflict | StackProjectTarget::Local => {
                    bail!(
                        "TARGET_CONFLICT: Deployment target is occupied or has changed: {}",
                        target_path.display()
                    );
                }
            }
        };

        let dep = self.prepare_project_deployment(
            &skill,
            &project,
            agent,
            scope,
            target_path,
            actual_mode,
        )?;
        if actual_mode == DeployMode::Copy && !registering {
            ensure!(
                dep.copy_hash.as_deref() == Some(managed_copy_fingerprint(source)?.as_str()),
                "TARGET_CONFLICT: The deployed copy changed during installation"
            );
        }
        self.store.upsert_project_deployment(&dep)?;
        Ok(dep)
    }

    fn prepare_project_deployment(
        &self,
        skill: &Skill,
        project: &ProjectWorkspace,
        agent: &AgentInfo,
        scope: &Path,
        target_path: PathBuf,
        actual_mode: DeployMode,
    ) -> Result<ProjectDeployment> {
        let copy_hash = (actual_mode == DeployMode::Copy)
            .then(|| managed_copy_fingerprint(&target_path))
            .transpose()?;
        let locked_hash = skill.project_lock(&project.id, scope).map(str::to_owned);
        if let Some(hash) = &locked_hash {
            ensure!(
                actual_mode == DeployMode::Copy
                    && project_version_hash(&target_path)? == *hash
                    && copy_hash.is_some(),
                "STALE_PROJECT_VERSION: The deployed files differ from the project version"
            );
        }
        let follow_mode = locked_hash.as_ref().map(|_| {
            skill
                .project_deployments
                .iter()
                .find(|dep| {
                    dep.target_path == target_path
                        && dep.project_id == project.id
                        && dep.scope_path == scope
                })
                .and_then(|dep| dep.follow_mode)
                .unwrap_or_else(|| self.default_deploy_mode())
        });
        Ok(ProjectDeployment {
            id: skill
                .project_deployments
                .iter()
                .find(|dep| {
                    dep.project_id == project.id
                        && dep.agent_key == agent.key
                        && dep.scope_path == scope
                })
                .map(|d| d.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            skill_id: skill.id.clone(),
            project_id: project.id.clone(),
            agent_key: agent.key.clone(),
            scope_path: scope.to_path_buf(),
            target_path,
            mode: actual_mode,
            copy_hash,
            locked_hash,
            follow_mode,
            synced_at: Utc::now().timestamp(),
        })
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
        let _lock = self.lock_skills()?;
        self.ensure_unlocked_global_target(&candidate.path)?;
        let file_name = candidate.path.file_name().and_then(|s| s.to_str());
        let slug = file_name
            .and_then(sanitize_slug)
            .context("Could not derive a safe directory slug for skill")?;
        let target = physical_target_path(&candidate.path)
            .context("Could not resolve the parent of the local skill")?;
        let skills = self.store.list_skills()?;
        for skill in &skills {
            if let Some(deployment) = skill.deployments.iter().find(|deployment| {
                deployment.target_path == candidate.path
                    || physical_target_path(&deployment.target_path).as_ref() == Some(&target)
            }) {
                ensure!(
                    verify_managed_target(
                        &skill.central_path,
                        &candidate.path,
                        deployment.mode,
                        deployment.copy_hash.as_deref(),
                    )? != TargetState::Absent,
                    "TARGET_CONFLICT: The deployment no longer exists: {}",
                    candidate.path.display()
                );
                return Ok(skill.clone());
            }
        }

        let existing = skills
            .into_iter()
            .find(|skill| skill.slug.eq_ignore_ascii_case(&slug));
        let (skill, actual_mode, copy_hash) = if let Some(skill) = existing {
            if skill
                .deployments
                .iter()
                .any(|deployment| deployment.agent_key == candidate.agent_key)
            {
                return Err(AdoptionMergeConflict.into());
            }
            let fingerprint = managed_copy_fingerprint(&skill.central_path).ok();
            ensure!(
                skill.central_path == self.skills_dir.join(&skill.slug)
                    && is_valid_skill_dir(&skill.central_path)
                    && is_valid_skill_dir(&candidate.path)
                    && ensure_dst_not_overlapping_src(&skill.central_path, &candidate.path).is_ok()
                    && classify_target(&skill.central_path, &candidate.path)
                        == TargetState::RealDir
                    && fingerprint.as_ref().is_some_and(|expected| {
                        managed_copy_fingerprint(&candidate.path)
                            .is_ok_and(|actual| &actual == expected)
                    }),
                "SKILL_SLUG_CONFLICT: A different skill already uses the library directory '{slug}'"
            );
            // The existing local directory already matches the library exactly.
            // Register it as a managed copy without rewriting it or changing the library source.
            (skill, DeployMode::Copy, fingerprint)
        } else {
            let skill = self.import_directory_locked(
                &candidate.path,
                file_name,
                SkillImportSource {
                    source_type: SourceType::Adopted,
                    source_ref: Some(candidate.path.to_string_lossy().into_owned()),
                    subpath: None,
                    branch: None,
                },
            )?;
            let mode = deploy_skill_dir(
                &skill.central_path,
                &candidate.path,
                self.default_deploy_mode(),
                ReplacePolicy::UserConfirmed,
            )?;
            let copy_hash = (mode == DeployMode::Copy)
                .then(|| managed_copy_fingerprint(&skill.central_path))
                .transpose()?;
            (skill, mode, copy_hash)
        };
        // Record the exact discovered candidate path (whether inside the agent's
        // primary `skills_dir` or an `additional_scan_dirs` entry like `~/.agents/skills`).
        let dep = Deployment {
            id: Uuid::new_v4().to_string(),
            skill_id: skill.id.clone(),
            agent_key: candidate.agent_key.clone(),
            target_path: candidate.path.clone(),
            mode: actual_mode,
            copy_hash,
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

    pub fn find_local_stack(&self, id: &str) -> Result<Option<StackManifest>> {
        Ok(self.store.find_local_stack(id)?.map(|mut stack| {
            stack.source = StackSource::Local;
            stack
        }))
    }

    pub fn save_local_stack_if_unchanged(
        &self,
        stack: &StackManifest,
        previous: Option<&StackManifest>,
    ) -> Result<()> {
        let _lock = self.lock_skills()?;
        let saved = self.find_local_stack(&stack.id)?;
        ensure!(
            saved.as_ref() == previous,
            "STACK_ID_CONFLICT: The local stack already exists or has changed: {}",
            stack.id
        );
        self.store.save_local_stack(stack, Utc::now().timestamp())
    }

    pub fn save_local_stack(&self, stack: &StackManifest) -> Result<()> {
        let _lock = self.lock_skills()?;
        self.store.save_local_stack(stack, Utc::now().timestamp())
    }

    pub fn delete_local_stack(&self, stack_id: &str) -> Result<()> {
        let _lock = self.lock_skills()?;
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
        self.save_local_stack_if_unchanged(&manifest, None)?;
        Ok(manifest)
    }

    pub fn export_stack_toml(&self, stack: &StackManifest) -> Result<String> {
        export_stack_to_toml(stack)
    }

    pub fn import_stack_toml(&self, toml_text: &str) -> Result<StackManifest> {
        self.keep_local_stack(&parse_stack_from_toml(toml_text)?)
    }

    fn keep_local_stack(&self, stack: &StackManifest) -> Result<StackManifest> {
        let mut stack = stack.clone();
        stack.source = StackSource::Local;
        let _lock = self.lock_skills()?;
        if let Some(saved) = self.find_local_stack(&stack.id)? {
            ensure!(
                saved == stack,
                "STACK_ID_CONFLICT: A customized local stack uses this ID: {}",
                stack.id
            );
        } else {
            self.store
                .save_local_stack(&stack, Utc::now().timestamp())?;
        }
        Ok(stack)
    }

    /// Installs every skill in a Stack (skipping ones already in the library),
    /// saves the stack locally, and optionally deploys all skills to `deploy_agents`.
    pub fn install_stack(
        &self,
        stack: &StackManifest,
        deploy_agents: &[String],
    ) -> Result<StackInstallSummary> {
        let stack = self.keep_local_stack(stack)?;
        let mut installed = Vec::new();
        let mut skipped_existing = Vec::new();
        let mut failed = Vec::new();
        let mut deployment_failures = Vec::new();

        for item in &stack.skills {
            let existing = item.deployable_in(&self.store.list_skills()?).cloned();
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
                    if let Err(error) = self.deploy_skill(&sk.id, agent_key, None, false) {
                        deployment_failures.push(StackDeploymentFailure {
                            skill: item.name.clone(),
                            agent: agent_key.clone(),
                            error: format!("{error:#}"),
                        });
                    }
                }
            }
        }

        let deployed_agents = if failed.is_empty() && !stack.skills.is_empty() {
            deploy_agents
                .iter()
                .filter(|key| {
                    !deployment_failures
                        .iter()
                        .any(|failure| &failure.agent == *key)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        Ok(StackInstallSummary {
            stack_id: stack.id.clone(),
            installed,
            skipped_existing,
            failed,
            deployed_agents,
            deployment_failures,
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
        self.stack_project_deploy_status_at(stack, project_id, agent_key, Path::new(""))
    }

    pub fn stack_project_deploy_status_at(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
        scope: &Path,
    ) -> Result<StackProjectDeployStatus> {
        let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
        let directory = scoped_project_skills_dir(&project, scope, &agent)?;
        let skills = self.list_skills()?;
        let mut status = StackProjectDeployStatus::default();
        for item in &stack.skills {
            let Some(skill) = item.deployable_in(&skills) else {
                status.unavailable += 1;
                continue;
            };
            let target = directory.join(&skill.slug);
            match inspect_stack_project_target(skill, &project, &agent, scope, &target) {
                StackProjectTarget::Deployed(_) => status.deployed += 1,
                StackProjectTarget::Local => status.project_local += 1,
                StackProjectTarget::Missing(_)
                    if skill.project_lock(&project.id, scope).is_none()
                        && is_globally_available(skill, &agent) =>
                {
                    status.global_available += 1;
                }
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
        self.deploy_stack_to_project_at(stack, project_id, agent_key, Path::new(""))
    }

    pub fn deploy_stack_to_project_at(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
        scope: &Path,
    ) -> Result<StackProjectDeploySummary> {
        self.deploy_stack_to_project_at_with_globals(stack, project_id, agent_key, scope, false)
    }

    /// With `include_global`, also place globally available skills in the project directory.
    pub fn deploy_stack_to_project_at_with_globals(
        &self,
        stack: &StackManifest,
        project_id: &str,
        agent_key: &str,
        scope: &Path,
        include_global: bool,
    ) -> Result<StackProjectDeploySummary> {
        let _lock = self.lock_skills()?;
        let (project, agent) = self.stack_deploy_destination(project_id, agent_key)?;
        let directory = scoped_project_skills_dir(&project, scope, &agent)?;
        let scope = project_scope_key(scope);
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
            let target = directory.join(&skill.slug);
            let placement = match inspect_stack_project_target(
                skill,
                &project,
                &agent,
                scope.as_ref(),
                &target,
            ) {
                StackProjectTarget::Deployed(_) => {
                    summary.already_deployed += 1;
                    continue;
                }
                StackProjectTarget::Local => {
                    summary.project_local += 1;
                    continue;
                }
                StackProjectTarget::Missing(_)
                    if !include_global
                        && skill.project_lock(&project.id, &scope).is_none()
                        && is_globally_available(skill, &agent) =>
                {
                    summary.global_available += 1;
                    continue;
                }
                StackProjectTarget::Missing(recorded_mode) => self
                    .project_deploy_snapshot(skill, &project.id, &scope, agent_key, &target)
                    .and_then(|locked| {
                        let (source, mode) = match &locked {
                            Some((source, _)) => (source.as_path(), DeployMode::Copy),
                            None => (
                                skill.central_path.as_path(),
                                recorded_mode.unwrap_or_else(|| self.default_deploy_mode()),
                            ),
                        };
                        let mode =
                            deploy_skill_dir(source, &target, mode, ReplacePolicy::CreateOnly)?;
                        let expected = (mode == DeployMode::Copy)
                            .then(|| managed_copy_fingerprint(source))
                            .transpose()?;
                        Ok((mode, expected))
                    }),
                StackProjectTarget::Register(mode) => self
                    .project_deploy_snapshot(skill, &project.id, &scope, agent_key, &target)
                    .map(|_| (mode, None)),
                StackProjectTarget::Conflict => Err(anyhow::anyhow!(
                    "TARGET_CONFLICT: Deployment target is occupied or has changed: {}",
                    target.display()
                )),
                StackProjectTarget::Unavailable => {
                    summary.unavailable += 1;
                    continue;
                }
            };
            let deployment = placement.and_then(|(mode, expected)| {
                let dep =
                    self.prepare_project_deployment(skill, &project, &agent, &scope, target, mode)?;
                ensure!(
                    expected.is_none_or(|hash| dep.copy_hash.as_ref() == Some(&hash)),
                    "TARGET_CONFLICT: The deployed copy changed during installation"
                );
                self.store.upsert_project_deployment(&dep)
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

    pub fn check_app_update(&self, current_version: &str) -> Result<UpdateCheckOutcome> {
        let proxy = self.http_proxy();
        let token = self.github_token();
        let latest =
            fetch_latest_release(DEFAULT_RELEASES_API_URL, proxy.as_deref(), token.as_deref())?;
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
        self.download_app_update_with_progress(version, asset, checksums_asset, |_| {})
    }

    pub fn download_app_update_with_progress(
        &self,
        version: &str,
        asset: &ReleaseAsset,
        checksums_asset: Option<&ReleaseAsset>,
        progress: impl FnMut(UpdateProgress),
    ) -> Result<DownloadedUpdate> {
        let proxy = self.http_proxy();
        let token = self.github_token();
        let updates_dir = self.root_dir.join("updates");
        download_and_verify_update_with_progress(
            version,
            asset,
            checksums_asset,
            &updates_dir,
            proxy.as_deref(),
            token.as_deref(),
            progress,
        )
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
