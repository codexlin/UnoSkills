//! Plain-text Git backup & multi-machine sync for UnoSkills.
//!
//! Mirrors ONLY portable plain-text content (`skills/<slug>/...` and
//! `stacks/<id>.toml`) into a dedicated `<root_dir>/sync-repo` working tree.
//! Binary SQLite (`unoskills.db`), secrets (GitHub tokens, proxies), and
//! machine-local paths are NEVER committed or synced. On pull, the local
//! SQLite index is rebuilt/updated cleanly via `UnoSkillsManager`.

use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::domain::{Skill, StackManifest};
use crate::parser::sanitize_slug;
use crate::stacks::export_stack_to_toml;

pub const DEFAULT_BACKUP_BRANCH: &str = "main";

const SYNC_GITIGNORE: &str = "\
# UnoSkills plain-text backup repository — never commit local binary DBs or OS junk.
*.db
*.db-wal
*.db-shm
*.sqlite
*.sqlite3
.DS_Store
Thumbs.db
.staging-*
";

/// Options for invoking `git` during backup push/pull.
pub struct GitBackupOpts<'a> {
    pub repo_url: &'a str,
    pub branch: &'a str,
    pub proxy_url: Option<&'a str>,
    pub github_token: Option<&'a str>,
}

/// Ensures `<sync_dir>` is a valid Git working tree whose `origin` matches
/// `opts.repo_url` and fetches the remote branch if it exists.
pub fn prepare_sync_repo(sync_dir: &Path, opts: &GitBackupOpts<'_>) -> Result<bool> {
    let url = opts.repo_url.trim();
    if url.is_empty() {
        bail!("No Git backup repository URL configured (Settings → Git Backup)");
    }
    let branch = normalize_branch(opts.branch);

    if sync_dir.join(".git").exists() {
        // Keep `origin` aligned if the user updated the configured URL.
        let current_url =
            run_git_capture(sync_dir, opts, &["remote", "get-url", "origin"]).unwrap_or_default();
        if current_url.trim() != url {
            if current_url.trim().is_empty() {
                run_git(sync_dir, opts, &["remote", "add", "origin", url])?;
            } else {
                run_git(sync_dir, opts, &["remote", "set-url", "origin", url])?;
            }
        }
    } else {
        if sync_dir.exists() {
            std::fs::remove_dir_all(sync_dir)?;
        }
        if let Some(parent) = sync_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(sync_dir)?;
        run_git(sync_dir, opts, &["init"])?;
        let head_ref = format!("refs/heads/{branch}");
        run_git(sync_dir, opts, &["symbolic-ref", "HEAD", &head_ref])?;
        run_git(sync_dir, opts, &["remote", "add", "origin", url])?;
    }

    // Fetch remote refs; an empty brand-new remote repo succeeds with no remote branch yet.
    run_git(sync_dir, opts, &["fetch", "origin"])?;

    let remote_ref = format!("refs/remotes/origin/{branch}");
    let remote_has_branch =
        run_git_capture(sync_dir, opts, &["rev-parse", "--verify", &remote_ref]).is_ok();

    if remote_has_branch {
        // Ensure local branch tracks remote cleanly.
        let local_ref = format!("refs/heads/{branch}");
        let local_has_branch =
            run_git_capture(sync_dir, opts, &["rev-parse", "--verify", &local_ref]).is_ok();
        if local_has_branch {
            run_git(sync_dir, opts, &["checkout", &branch])?;
            // Fast-forward or reset mirror working tree to remote tip before
            // mirroring/importing (sync-repo is a pure mirror managed by UnoSkills).
            run_git(sync_dir, opts, &["reset", "--hard", &remote_ref])?;
        } else {
            run_git(sync_dir, opts, &["checkout", "-B", &branch, &remote_ref])?;
        }
    } else {
        // Brand-new empty remote: ensure HEAD points at the target branch name.
        let _ = run_git(sync_dir, opts, &["checkout", "-B", &branch]);
    }

    Ok(remote_has_branch)
}

