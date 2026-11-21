use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use unoskills_core::{ProjectSkillPlacement, ProjectSkillStatus, UnoSkillsManager, parser};

use super::{optional_bool, optional_deploy_mode, optional_str, required_str};

pub(super) fn dispatch(manager: &UnoSkillsManager, sub: &str, input: Value) -> Result<Value> {
    match sub {
        "state" => load_state(manager, &input),
        "project_statuses" => project_statuses(manager, &input),
        "get_markdown" => get_markdown(manager, &input),
        "save_markdown" => save_markdown(manager, &input),
        "lint_skill" => lint_skill(manager, &input),
        "update_skill" => update_skill(manager, &input),
        "remove_skill" => remove_skill(manager, &input),
        "deploy_skill" => deploy_skill(manager, &input),
        "undeploy_skill" => undeploy_skill(manager, &input),
        "deploy_project_skill" => deploy_project_skill(manager, &input),
        "undeploy_project_skill" => undeploy_project_skill(manager, &input),
        "attach_project" => attach_project(manager, &input),
        "detach_project" => detach_project(manager, &input),
        "reveal_path" => reveal_path(&input),
        _ => bail!("Unknown library host command: 'library.{sub}'"),
    }
}

fn load_state(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skills = manager.list_skills()?;
    let agents = manager.list_agents()?;
    let projects = manager.list_projects()?;
    let deploy_mode = manager.default_deploy_mode();

    let requested_project = optional_str(input, "project_id")?
        .map(str::trim)
        .filter(|id| !id.is_empty());

    let (project_statuses, project_status_error) = match requested_project {
        Some(project_id) if projects.iter().any(|project| project.id == project_id) => {
            match manager.project_skill_statuses(project_id) {
                Ok(statuses) => (Some(serialize_project_statuses(&statuses)), None),
                Err(error) => (None, Some(format!("{error:#}"))),
            }
        }
        _ => (None, None),
    };

    Ok(json!({
        "skills": skills,
        "agents": agents,
        "projects": projects,
        "deploy_mode": deploy_mode.as_str(),
        "project_statuses": project_statuses,
        "project_status_error": project_status_error,
    }))
}

fn project_statuses(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let project_id = required_str(input, "project_id")?.trim();
    if project_id.is_empty() {
        bail!("Field 'project_id' must not be empty");
    }
    let statuses = manager.project_skill_statuses(project_id)?;
    Ok(json!({
        "project_id": project_id,
        "statuses": serialize_project_statuses(&statuses),
    }))
}

fn serialize_project_statuses(
    statuses: &HashMap<String, HashMap<String, ProjectSkillStatus>>,
) -> Value {
    let mut by_agent = Map::new();
    for (agent_key, skills) in statuses {
        let mut by_skill = Map::new();
        for (skill_id, status) in skills {
            by_skill.insert(
                skill_id.clone(),
                json!({
                    "placement": placement_str(status.placement),
                    "global_available": status.global_available,
                }),
            );
        }
        by_agent.insert(agent_key.clone(), Value::Object(by_skill));
    }
    Value::Object(by_agent)
}

fn placement_str(placement: ProjectSkillPlacement) -> &'static str {
    match placement {
        ProjectSkillPlacement::Missing => "missing",
        ProjectSkillPlacement::Managed => "managed",
        ProjectSkillPlacement::Local => "local",
        ProjectSkillPlacement::Conflict => "conflict",
        ProjectSkillPlacement::Unavailable => "unavailable",
    }
}

fn get_markdown(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    if skill_id.is_empty() {
        bail!("Field 'skill_id' must not be empty");
    }
    let (entry_file, raw_markdown) = manager.read_skill_markdown(skill_id)?;
    let preview_markdown = parser::strip_frontmatter(&raw_markdown).to_string();
    let lint = manager.lint_skill(skill_id).ok();

    Ok(json!({
        "skill_id": skill_id,
        "entry_file": entry_file,
        "raw_markdown": raw_markdown,
        "preview_markdown": preview_markdown,
        "lint": lint,
    }))
}

