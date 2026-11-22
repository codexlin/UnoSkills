//! Plain-text Git backup & multi-machine sync for UnoSkills.
//!
//! Mirrors ONLY portable plain-text content (`skills/<slug>/...` and
//! `stacks/<id>.toml`) into a working tree dedicated to one repository and branch.
//! Binary SQLite (`unoskills.db`), secrets (GitHub tokens, proxies), and
//! machine-local paths are NEVER committed or synced. On pull, the local
//! SQLite index is rebuilt/updated cleanly via `UnoSkillsManager`.

use anyhow::{Context, Result, bail, ensure};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::domain::{GitBackupPushResult, Skill, StackManifest};
use crate::parser::{checked_entries, is_valid_skill_dir, sanitize_slug};
use crate::stacks::{export_stack_to_toml, parse_stack_from_toml};
use crate::sync::ensure_dst_not_overlapping_src;

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
    let local_ref = format!("refs/heads/{branch}");
    let remote_ref = format!("refs/remotes/origin/{branch}");

    let mirror_root = sync_dir
        .parent()
        .context("Missing backup mirror directory")?;
    ensure_managed_folder(mirror_root)?;
    ensure_managed_folder(sync_dir)?;
    run_git(sync_dir, opts, &["check-ref-format", &local_ref])?;
    let git_dir = sync_dir.join(".git");
    match fs::symlink_metadata(&git_dir) {
        Ok(metadata) => {
            ensure!(
                metadata.is_dir()
                    && git_dir
                        .join("commondir")
                        .symlink_metadata()
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                "BACKUP_PATH_CONFLICT: Unexpected Git directory: {}",
                git_dir.display()
            );
            let current_url =
                run_git_capture(sync_dir, opts, &["config", "--get", "remote.origin.url"])?;
            ensure!(
                current_url.trim() == url,
                "Unexpected remote in the backup working tree"
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ensure!(
                fs::read_dir(sync_dir)?.next().is_none(),
                "BACKUP_PATH_CONFLICT: Backup working tree is occupied: {}",
                sync_dir.display()
            );
            // Initialize off to the side so an interrupted setup can be retried.
            let staging = tempfile::tempdir_in(mirror_root)?;
            run_git(staging.path(), opts, &["init"])?;
            run_git(staging.path(), opts, &["symbolic-ref", "HEAD", &local_ref])?;
            run_git(
                staging.path(),
                opts,
                &["remote", "add", "--", "origin", url],
            )?;
            fs::rename(staging.path().join(".git"), git_dir)?;
        }
        Err(error) => return Err(error.into()),
    }

    // Fetch remote refs; an empty brand-new remote repo succeeds with no remote branch yet.
    run_git(sync_dir, opts, &["fetch", "--prune", "origin"])?;

    let remote_has_branch =
        run_git_capture(sync_dir, opts, &["rev-parse", "--verify", &remote_ref]).is_ok();

    run_git(sync_dir, opts, &["symbolic-ref", "HEAD", &local_ref])?;
    if remote_has_branch {
        // A failed first push may leave staged files even without a local commit.
        run_git(sync_dir, opts, &["reset", "--hard", &remote_ref])?;
    } else {
        // Start from an empty index with no parent even if the remote branch
        // previously existed or an earlier push stopped after committing.
        let empty_tree = run_git_capture(sync_dir, opts, &["mktree"])?;
        run_git(
            sync_dir,
            opts,
            &["read-tree", "--reset", "-u", empty_tree.trim()],
        )?;
        run_git(sync_dir, opts, &["update-ref", "-d", &local_ref])?;
    }
    // A previous attempt can fail before staging. Start each action with only
    // the tracked content of this app-owned backup mirror.
    run_git(sync_dir, opts, &["clean", "-fdx"])?;

    Ok(remote_has_branch)
}

