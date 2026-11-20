use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::{StackManifest, StackSource};
use crate::source::build_http_client;

const FEATURED_STACKS_TOML: &str = include_str!("../data/featured_stacks.toml");

/// How long the community-index HTTP request may take before giving up.
const COMMUNITY_INDEX_TIMEOUT_SECS: u64 = 10;
/// GitHub Device Flow endpoints (used only when an OAuth client id is set).
const GH_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GH_DEVICE_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GIST_API_URL: &str = "https://api.github.com/gists";

#[derive(Debug, Deserialize)]
struct FeaturedStacksFile {
    stacks: Vec<StackManifest>,
}

/// Wrapper for single-file `.unoskills-stack.toml` sharing / Gist / community PRs.
#[derive(Debug, Serialize, Deserialize)]
struct SingleStackFile {
    stack: StackManifest,
}

/// Result of a conditional community-index fetch (ETag aware).
#[derive(Debug)]
pub struct CommunityFetch {
    /// Parsed remote stacks (empty when `not_modified`).
    pub stacks: Vec<StackManifest>,
    /// New ETag reported by the server, if any.
    pub etag: Option<String>,
    /// True when the server answered `304 Not Modified` for the stored ETag.
    pub not_modified: bool,
}

/// Abstraction over community Stack sources so UnoSkills can evolve cleanly from
/// Stage 1 (built-in + GitHub static `index.json`) to Stage 2 (Cloudflare Workers / Supabase Hub)
/// without changing UI or core orchestration code.
pub trait StackRegistry {
    /// Built-in stacks shipped inside the app binary.
    fn fetch_featured(&self) -> Result<Vec<StackManifest>>;
    /// Remote community index with `If-None-Match` conditional fetching.
    fn fetch_community(&self, etag: Option<&str>) -> Result<CommunityFetch>;
}

#[derive(Default)]
pub struct DefaultStackRegistry {
    pub remote_index_url: Option<String>,
    /// Optional HTTP/SOCKS proxy applied to the remote index fetch.
    pub proxy_url: Option<String>,
}

impl StackRegistry for DefaultStackRegistry {
    fn fetch_featured(&self) -> Result<Vec<StackManifest>> {
        load_builtin_featured_stacks()
    }

    fn fetch_community(&self, etag: Option<&str>) -> Result<CommunityFetch> {
        let url = self
            .remote_index_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .context("No community stacks index URL configured")?;
        let client = build_http_client(self.proxy_url.as_deref(), COMMUNITY_INDEX_TIMEOUT_SECS)?;
        let mut request = client.get(url);
        if let Some(etag) = etag.map(str::trim).filter(|s| !s.is_empty()) {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send()?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(CommunityFetch {
                stacks: Vec::new(),
                etag: etag.map(str::to_string),
                not_modified: true,
            });
        }
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let stacks: Vec<StackManifest> = response.error_for_status()?.json()?;
        let mut stacks = stacks;
        for stack in &mut stacks {
            stack.source = StackSource::Community;
        }
        Ok(CommunityFetch {
            stacks,
            etag,
            not_modified: false,
        })
    }
}

pub fn load_builtin_featured_stacks() -> Result<Vec<StackManifest>> {
    let mut parsed: FeaturedStacksFile =
        toml::from_str(FEATURED_STACKS_TOML).context("Failed to parse built-in featured stacks")?;
    for stack in &mut parsed.stacks {
        stack.source = StackSource::Featured;
    }
    Ok(parsed.stacks)
}

/// Parse a community `index.json` body (a JSON array of stack manifests).
pub fn parse_community_index(body: &str) -> Result<Vec<StackManifest>> {
    let mut stacks: Vec<StackManifest> =
        serde_json::from_str(body).context("Invalid community stacks index.json")?;
    for stack in &mut stacks {
        stack.source = StackSource::Community;
    }
    Ok(stacks)
}