fn save_markdown(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    if skill_id.is_empty() {
        bail!("Field 'skill_id' must not be empty");
    }
    let content = required_str(input, "content")?;
    let skill = manager.save_skill_markdown(skill_id, content)?;
    let (entry_file, raw_markdown) = manager.read_skill_markdown(&skill.id)?;
    let preview_markdown = parser::strip_frontmatter(&raw_markdown).to_string();
    let lint = manager.lint_skill(&skill.id).ok();

    Ok(json!({
        "skill": skill,
        "entry_file": entry_file,
        "raw_markdown": raw_markdown,
        "preview_markdown": preview_markdown,
        "lint": lint,
    }))
}

fn lint_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    if skill_id.is_empty() {
        bail!("Field 'skill_id' must not be empty");
    }
    let report = manager.lint_skill(skill_id)?;
    Ok(serde_json::to_value(report)?)
}

fn update_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    if skill_id.is_empty() {
        bail!("Field 'skill_id' must not be empty");
    }
    let outcome = manager.update_skill(skill_id)?;
    Ok(serde_json::to_value(outcome)?)
}

fn remove_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    if skill_id.is_empty() {
        bail!("Field 'skill_id' must not be empty");
    }
    manager.remove_skill(skill_id)?;
    Ok(json!({
        "removed": true,
        "skill_id": skill_id,
    }))
}

fn deploy_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    let agent_key = required_str(input, "agent_key")?.trim();
    if skill_id.is_empty() || agent_key.is_empty() {
        bail!("Fields 'skill_id' and 'agent_key' must not be empty");
    }
    let mode = optional_deploy_mode(input, "mode")?;
    let force = optional_bool(input, "force")?;
    let deployment = manager.deploy_skill(skill_id, agent_key, mode, force)?;
    Ok(serde_json::to_value(deployment)?)
}

fn undeploy_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    let agent_key = required_str(input, "agent_key")?.trim();
    if skill_id.is_empty() || agent_key.is_empty() {
        bail!("Fields 'skill_id' and 'agent_key' must not be empty");
    }
    manager.undeploy_skill(skill_id, agent_key)?;
    Ok(json!({
        "undeployed": true,
        "skill_id": skill_id,
        "agent_key": agent_key,
    }))
}

fn deploy_project_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    let project_id = required_str(input, "project_id")?.trim();
    let agent_key = required_str(input, "agent_key")?.trim();
    if skill_id.is_empty() || project_id.is_empty() || agent_key.is_empty() {
        bail!("Fields 'skill_id', 'project_id', and 'agent_key' must not be empty");
    }
    let mode = optional_deploy_mode(input, "mode")?;
    let force = optional_bool(input, "force")?;
    let deployment =
        manager.deploy_skill_to_project(skill_id, project_id, agent_key, mode, force)?;
    Ok(serde_json::to_value(deployment)?)
}

fn undeploy_project_skill(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let skill_id = required_str(input, "skill_id")?.trim();
    let project_id = required_str(input, "project_id")?.trim();
    let agent_key = required_str(input, "agent_key")?.trim();
    if skill_id.is_empty() || project_id.is_empty() || agent_key.is_empty() {
        bail!("Fields 'skill_id', 'project_id', and 'agent_key' must not be empty");
    }
    manager.undeploy_skill_from_project(skill_id, project_id, agent_key)?;
    Ok(json!({
        "undeployed": true,
        "skill_id": skill_id,
        "project_id": project_id,
        "agent_key": agent_key,
    }))
}

fn attach_project(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let raw_path = required_str(input, "path")?.trim();
    if raw_path.is_empty() {
        bail!("Field 'path' must not be empty");
    }
    let name = optional_str(input, "name")?
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let project = manager.add_project(Path::new(raw_path), name)?;
    Ok(serde_json::to_value(project)?)
}

fn detach_project(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let project_id = required_str(input, "project_id")?.trim();
    if project_id.is_empty() {
        bail!("Field 'project_id' must not be empty");
    }
    manager.remove_project(project_id)?;
    Ok(json!({
        "detached": true,
        "project_id": project_id,
    }))
}

fn reveal_path(input: &Value) -> Result<Value> {
    let raw_path = required_str(input, "path")?.trim();
    if raw_path.is_empty() {
        bail!("Field 'path' must not be empty");
    }
    let path = Path::new(raw_path);
    if !path.exists() {
        bail!("Path does not exist: {}", path.display());
    }

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut command = Command::new("explorer");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    cmd.spawn()
        .with_context(|| format!("Failed to reveal path in file manager: {}", path.display()))?;

    Ok(json!({
        "revealed": true,
        "path": raw_path,
    }))
}