/// Mirrors local skills and local stacks into `<sync_dir>` as plain text,
/// commits any changes, and pushes to `origin/<branch>`.
pub fn mirror_commit_and_push(
    sync_dir: &Path,
    opts: &GitBackupOpts<'_>,
    skills: &[Skill],
    local_stacks: &[StackManifest],
) -> Result<(bool, bool, usize, usize, Option<String>)> {
    let branch = normalize_branch(opts.branch);
    let remote_has_branch = prepare_sync_repo(sync_dir, opts)?;

    std::fs::write(sync_dir.join(".gitignore"), SYNC_GITIGNORE)?;

    // 1. Mirror skills/<slug>/ (plain-text files only, skipping any nested .git)
    let skills_root = sync_dir.join("skills");
    std::fs::create_dir_all(&skills_root)?;
    let mut active_slugs: HashSet<String> = HashSet::new();
    let mut skills_exported = 0usize;

    for skill in skills {
        if !skill.central_path.is_dir() {
            continue;
        }
        let Some(safe_slug) = sanitize_slug(&skill.slug) else {
            continue;
        };
        let dst = skills_root.join(&safe_slug);
        if dst.exists() {
            std::fs::remove_dir_all(&dst)?;
        }
        copy_plain_tree(&skill.central_path, &dst)?;
        active_slugs.insert(safe_slug);
        skills_exported += 1;
    }

    // Remove deleted skills from the mirror so deletions propagate cleanly.
    if let Ok(entries) = std::fs::read_dir(&skills_root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !active_slugs.contains(&name) {
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // 2. Mirror stacks/<id>.toml
    let stacks_root = sync_dir.join("stacks");
    std::fs::create_dir_all(&stacks_root)?;
    let mut active_stack_files: HashSet<String> = HashSet::new();
    let mut stacks_exported = 0usize;

    for stack in local_stacks {
        let Some(safe_id) = sanitize_slug(&stack.id) else {
            continue;
        };
        let file_name = format!("{safe_id}.toml");
        let toml_text = export_stack_to_toml(stack)?;
        std::fs::write(stacks_root.join(&file_name), toml_text)?;
        active_stack_files.insert(file_name);
        stacks_exported += 1;
    }

    if let Ok(entries) = std::fs::read_dir(&stacks_root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !active_stack_files.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    // 3. Stage, commit if dirty, and push
    run_git(sync_dir, opts, &["add", "-A"])?;
    let status = run_git_capture(sync_dir, opts, &["status", "--porcelain"])?;
    let has_changes = !status.trim().is_empty();

    let committed = if has_changes {
        let msg = format!(
            "chore(sync): backup {} skill(s) and {} stack(s)",
            skills_exported, stacks_exported
        );
        commit_with_fallback_identity(sync_dir, opts, &msg)?;
        true
    } else {
        false
    };

    let has_head = run_git_capture(sync_dir, opts, &["rev-parse", "--verify", "HEAD"]).is_ok();
    let pushed = if has_head && (committed || !remote_has_branch) {
        run_git(sync_dir, opts, &["push", "-u", "origin", &branch])?;
        true
    } else {
        false
    };

    let commit_sha = if has_head {
        run_git_capture(sync_dir, opts, &["rev-parse", "--short", "HEAD"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    Ok((
        committed,
        pushed,
        skills_exported,
        stacks_exported,
        commit_sha,
    ))
}

/// Lists valid skill directories and stack TOML files present in `<sync_dir>`
/// after pulling from remote.
pub fn list_synced_artifacts(sync_dir: &Path) -> (Vec<(String, PathBuf)>, Vec<PathBuf>) {
    let mut skill_dirs = Vec::new();
    let skills_root = sync_dir.join("skills");
    if let Ok(entries) = std::fs::read_dir(&skills_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let raw_name = entry.file_name().to_string_lossy().to_string();
                if let Some(slug) = sanitize_slug(&raw_name) {
                    skill_dirs.push((slug, path));
                }
            }
        }
    }
    skill_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut stack_files = Vec::new();
    let stacks_root = sync_dir.join("stacks");
    if let Ok(entries) = std::fs::read_dir(&stacks_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("toml"))
            {
                stack_files.push(path);
            }
        }
    }
    stack_files.sort();

    (skill_dirs, stack_files)
}

pub fn normalize_branch(branch: &str) -> String {
    let trimmed = branch.trim();
    if trimmed.is_empty() {
        DEFAULT_BACKUP_BRANCH.to_string()
    } else {
        trimmed.to_string()
    }
}

fn copy_plain_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Never copy nested .git directories or OS junk into the backup repo.
        if name_str == ".git" || name_str == ".DS_Store" || name_str == "Thumbs.db" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let meta = std::fs::symlink_metadata(&src_path)?;
        if meta.is_dir() {
            copy_plain_tree(&src_path, &dst_path)?;
        } else if meta.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        } else if meta.file_type().is_symlink() {
            // Dereference symlinks inside a skill only if they point to a regular file.
            if src_path.is_file() {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }
    }
    Ok(())
}