pub fn export_stack_to_toml(stack: &StackManifest) -> Result<String> {
    let wrapper = SingleStackFile {
        stack: stack.clone(),
    };
    toml::to_string_pretty(&wrapper).context("Failed to serialize stack manifest to TOML")
}

pub fn parse_stack_from_toml(toml_str: &str) -> Result<StackManifest> {
    if let Ok(wrapped) = toml::from_str::<SingleStackFile>(toml_str) {
        return Ok(wrapped.stack);
    }
    toml::from_str::<StackManifest>(toml_str).context("Invalid UnoSkills stack TOML manifest")
}

// ── GitHub Gist sharing (PAT or Device Flow token) ──

#[derive(Serialize)]
struct GistFilePayload {
    content: String,
}

#[derive(Serialize)]
struct GistPayload {
    description: String,
    #[serde(rename = "public")]
    public: bool,
    files: std::collections::BTreeMap<String, GistFilePayload>,
}

#[derive(Deserialize)]
struct GistResponse {
    html_url: String,
}

/// Publish a stack manifest as a secret GitHub Gist and return its URL.
/// `token` is a GitHub PAT or an OAuth token with the `gist` scope.
pub fn share_stack_to_gist(
    token: &str,
    proxy_url: Option<&str>,
    stack: &StackManifest,
) -> Result<String> {
    let toml_body = export_stack_to_toml(stack)?;
    let mut files = std::collections::BTreeMap::new();
    files.insert(
        format!("{}.unoskills-stack.toml", stack.id),
        GistFilePayload { content: toml_body },
    );
    let payload = GistPayload {
        description: format!("UnoSkills Stack: {} — {}", stack.name, stack.id),
        public: false,
        files,
    };
    let client = build_http_client(proxy_url, 15)?;
    let response = client
        .post(GIST_API_URL)
        .bearer_auth(token)
        .json(&payload)
        .send()?
        .error_for_status()
        .with_context(|| "GitHub rejected the Gist request (check token 'gist' scope)")?;
    let gist: GistResponse = response.json()?;
    Ok(gist.html_url)
}

/// Step 1 of the GitHub OAuth Device Flow: request a user code the user must
/// confirm at `verification_uri`. The returned interval is the polling period.
#[derive(Debug)]
pub struct DeviceCodeRequest {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    /// Suggested polling interval in seconds.
    pub interval: u64,
    /// Seconds before the device code expires.
    pub expires_in: u64,
}

pub fn request_github_device_code(
    client_id: &str,
    proxy_url: Option<&str>,
) -> Result<DeviceCodeRequest> {
    let client = build_http_client(proxy_url, 15)?;
    let response = client
        .post(GH_DEVICE_CODE_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[("client_id", client_id), ("scope", "gist")])
        .send()?
        .error_for_status()
        .context("GitHub rejected the device-code request")?;
    #[derive(Deserialize)]
    struct CodeResponse {
        device_code: String,
        user_code: String,
        verification_uri: String,
        #[serde(default = "default_expires")]
        expires_in: u64,
    }
    #[derive(Deserialize)]
    struct JsonInterval {
        #[serde(default = "default_interval")]
        interval: u64,
    }
    fn default_interval() -> u64 {
        5
    }
    fn default_expires() -> u64 {
        900
    }
    let text = response.text()?;
    // GitHub returns `interval` as a JSON number, not a quoted string.
    let interval = serde_json::from_str::<JsonInterval>(&text)
        .map(|j| j.interval)
        .unwrap_or(5);
    let parsed: CodeResponse =
        serde_json::from_str(&text).context("Invalid GitHub device-code response")?;
    Ok(DeviceCodeRequest {
        device_code: parsed.device_code,
        user_code: parsed.user_code,
        verification_uri: parsed.verification_uri,
        interval: interval.max(1),
        expires_in: parsed.expires_in.max(1),
    })
}

