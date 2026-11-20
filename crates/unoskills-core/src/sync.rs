use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::domain::DeployMode;

/// Classifies what currently exists at a deployment target using `symlink_metadata`
/// so directory symlinks / Windows junctions are never confused with real directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
    Absent,
    LinkToSource,
    ForeignLink,
    RealDir,
    RealFile,
}

/// Authorizes what `deploy_skill_dir` is allowed to replace at the destination.
#[derive(Debug, Clone, Copy)]
pub enum ReplacePolicy {
    /// Create a new deployment without replacing any existing target.
    CreateOnly,
    /// Refuse to overwrite anything we cannot prove is our own link.
    NoClobber,
    /// We have a DB deployment record matching this exact path and mode.
    Recorded(DeployMode),
    /// User explicitly confirmed replacing the existing target.
    UserConfirmed,
}

impl ReplacePolicy {
    fn permits(self, state: TargetState) -> bool {
        match state {
            TargetState::Absent => true,
            TargetState::LinkToSource => !matches!(self, ReplacePolicy::CreateOnly),
            TargetState::ForeignLink => match self {
                ReplacePolicy::CreateOnly | ReplacePolicy::NoClobber => false,
                ReplacePolicy::Recorded(DeployMode::Symlink) => true,
                ReplacePolicy::Recorded(DeployMode::Copy) => false,
                ReplacePolicy::UserConfirmed => true,
            },
            TargetState::RealDir => match self {
                ReplacePolicy::CreateOnly | ReplacePolicy::NoClobber => false,
                ReplacePolicy::Recorded(DeployMode::Copy) => true,
                ReplacePolicy::Recorded(DeployMode::Symlink) => false,
                ReplacePolicy::UserConfirmed => true,
            },
            TargetState::RealFile => matches!(self, ReplacePolicy::UserConfirmed),
        }
    }
}

pub fn ensure_dst_not_overlapping_src(src: &Path, dst: &Path) -> Result<()> {
    if src == dst {
        bail!("Source and destination are identical: {}", src.display());
    }
    let src_canon = src
        .canonicalize()
        .with_context(|| format!("Skill source does not exist: {}", src.display()))?;

    let dst_is_link = std::fs::symlink_metadata(dst)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    let lexical_dst = || -> Option<PathBuf> {
        let parent = dst.parent()?.canonicalize().ok()?;
        let name = dst.file_name()?;
        Some(parent.join(name))
    };

    let dst_canon = if dst_is_link {
        lexical_dst()
    } else {
        dst.canonicalize().ok().or_else(lexical_dst)
    };

    if let Some(dst_canon) = dst_canon {
        if dst_canon == src_canon {
            bail!(
                "Source and destination resolve to the same path: {}",
                src.display()
            );
        }
        if dst_canon.starts_with(&src_canon) {
            bail!(
                "Destination {} is inside source {}; refusing to avoid recursion",
                dst.display(),
                src.display()
            );
        }
        if src_canon.starts_with(&dst_canon) {
            bail!(
                "Source {} is inside destination {}; refusing to protect source",
                src.display(),
                dst.display()
            );
        }
    }
    Ok(())
}

pub fn classify_target(src: &Path, dst: &Path) -> TargetState {
    let Ok(meta) = std::fs::symlink_metadata(dst) else {
        return TargetState::Absent;
    };

    if meta.file_type().is_symlink() {
        if let (Ok(src_c), Ok(dst_c)) = (src.canonicalize(), dst.canonicalize())
            && src_c == dst_c
        {
            return TargetState::LinkToSource;
        }
        return TargetState::ForeignLink;
    }

    #[cfg(windows)]
    if junction::exists(dst).unwrap_or(false) {
        if let (Ok(src_c), Ok(dst_c)) = (src.canonicalize(), dst.canonicalize())
            && src_c == dst_c
        {
            return TargetState::LinkToSource;
        }
        return TargetState::ForeignLink;
    }

    if meta.is_dir() {
        TargetState::RealDir
    } else {
        TargetState::RealFile
    }
}

