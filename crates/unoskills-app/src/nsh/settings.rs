use std::sync::Mutex;

use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use unoskills_core::{AgentCategory, CustomAgentRecord, DeployMode, UnoSkillsManager};

const PREFERENCES: [(&str, &str); 7] = [
    ("theme", "system"),
    ("accent_palette", "unoskills"),
    ("font_size", "14"),
    ("corner_radius", "auto"),
    ("ui_font", "auto"),
    ("mono_font", "auto"),
    ("sidebar_collapsed", "false"),
];

static SETTINGS_MUTATION: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct Preference {
    key: String,
    value: String,
}

#[derive(Deserialize)]
struct AgentToggle {
    key: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct Network {
    proxy: String,
    token: String,
}

#[derive(Deserialize)]
struct Backup {
    repo_url: String,
    branch: String,
}

#[derive(Deserialize)]
struct CustomAgent {
    name: String,
    skills_dir: String,
}

#[derive(Deserialize)]
struct AgentKey {
    key: String,
}

pub(super) fn dispatch(manager: &UnoSkillsManager, command: &str, input: Value) -> Result<Value> {
    if command == "settings.get" {
        return snapshot(manager);
    }
    if command == "settings.agents" {
        return Ok(json!(manager.list_agents()?));
    }
    if command == "settings.backup_status" {
        return Ok(json!(manager.git_backup_status()?));
    }

    let _guard = SETTINGS_MUTATION
        .lock()
        .map_err(|_| anyhow::anyhow!("SETTINGS_LOCK_FAILED"))?;
    match command {
        "settings.preference" => {
            let Preference { key, value } = serde_json::from_value(input)?;
            check_preference(&key, &value)?;
            manager.set_setting(&key, &value)?;
        }
        "settings.deploy_mode" => {
            manager.set_default_deploy_mode(serde_json::from_value::<DeployMode>(input)?)?;
        }
        "settings.network" => {
            let Network { proxy, token } = serde_json::from_value(input)?;
            manager.set_http_proxy(Some(&proxy))?;
            manager.set_github_token(Some(&token))?;
        }
        "settings.agent_enable" => {
            let AgentToggle { key, enabled } = serde_json::from_value(input)?;
            manager.set_agent_enabled(&key, enabled)?;
        }
        "settings.agent_add" => add_agent(manager, serde_json::from_value(input)?)?,
        "settings.agent_remove" => {
            let AgentKey { key } = serde_json::from_value(input)?;
            manager.remove_custom_agent(&key)?;
        }
        "settings.backup_save" | "settings.backup_push" | "settings.backup_pull" => {
            let Backup { repo_url, branch } = serde_json::from_value(input)?;
            return match command {
                "settings.backup_push" => {
                    Ok(json!(manager.git_backup_push_to(&repo_url, &branch)?))
                }
                "settings.backup_pull" => {
                    Ok(json!(manager.git_backup_pull_from(&repo_url, &branch)?))
                }
                _ => {
                    manager.configure_git_backup(&repo_url, Some(&branch))?;
                    Ok(json!(manager.git_backup_status()?))
                }
            };
        }
        _ => bail!("UNKNOWN_COMMAND: {command}"),
    }
    Ok(Value::Null)
}

fn snapshot(manager: &UnoSkillsManager) -> Result<Value> {
    let mut result = serde_json::Map::new();
    result.insert("language".into(), json!(manager.get_setting("language")?));
    for (key, default) in PREFERENCES {
        let value = manager
            .get_setting(key)?
            .unwrap_or_else(|| default.to_owned());
        result.insert(key.to_owned(), json!(value));
    }
    result.insert("deploy_mode".into(), json!(manager.default_deploy_mode()));
    result.insert(
        "proxy".into(),
        json!(manager.http_proxy().unwrap_or_default()),
    );
    result.insert(
        "token".into(),
        json!(manager.github_token().unwrap_or_default()),
    );
    result.insert("root_dir".into(), json!(manager.root_dir()));
    result.insert("version".into(), json!(env!("CARGO_PKG_VERSION")));
    Ok(Value::Object(result))
}

fn check_preference(key: &str, value: &str) -> Result<()> {
    let valid = match key {
        "language" => matches!(value, "en" | "zh-CN" | "ja"),
        "theme" => matches!(value, "system" | "light" | "dark"),
        "accent_palette" => matches!(
            value,
            "claude" | "vercel" | "supabase" | "catppuccin" | "ocean" | "unoskills"
        ),
        "font_size" => matches!(value, "13" | "14" | "15" | "16"),
        "corner_radius" => matches!(value, "auto" | "0" | "4" | "6" | "8" | "12"),
        "sidebar_collapsed" => matches!(value, "true" | "false"),
        "ui_font" | "mono_font" => !value.trim().is_empty(),
        _ => false,
    };
    if !valid {
        bail!("INVALID_PREFERENCE: {key}");
    }
    Ok(())
}

fn add_agent(manager: &UnoSkillsManager, input: CustomAgent) -> Result<()> {
    let name = input.name.trim();
    let directory = input.skills_dir.trim();
    if name.is_empty() || directory.is_empty() {
        bail!("CUSTOM_AGENT_REQUIRED");
    }
    let slug = unoskills_core::parser::sanitize_slug(name)
        .unwrap_or_else(|| "custom".to_owned())
        .to_lowercase()
        .replace([' ', '-'], "_");
    let key = format!("custom_{slug}");
    manager.add_custom_agent(&CustomAgentRecord {
        key,
        display_name: name.to_owned(),
        skills_dir: directory.to_owned(),
        category: AgentCategory::Coding,
    })
}
