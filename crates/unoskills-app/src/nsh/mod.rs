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
use chrono::{DateTime, Days, Local, NaiveDate};
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
        let open_manager = Arc::clone(&open_manager);
        pool.spawn(move || {
            let result = serde_json::from_value(call.data().clone())
                .map_err(anyhow::Error::from)
                .and_then(|request| dispatch(request, &updates, &skill_revisions, &*open_manager));
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
    match domain {
        "settings" => settings::dispatch(&manager, &command, input),
        "update" => updates.dispatch(&manager, &command),
        "home" => home(&manager, &command, input),
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

fn home(manager: &UnoSkillsManager, command: &str, input: Value) -> Result<Value> {
    match command {
        "home.summary" => {
            let skills = manager.list_skills()?;
            let mut recent = skills.iter().collect::<Vec<_>>();
            recent.sort_by_key(|skill| std::cmp::Reverse(skill.created_at));
            let mut projects = manager.list_projects()?;
            projects.sort_by_key(|project| std::cmp::Reverse(project.created_at));
            projects.truncate(2);
            let dates = skills.iter().filter_map(|skill| {
                DateTime::from_timestamp(skill.created_at, 0).map(|date| date.with_timezone(&Local))
            });
            let deployed = skills
                .iter()
                .filter(|skill| {
                    !skill.deployments.is_empty() || !skill.project_deployments.is_empty()
                })
                .count();
            let agents = manager
                .list_agents()?
                .iter()
                .filter(|agent| agent.installed)
                .count();
            let adoptable = manager.scan_adoptable()?.len();
            Ok(json!({
                "skill_count": skills.len(),
                "deployed_count": deployed,
                "agent_count": agents,
                "adoptable_count": adoptable,
                "recent": recent.iter().take(3).map(|skill| json!({
                    "id": skill.id,
                    "name": skill.name,
                    "date": DateTime::from_timestamp(skill.created_at, 0)
                        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string()),
                })).collect::<Vec<_>>(),
                "projects": projects,
                "activity": activity(Local::now().date_naive(), dates.map(|date| date.date_naive())),
            }))
        }
        "home.market" => {
            let board = input["board"].as_str().unwrap_or("trending");
            if !matches!(board, "trending" | "all-time") {
                bail!("INVALID_BOARD");
            }
            let page = manager.marketplace_leaderboard_page(board, 0)?;
            Ok(json!(page.items.iter().take(5).map(|skill| json!({
                "id": skill.id,
                "name": skill.name,
                "installs": skill.installs,
                "source": skill.install_ref(),
                "url": unoskills_core::source::skillssh_detail_url(skill).map(|url| url.to_string()),
            })).collect::<Vec<_>>()))
        }
        _ => bail!("UNKNOWN_COMMAND: {command}"),
    }
}

fn activity(today: NaiveDate, dates: impl Iterator<Item = NaiveDate>) -> Vec<Value> {
    let mut counts = [0; 7];
    for date in dates {
        if let Ok(index) = usize::try_from(today.signed_duration_since(date).num_days())
            && let Some(count) = counts.get_mut(index)
        {
            *count += 1;
        }
    }
    counts
        .into_iter()
        .enumerate()
        .rev()
        .filter_map(|(ago, count)| {
            today
                .checked_sub_days(Days::new(ago as u64))
                .map(|date| json!({"label": date.format("%m/%d").to_string(), "count": count}))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_counts_today_and_six_preceding_days() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
        let dates = [
            today + Days::new(1),
            today,
            today,
            today - Days::new(6),
            today - Days::new(7),
        ];
        let days = activity(today, dates.into_iter());
        assert_eq!(days[0], json!({"label": "12/28", "count": 1}));
        assert_eq!(days[6], json!({"label": "01/03", "count": 2}));
        assert_eq!(days.len(), 7);
    }
}