/// Mirrors local skills and local stacks into `<sync_dir>` as plain text,
/// commits any changes, and pushes to `origin/<branch>`.
pub(crate) fn mirror_commit_and_push<T>(
    sync_dir: &Path,
    opts: &GitBackupOpts<'_>,
    skills: &[Skill],
    local_stacks: &[StackManifest],
    check: impl FnOnce(Option<&Path>, &Path) -> Result<T>,
) -> Result<(GitBackupPushResult, T)> {
    for skill in skills {
        ensure!(
            fs::symlink_metadata(&skill.central_path).is_ok_and(|meta| meta.is_dir()),
            "BACKUP_PATH_CONFLICT: Skill directory is missing or points to another location: {}",
            skill.central_path.display()
        );
        ensure_dst_not_overlapping_src(&skill.central_path, sync_dir)?;
    }
    let branch = normalize_branch(opts.branch);
    let remote_has_branch = prepare_sync_repo(sync_dir, opts)?;
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let remote_tip = remote_has_branch
        .then(|| run_git_capture(sync_dir, opts, &["rev-parse", "--verify", &remote_ref]))
        .transpose()?;
    let remote = remote_has_branch
        .then(|| snapshot_backup(sync_dir, opts))
        .transpose()?;

    let info = sync_dir.join(".git/info");
    ensure_managed_folder(&info)?;
    fs::write(info.join("exclude"), SYNC_GITIGNORE)?;

    // 1. Mirror skills/<slug>/ (plain-text files only, skipping any nested .git)
    let skills_root = sync_dir.join("skills");
    ensure_managed_folder(&skills_root)?;
    let (known_skills, known_stacks) = list_synced_artifacts(sync_dir);
    let known_skills: HashSet<_> = known_skills.into_iter().map(|(_, path)| path).collect();
    let known_stacks: HashSet<_> = known_stacks.into_iter().collect();
    let mut active_slugs: HashSet<String> = HashSet::new();
    let mut skills_exported = 0usize;
    let mut exported_files = Vec::new();

    for skill in skills {
        let Some(safe_slug) = sanitize_slug(&skill.slug) else {
            continue;
        };
        let dst = skills_root.join(&safe_slug);
        check_output_path(&dst, &known_skills)?;
        if known_skills.contains(&dst) {
            fs::remove_dir_all(&dst)?;
        }
        copy_plain_tree(&skill.central_path, &dst, &mut exported_files)?;
        ensure!(
            is_valid_skill_dir(&dst),
            "BACKUP_PATH_CONFLICT: Skill directory has no SKILL.md or skill.md: {}",
            skill.central_path.display()
        );
        if let Some(snapshot) = &remote
            && known_skills.contains(&dst)
        {
            let tracked_skill = snapshot.path().join("skills").join(&safe_slug);
            keep_tracked_exclusions(&tracked_skill, &dst)?;
        }
        active_slugs.insert(safe_slug);
        skills_exported += 1;
    }

    // Remove deleted skills from the mirror so deletions propagate cleanly.
    for path in &known_skills {
        if !active_slugs.contains(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        ) {
            fs::remove_dir_all(path)?;
        }
    }

    // 2. Mirror stacks/<id>.toml
    let stacks_root = sync_dir.join("stacks");
    ensure_managed_folder(&stacks_root)?;
    let mut active_stack_files: HashSet<String> = HashSet::new();
    let mut stacks_exported = 0usize;

    for stack in local_stacks {
        let Some(safe_id) = sanitize_slug(&stack.id) else {
            continue;
        };
        let file_name = format!("{safe_id}.toml");
        let dst = stacks_root.join(&file_name);
        check_output_path(&dst, &known_stacks)?;
        let toml_text = export_stack_to_toml(stack)?;
        fs::write(&dst, toml_text)?;
        exported_files.push(dst);
        active_stack_files.insert(file_name);
        stacks_exported += 1;
    }

    for path in &known_stacks {
        if !active_stack_files.contains(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        ) {
            fs::remove_file(path)?;
        }
    }

    // 3. Stage, commit if dirty, and push
    run_git(sync_dir, opts, &["add", "-A"])?;
    let staged = snapshot_backup(sync_dir, opts)?;
    for file in exported_files {
        let relative = file.strip_prefix(sync_dir)?;
        ensure!(
            fs::symlink_metadata(staged.path().join(relative)).is_ok_and(|meta| meta.is_file()),
            "BACKUP_STAGE_CONFLICT: Git excluded {}. Review Git ignore rules before pushing.",
            relative.display()
        );
    }
    let checked = check(remote.as_ref().map(tempfile::TempDir::path), staged.path())?;
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
        let branch_ref = format!("refs/heads/{branch}");
        let expected = remote_tip.as_deref().unwrap_or_default().trim();
        // The comparison above only covers the fetched tip. Require that tip
        // at push time so a concurrent branch deletion cannot be undone.
        let lease = format!("--force-with-lease={branch_ref}:{expected}");
        run_git(
            sync_dir,
            opts,
            &["push", "-u", &lease, "origin", &branch_ref],
        )?;
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
        GitBackupPushResult {
            committed,
            pushed,
            skills_exported,
            stacks_exported,
            commit_sha,
        },
        checked,
    ))
}

