use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::domain::{AdoptCandidate, AgentInfo, MarketplacePage, MarketplaceSkill, SourceType};
use crate::parser::{is_valid_skill_dir, parse_skill_dir};

#[derive(Debug, Clone)]
pub enum ResolvedSource {
    Local(PathBuf),
    Git {
        repo_url: String,
        branch: Option<String>,
        subpath: Option<String>,
        display_ref: String,
        source_type: SourceType,
    },
}

pub(crate) fn same_source(left: &str, right: &str) -> bool {
    let (left, right) = (left.trim(), right.trim());
    if left == right {
        return true;
    }
    match (resolve_source_ref(left), resolve_source_ref(right)) {
        (Ok(ResolvedSource::Local(left)), Ok(ResolvedSource::Local(right))) => {
            left == right
                || left
                    .canonicalize()
                    .ok()
                    .is_some_and(|left| right.canonicalize().is_ok_and(|right| left == right))
        }
        (
            Ok(ResolvedSource::Git {
                repo_url: left,
                branch: left_branch,
                subpath: left_path,
                ..
            }),
            Ok(ResolvedSource::Git {
                repo_url: right,
                branch: right_branch,
                subpath: right_path,
                ..
            }),
        ) => {
            left.trim_end_matches('/').trim_end_matches(".git")
                == right.trim_end_matches('/').trim_end_matches(".git")
                && left_branch == right_branch
                && left_path == right_path
        }
        _ => false,
    }
}

/// Deterministic ref parser:
/// 1. Starts with `./`, `../`, `/`, `~/` or Windows drive -> Local path
/// 2. Starts with `http://`, `https://`, `git@`, or ends with `.git` -> Git URL
/// 3. Matches `owner/repo@skill`, `owner/repo/skill`, or `owner/repo` -> GitHub / skills.sh shorthand
pub fn resolve_source_ref(input: &str) -> Result<ResolvedSource> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("Install source cannot be empty");
    }

    if trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.starts_with('/')
        || trimmed.starts_with("~/")
        || Path::new(trimmed).is_absolute()
        || (trimmed.len() >= 3 && trimmed.as_bytes()[1] == b':')
    {
        let expanded = if let Some(rest) = trimmed.strip_prefix("~/") {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(rest)
        } else {
            PathBuf::from(trimmed)
        };
        return Ok(ResolvedSource::Local(expanded));
    }

    if trimmed.starts_with("https://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("git@")
        || trimmed.ends_with(".git")
    {
        let (repo_url, branch, subpath) = parse_git_url(trimmed);
        return Ok(ResolvedSource::Git {
            repo_url,
            branch,
            subpath,
            display_ref: trimmed.to_string(),
            source_type: SourceType::Git,
        });
    }

    // Handle `owner/repo@skill`
    if let Some((repo_part, skill_name)) = trimmed.split_once('@') {
        let parts: Vec<&str> = repo_part.split('/').collect();
        if parts.len() == 2 && !skill_name.trim().is_empty() {
            return Ok(ResolvedSource::Git {
                repo_url: format!("https://github.com/{}/{}.git", parts[0], parts[1]),
                branch: None,
                subpath: Some(skill_name.trim().to_string()),
                display_ref: trimmed.to_string(),
                source_type: SourceType::SkillsSh,
            });
        }
    }

    // Handle `owner/repo` or `owner/repo/subpath...`
    let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() >= 2 && !trimmed.contains(' ') {
        let owner = segments[0];
        let repo = segments[1];
        let subpath = if segments.len() > 2 {
            Some(segments[2..].join("/"))
        } else {
            None
        };
        return Ok(ResolvedSource::Git {
            repo_url: format!("https://github.com/{owner}/{repo}.git"),
            branch: None,
            subpath,
            display_ref: trimmed.to_string(),
            source_type: SourceType::SkillsSh,
        });
    }

    bail!(
        "Unrecognized skill reference '{trimmed}'. Use a local path (./my-skill), Git URL, or owner/repo@skill."
    )
}

