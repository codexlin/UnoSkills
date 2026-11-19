use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::domain::{
    AdoptCandidate, AgentInfo, DeployMode, Deployment, MarketplacePage, MarketplaceSkill, Skill,
    SkillLintReport, SourceType, StackInstallSummary, StackManifest, StackSkillRef,
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
    DefaultStackRegistry, StackRegistry, export_stack_to_toml, parse_stack_from_toml,
};
use crate::store::Store;
use crate::sync::{
    ReplacePolicy, copy_dir_recursive, deploy_skill_dir, ensure_dst_not_overlapping_src,
    remove_managed_target,
};

pub struct UnoSkillsManager {
    root_dir: PathBuf,
    skills_dir: PathBuf,
    store: Store,
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
                let fetched = fetch_git_skill(&repo_url, branch.as_deref(), subpath.as_deref())?;
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

        // Undeploy from every agent first (respecting shared directories).
        let agent_keys: Vec<String> = skill
            .deployments
            .iter()
            .map(|d| d.agent_key.clone())
            .collect();
        for agent_key in agent_keys {
            let _ = self.undeploy_skill(&skill.id, &agent_key);
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

    pub fn featured_stacks(&self) -> Result<Vec<StackManifest>> {
        let remote_url = self.store.get_setting("stacks_registry_url")?;
        let registry = DefaultStackRegistry {
            remote_index_url: remote_url.filter(|s| !s.trim().is_empty()),
        };
        registry.fetch_featured()
    }

    pub fn list_local_stacks(&self) -> Result<Vec<StackManifest>> {
        self.store.list_local_stacks()
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
            skills: refs,
        };
        self.save_local_stack(&manifest)?;
        Ok(manifest)
    }

    pub fn export_stack_toml(&self, stack: &StackManifest) -> Result<String> {
        export_stack_to_toml(stack)
    }

    pub fn import_stack_toml(&self, toml_text: &str) -> Result<StackManifest> {
        let manifest = parse_stack_from_toml(toml_text)?;
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
            let existing = self.store.find_skill(&item.name)?;
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

    // ── Marketplace (skills.sh) ──

    pub fn marketplace_leaderboard(&self, board: &str) -> Result<Vec<MarketplaceSkill>> {
        fetch_skillssh_leaderboard(board)
    }

    pub fn marketplace_leaderboard_page(
        &self,
        board: &str,
        page: usize,
    ) -> Result<MarketplacePage> {
        fetch_skillssh_leaderboard_page(board, page)
    }

    pub fn marketplace_search(&self, query: &str, limit: usize) -> Result<Vec<MarketplaceSkill>> {
        search_skillssh(query, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
