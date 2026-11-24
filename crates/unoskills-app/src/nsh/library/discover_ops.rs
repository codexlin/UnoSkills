use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use unoskills_core::sync::RemovalRollback;
use unoskills_core::{
    AdoptCandidate, AdoptionMergeConflict, AdoptionMergeRequest, MarketplaceSkill,
    UnoSkillsManager, source::skillssh_detail_url,
};

use super::{optional_usize, required_str};

const DEFAULT_SEARCH_LIMIT: usize = 60;

pub(super) fn dispatch(manager: &UnoSkillsManager, sub: &str, input: Value) -> Result<Value> {
    match sub {
        "install" => install_source(manager, &input),
        "leaderboard" => leaderboard(manager, &input),
        "search" => search_marketplace(manager, &input),
        "scan_adoptable" => scan_adoptable(manager),
        "adopt" => adopt_candidates(manager, &input),
        "merge_adoption" => merge_adoption(manager, input),
        _ => bail!("Unknown discover host command: 'discover.{sub}'"),
    }
}

fn install_source(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let source_ref = required_str(input, "source_ref")?.trim();
    if source_ref.is_empty() {
        bail!("Field 'source_ref' must not be empty");
    }
    let skill = manager.install_from_ref(source_ref)?;
    Ok(serde_json::to_value(skill)?)
}

fn leaderboard(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let board = required_str(input, "board")?.trim();
    match board {
        "trending" | "hot" | "all-time" => {}
        other => {
            bail!("Invalid marketplace board '{other}'; expected 'trending', 'hot', or 'all-time'")
        }
    }
    let page = optional_usize(input, "page", 0)?;
    let data = manager.marketplace_leaderboard_page(board, page)?;
    let items: Vec<Value> = data.items.iter().map(serialize_marketplace_skill).collect();
    Ok(json!({
        "board": board,
        "page": data.page,
        "total": data.total,
        "has_more": data.has_more,
        "items": items,
    }))
}

fn search_marketplace(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let query = required_str(input, "query")?.trim();
    if query.is_empty() {
        bail!("Field 'query' must not be empty");
    }
    let limit = optional_usize(input, "limit", DEFAULT_SEARCH_LIMIT)?;
    let results = manager.marketplace_search(query, limit)?;
    let items: Vec<Value> = results.iter().map(serialize_marketplace_skill).collect();
    Ok(json!({
        "query": query,
        "items": items,
    }))
}

fn serialize_marketplace_skill(item: &MarketplaceSkill) -> Value {
    json!({
        "id": item.id,
        "skill_id": item.skill_id,
        "name": item.name,
        "source": item.source,
        "installs": item.installs,
        "description": item.description,
        "is_official": item.is_official,
        "install_ref": item.install_ref(),
        "detail_url": skillssh_detail_url(item).map(|url| url.to_string()),
    })
}

fn scan_adoptable(manager: &UnoSkillsManager) -> Result<Value> {
    let candidates = manager.scan_adoptable()?;
    Ok(json!({
        "candidates": candidates,
    }))
}

fn adopt_candidates(manager: &UnoSkillsManager, input: &Value) -> Result<Value> {
    let candidates = resolve_adopt_candidates(manager, input)?;
    if candidates.is_empty() {
        bail!("Select at least one local skill candidate to adopt");
    }

    let mut adopted = Vec::new();
    let mut failed = Vec::new();
    for candidate in &candidates {
        match manager.adopt_candidate(candidate) {
            Ok(skill) => adopted.push(skill),
            Err(error) => {
                let review = error.is::<AdoptionMergeConflict>();
                failed.push(adoption_failure(manager, candidate, error, review));
            }
        }
    }

    Ok(json!({
        "adopted": adopted,
        "adopted_count": adopted.len(),
        "failed": failed,
    }))
}

fn merge_adoption(manager: &UnoSkillsManager, input: Value) -> Result<Value> {
    #[derive(serde::Deserialize)]
    struct Input {
        requests: Vec<AdoptionMergeRequest>,
    }
    let Input { requests } =
        serde_json::from_value(input).context("Missing adoption merge requests")?;
    let mut merged = Vec::new();
    let mut failed = Vec::new();
    for request in requests {
        let candidate = request.candidate.clone();
        match manager.apply_adoption_merge(request) {
            Ok(result) => merged.push(result),
            Err(error) => failed.push(adoption_failure(manager, &candidate, error, true)),
        }
    }
    Ok(json!({ "merged": merged, "failed": failed }))
}

fn adoption_failure(
    manager: &UnoSkillsManager,
    candidate: &AdoptCandidate,
    error: anyhow::Error,
    review: bool,
) -> Value {
    let mut failure = json!({
        "name": candidate.name,
        "path": candidate.path,
        "error": format!("{error:#}"),
    });
    if let Some(rollback) = error.downcast_ref::<RemovalRollback>() {
        failure["recovery_paths"] = json!(rollback.paths);
        return failure;
    }
    if review {
        match manager.preview_adoption_merge(candidate) {
            Ok(preview) => failure["merge"] = json!(preview),
            Err(error) => failure["merge_issue"] = json!(format!("{error:#}")),
        }
    }
    failure
}

fn resolve_adopt_candidates(
    manager: &UnoSkillsManager,
    input: &Value,
) -> Result<Vec<AdoptCandidate>> {
    if let Some(raw_candidates) = input.get("candidates") {
        let parsed: Vec<AdoptCandidate> = serde_json::from_value(raw_candidates.clone())
            .context("Field 'candidates' must be an array of AdoptCandidate objects")?;
        return Ok(parsed);
    }

    if let Some(raw_paths) = input.get("paths") {
        let path_values = raw_paths
            .as_array()
            .context("Field 'paths' must be an array of path strings")?;
        let mut requested = HashSet::new();
        for entry in path_values {
            let path = entry
                .as_str()
                .context("Every entry in 'paths' must be a string")?;
            requested.insert(PathBuf::from(path));
        }
        let scanned = manager.scan_adoptable()?;
        return Ok(scanned
            .into_iter()
            .filter(|candidate| requested.contains(&candidate.path))
            .collect());
    }

    bail!("Missing required field 'candidates' or 'paths'")
}
