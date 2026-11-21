//! In-app guided updater (v1): GitHub Releases check, 24h startup rate limiting,
//! per-platform installer selection, and strict `SHA256SUMS.txt` verification.
//!
//! Always opt-in — downloads and installer launches happen only after explicit
//! user consent in the UI or CLI.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::{
    Url,
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_TYPE},
};

use crate::parser::sanitize_slug;
use crate::source::build_http_client;

pub const DEFAULT_RELEASES_API_URL: &str =
    "https://api.github.com/repos/codexlin/UnoSkills/releases/latest";

/// Minimum interval between automatic startup update checks (24 hours).
pub const STARTUP_CHECK_INTERVAL_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    WindowsX64,
    MacosArm64,
    LinuxX64,
    Other,
}

impl PlatformKind {
    pub fn current() -> Self {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            Self::WindowsX64
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Self::MacosArm64
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            Self::LinuxX64
        }
        #[cfg(not(any(
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        )))]
        {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_download_url: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub body: String,
    pub published_at: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckOutcome {
    pub current_version: String,
    pub latest: Option<ReleaseInfo>,
    pub update_available: bool,
    pub recommended_asset: Option<ReleaseAsset>,
    pub checksums_asset: Option<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedUpdate {
    pub version: String,
    pub file_path: PathBuf,
    pub sha256: String,
    pub verified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateProgress {
    Preparing,
    Downloading(Option<u8>),
    Verifying,
}

/// Returns `true` when enough time has elapsed since the previous startup check.
pub fn should_run_startup_check(last_checked_at: Option<i64>, now_secs: i64) -> bool {
    match last_checked_at {
        None => true,
        Some(prev) => now_secs.saturating_sub(prev) >= STARTUP_CHECK_INTERVAL_SECS,
    }
}

struct Semver<'a> {
    core: (u64, u64, u64),
    pre: Option<&'a str>,
}

/// Parses a `v?MAJOR.MINOR.PATCH(-prerelease)?` tag and returns whether `latest`
/// is strictly newer than `current`.
pub fn is_newer_semver(latest: &str, current: &str) -> bool {
    let Some(l) = parse_semver(latest) else {
        return false;
    };
    let Some(c) = parse_semver(current) else {
        return false;
    };
    if l.core != c.core {
        return l.core > c.core;
    }
    match (l.pre, c.pre) {
        (None, Some(_)) => true,
        (Some(_), None) => false,
        (Some(lp), Some(cp)) => lp > cp,
        (None, None) => false,
    }
}

fn parse_semver(raw: &str) -> Option<Semver<'_>> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let without_build = trimmed.split('+').next()?;
    let (core, pre) = match without_build.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (without_build, None),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some(Semver {
        core: (major, minor, patch),
        pre,
    })
}

/// Selects the best user-facing installer asset for the given platform, falling
/// back to the platform archive when no installer is present.
pub fn select_platform_asset(
    assets: &[ReleaseAsset],
    platform: PlatformKind,
) -> Option<ReleaseAsset> {
    let find_matching = |pred: fn(&str) -> bool| {
        assets
            .iter()
            .find(|a| pred(&a.name.to_ascii_lowercase()))
            .cloned()
    };

    match platform {
        PlatformKind::WindowsX64 => {
            find_matching(|n| n.starts_with("unoskills-setup-") && n.ends_with(".exe"))
                .or_else(|| find_matching(|n| n.ends_with("windows-x64.zip")))
        }
        PlatformKind::MacosArm64 => find_matching(|n| n.ends_with("macos-arm64.dmg"))
            .or_else(|| find_matching(|n| n.ends_with("macos-arm64.tar.gz"))),
        PlatformKind::LinuxX64 => {
            find_matching(|n| n.starts_with("unoskills_") && n.ends_with("_amd64.deb"))
                .or_else(|| find_matching(|n| n.ends_with("linux-x64.tar.gz")))
        }
        PlatformKind::Other => None,
    }
}

pub fn find_checksums_asset(assets: &[ReleaseAsset]) -> Option<ReleaseAsset> {
    assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        .cloned()
}

/// Parses GNU `sha256sum` output (`<hex>  <filename>` or `<hex> *<filename>`)
/// into a lowercase-filename -> lowercase-hex-digest map.
pub fn parse_sha256sums(contents: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, |c: char| c.is_ascii_whitespace());
        let Some(hash) = parts.next().map(str::trim) else {
            continue;
        };
        let Some(rest) = parts.next().map(str::trim) else {
            continue;
        };
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let fname = rest
            .trim_start_matches('*')
            .trim_start_matches("./")
            .trim()
            .to_ascii_lowercase();
        if !fname.is_empty() {
            map.insert(fname, hash.to_ascii_lowercase());
        }
    }
    map
}

