use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::path::Path;

use super::{UnoSkillsManager, skill_revisions::backup_deployment_issues};
use crate::backup::{
    DEFAULT_BACKUP_BRANCH, GitBackupOpts, list_synced_artifacts, mirror_commit_and_push,
    normalize_branch, prepare_sync_repo, snapshot_backup,
};
use crate::domain::{
    DeploymentImpact, DeploymentImpactKind, GitBackupPullResult, GitBackupPushResult,
    GitBackupStatus, Skill, SourceType, StackManifest, StackSource,
};
use crate::parser::{checked_directory_hash as revision_hash, is_valid_skill_dir};
use crate::stacks::{export_stack_to_toml, parse_stack_from_toml};

const BASELINE_KEY: &str = "git_backup_baseline";

#[derive(Default, Deserialize, Serialize)]
struct BackupBaseline {
    remote: String,
    skills: HashMap<String, String>,
    stacks: HashMap<String, String>,
}

impl BackupBaseline {
    fn snapshot(remote: String, path: &Path) -> Result<Self> {
        let mut baseline = Self {
            remote,
            ..Self::default()
        };
        let (skill_dirs, stack_files) = list_synced_artifacts(path);
        for (slug, dir) in skill_dirs {
            if is_valid_skill_dir(&dir) {
                baseline.skills.insert(slug, revision_hash(&dir)?);
            }
        }
        for path in stack_files {
            let stack = read_stack(&path)?;
            baseline
                .stacks
                .insert(stack.id.clone(), stack_hash(&stack)?);
        }
        Ok(baseline)
    }

    fn check_push(&self, remote: &Self, proposed: &Self) -> Result<()> {
        let mut conflicts = BTreeSet::new();
        for (group, base, current, next) in [
            ("skills", &self.skills, &remote.skills, &proposed.skills),
            ("stacks", &self.stacks, &remote.stacks, &proposed.stacks),
        ] {
            for id in base.keys().chain(current.keys()) {
                if current.get(id) != base.get(id) && current.get(id) != next.get(id) {
                    conflicts.insert(format!("{group}/{id}"));
                }
            }
        }
        ensure!(
            conflicts.is_empty(),
            "BACKUP_REMOTE_CONFLICT: Pull and resolve remote changes before pushing: {}",
            conflicts.into_iter().collect::<Vec<_>>().join(", ")
        );
        Ok(())
    }
}

fn remote_key(url: &str, branch: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(url.as_bytes());
    hash.update(b"\0");
    hash.update(branch.as_bytes());
    hex::encode(hash.finalize())
}

fn read_stack(path: &Path) -> Result<StackManifest> {
    let mut stack = parse_stack_from_toml(&fs::read_to_string(path)?)?;
    stack.source = StackSource::Local;
    Ok(stack)
}

fn stack_hash(stack: &StackManifest) -> Result<String> {
    Ok(hex::encode(Sha256::digest(export_stack_to_toml(stack)?)))
}

fn deployment_issue(issue: DeploymentImpact) -> String {
    let detail = match issue.kind {
        DeploymentImpactKind::Missing => "Deployment is missing",
        DeploymentImpactKind::RefreshFailed => "Deployment could not be refreshed",
        _ => "Deployment differs from its synchronized version",
    };
    format!("{detail}: {}", issue.path.display())
}

impl UnoSkillsManager {
    pub fn backup_repo_url(&self) -> Option<String> {
        self.backup_target().ok().and_then(|(repo_url, _)| repo_url)
    }

    pub fn backup_branch(&self) -> String {
        self.backup_target()
            .map(|(_, branch)| branch)
            .unwrap_or_else(|_| DEFAULT_BACKUP_BRANCH.to_string())
    }

    fn backup_target(&self) -> Result<(Option<String>, String)> {
        let (repo_url, branch) = self.store.get_backup_target()?;
        let repo_url = repo_url
            .map(|url| url.trim().to_owned())
            .filter(|url| !url.is_empty());
        Ok((
            repo_url,
            normalize_branch(branch.as_deref().unwrap_or_default()),
        ))
    }

    pub fn configure_git_backup(&self, repo_url: &str, branch: Option<&str>) -> Result<()> {
        let _lock = self.lock_backup()?;
        let branch = match branch {
            Some(branch) => normalize_branch(branch),
            None => self.backup_target()?.1,
        };
        self.store.set_backup_target(repo_url.trim(), &branch)
    }