fn commit_with_fallback_identity(
    sync_dir: &Path,
    opts: &GitBackupOpts<'_>,
    message: &str,
) -> Result<()> {
    let has_name = run_git_capture(sync_dir, opts, &["config", "user.name"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_email = run_git_capture(sync_dir, opts, &["config", "user.email"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    if has_name && has_email {
        run_git(sync_dir, opts, &["commit", "-m", message])
    } else {
        run_git(
            sync_dir,
            opts,
            &[
                "-c",
                "user.name=UnoSkills Sync",
                "-c",
                "user.email=sync@unoskills.local",
                "commit",
                "-m",
                message,
            ],
        )
    }
}

fn build_git_command(sync_dir: &Path, opts: &GitBackupOpts<'_>, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(sync_dir).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(proxy) = opts.proxy_url.map(str::trim).filter(|s| !s.is_empty()) {
        cmd.arg("-c")
            .arg(format!("http.proxy={proxy}"))
            .arg("-c")
            .arg(format!("https.proxy={proxy}"));
    }
    // Pass GitHub PAT in-memory via http.extraHeader for HTTPS github.com URLs
    // so the token is never written to `.git/config`.
    if let Some(token) = opts.github_token.map(str::trim).filter(|s| !s.is_empty()) {
        let lower = opts.repo_url.trim().to_ascii_lowercase();
        if lower.starts_with("https://github.com/") && !lower.contains('@') {
            let encoded = base64_encode(format!("x-access-token:{token}").as_bytes());
            cmd.arg("-c").arg(format!(
                "http.https://github.com/.extraHeader=AUTHORIZATION: basic {encoded}"
            ));
        }
    }
    cmd.args(args);
    cmd
}

fn run_git(sync_dir: &Path, opts: &GitBackupOpts<'_>, args: &[&str]) -> Result<()> {
    let output = build_git_command(sync_dir, opts, args)
        .output()
        .context("Failed to execute git (is git installed?)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let safe_stderr = redact_token(stderr.trim(), opts.github_token);
        bail!("git {} failed: {}", args.join(" "), safe_stderr);
    }
    Ok(())
}

fn run_git_capture(sync_dir: &Path, opts: &GitBackupOpts<'_>, args: &[&str]) -> Result<String> {
    let output = build_git_command(sync_dir, opts, args)
        .output()
        .context("Failed to execute git (is git installed?)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let safe_stderr = redact_token(stderr.trim(), opts.github_token);
        bail!("git {} failed: {}", args.join(" "), safe_stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn redact_token(text: &str, token: Option<&str>) -> String {
    match token.map(str::trim).filter(|s| !s.is_empty()) {
        Some(secret) => {
            let encoded = base64_encode(format!("x-access-token:{secret}").as_bytes());
            text.replace(secret, "***").replace(&encoded, "***")
        }
        None => text.to_string(),
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(
            base64_encode(b"x-access-token:ghp_test"),
            "eC1hY2Nlc3MtdG9rZW46Z2hwX3Rlc3Q="
        );
    }

    #[test]
    fn redacts_secret_tokens_in_git_errors() {
        let encoded = base64_encode(b"x-access-token:ghp_secret123");
        let raw = format!("auth failed for ghp_secret123 and header basic {encoded}");
        let msg = redact_token(&raw, Some("ghp_secret123"));
        assert_eq!(msg, "auth failed for *** and header basic ***");
    }
}
