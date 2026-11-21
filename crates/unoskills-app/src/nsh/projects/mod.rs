use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::{Value, json};
use unoskills_core::parser::sanitize_slug;
use unoskills_core::project_analysis::{RecommendationPurpose, SkillLocation};
use unoskills_core::stacks::parse_stack_from_toml;
use unoskills_core::{Skill, StackManifest, StackSkillRef, StackSource, UnoSkillsManager};

// Host requests run on multiple workers. Keep stale-preview checks and their
// writes ordered with other mutations initiated from these screens.
static MUTATIONS: Mutex<()> = Mutex::new(());

pub(super) fn dispatch(manager: &UnoSkillsManager, command: &str, input: Value) -> Result<Value> {
    let _mutation = matches!(
        command,
        "project.attach"
            | "project.detach"
            | "project.save_recommendations"
            | "stack.create"
            | "stack.edit"
            | "stack.delete"
            | "stack.import"
            | "stack.install"
            | "stack.install_member"
            | "stack.confirm_deploy"
    )
    .then(|| {
        MUTATIONS
            .lock()
            .map_err(|_| anyhow!("Projects or Stacks operation failed"))
    })
    .transpose()?;
    match command {
        "project.list" => Ok(json!({
            "projects": manager.list_projects()?,
            "agents": manager.list_agents()?.into_iter()
                .filter(|agent| agent.installed && agent.enabled).collect::<Vec<_>>(),
        })),
        "project.attach" => {
            let project = manager.add_project(Path::new(required(&input, "path")?), None)?;
            Ok(json!(project))
        }
        "project.detach" => {
            manager.remove_project(required(&input, "project_id")?)?;
            Ok(Value::Null)
        }
        "project.scan" => scan(manager, required(&input, "project_id")?),
        "project.scopes" => Ok(json!(
            manager.project_module_paths(required(&input, "project_id")?)?
        )),
        "project.inventory" => inventory(manager, &input),
        "project.save_recommendations" => save_project_stack(manager, &input),
        "stack.list" => Ok(json!({
            "featured": manager.featured_stacks()?,
            "local": manager.list_local_stacks()?,
        })),
        "stack.community" => Ok(json!(manager.community_stacks()?)),
        "stack.library" => Ok(json!(
            manager
                .list_skills()?
                .iter()
                .map(library_row)
                .collect::<Vec<_>>()
        )),
        "stack.members" => {
            let stack = find_stack(manager, &input)?;
            let skills = manager.list_skills()?;
            Ok(json!(stack.skills.iter().map(|member| {
                let installed = member.installed_in(&skills);
                json!({
                    "member": member,
                    "installed": installed.map(library_row),
                    "source_conflict": installed.is_some_and(|skill| member.has_source_conflict(skill)),
                })
            }).collect::<Vec<_>>()))
        }
        "stack.create" => create_stack(manager, &input),
        "stack.edit" => edit_stack(manager, &input),
        "stack.delete" => {
            let stack = find_local_stack(manager, required(&input, "stack_id")?)?;
            manager.delete_local_stack(&stack.id)?;
            Ok(Value::Null)
        }
        "stack.read_toml" => Ok(json!(
            fs::read_to_string(required(&input, "path")?)
                .context("Could not read the stack file")?
        )),
        "stack.preview_import" | "stack.import" => import_stack(manager, command, &input),
        "stack.export" => {
            let stack = find_stack(manager, &input)?;
            let toml = manager.export_stack_toml(&stack)?;
            if let Some(path) = input["path"].as_str() {
                fs::write(path, &toml).context("Could not write the stack file")?;
            }
            Ok(json!(toml))
        }
        "stack.share_gist" => {
            let stack = find_stack(manager, &input)?;
            Ok(json!(manager.share_stack_to_gist(&stack)?))
        }
        "stack.install" => install_stack(manager, &input),
        "stack.install_member" => install_member(manager, &input),
        "stack.preview_deploy" | "stack.confirm_deploy" => deploy(manager, command, &input),
        _ => bail!("Unknown Projects or Stacks command: {command}"),
    }
}

fn required<'a>(input: &'a Value, key: &str) -> Result<&'a str> {
    input[key]
        .as_str()
        .ok_or_else(|| anyhow!("Missing text field '{key}'"))
}

fn scope(input: &Value) -> Result<&Path> {
    Ok(Path::new(required(input, "scope_path")?))
}