/// Step 2 of the Device Flow: poll until the user approves, returning the
/// OAuth token. `wait` blocks the calling thread (call from a background task);
/// it polls every `interval` seconds and gives up after `expires_in` seconds.
pub fn poll_github_device_token(
    client_id: &str,
    device_code: &str,
    interval: u64,
    expires_in: u64,
    proxy_url: Option<&str>,
) -> Result<String> {
    let client = build_http_client(proxy_url, 15)?;
    let started = std::time::Instant::now();
    let mut poll_period = Duration::from_secs(interval.max(1));
    loop {
        if started.elapsed() >= Duration::from_secs(expires_in.max(1)) {
            anyhow::bail!("Device-flow confirmation timed out");
        }
        std::thread::sleep(poll_period);
        let response = client
            .post(GH_DEVICE_TOKEN_URL)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()?;
        #[derive(Deserialize)]
        struct TokenResponse {
            #[serde(default)]
            access_token: Option<String>,
            #[serde(default)]
            error: Option<String>,
            #[serde(default)]
            interval: Option<u64>,
        }
        let parsed: TokenResponse = response.json()?;
        if let Some(token) = parsed.access_token {
            return Ok(token);
        }
        match parsed.error.as_deref() {
            // The user has not approved yet; keep polling.
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                // RFC 8628 / GitHub Device Flow: permanently back off by +5s
                // (or honor server-provided `interval` if larger).
                let min_next = poll_period.as_secs().saturating_add(5);
                let next_secs = parsed.interval.unwrap_or(min_next).max(min_next);
                poll_period = Duration::from_secs(next_secs);
                continue;
            }
            Some(err) => anyhow::bail!("Device flow failed: {err}"),
            None => anyhow::bail!("Device flow ended without a token"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_featured_and_roundtrips_toml() {
        let featured = load_builtin_featured_stacks().unwrap();
        assert!(featured.len() >= 3);
        assert!(featured.iter().all(|s| s.source == StackSource::Featured));
        let toml_text = export_stack_to_toml(&featured[0]).unwrap();
        let parsed = parse_stack_from_toml(&toml_text).unwrap();
        assert_eq!(parsed.id, featured[0].id);
        assert_eq!(parsed.skills.len(), featured[0].skills.len());
    }

    #[test]
    fn parses_community_index_json() {
        let body = r#"[
            {
                "id": "test-community-stack",
                "name": "Test Community Stack",
                "author": "someone",
                "tags": ["rust"],
                "skills": [
                    {"name": "skill-creator", "source": "anthropics/skills@skill-creator"}
                ]
            }
        ]"#;
        let stacks = parse_community_index(body).unwrap();
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].source, StackSource::Community);
        assert_eq!(
            stacks[0].skills[0].source,
            "anthropics/skills@skill-creator"
        );
    }

    #[test]
    fn community_index_rejects_invalid_entries() {
        assert!(parse_community_index("not json").is_err());
        assert!(parse_community_index("[{\"skills\":[]}]").is_err());
    }

    #[test]
    fn manifest_defaults_keep_legacy_toml_parsing() {
        // No `source` key: serde default must keep this a Featured stack.
        let legacy = r#"
[stack]
id = "legacy"
name = "Legacy Stack"
skills = []
"#;
        let parsed = parse_stack_from_toml(legacy).unwrap();
        assert_eq!(parsed.source, StackSource::Featured);
    }

    #[test]
    fn parses_shipped_community_index() {
        // The index served from the repo must always be valid and duplicate-free.
        let body = include_str!("../../../community/index.json");
        let stacks = parse_community_index(body).unwrap();
        assert!(stacks.len() >= 5);
        assert!(stacks.iter().all(|s| s.source == StackSource::Community));
        assert!(stacks.iter().all(|s| !s.skills.is_empty()));
        let mut ids: Vec<&str> = stacks.iter().map(|s| s.id.as_str()).collect();
        let before = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate stack ids in community index");

        let featured = load_builtin_featured_stacks().unwrap();
        for f in &featured {
            assert!(
                !stacks.iter().any(|c| c.id == f.id),
                "community stack id '{}' collides with built-in featured stack",
                f.id
            );
        }
    }
}
