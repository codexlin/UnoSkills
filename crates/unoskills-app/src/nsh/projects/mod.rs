use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use unoskills_core::project_analysis::{RecommendationPurpose, SkillLocation};
use unoskills_core::stacks::parse_stack_from_toml;
use unoskills_core::{
    PreparedProjectVersion, ProjectDeployPreview, ProjectVersionIntent, Skill, StackManifest,
    StackSkillRef, StackSource, UnoSkillsManager,
};

// Host requests run on multiple workers. Keep stale-preview checks and their
// writes ordered with other mutations initiated from these screens.
static MUTATIONS: Mutex<()> = Mutex::new(());

#[derive(Default)]
pub(super) struct VersionPreviews {
    pending: Mutex<Vec<PreparedProjectVersion>>,
}

#[derive(Deserialize)]
struct VersionTarget {
    project_id: String,
    scope_path: PathBuf,
    agent_key: String,
    skill_id: String,
}

#[derive(Deserialize)]
struct VersionRequest {
    #[serde(flatten)]
    target: VersionTarget,
    intent: ProjectVersionIntent,
}

impl VersionPreviews {
    pub(super) fn dispatch(
        &self,
        manager: &UnoSkillsManager,
        command: &str,
        input: &Value,
    ) -> Option<Result<Value>> {
        Some(match command {
            "project.lock_skill" => (|| {
                let target: VersionTarget = serde_json::from_value(input.clone())?;
                manager.lock_project_skill(
                    &target.project_id,
                    &target.scope_path,
                    &target.agent_key,
                    &target.skill_id,
                )?;
                Ok(Value::Null)
            })(),
            "project.prepare_version" => (|| {
                let request: VersionRequest = serde_json::from_value(input.clone())?;
                let target = request.target;
                let prepared = manager.prepare_project_version(
                    &target.project_id,
                    &target.scope_path,
                    &target.agent_key,
                    &target.skill_id,
                    request.intent,
                )?;
                let result = serde_json::to_value(prepared.review())?;
                let mut pending = self.pending.lock().unwrap();
                if pending.len() == 8 {
                    pending.remove(0);
                }
                pending.push(prepared);
                Ok(result)
            })(),
            "project.apply_version" | "project.cancel_version" => (|| {
                let id = required(input, "preview_id")?;
                let prepared = {
                    let mut pending = self.pending.lock().unwrap();
                    pending
                        .iter()
                        .position(|item| item.review().id == id)
                        .map(|index| pending.remove(index))
                };
                if command == "project.apply_version" {
                    manager
                        .apply_project_version(prepared.context(
                            "STALE_PROJECT_VERSION: Review the project version again",
                        )?)?;
                }
                Ok(Value::Null)
            })(),
            _ => return None,
        })
    }
}

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
            Ok(json!(
                stack
                    .skills
                    .iter()
                    .map(|member| {
                        let installed = member.installed_in(&skills);
                        let source_conflict =
                            installed.is_some_and(|skill| member.has_source_conflict(skill));
                        json!({
                            "member": member,
                            "installed": installed.filter(|_| !source_conflict).map(library_row),
                            "source_conflict": source_conflict,
                        })
                    })
                    .collect::<Vec<_>>()
            ))
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
                    "skill_id": skill.skill_id,
                    "locked_hash": skill.locked_hash,
                    "library_different": skill.library_different,
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
        .find_local_stack(id)?
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
    let previous = stack.clone();
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
                .deployable_in(&skills)
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
    manager.save_local_stack_if_unchanged(&stack, Some(&previous))?;
    Ok(json!(stack))
}

fn import_stack(manager: &UnoSkillsManager, command: &str, input: &Value) -> Result<Value> {
    let raw = required(input, "toml")?;
    let mut stack = parse_stack_from_toml(raw)?;
    stack.source = StackSource::Local;
    if command == "stack.preview_import" {
        let previous = manager
            .find_local_stack(&stack.id)?
            .as_ref()
            .map(|stack| manager.export_stack_toml(stack))
            .transpose()?;
        return Ok(json!({ "stack": stack, "previous": previous }));
    }
    let previous: Option<String> = serde_json::from_value(
        input
            .get("previous")
            .context("Preview this import before confirming")?
            .clone(),
    )?;
    let previous = previous.as_deref().map(parse_stack_from_toml).transpose()?;
    manager.save_local_stack_if_unchanged(&stack, previous.as_ref())?;
    Ok(json!(stack))
}

fn install_stack(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let stack = find_stack(manager, input)?;
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
    if let Some(existing) = member.deployable_in(&manager.list_skills()?) {
        return Ok(library_row(existing));
    }
    Ok(library_row(&manager.install_from_ref(&member.source)?))
}

