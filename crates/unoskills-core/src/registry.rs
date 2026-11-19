use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::domain::{AgentCategory, AgentConfig, AgentInfo};

const BUILTIN_AGENTS_TOML: &str = include_str!("../data/agents.toml");

#[derive(Debug, Deserialize)]
struct BuiltinAgentsFile {
    agent: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentRecord {
    pub key: String,
    pub display_name: String,
    pub skills_dir: String,
    #[serde(default)]
    pub category: AgentCategory,
}

pub fn builtin_agent_configs() -> Result<Vec<AgentConfig>> {
    let parsed: BuiltinAgentsFile = toml::from_str(BUILTIN_AGENTS_TOML)?;
    Ok(parsed.agent)
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn candidate_paths(relative: &str) -> Vec<PathBuf> {
    let mut candidates = vec![home_dir().join(relative)];
    if let Some(suffix) = relative.strip_prefix(".config/")
        && let Some(config_dir) = dirs::config_dir()
    {
        let alt = config_dir.join(suffix);
        if !candidates.contains(&alt) {
            candidates.push(alt);
        }
    }
    candidates
}

fn resolve_relative_path(relative: &str) -> PathBuf {
    let candidates = candidate_paths(relative);
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}

pub fn resolve_agents(
    overrides: &HashMap<String, String>,
    disabled_keys: &[String],
    custom_agents: &[CustomAgentRecord],
) -> Result<Vec<AgentInfo>> {
    let mut list = Vec::new();

    for cfg in builtin_agent_configs()? {
        let override_path = overrides.get(&cfg.key).cloned();
        let has_override = override_path.is_some();
        let skills_dir = override_path
            .map(PathBuf::from)
            .unwrap_or_else(|| resolve_relative_path(&cfg.relative_skills_dir));
        let detect_dir = resolve_relative_path(&cfg.relative_detect_dir);
        let installed = has_override
            || candidate_paths(&cfg.relative_detect_dir)
                .iter()
                .any(|p| p.exists());
        let additional_scan_dirs = cfg
            .additional_scan_dirs
            .iter()
            .map(|rel| resolve_relative_path(rel))
            .collect();
        let project_relative_skills_dir = cfg
            .project_relative_skills_dir
            .clone()
            .unwrap_or_else(|| cfg.relative_skills_dir.clone());
        let enabled = !disabled_keys.iter().any(|k| k == &cfg.key);

        list.push(AgentInfo {
            key: cfg.key,
            display_name: cfg.display_name,
            category: cfg.category,
            skills_dir,
            detect_dir,
            additional_scan_dirs,
            project_relative_skills_dir,
            installed,
            enabled,
            is_custom: false,
            has_path_override: has_override,
            recursive_scan: cfg.recursive_scan,
        });
    }

    for custom in custom_agents {
        let skills_dir = PathBuf::from(&custom.skills_dir);
        let enabled = !disabled_keys.iter().any(|k| k == &custom.key);
        list.push(AgentInfo {
            key: custom.key.clone(),
            display_name: custom.display_name.clone(),
            category: custom.category,
            skills_dir: skills_dir.clone(),
            detect_dir: skills_dir,
            additional_scan_dirs: vec![],
            project_relative_skills_dir: format!(".{}/skills", custom.key),
            installed: true,
            enabled,
            is_custom: true,
            has_path_override: false,
            recursive_scan: false,
        });
    }

    // Sort installed agents first while preserving declarative order within groups.
    list.sort_by_key(|a| !a.installed);
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_declarative_builtin_agents() {
        let agents = builtin_agent_configs().unwrap();
        assert!(agents.len() >= 20);
        assert!(agents.iter().any(|a| a.key == "claude_code"));
        assert!(agents.iter().any(|a| a.key == "codex"));
        assert!(agents.iter().any(|a| a.key == "cursor"));
        assert!(agents.iter().any(|a| a.key == "antigravity"));
    }
}