fn parse_git_url(url: &str) -> (String, Option<String>, Option<String>) {
    // Support GitHub `/tree/<branch>/<subpath>`
    let re = Regex::new(r"^(https?://github\.com/[^/]+/[^/]+)/tree/([^/]+)(?:/(.*))?$").unwrap();
    if let Some(caps) = re.captures(url) {
        let base = caps[1].trim_end_matches('/').to_string();
        let repo_url = if base.ends_with(".git") {
            base
        } else {
            format!("{base}.git")
        };
        let branch = Some(caps[2].to_string());
        let subpath = caps
            .get(3)
            .map(|m| m.as_str().trim_matches('/').to_string())
            .filter(|s| !s.is_empty());
        return (repo_url, branch, subpath);
    }
    (url.to_string(), None, None)
}

pub struct FetchedSkillDir {
    pub _temp_guard: Option<TempDir>,
    pub skill_dir: PathBuf,
    pub suggested_slug: Option<String>,
}

pub fn build_http_client(
    proxy_url: Option<&str>,
    timeout_secs: u64,
) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent(concat!("unoskills/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(timeout_secs));
    if let Some(proxy) = proxy_url.map(str::trim).filter(|s| !s.is_empty()) {
        let parsed = reqwest::Proxy::all(proxy)
            .with_context(|| format!("Invalid HTTP proxy URL: '{proxy}'"))?;
        builder = builder.proxy(parsed);
    }
    Ok(builder.build()?)
}

pub fn fetch_git_skill(
    repo_url: &str,
    branch: Option<&str>,
    subpath: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<FetchedSkillDir> {
    let temp = tempfile::Builder::new()
        .prefix("unoskills-git-")
        .tempdir()
        .context("Failed to create temp directory for git clone")?;

    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(proxy) = proxy_url.map(str::trim).filter(|s| !s.is_empty()) {
        cmd.arg("-c")
            .arg(format!("http.proxy={proxy}"))
            .arg("-c")
            .arg(format!("https.proxy={proxy}"));
    }
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(b) = branch {
        cmd.arg("--branch").arg(b);
    }
    cmd.arg(repo_url).arg(temp.path());

    let output = cmd
        .output()
        .context("Failed to execute `git` (ensure git is installed on PATH)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git clone failed: {}", stderr.trim());
    }

    let root = temp.path();
    if let Some(sub) = subpath {
        // 1. Direct path inside repo
        let direct = root.join(sub);
        if is_valid_skill_dir(&direct) {
            let slug = direct.file_name().map(|s| s.to_string_lossy().to_string());
            return Ok(FetchedSkillDir {
                _temp_guard: Some(temp),
                skill_dir: direct,
                suggested_slug: slug,
            });
        }
        // 2. Common conventions: skills/<name> or .claude/skills/<name>
        for prefix in ["skills", ".claude/skills", ".agents/skills"] {
            let candidate = root.join(prefix).join(sub);
            if is_valid_skill_dir(&candidate) {
                return Ok(FetchedSkillDir {
                    _temp_guard: Some(temp),
                    skill_dir: candidate,
                    suggested_slug: Some(sub.to_string()),
                });
            }
        }
        // 3. Walk repo to match directory name or frontmatter `name`
        for entry in WalkDir::new(root)
            .max_depth(5)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".git")
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() && is_valid_skill_dir(entry.path()) {
                let dir_matches = entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(sub);
                let meta = parse_skill_dir(entry.path());
                let name_matches = meta
                    .name
                    .as_deref()
                    .map(|n| n.eq_ignore_ascii_case(sub))
                    .unwrap_or(false);
                if dir_matches || name_matches {
                    return Ok(FetchedSkillDir {
                        _temp_guard: Some(temp),
                        skill_dir: entry.into_path(),
                        suggested_slug: Some(sub.to_string()),
                    });
                }
            }
        }
        bail!("Could not find skill '{sub}' (containing SKILL.md) inside {repo_url}");
    }

    if is_valid_skill_dir(root) {
        let repo_name = repo_url
            .rsplit('/')
            .next()
            .unwrap_or("skill")
            .trim_end_matches(".git")
            .to_string();
        let skill_dir = root.to_path_buf();
        return Ok(FetchedSkillDir {
            _temp_guard: Some(temp),
            skill_dir,
            suggested_slug: Some(repo_name),
        });
    }

    // Look for a single unambiguous skill directory in the repository
    let mut found = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() && is_valid_skill_dir(entry.path()) {
            found.push(entry.into_path());
        }
    }

    match found.len() {
        0 => bail!("No valid skill directory (containing SKILL.md) found in {repo_url}"),
        1 => {
            let dir = found.remove(0);
            let slug = dir.file_name().map(|s| s.to_string_lossy().to_string());
            Ok(FetchedSkillDir {
                _temp_guard: Some(temp),
                skill_dir: dir,
                suggested_slug: slug,
            })
        }
        n => {
            let names: Vec<String> = found
                .iter()
                .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
                .take(8)
                .collect();
            bail!(
                "Repository contains {n} skills ({}). Specify one via owner/repo@skill_name or /tree/branch/subpath.",
                names.join(", ")
            )
        }
    }
}