fn scan(manager: &UnoSkillsManager, project_id: &str) -> Result<Value> {
    let mut modules = manager.analyze_project(project_id)?;
    // The project root is always a valid inventory scope, even when only a
    // nested module has a technology manifest.
    if !modules
        .iter()
        .any(|module| module.path.as_os_str().is_empty())
    {
        modules.insert(
            0,
            unoskills_core::project_analysis::ProjectModule {
                path: PathBuf::new(),
                evidence: Default::default(),
                added: Vec::new(),
                recommendations: Vec::new(),
            },
        );
    }
    Ok(json!(
        modules
            .into_iter()
            .map(|module| {
                json!({
                    "path": module.path,
                    "evidence": module.evidence.into_iter().map(|(technology, path)| json!({
                        "technology": technology.label(),
                        "path": path,
                        "added": module.added.contains(&technology),
                    })).collect::<Vec<_>>(),
                    "recommendations": module.recommendations.into_iter().map(|item| json!({
                        "member": item.skill,
                        "technology": item.technology.label(),
                        "installed_slug": item.installed_slug,
                        "purpose": match item.purpose {
                            RecommendationPurpose::ReactPerformance => "react_performance",
                            RecommendationPurpose::Accessibility => "accessibility",
                            RecommendationPurpose::Testing => "testing",
                            RecommendationPurpose::Debugging => "debugging",
                        },
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>()
    ))
}

fn inventory(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let inventory = manager.inspect_project_skills_at(
        required(input, "project_id")?,
        required(input, "agent_key")?,
        scope(input)?,
    )?;
    Ok(json!(
        inventory
            .into_iter()
            .map(|skill| json!({
                "name": skill.name,
                "path": skill.path,
                "location": match skill.location {
                    SkillLocation::ProjectDeployment => "project_deployment",
                    SkillLocation::ProjectChanged => "project_changed",
                    SkillLocation::ProjectLocal => "project_local",
                    SkillLocation::Global => "global",
                },
            }))
            .collect::<Vec<_>>()
    ))
}

fn save_project_stack(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let project_id = required(input, "project_id")?;
    let scope = scope(input)?;
    let project = manager
        .list_projects()?
        .into_iter()
        .find(|project| project.id == project_id)
        .context("Project workspace not found")?;
    let skills: Vec<StackSkillRef> = serde_json::from_value(input["members"].clone())?;
    let name = if scope.as_os_str().is_empty() {
        project.name
    } else {
        // Scope paths are discovered by the same manager used for deployment.
        ensure!(
            manager
                .project_module_paths(project_id)?
                .iter()
                .any(|path| path == scope),
            "Project scope is no longer available"
        );
        format!("{} · {}", project.name, scope.display())
    };
    Ok(json!(
        manager.save_project_skill_stack(project_id, name, skills)?
    ))
}

fn library_row(skill: &Skill) -> Value {
    json!({ "id": skill.id, "name": skill.name, "slug": skill.slug, "description": skill.description })
}

fn find_local_stack(manager: &UnoSkillsManager, id: &str) -> Result<StackManifest> {
    manager
        .list_local_stacks()?
        .into_iter()
        .find(|stack| stack.id == id)
        .context("Local stack no longer exists")
}

fn find_stack(manager: &UnoSkillsManager, input: &Value) -> Result<StackManifest> {
    let id = required(input, "stack_id")?;
    let source: StackSource = serde_json::from_value(input["source"].clone())?;
    let stacks = match source {
        StackSource::Local => return find_local_stack(manager, id),
        StackSource::Featured => manager.featured_stacks()?,
        StackSource::Community => manager.community_stacks()?,
    };
    stacks
        .into_iter()
        .find(|stack| stack.id == id)
        .context("Stack no longer exists")
}

fn create_stack(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let name = required(input, "name")?.trim();
    ensure!(!name.is_empty(), "Enter a stack name");
    if let Some(slug) = sanitize_slug(name) {
        let id = slug.to_lowercase().replace(' ', "-");
        ensure!(
            !manager
                .list_local_stacks()?
                .iter()
                .any(|stack| stack.id == id),
            "A local stack with this name already exists"
        );
    }
    let ids: Vec<String> = serde_json::from_value(input["skill_ids"].clone())?;
    let library = manager.list_skills()?;
    ensure!(
        !ids.is_empty()
            && ids
                .iter()
                .all(|id| library.iter().any(|skill| &skill.id == id)),
        "Select skills that are still in the library"
    );
    let tags = serde_json::from_value(input["tags"].clone())?;
    Ok(json!(manager.create_stack_from_skills(
        name,
        required(input, "author")?,
        required(input, "description")?,
        tags,
        &ids,
    )?))
}

fn edit_stack(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let mut stack = find_local_stack(manager, required(input, "stack_id")?)?;
    ensure!(
        input["previous"] == json!(stack),
        "The local stack changed. Reopen the editor"
    );
    let name = required(input, "name")?.trim();
    ensure!(!name.is_empty(), "Enter a stack name");
    let remove: Vec<usize> = serde_json::from_value(input["remove_indices"].clone())?;
    ensure!(
        remove.iter().all(|index| *index < stack.skills.len()),
        "Stack membership changed while editing"
    );
    let ids: Vec<String> = serde_json::from_value(input["skill_ids"].clone())?;
    let skills = manager.list_skills()?;
    stack.skills = stack
        .skills
        .into_iter()
        .enumerate()
        .filter_map(|(index, member)| (!remove.contains(&index)).then_some(member))
        .collect();
    for id in ids {
        let skill = skills
            .iter()
            .find(|skill| skill.id == id)
            .context("Library skill no longer exists")?;
        if !stack.skills.iter().any(|member| {
            member
                .installed_in(&skills)
                .is_some_and(|existing| existing.id == id)
        }) {
            stack.skills.push(StackSkillRef {
                name: skill.name.clone(),
                source: skill
                    .source_ref
                    .clone()
                    .unwrap_or_else(|| skill.central_path.to_string_lossy().into_owned()),
                note: skill.description.clone().unwrap_or_default(),
            });
        }
    }
    ensure!(!stack.skills.is_empty(), "Select at least one skill");
    stack.name = name.to_owned();
    stack.author = required(input, "author")?.trim().to_owned();
    stack.description = required(input, "description")?.trim().to_owned();
    stack.tags = serde_json::from_value(input["tags"].clone())?;
    manager.save_local_stack(&stack)?;
    Ok(json!(stack))
}

fn import_stack(manager: &UnoSkillsManager, command: &str, input: &Value) -> Result<Value> {
    let raw = required(input, "toml")?;
    let stack = parse_stack_from_toml(raw)?;
    let previous = manager
        .list_local_stacks()?
        .into_iter()
        .find(|local| local.id == stack.id);
    let previous = previous
        .as_ref()
        .map(|stack| manager.export_stack_toml(stack))
        .transpose()?;
    if command == "stack.preview_import" {
        return Ok(json!({ "stack": stack, "previous": previous }));
    }
    ensure!(
        input.get("previous").is_some() && input["previous"] == json!(previous),
        "The local stack changed. Preview this import again"
    );
    Ok(json!(manager.import_stack_toml(raw)?))
}

fn install_stack(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let mut stack = find_stack(manager, input)?;
    if stack.source != StackSource::Local {
        stack.source = StackSource::Local;
        if let Some(local) = manager
            .list_local_stacks()?
            .into_iter()
            .find(|local| local.id == stack.id)
        {
            ensure!(
                json!(local) == json!(stack),
                "STACK_ID_CONFLICT: A customized local stack uses this ID"
            );
        }
    }
    Ok(json!(manager.install_stack(&stack, &[])?))
}

fn install_member(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let stack = find_stack(manager, input)?;
    let source = required(input, "member_source")?;
    let name = required(input, "member_name")?;
    let member = stack
        .skills
        .iter()
        .find(|member| member.source == source && member.name == name)
        .context("Stack member no longer exists")?;
    if let Some(existing) = member.installed_in(&manager.list_skills()?) {
        return Ok(library_row(existing));
    }
    Ok(library_row(&manager.install_from_ref(&member.source)?))
}

fn deploy(manager: &UnoSkillsManager, command: &str, input: &Value) -> Result<Value> {
    let stack = find_stack(manager, input)?;
    let project = required(input, "project_id")?;
    let agent = required(input, "agent_key")?;
    let scope = scope(input)?;
    let status = manager.stack_project_deploy_status_at(&stack, project, agent, scope)?;
    if command == "stack.preview_deploy" {
        return Ok(json!({
            "deployed": status.deployed,
            "global_available": status.global_available,
            "project_local": status.project_local,
            "pending": status.pending,
            "unavailable": status.unavailable,
            "conflicts": status.conflicts,
        }));
    }
    let include_global = input["include_global"].as_bool().unwrap_or_default();
    ensure!(
        status.pending_count(include_global) > 0,
        "No skills can be deployed to this scope"
    );
    let result = manager.deploy_stack_to_project_at_with_globals(
        &stack,
        project,
        agent,
        scope,
        include_global,
    )?;
    Ok(json!({
        "deployed": result.deployed,
        "already_deployed": result.already_deployed,
        "global_available": result.global_available,
        "project_local": result.project_local,
        "unavailable": result.unavailable,
        "failed": result.failed,
    }))
}