    pub fn git_backup_status(&self) -> Result<GitBackupStatus> {
        let _lock = self.lock_backup()?;
        let (repo_url, branch) = self.backup_target()?;
        let last_pushed_at = self
            .store
            .get_setting("git_backup_last_pushed_at")?
            .and_then(|s| s.parse::<i64>().ok());
        let last_pulled_at = self
            .store
            .get_setting("git_backup_last_pulled_at")?
            .and_then(|s| s.parse::<i64>().ok());
        let local_skill_count = self.list_skills()?.len();
        let local_stack_count = self.list_local_stacks()?.len();
        Ok(GitBackupStatus {
            repo_url,
            branch,
            last_pushed_at,
            last_pulled_at,
            local_skill_count,
            local_stack_count,
        })
    }

    fn lock_backup(&self) -> Result<File> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root_dir.join(".backup.lock"))?;
        lock.lock().context("Could not lock the Git backup")?;
        Ok(lock)
    }

    fn save_backup_baseline(&self, baseline: &BackupBaseline) -> Result<()> {
        self.store.set_setting(
            &format!("{BASELINE_KEY}/{}", baseline.remote),
            &serde_json::to_string(baseline)?,
        )
    }

    fn backup_baseline(&self, remote: &str) -> Result<BackupBaseline> {
        let saved = match self
            .store
            .get_setting(&format!("{BASELINE_KEY}/{remote}"))?
        {
            Some(saved) => Some(saved),
            None => self.store.get_setting(BASELINE_KEY)?,
        };
        Ok(saved
            .and_then(|raw| serde_json::from_str::<BackupBaseline>(&raw).ok())
            .filter(|baseline| baseline.remote == remote)
            .unwrap_or_default())
    }

    pub fn git_backup_push(&self) -> Result<GitBackupPushResult> {
        let _lock = self.lock_backup()?;
        self.push_configured_backup()
    }

    pub fn git_backup_push_to(&self, repo_url: &str, branch: &str) -> Result<GitBackupPushResult> {
        let _lock = self.lock_backup()?;
        self.store
            .set_backup_target(repo_url.trim(), &normalize_branch(branch))?;
        self.push_configured_backup()
    }

    fn push_configured_backup(&self) -> Result<GitBackupPushResult> {
        let (repo_url, branch) = self.backup_target()?;
        let repo_url =
            repo_url.context("No Git backup repository URL configured (Settings → Git Backup)")?;
        let proxy = self.http_proxy();
        let token = self.github_token();
        let opts = GitBackupOpts {
            repo_url: &repo_url,
            branch: &branch,
            proxy_url: proxy.as_deref(),
            github_token: token.as_deref(),
        };
        let remote = remote_key(&repo_url, &branch);
        let sync_dir = self.root_dir.join("backup-mirrors").join(&remote);
        let previous = self.backup_baseline(&remote)?;
        let (result, baseline) = mirror_commit_and_push(
            &sync_dir,
            &opts,
            &self.list_skills()?,
            &self.list_local_stacks()?,
            |remote_snapshot, staged| {
                let proposed = BackupBaseline::snapshot(remote.clone(), staged)?;
                let current = remote_snapshot
                    .map(|path| BackupBaseline::snapshot(remote.clone(), path))
                    .transpose()?
                    .unwrap_or_default();
                previous.check_push(&current, &proposed)?;
                Ok(proposed)
            },
        )?;
        self.save_backup_baseline(&baseline)?;
        if result.pushed || result.committed {
            self.store.set_setting(
                "git_backup_last_pushed_at",
                &Utc::now().timestamp().to_string(),
            )?;
        }

        Ok(result)
    }

    pub fn git_backup_pull(&self) -> Result<GitBackupPullResult> {
        let _lock = self.lock_backup()?;
        self.pull_configured_backup()
    }

    pub fn git_backup_pull_from(
        &self,
        repo_url: &str,
        branch: &str,
    ) -> Result<GitBackupPullResult> {
        let _lock = self.lock_backup()?;
        self.store
            .set_backup_target(repo_url.trim(), &normalize_branch(branch))?;
        self.pull_configured_backup()
    }

    fn pull_configured_backup(&self) -> Result<GitBackupPullResult> {
        let (repo_url, branch) = self.backup_target()?;
        let repo_url =
            repo_url.context("No Git backup repository URL configured (Settings → Git Backup)")?;
        let proxy = self.http_proxy();
        let token = self.github_token();
        let opts = GitBackupOpts {
            repo_url: &repo_url,
            branch: &branch,
            proxy_url: proxy.as_deref(),
            github_token: token.as_deref(),
        };
        let mut result = GitBackupPullResult::default();
        let remote = remote_key(&repo_url, &branch);
        let sync_dir = self.root_dir.join("backup-mirrors").join(&remote);
        let previous = self.backup_baseline(&remote)?;
        let mut next = BackupBaseline {
            remote,
            ..BackupBaseline::default()
        };
        let snapshot = prepare_sync_repo(&sync_dir, &opts)?
            .then(|| snapshot_backup(&sync_dir, &opts))
            .transpose()?;
        let (skill_dirs, stack_files) = snapshot
            .as_ref()
            .map(|snapshot| list_synced_artifacts(snapshot.path()))
            .unwrap_or_default();
        let mut skills: HashMap<_, _> = self
            .list_skills()?
            .into_iter()
            .map(|skill| (skill.slug.clone(), skill))
            .collect();

        for (slug, dir) in skill_dirs {
            if !is_valid_skill_dir(&dir) {
                continue;
            }
            let base = previous.skills.get(&slug);
            match self.restore_backup_skill(&slug, &dir, skills.remove(&slug), base, &mut result) {
                Ok(hash) => {
                    next.skills.insert(slug, hash);
                }
                Err(error) => {
                    result.conflicts.push(format!("{slug}: {error:#}"));
                    if let Some(hash) = base {
                        next.skills.insert(slug, hash.clone());
                    }
                }
            }
        }
        for path in stack_files {
            if let Err(error) = self.restore_backup_stack(&path, &previous, &mut next, &mut result)
            {
                let file = path.file_name().unwrap_or_default().to_string_lossy();
                result.conflicts.push(format!("{file}: {error:#}"));
            }
        }
        // Report remote deletions without removing local data or treating the
        // retained copies as new uploads on a later push.
        let local_stacks = self
            .list_local_stacks()?
            .into_iter()
            .map(|stack| stack.id)
            .collect::<BTreeSet<_>>();
        for (group, base, synced, local) in [
            (
                "skills",
                previous.skills,
                &mut next.skills,
                skills.into_keys().collect::<BTreeSet<_>>(),
            ),
            ("stacks", previous.stacks, &mut next.stacks, local_stacks),
        ] {
            for id in local {
                if let Some(hash) = base.get(&id)
                    && !synced.contains_key(&id)
                {
                    result.conflicts.push(format!(
                        "{group}/{id}: Deleted in the backup. The local copy was kept."
                    ));
                    synced.insert(id, hash.clone());
                }
            }
        }

        self.save_backup_baseline(&next)?;
        self.store.set_setting(
            "git_backup_last_pulled_at",
            &Utc::now().timestamp().to_string(),
        )?;
        Ok(result)
    }

    fn restore_backup_skill(
        &self,
        slug: &str,
        path: &Path,
        skill: Option<Skill>,
        base: Option<&String>,
        result: &mut GitBackupPullResult,
    ) -> Result<String> {
        let incoming = revision_hash(path)?;
        if let Some(skill) = skill {
            let local = revision_hash(&skill.central_path)?;
            if local == incoming {
                result.skills_unchanged += 1;
                result.conflicts.extend(
                    backup_deployment_issues(&skill, &local)
                        .into_iter()
                        .map(deployment_issue),
                );
            } else {
                ensure!(
                    base == Some(&local),
                    "The local skill differs from the last synchronized backup"
                );
                let update = self.apply_backup_revision(skill, path, &local)?;
                result.skills_updated += 1;
                result.refreshed_copies += update.refreshed_copies;
                result
                    .conflicts
                    .extend(update.deployment_issues.into_iter().map(deployment_issue));
            }
        } else {
            self.import_directory(path, Some(slug), SourceType::Local, None, None, None)?;
            result.skills_imported += 1;
        }
        Ok(incoming)
    }

    fn restore_backup_stack(
        &self,
        path: &Path,
        previous: &BackupBaseline,
        next: &mut BackupBaseline,
        result: &mut GitBackupPullResult,
    ) -> Result<()> {
        let stack = read_stack(path)?;
        let incoming = stack_hash(&stack)?;
        let _lock = self.lock_skills()?;
        let local = self.find_local_stack(&stack.id)?;
        if let Some(local) = local.as_ref() {
            let base = previous.stacks.get(&stack.id);
            let hash = stack_hash(local)?;
            if local != &stack && base != Some(&hash) {
                if let Some(base) = base {
                    next.stacks.insert(stack.id, base.clone());
                }
                bail!("The local stack differs from the last synchronized backup");
            }
        }
        if local.as_ref() != Some(&stack) {
            self.store
                .save_local_stack(&stack, Utc::now().timestamp())?;
            result.stacks_imported += 1;
        }
        next.stacks.insert(stack.id, incoming);
        Ok(())
    }
}