fn ensure_managed_folder(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            ensure!(
                fs::symlink_metadata(path)?.is_dir(),
                "BACKUP_PATH_CONFLICT: {}",
                path.display()
            );
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

fn check_output_path(path: &Path, known: &HashSet<PathBuf>) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => ensure!(
            known.contains(path),
            "BACKUP_PATH_CONFLICT: Unmanaged backup path {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Lists valid skill directories and stack TOML files present in `<sync_dir>`
/// after pulling from remote.
pub fn list_synced_artifacts(sync_dir: &Path) -> (Vec<(String, PathBuf)>, Vec<PathBuf>) {
    let mut skill_dirs = Vec::new();
    let skills_root = sync_dir.join("skills");
    if fs::symlink_metadata(&skills_root).is_ok_and(|metadata| metadata.is_dir())
        && let Ok(entries) = fs::read_dir(&skills_root)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) && is_valid_skill_dir(&path) {
                let raw_name = entry.file_name().to_string_lossy().to_string();
                if let Some(slug) = sanitize_slug(&raw_name).filter(|slug| slug == &raw_name) {
                    skill_dirs.push((slug, path));
                }
            }
        }
    }
    skill_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut stack_files = Vec::new();
    let stacks_root = sync_dir.join("stacks");
    if fs::symlink_metadata(&stacks_root).is_ok_and(|metadata| metadata.is_dir())
        && let Ok(entries) = fs::read_dir(&stacks_root)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_file())
                && path
                    .extension()
                    .is_some_and(|extension| extension == "toml")
                && fs::read_to_string(&path)
                    .ok()
                    .and_then(|raw| parse_stack_from_toml(&raw).ok())
                    .and_then(|stack| sanitize_slug(&stack.id))
                    .is_some_and(|id| entry.file_name() == format!("{id}.toml").as_str())
            {
                stack_files.push(path);
            }
        }
    }
    stack_files.sort();

    (skill_dirs, stack_files)
}

/// Read the version tracked by Git, excluding ignored files in the mirror.
pub(crate) fn snapshot_backup(
    sync_dir: &Path,
    opts: &GitBackupOpts<'_>,
) -> Result<tempfile::TempDir> {
    let snapshot = tempfile::tempdir()?;
    let prefix = format!(
        "--prefix={}/",
        snapshot.path().to_string_lossy().replace('\\', "/")
    );
    run_git(sync_dir, opts, &["checkout-index", "--all", &prefix])?;
    Ok(snapshot)
}

pub fn normalize_branch(branch: &str) -> String {
    let trimmed = branch.trim();
    if trimmed.is_empty() {
        DEFAULT_BACKUP_BRANCH.to_string()
    } else {
        trimmed.to_string()
    }
}

fn copy_plain_tree(src: &Path, dst: &Path, staged_files: &mut Vec<PathBuf>) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        // Never copy nested .git directories or OS junk into the backup repo.
        if name == ".git" || excluded_backup_name(&name) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let meta = std::fs::symlink_metadata(&src_path)?;
        if meta.is_dir() {
            copy_plain_tree(&src_path, &dst_path, staged_files)?;
        } else if meta.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
            staged_files.push(dst_path);
        } else {
            bail!(
                "BACKUP_PATH_CONFLICT: Unsupported file in skill directory: {}",
                src_path.display()
            );
        }
    }
    Ok(())
}

fn excluded_backup_name(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name == ".DS_Store"
        || name == "Thumbs.db"
        || [".db", ".db-wal", ".db-shm", ".sqlite", ".sqlite3"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

fn keep_tracked_exclusions(src: &Path, dst: &Path) -> Result<()> {
    ensure_dst_not_overlapping_src(src, dst)?;
    for entry in checked_entries(src) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(src)?;
        if entry.file_type().is_file() && relative.iter().any(excluded_backup_name) {
            // Preserve bytes already tracked by Git without sourcing excluded files locally.
            let target = dst.join(relative);
            fs::create_dir_all(target.parent().context("Missing backup file parent")?)?;
            fs::copy(entry.path(), &target)?;
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
    cmd.current_dir(sync_dir)
        .args(["--git-dir=.git", "--work-tree=."])
        // Library fingerprints and restores must not depend on the host's Git EOL defaults.
        .args(["-c", "core.autocrlf=false", "-c", "core.eol=lf"])
        .env("GIT_TERMINAL_PROMPT", "0");
    for local in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_SHALLOW_FILE",
        "GIT_CONFIG",
    ] {
        cmd.env_remove(local);
    }
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