/// Scan installed agents' directories for unmanaged skills that can be adopted into UnoSkills.
pub fn scan_adoptable_skills(
    agents: &[AgentInfo],
    central_skills_dir: &Path,
    managed_target_paths: &HashSet<PathBuf>,
) -> Vec<AdoptCandidate> {
    let mut candidates = Vec::new();
    let mut seen_paths = HashSet::new();
    let central_canon = central_skills_dir.canonicalize().ok();

    for agent in agents.iter().filter(|a| a.installed) {
        let mut roots = vec![agent.skills_dir.clone()];
        for extra in &agent.additional_scan_dirs {
            if !roots.contains(extra) {
                roots.push(extra.clone());
            }
        }

        for root in roots {
            if !root.is_dir() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if managed_target_paths.contains(&path) {
                    continue;
                }
                if let (Some(c_root), Ok(p_canon)) = (&central_canon, path.canonicalize())
                    && p_canon.starts_with(c_root)
                {
                    continue;
                }
                if !is_valid_skill_dir(&path) {
                    continue;
                }
                let dedup_key = path.canonicalize().unwrap_or_else(|_| path.clone());
                if !seen_paths.insert(dedup_key) {
                    continue;
                }
                let meta = parse_skill_dir(&path);
                let fallback_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unnamed-skill".to_string());
                candidates.push(AdoptCandidate {
                    agent_key: agent.key.clone(),
                    agent_name: agent.display_name.clone(),
                    name: meta.name.unwrap_or(fallback_name),
                    description: meta.description,
                    path,
                    estimated_tokens: meta.estimated_tokens,
                });
            }
        }
    }

    candidates
}

/// Fetch one page from the native skills.sh JSON leaderboard API
/// (`GET /api/skills/{trending|hot|all-time}/{page}`), falling back to HTML
/// scraping on page 0 if the JSON API ever changes.
pub fn fetch_skillssh_leaderboard_page(
    board: &str,
    page: usize,
    proxy_url: Option<&str>,
) -> Result<MarketplacePage> {
    let api_board = match board {
        "trending" => "trending",
        "hot" => "hot",
        _ => "all-time",
    };
    let api_url = format!("https://skills.sh/api/skills/{api_board}/{page}");
    let client = build_http_client(proxy_url, 12)?;

    let api_attempt = (|| -> Result<MarketplacePage> {
        let resp = client.get(&api_url).send()?.error_for_status()?;
        let json: serde_json::Value = resp.json()?;
        let arr = json
            .get("skills")
            .and_then(|v| v.as_array())
            .context("skills.sh JSON response missing 'skills' array")?;
        let items = parse_skills_json_array(arr);
        let total = json
            .get("total")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let has_more = json
            .get("hasMore")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let reported_page = json
            .get("page")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(page);
        Ok(MarketplacePage {
            items,
            page: reported_page,
            total,
            has_more,
        })
    })();

    match api_attempt {
        Ok(result) if !result.items.is_empty() || page > 0 => Ok(result),
        Err(api_err) if page > 0 => Err(api_err),
        _ => {
            // Fallback to HTML scrape for page 0 if JSON endpoint is unavailable.
            let items = fetch_skillssh_leaderboard_html(board, &client)?;
            let total = Some(items.len());
            Ok(MarketplacePage {
                items,
                page: 0,
                total,
                has_more: false,
            })
        }
    }
}

