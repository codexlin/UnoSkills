mod dialogs;
mod fonts;
mod library;
mod projects;
mod settings;
mod update;
#[cfg(target_os = "linux")]
mod window_colors;

use std::{collections::BTreeSet, sync::Arc};

use anyhow::{Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, Local};
use nsh_engine::HostChannel;
use serde::Deserialize;
use serde_json::{Value, json};
use unoskills_core::UnoSkillsManager;

#[derive(Deserialize)]
struct Request {
    command: String,
    input: Value,
}

pub fn channel() -> Result<HostChannel> {
    channel_with_fonts(UnoSkillsManager::open_default, true)
}

pub(super) fn ui_sets() -> Vec<nsh_core::element::Registry> {
    vec![nsh_base::registry(), dialogs::registry()]
}

#[cfg(test)]
pub(super) fn channel_for(
    open_manager: impl Fn() -> Result<UnoSkillsManager> + Send + Sync + 'static,
) -> Result<HostChannel> {
    channel_with_fonts(open_manager, false)
}

fn channel_with_fonts(
    open_manager: impl Fn() -> Result<UnoSkillsManager> + Send + Sync + 'static,
    bundled_fonts: bool,
) -> Result<HostChannel> {
    let open_manager = Arc::new(open_manager);
    let host = HostChannel::new();
    let mark = STANDARD.encode(include_bytes!("../../assets/icons/uno-mark.svg"));
    let params = serde_json::Map::from_iter([
        (
            "brand_mark".to_owned(),
            json!(format!("data:image/svg+xml;base64,{mark}")),
        ),
        ("bundled_fonts".to_owned(), json!(bundled_fonts)),
    ]);
    #[cfg(target_os = "linux")]
    let params = {
        let mut params = params;
        params.insert(
            "native_window_controls_inset".to_owned(),
            json!(window_colors::CONTROLS_INSET),
        );
        params
    };
    host.set_params(params);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .thread_name(|worker| format!("unoskills-host-{worker}"))
        .build()?;
    let updates = Arc::new(update::Updates::default());
    let skill_revisions = Arc::new(library::RevisionPreviews::default());
    let project_versions = Arc::new(projects::VersionPreviews::default());
    host.handle("unoskills.command", move |call| {
        #[cfg(target_os = "linux")]
        if call.data()["command"] == "window.titlebar_colors" {
            if let Ok(colors) = serde_json::from_value(call.data()["input"].clone()) {
                window_colors::apply(colors);
            }
            call.resolve(json!({"ok": true, "value": null}));
            return;
        }
        let updates = Arc::clone(&updates);
        let skill_revisions = Arc::clone(&skill_revisions);
        let project_versions = Arc::clone(&project_versions);
        let open_manager = Arc::clone(&open_manager);
        pool.spawn(move || {
            let result = serde_json::from_value(call.data().clone())
                .map_err(anyhow::Error::from)
                .and_then(|request| {
                    dispatch(
                        request,
                        &updates,
                        &skill_revisions,
                        &project_versions,
                        &*open_manager,
                    )
                });
            let response = match result {
                Ok(value) => json!({"ok": true, "value": value}),
                Err(error) => {
                    let message = format!("{error:#}");
                    json!({"ok": false, "message": message, "code": error_code(&message)})
                }
            };
            call.resolve(response);
        });
    });
    Ok(host)
}

fn dispatch(
    request: Request,
    updates: &update::Updates,
    skill_revisions: &library::RevisionPreviews,
    project_versions: &projects::VersionPreviews,
    open_manager: impl FnOnce() -> Result<UnoSkillsManager>,
) -> Result<Value> {
    let Request { command, input } = request;
    let (domain, _) = command.split_once('.').unwrap_or_default();
    if command == "settings.fonts" {
        return fonts();
    }
    if command == "update.status" {
        return updates.status();
    }
    let manager = open_manager()?;
    if let Some(result) = skill_revisions.dispatch(&manager, &command, &input) {
        return result;
    }
    if let Some(result) = project_versions.dispatch(&manager, &command, &input) {
        return result;
    }
    match domain {
        "settings" => settings::dispatch(&manager, &command, input),
        "update" => updates.dispatch(&manager, &command),
        "home" => home(&manager, &command),
        "library" | "discover" => library::dispatch(&manager, &command, input),
        "project" | "stack" => projects::dispatch(&manager, &command, input),
        _ => bail!("UNKNOWN_COMMAND: {command}"),
    }
}

fn error_code(error: &str) -> &str {
    error
        .split(|ch: char| !(ch.is_ascii_uppercase() || ch == '_'))
        .find(|code| code.contains('_') && code.chars().any(|ch| ch.is_ascii_uppercase()))
        .unwrap_or("HOST_ERROR")
}

fn fonts() -> Result<Value> {
    let mut system = fontdb::Database::new();
    system.load_system_fonts();
    let families: BTreeSet<&str> = system
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _)| name.as_str()))
        .filter(|name| !name.is_empty() && !name.starts_with('.'))
        .collect();
    Ok(serde_json::to_value(families)?)
}

fn home(manager: &UnoSkillsManager, command: &str) -> Result<Value> {
    match command {
        "home.summary" => {
            let skills = manager.list_skills()?;
            let mut recent = skills.iter().collect::<Vec<_>>();
            recent.sort_by_key(|skill| std::cmp::Reverse(skill.created_at));
            let mut projects = manager.list_projects()?;
            let project_count = projects.len();
            projects.sort_by_key(|project| std::cmp::Reverse(project.created_at));
            projects.truncate(2);
            let agents = manager.list_agents()?;
            let detected = agents.iter().filter(|agent| agent.installed);
            let deployed = skills
                .iter()
                .filter(|skill| {
                    !skill.deployments.is_empty() || !skill.project_deployments.is_empty()
                })
                .count();
            let adoptable = manager.scan_adoptable()?.len();
            Ok(json!({
                "skill_count": skills.len(),
                "deployed_count": deployed,
                "project_count": project_count,
                "agent_count": detected.clone().count(),
                "enabled_agent_count": detected.filter(|agent| agent.enabled).count(),
                "adoptable_count": adoptable,
                "recent": recent.iter().take(3).map(|skill| json!({
                    "id": skill.id,
                    "name": skill.name,
                    "date": DateTime::from_timestamp(skill.created_at, 0)
                        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string()),
                })).collect::<Vec<_>>(),
                "projects": projects,
            }))
        }
        "home.market" => {
            let page = manager.marketplace_leaderboard_page("trending", 0)?;
            Ok(json!(
                page.items
                    .iter()
                    .take(6)
                    .map(|skill| json!({
                        "id": skill.id,
                        "name": skill.name,
                        "source": skill.source,
                        "installs": skill.installs,
                        "url": unoskills_core::source::skillssh_detail_url(skill).map(|url| url.to_string()),
                    }))
                    .collect::<Vec<_>>()
            ))
        }
        _ => bail!("UNKNOWN_COMMAND: {command}"),
    }
}