pub fn deploy_skill_dir(
    src: &Path,
    dst: &Path,
    mode: DeployMode,
    policy: ReplacePolicy,
) -> Result<DeployMode> {
    ensure_dst_not_overlapping_src(src, dst)?;

    let state = classify_target(src, dst);
    if !policy.permits(state) {
        bail!(
            "TARGET_CONFLICT: Refusing to overwrite unmanaged target {} ({:?})",
            dst.display(),
            state
        );
    }

    if state != TargetState::Absent {
        remove_path_safely(dst, state)?;
    }

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent dir {}", parent.display()))?;
    }

    if mode == DeployMode::Symlink && create_dir_link(src, dst).is_ok() {
        return Ok(DeployMode::Symlink);
    }

    // Reserve a new copy destination without following a path that appeared
    // after the initial metadata check.
    if matches!(policy, ReplacePolicy::CreateOnly) {
        std::fs::create_dir(dst).with_context(|| {
            format!(
                "Failed to create a new deployment directory at {}",
                dst.display()
            )
        })?;
    }
    copy_dir_recursive(src, dst)?;
    Ok(DeployMode::Copy)
}

pub fn remove_managed_target(src: &Path, dst: &Path, recorded_mode: DeployMode) -> Result<bool> {
    let state = classify_target(src, dst);
    match state {
        TargetState::Absent => Ok(false),
        TargetState::LinkToSource => {
            remove_path_safely(dst, state)?;
            Ok(true)
        }
        TargetState::ForeignLink if recorded_mode == DeployMode::Symlink => {
            remove_path_safely(dst, state)?;
            Ok(true)
        }
        TargetState::RealDir if recorded_mode == DeployMode::Copy => {
            remove_path_safely(dst, state)?;
            Ok(true)
        }
        other => {
            bail!(
                "TARGET_CONFLICT: Refusing to delete {} because on-disk type ({:?}) does not match recorded mode ({:?})",
                dst.display(),
                other,
                recorded_mode
            );
        }
    }
}

fn remove_path_safely(dst: &Path, state: TargetState) -> Result<()> {
    match state {
        TargetState::Absent => Ok(()),
        TargetState::LinkToSource | TargetState::ForeignLink => {
            #[cfg(windows)]
            {
                if junction::exists(dst).unwrap_or(false) {
                    junction::delete(dst)?;
                    return Ok(());
                }
                if std::fs::remove_dir(dst).is_ok() {
                    return Ok(());
                }
            }
            std::fs::remove_file(dst)
                .with_context(|| format!("Failed to remove symlink {}", dst.display()))?;
            Ok(())
        }
        TargetState::RealDir => {
            std::fs::remove_dir_all(dst)
                .with_context(|| format!("Failed to remove directory {}", dst.display()))?;
            Ok(())
        }
        TargetState::RealFile => {
            std::fs::remove_file(dst)
                .with_context(|| format!("Failed to remove file {}", dst.display()))?;
            Ok(())
        }
    }
}

fn create_dir_link(src: &Path, dst: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(src, dst).is_ok() {
            return Ok(());
        }
        junction::create(src, dst)?;
        Ok(())
    }
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
    {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_clobber_refuses_unmanaged_real_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("central/my-skill");
        let dst = tmp.path().join("agent/my-skill");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(src.join("SKILL.md"), "---\nname: s\n---\n").unwrap();
        std::fs::write(dst.join("custom.txt"), "user work").unwrap();

        let err = deploy_skill_dir(&src, &dst, DeployMode::Symlink, ReplacePolicy::NoClobber)
            .unwrap_err();
        assert!(err.to_string().contains("TARGET_CONFLICT"));
        assert!(dst.join("custom.txt").exists());
    }

    #[test]
    fn symlink_deploy_and_clean_remove_works() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("central/demo");
        let dst = tmp.path().join("agent/demo");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();

        let actual =
            deploy_skill_dir(&src, &dst, DeployMode::Symlink, ReplacePolicy::NoClobber).unwrap();
        assert_eq!(actual, DeployMode::Symlink);
        assert_eq!(classify_target(&src, &dst), TargetState::LinkToSource);

        let removed = remove_managed_target(&src, &dst, DeployMode::Symlink).unwrap();
        assert!(removed);
        assert!(!dst.exists());
        assert!(src.join("SKILL.md").exists());
    }
}