/// Computes the lowercase hex SHA-256 digest of a file on disk.
pub fn compute_file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open {} for SHA-256 verification", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Verifies a local file against an expected 64-char hex SHA-256 digest.
pub fn verify_file_sha256(path: &Path, expected_hex: &str) -> Result<String> {
    let actual = compute_file_sha256(path)?;
    let expected = expected_hex.trim().to_ascii_lowercase();
    if actual != expected {
        bail!(
            "SHA-256 checksum mismatch for {}: expected {}, got {}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("downloaded file"),
            expected,
            actual
        );
    }
    Ok(actual)
}

/// Fetches the latest GitHub Release metadata (respecting proxy and optional PAT).
pub fn fetch_latest_release(
    api_url: &str,
    proxy_url: Option<&str>,
    github_token: Option<&str>,
) -> Result<Option<ReleaseInfo>> {
    let client = build_http_client(proxy_url, 12)?;
    let mut req = client
        .get(api_url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");
    if let Some(tok) = github_token.map(str::trim).filter(|s| !s.is_empty()) {
        req = req.bearer_auth(tok);
    }
    let resp = req.send()?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    let resp = resp.error_for_status()?;
    #[derive(Deserialize)]
    struct GhAsset {
        name: String,
        browser_download_url: String,
        #[serde(rename = "url")]
        api_download_url: Option<String>,
        #[serde(default)]
        size: u64,
    }
    #[derive(Deserialize)]
    struct GhRelease {
        tag_name: String,
        #[serde(default)]
        html_url: String,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        published_at: Option<String>,
        #[serde(default)]
        assets: Vec<GhAsset>,
    }
    let parsed: GhRelease = resp.json()?;
    Ok(Some(ReleaseInfo {
        tag_name: parsed.tag_name,
        html_url: parsed.html_url,
        body: parsed.body.unwrap_or_default(),
        published_at: parsed.published_at,
        assets: parsed
            .assets
            .into_iter()
            .map(|a| ReleaseAsset {
                name: a.name,
                download_url: a.browser_download_url,
                api_download_url: a.api_download_url,
                size: a.size,
            })
            .collect(),
    }))
}

fn download_release_asset(
    client: &Client,
    asset: &ReleaseAsset,
    github_token: Option<&str>,
    timeout: Duration,
) -> Result<Response> {
    let request = |url: &str, binary: bool| -> Result<Response> {
        let url = Url::parse(url).context("Invalid release download URL")?;
        let mut request = client.get(url.clone()).timeout(timeout);
        if binary {
            request = request.header(ACCEPT, "application/octet-stream");
        }
        if url.scheme() == "https"
            && url.port().is_none()
            && matches!(url.host_str(), Some("github.com" | "api.github.com"))
            && let Some(token) = github_token
                .map(str::trim)
                .filter(|token| !token.is_empty())
        {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .and_then(Response::error_for_status)
            .map_err(reqwest::Error::without_url)?;
        if binary
            && response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.contains("json"))
        {
            bail!("GitHub returned asset metadata instead of the download");
        }
        Ok(response)
    };

    let result = request(&asset.download_url, false).or_else(|error| {
        let Some(url) = &asset.api_download_url else {
            return Err(error);
        };
        request(url, true).with_context(|| format!("Direct download failed ({error})"))
    });
    result.with_context(|| format!("Failed to download {}", asset.name))
}

/// Downloads a release asset into `dest_dir` and verifies its SHA-256 checksum
/// against `SHA256SUMS.txt` when available. Tampered files are deleted immediately.
pub fn download_and_verify_update(
    version: &str,
    asset: &ReleaseAsset,
    checksums_asset: Option<&ReleaseAsset>,
    dest_dir: &Path,
    proxy_url: Option<&str>,
    github_token: Option<&str>,
) -> Result<DownloadedUpdate> {
    download_and_verify_update_with_progress(
        version,
        asset,
        checksums_asset,
        dest_dir,
        proxy_url,
        github_token,
        |_| {},
    )
}

pub(crate) fn download_and_verify_update_with_progress(
    version: &str,
    asset: &ReleaseAsset,
    checksums_asset: Option<&ReleaseAsset>,
    dest_dir: &Path,
    proxy_url: Option<&str>,
    github_token: Option<&str>,
    mut progress: impl FnMut(UpdateProgress),
) -> Result<DownloadedUpdate> {
    progress(UpdateProgress::Preparing);
    let safe_name = Path::new(&asset.name)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && !s.contains(".."))
        .context("Invalid release asset filename")?;
    let safe_ver = sanitize_slug(version).unwrap_or_else(|| "latest".to_string());
    let target_dir = dest_dir.join(safe_ver);
    std::fs::create_dir_all(&target_dir)?;

    const INSTALLER_TIMEOUT_SECS: u64 = 180;
    let client = build_http_client(proxy_url, INSTALLER_TIMEOUT_SECS)
        .context("Failed to initialize the update downloader")?;

    // 1. Fetch expected SHA-256 from SHA256SUMS.txt first (fail closed if present
    //    on the release but missing an entry for this asset).
    let expected_sha = if let Some(sums_asset) = checksums_asset {
        let sums_text =
            download_release_asset(&client, sums_asset, github_token, Duration::from_secs(20))?
                .text()
                .map_err(reqwest::Error::without_url)
                .context("Failed to read SHA256SUMS.txt")?;
        let parsed = parse_sha256sums(&sums_text);
        let key = safe_name.to_ascii_lowercase();
        let digest = parsed
            .get(&key)
            .cloned()
            .with_context(|| format!("SHA256SUMS.txt does not contain an entry for {safe_name}"))?;
        Some(digest)
    } else {
        None
    };

    // 2. Download installer to a `.part` temp file inside `target_dir`.
    let final_path = target_dir.join(safe_name);
    let mut resp = download_release_asset(
        &client,
        asset,
        github_token,
        Duration::from_secs(INSTALLER_TIMEOUT_SECS),
    )?;
    let mut part = tempfile::Builder::new()
        .suffix(".part")
        .tempfile_in(&target_dir)?;
    let total = resp.content_length().unwrap_or(asset.size);
    let mut percent = (total > 0).then_some(0);
    progress(UpdateProgress::Downloading(percent));
    let mut received = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let len = resp
            .read(&mut buffer)
            .with_context(|| format!("Failed to read {safe_name}"))?;
        if len == 0 {
            break;
        }
        part.write_all(&buffer[..len])
            .with_context(|| format!("Failed to save {safe_name}"))?;
        received += len as u64;
        let next = (total > 0).then(|| (received.saturating_mul(100) / total).min(100) as u8);
        if next != percent {
            percent = next;
            progress(UpdateProgress::Downloading(percent));
        }
    }

    // 3. Verify SHA-256 before moving into place.
    let (sha256, verified) = match expected_sha {
        Some(expected) => {
            progress(UpdateProgress::Verifying);
            (verify_file_sha256(part.path(), &expected)?, true)
        }
        None => (compute_file_sha256(part.path())?, false),
    };

    part.persist(&final_path)?;

    Ok(DownloadedUpdate {
        version: version.to_string(),
        file_path: final_path,
        sha256,
        verified,
    })
}