/// Fetch skills.sh marketplace leaderboard (first page via JSON API, with HTML fallback).
pub fn fetch_skillssh_leaderboard(
    board: &str,
    proxy_url: Option<&str>,
) -> Result<Vec<MarketplaceSkill>> {
    Ok(fetch_skillssh_leaderboard_page(board, 0, proxy_url)?.items)
}

fn fetch_skillssh_leaderboard_html(
    board: &str,
    client: &reqwest::blocking::Client,
) -> Result<Vec<MarketplaceSkill>> {
    let url = match board {
        "trending" => "https://skills.sh/trending",
        "hot" => "https://skills.sh/hot",
        _ => "https://skills.sh/",
    };
    let html = client.get(url).send()?.error_for_status()?.text()?;
    parse_skillssh_html(&html)
}

pub fn search_skillssh(
    query: &str,
    limit: usize,
    proxy_url: Option<&str>,
) -> Result<Vec<MarketplaceSkill>> {
    let client = build_http_client(proxy_url, 12)?;
    let resp = client
        .get("https://skills.sh/api/search")
        .query(&[("q", query), ("limit", &limit.to_string())])
        .send()?;

    if resp.status().is_success()
        && let Ok(json) = resp.json::<serde_json::Value>()
    {
        if let Some(arr) = json.as_array() {
            return Ok(parse_skills_json_array(arr));
        }
        if let Some(arr) = json.get("skills").and_then(|v| v.as_array()) {
            return Ok(parse_skills_json_array(arr));
        }
    }

    // Fallback to filtering leaderboard if search API changes
    let all = fetch_skillssh_leaderboard("all", proxy_url)?;
    let q = query.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&q)
                || s.skill_id.to_lowercase().contains(&q)
                || s.source.to_lowercase().contains(&q)
        })
        .take(limit)
        .collect())
}

/// The public detail page for a skill from the skills.sh catalog.
pub fn skillssh_detail_url(skill: &MarketplaceSkill) -> Option<reqwest::Url> {
    let source = skill.source.split('/');
    if source.clone().count() > 2
        || source.clone().any(|part| matches!(part, "" | "." | ".."))
        || matches!(skill.skill_id.as_str(), "" | "." | "..")
    {
        return None;
    }
    let mut url = reqwest::Url::parse("https://www.skills.sh/").ok()?;
    url.path_segments_mut()
        .ok()?
        .pop_if_empty()
        .extend(source.chain(std::iter::once(skill.skill_id.as_str())));
    Some(url)
}

fn normalize_market_description(description: &str) -> Option<String> {
    let normalized = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    if let Some((end, _)) = normalized.char_indices().nth(400) {
        return Some(format!("{}…", normalized[..end].trim_end()));
    }
    Some(normalized)
}