fn deploy(manager: &UnoSkillsManager, command: &str, input: &Value) -> Result<Value> {
    let stack = find_stack(manager, input)?;
    let project = required(input, "project_id")?;
    let agent = required(input, "agent_key")?;
    let scope = scope(input)?;
    if command == "stack.preview_deploy" {
        return Ok(json!(
            manager.preview_project_stack(&stack, project, agent, scope)?
        ));
    }
    let include_global = input["include_global"].as_bool().unwrap_or_default();
    let reviewed: ProjectDeployPreview = serde_json::from_value(input["reviewed"].clone())
        .context("Missing reviewed deployment preview")?;
    Ok(json!(manager.apply_project_stack_preview(
        &stack,
        project,
        agent,
        scope,
        reviewed,
        include_global,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_and_evicting_project_reviews_drops_their_staging() {
        let dir = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(dir.path().join("library")).unwrap();
        let source = dir.path().join("reviewer");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Reviewer\nFirst version").unwrap();
        let skill = manager.install_from_ref(source.to_str().unwrap()).unwrap();
        let workspace = dir.path().join("project");
        fs::create_dir(&workspace).unwrap();
        let project = manager.add_project(&workspace, None).unwrap();
        manager
            .set_agent_override("codex", dir.path().join("codex").to_str())
            .unwrap();
        manager
            .deploy_skill_to_project(
                &skill.id,
                &project.id,
                "codex",
                Some(unoskills_core::DeployMode::Copy),
                false,
            )
            .unwrap();
        manager
            .lock_project_skill(&project.id, Path::new(""), "codex", &skill.id)
            .unwrap();
        manager
            .save_skill_markdown(&skill.id, "# Reviewer\nNew version")
            .unwrap();

        let staging_count = || {
            fs::read_dir(manager.root_dir())
                .unwrap()
                .filter(|entry| {
                    entry
                        .as_ref()
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".project-version-review-")
                })
                .count()
        };
        let durable = manager.root_dir().join(".project-versions");
        let previews = VersionPreviews::default();
        let request = json!({ "project_id": project.id, "scope_path": "", "agent_key": "codex", "skill_id": skill.id, "intent": "upgrade" });
        let first = previews
            .dispatch(&manager, "project.prepare_version", &request)
            .unwrap()
            .unwrap();
        for _ in 0..8 {
            previews
                .dispatch(&manager, "project.prepare_version", &request)
                .unwrap()
                .unwrap();
        }
        assert_eq!(staging_count(), 8);
        assert_eq!(fs::read_dir(&durable).unwrap().count(), 1);
        let expired = json!({ "preview_id": first["id"] });
        assert!(
            previews
                .dispatch(&manager, "project.apply_version", &expired)
                .unwrap()
                .is_err()
        );
        let review = previews
            .dispatch(&manager, "project.prepare_version", &request)
            .unwrap()
            .unwrap();
        previews
            .dispatch(
                &manager,
                "project.cancel_version",
                &json!({ "preview_id": review["id"] }),
            )
            .unwrap()
            .unwrap();
        assert_eq!(staging_count(), 7);
        drop(previews);
        assert_eq!(staging_count(), 0);
        assert_eq!(fs::read_dir(durable).unwrap().count(), 1);
    }

    #[test]
    fn stack_import_confirm_requires_the_same_manifest_seen_at_preview() {
        let dir = tempfile::tempdir().unwrap();
        let manager = UnoSkillsManager::open_at(dir.path().join("library")).unwrap();
        let older = "id = 'saved'\nname = 'Original'\n";
        let incoming = "id = 'saved'\nname = 'Incoming'\n";
        let saved = manager.import_stack_toml(older).unwrap();
        let request = json!({ "toml": incoming });
        let preview = dispatch(&manager, "stack.preview_import", request.clone()).unwrap();
        let confirm = json!({ "toml": incoming, "previous": preview["previous"] });

        let mut edited = saved;
        edited.name = "Concurrent edit".into();
        manager.save_local_stack(&edited).unwrap();
        let error = dispatch(&manager, "stack.import", confirm).unwrap_err();
        assert!(error.to_string().contains("STACK_ID_CONFLICT"));
        assert_eq!(manager.find_local_stack("saved").unwrap(), Some(edited));

        let preview = dispatch(&manager, "stack.preview_import", request).unwrap();
        let confirm = json!({ "toml": incoming, "previous": preview["previous"] });
        let committed = dispatch(&manager, "stack.import", confirm).unwrap();
        assert_eq!(committed["name"], "Incoming");
    }
}