/// Opens the downloaded package with the platform installer.
pub fn launch_platform_installer(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("Installer file not found: {}", path.display());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(path)
            .spawn()
            .with_context(|| format!("Failed to launch Windows installer {}", path.display()))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .with_context(|| format!("Failed to open macOS DMG {}", path.display()))?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        if path.extension().and_then(|extension| extension.to_str()) != Some("deb") {
            bail!("A Debian package is required");
        }
        if !std::process::Command::new("xdg-open")
            .arg(path)
            .status()?
            .success()
        {
            bail!("Could not open the Debian package");
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        bail!("No installer launch is available for this platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_versions_correctly() {
        assert!(is_newer_semver("v0.4.0", "0.3.0"));
        assert!(is_newer_semver("1.0.0", "0.9.9"));
        assert!(is_newer_semver("v0.3.1", "0.3.0"));
        assert!(is_newer_semver("v0.4.0", "0.4.0-rc.1"));
        assert!(!is_newer_semver("v0.3.0", "0.3.0"));
        assert!(!is_newer_semver("v0.2.0", "0.3.0"));
        assert!(!is_newer_semver("v0.4.0-beta.1", "0.4.0"));
        assert!(!is_newer_semver("invalid", "0.3.0"));
    }

    #[test]
    fn rate_limits_startup_update_checks_to_24_hours() {
        let now = 1_800_000_000;
        assert!(should_run_startup_check(None, now));
        assert!(!should_run_startup_check(Some(now - 3600), now));
        assert!(!should_run_startup_check(
            Some(now - STARTUP_CHECK_INTERVAL_SECS + 1),
            now
        ));
        assert!(should_run_startup_check(
            Some(now - STARTUP_CHECK_INTERVAL_SECS),
            now
        ));
    }

    #[test]
    fn selects_expected_installer_for_each_platform() {
        let assets = vec![
            ReleaseAsset {
                name: "unoskills-macos-arm64.tar.gz".into(),
                download_url: "https://example/macos.tar.gz".into(),
                api_download_url: None,
                size: 10,
            },
            ReleaseAsset {
                name: "unoskills-macos-arm64.dmg".into(),
                download_url: "https://example/macos.dmg".into(),
                api_download_url: None,
                size: 20,
            },
            ReleaseAsset {
                name: "unoskills-windows-x64.zip".into(),
                download_url: "https://example/win.zip".into(),
                api_download_url: None,
                size: 10,
            },
            ReleaseAsset {
                name: "unoskills-setup-0.4.0-windows-x64.exe".into(),
                download_url: "https://example/setup.exe".into(),
                api_download_url: None,
                size: 30,
            },
            ReleaseAsset {
                name: "unoskills-linux-x64.tar.gz".into(),
                download_url: "https://example/linux.tar.gz".into(),
                api_download_url: None,
                size: 10,
            },
            ReleaseAsset {
                name: "unoskills_0.4.0_amd64.deb".into(),
                download_url: "https://example/linux.deb".into(),
                api_download_url: None,
                size: 25,
            },
            ReleaseAsset {
                name: "SHA256SUMS.txt".into(),
                download_url: "https://example/SHA256SUMS.txt".into(),
                api_download_url: None,
                size: 1,
            },
        ];

        assert_eq!(
            select_platform_asset(&assets, PlatformKind::WindowsX64)
                .unwrap()
                .name,
            "unoskills-setup-0.4.0-windows-x64.exe"
        );
        assert_eq!(
            select_platform_asset(&assets, PlatformKind::MacosArm64)
                .unwrap()
                .name,
            "unoskills-macos-arm64.dmg"
        );
        assert_eq!(
            select_platform_asset(&assets, PlatformKind::LinuxX64)
                .unwrap()
                .name,
            "unoskills_0.4.0_amd64.deb"
        );
        assert_eq!(
            find_checksums_asset(&assets).unwrap().name,
            "SHA256SUMS.txt"
        );
    }

    #[test]
    fn parses_sha256sums_and_verifies_or_rejects_file_digest() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("unoskills_0.4.0_amd64.deb");
        std::fs::write(&file, b"unoskills release payload").unwrap();
        let actual = compute_file_sha256(&file).unwrap();

        let sums = format!(
            "# comment\n{actual}  ./unoskills_0.4.0_amd64.deb\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef *unoskills-macos-arm64.dmg\n"
        );
        let parsed = parse_sha256sums(&sums);
        assert_eq!(parsed.get("unoskills_0.4.0_amd64.deb"), Some(&actual));
        assert!(parsed.contains_key("unoskills-macos-arm64.dmg"));

        assert!(verify_file_sha256(&file, &actual).is_ok());
        assert!(
            verify_file_sha256(
                &file,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            )
            .is_err()
        );
    }

    #[test]
    fn downloads_verifies_sha256sums_and_rejects_tampered_payload() {
        use std::io::Write;
        use std::net::TcpListener;

        let payload = b"fake unoskills installer binary payload v0.4.0";
        let good_sha = hex::encode(Sha256::digest(payload));
        let bad_sha = "0".repeat(64);
        let good_sums = format!("{good_sha}  unoskills_0.4.0_amd64.deb\n");
        let bad_sums = format!("{bad_sha}  unoskills_0.4.0_amd64.deb\n");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let good_sums_c = good_sums.clone();
        let bad_sums_c = bad_sums.clone();
        let handle = std::thread::spawn(move || {
            for _ in 0..7 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut req_buf = [0u8; 1024];
                let n = stream.read(&mut req_buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&req_buf[..n]);
                assert!(!req.to_ascii_lowercase().contains("authorization:"));
                if req.starts_with("GET /blocked-") {
                    stream
                        .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    continue;
                }
                if !req.starts_with("GET /bad-sums") {
                    assert!(
                        req.to_ascii_lowercase()
                            .contains("accept: application/octet-stream")
                    );
                }
                let body: &[u8] = if req.starts_with("GET /good-sums") {
                    good_sums_c.as_bytes()
                } else if req.starts_with("GET /bad-sums") {
                    bad_sums_c.as_bytes()
                } else {
                    payload
                };
                let resp_head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(resp_head.as_bytes());
                let _ = stream.write_all(body);
            }
        });

        let tmp = tempfile::tempdir().unwrap();
        let asset = ReleaseAsset {
            name: "unoskills_0.4.0_amd64.deb".into(),
            download_url: format!("http://{addr}/blocked-installer"),
            api_download_url: Some(format!("http://{addr}/installer.deb")),
            size: payload.len() as u64,
        };
        let good_sums_asset = ReleaseAsset {
            name: "SHA256SUMS.txt".into(),
            download_url: format!("http://{addr}/blocked-sums"),
            api_download_url: Some(format!("http://{addr}/good-sums")),
            size: good_sums.len() as u64,
        };
        let bad_sums_asset = ReleaseAsset {
            name: "SHA256SUMS.txt".into(),
            download_url: format!("http://{addr}/bad-sums"),
            api_download_url: None,
            size: bad_sums.len() as u64,
        };

        // 1. Matching SHA256SUMS succeeds and writes verified file.
        let mut progress = Vec::new();
        let ok = download_and_verify_update_with_progress(
            "v0.4.0",
            &asset,
            Some(&good_sums_asset),
            &tmp.path().join("ok"),
            None,
            Some("fixture-token"),
            |event| progress.push(event),
        )
        .unwrap();
        assert!(ok.verified);
        assert_eq!(ok.sha256, good_sha);
        assert!(ok.file_path.is_file());
        assert_eq!(
            progress,
            [
                UpdateProgress::Preparing,
                UpdateProgress::Downloading(Some(0)),
                UpdateProgress::Downloading(Some(100)),
                UpdateProgress::Verifying,
            ]
        );

        // 2. Mismatched SHA256SUMS fails AND deletes the unverified `.part` file.
        let bad_dir = tmp.path().join("bad");
        let err = download_and_verify_update(
            "v0.4.0",
            &asset,
            Some(&bad_sums_asset),
            &bad_dir,
            None,
            None,
        )
        .expect_err("tampered download must fail checksum verification");
        assert!(err.to_string().contains("SHA-256 checksum mismatch"));
        assert_eq!(
            std::fs::read_dir(bad_dir.join("v0.4.0")).unwrap().count(),
            0
        );

        handle.join().unwrap();
    }
}