fn parse_skillssh_html(html: &str) -> Result<Vec<MarketplaceSkill>> {
    let marker = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    if let Some(start_idx) = html.find(marker) {
        let start = start_idx + marker.len();
        if let Some(end_rel) = html[start..].find("</script>") {
            let json_str = &html[start..start + end_rel];
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str)
                && let Some(arr) = data
                    .pointer("/props/pageProps/initialSkills")
                    .or_else(|| data.pointer("/props/pageProps/skills"))
                    .or_else(|| data.pointer("/props/pageProps/items"))
                    .and_then(|v| v.as_array())
            {
                let parsed = parse_skills_json_array(arr);
                if !parsed.is_empty() {
                    return Ok(parsed);
                }
            }
        }
    }

    // Flexible RSC / embedded JSON regex allowing extra fields between keys
    let re = Regex::new(
        r#"(?:\\)?\"source(?:\\)?\":(?:\\)?\"(?P<source>[^"\\]+)(?:\\)?\",(?:[^{}]|\\.)*?(?:(?:\\)?\"skillId(?:\\)?\"|(?:\\)?\"skill_id(?:\\)?\"):(?:\\)?\"(?P<skill_id>[^"\\]+)(?:\\)?\",(?:[^{}]|\\.)*?(?:\\)?\"name(?:\\)?\":(?:\\)?\"(?P<name>[^"\\]*)(?:\\)?\",(?:[^{}]|\\.)*?(?:\\)?\"installs(?:\\)?\":(?P<installs>\d+)"#,
    )?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for caps in re.captures_iter(html) {
        let source = caps["source"].to_string();
        let skill_id = caps["skill_id"].to_string();
        let key = format!("{source}/{skill_id}");
        if seen.insert(key.clone()) {
            let raw_name = caps["name"].to_string();
            let name = if raw_name.is_empty() {
                skill_id.clone()
            } else {
                raw_name
            };
            out.push(MarketplaceSkill {
                id: key,
                skill_id,
                name,
                source,
                installs: caps["installs"].parse().unwrap_or(0),
                description: None,
                is_official: false,
            });
        }
    }
    Ok(out)
}

fn parse_skills_json_array(arr: &[serde_json::Value]) -> Vec<MarketplaceSkill> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in arr {
        let source = item
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let skill_id = item
            .get("skillId")
            .or_else(|| item.get("skill_id"))
            .or_else(|| item.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&skill_id)
            .to_string();
        let installs = item.get("installs").and_then(|v| v.as_u64()).unwrap_or(0);
        if skill_id.is_empty() {
            continue;
        }
        let key = format!("{source}/{skill_id}");
        if seen.insert(key.clone()) {
            out.push(MarketplaceSkill {
                id: key,
                skill_id,
                name,
                source,
                installs,
                description: item["description"]
                    .as_str()
                    .and_then(normalize_market_description),
                is_official: item["isOfficial"].as_bool().unwrap_or(false),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skillssh_detail_url_keeps_catalog_identity() {
        let items = parse_skills_json_array(&[
            serde_json::json!({"source":"owner/repo", "skillId":"read docs", "isOfficial":true}),
            serde_json::json!({"source":"owner/..", "skillId":"bad"}),
        ]);
        let url = skillssh_detail_url(&items[0]).unwrap();
        assert_eq!(url.as_str(), "https://www.skills.sh/owner/repo/read%20docs");
        assert!(items[0].is_official);
        assert!(skillssh_detail_url(&items[1]).is_none());
    }

    #[test]
    fn resolves_all_source_ref_forms() {
        assert!(matches!(
            resolve_source_ref("./my-skill").unwrap(),
            ResolvedSource::Local(_)
        ));
        match resolve_source_ref("vercel-labs/agent-skills@react-best-practices").unwrap() {
            ResolvedSource::Git {
                repo_url, subpath, ..
            } => {
                assert_eq!(repo_url, "https://github.com/vercel-labs/agent-skills.git");
                assert_eq!(subpath.as_deref(), Some("react-best-practices"));
            }
            _ => panic!("expected git"),
        }
        match resolve_source_ref("https://github.com/foo/bar/tree/main/skills/baz").unwrap() {
            ResolvedSource::Git {
                repo_url,
                branch,
                subpath,
                ..
            } => {
                assert_eq!(repo_url, "https://github.com/foo/bar.git");
                assert_eq!(branch.as_deref(), Some("main"));
                assert_eq!(subpath.as_deref(), Some("skills/baz"));
            }
            _ => panic!("expected git"),
        }
    }
}
