use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::domain::DeployMode;
use crate::parser::{COPY_FINGERPRINT_PREFIX, checked_directory_hash, managed_copy_fingerprint};

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
    #[cfg(windows)]
    let dst_is_link = dst_is_link || junction::exists(dst).unwrap_or(false);

    let lexical_dst = || -> Option<PathBuf> {
        dst.ancestors().skip(1).find_map(|ancestor| {
            let canonical = if ancestor.as_os_str().is_empty() {
                std::env::current_dir().ok()?
            } else {
                ancestor.canonicalize().ok()?
            };
            Some(canonical.join(dst.strip_prefix(ancestor).ok()?))
        })
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

pub(crate) fn verify_managed_target(
    src: &Path,
    dst: &Path,
    recorded_mode: DeployMode,
    expected_copy_hash: Option<&str>,
) -> Result<TargetState> {
    let state = classify_target(src, dst);
    match state {
        TargetState::Absent => match std::fs::symlink_metadata(dst) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(state),
            Err(error) => Err(error.into()),
            Ok(_) => bail!(
                "TARGET_CONFLICT: Deployment target changed during inspection: {}",
                dst.display()
            ),
        },
        TargetState::LinkToSource if recorded_mode == DeployMode::Symlink => Ok(state),
        TargetState::RealDir
            if recorded_mode == DeployMode::Copy
                && expected_copy_hash
                    .and_then(|hash| copy_baseline(src, hash))
                    .is_some_and(|expected| {
                        managed_copy_fingerprint(dst).is_ok_and(|actual| actual == expected)
                    }) =>
        {
            ensure_dst_not_overlapping_src(src, dst)?;
            Ok(state)
        }
        _ => bail!(
            "TARGET_CONFLICT: Deployment target has changed: {}",
            dst.display()
        ),
    }
}

pub(crate) fn copy_baseline(source: &Path, recorded: &str) -> Option<String> {
    if recorded.starts_with(COPY_FINGERPRINT_PREFIX) {
        return Some(recorded.to_owned());
    }
    // A legacy digest can identify the source revision. Only the source tree
    // can supply the directory and executable metadata that digest omitted.
    if checked_directory_hash(source).is_ok_and(|hash| hash == recorded) {
        return managed_copy_fingerprint(source).ok();
    }
    None
}

pub fn remove_managed_target(
    src: &Path,
    dst: &Path,
    recorded_mode: DeployMode,
    expected_copy_hash: Option<&str>,
) -> Result<bool> {
    let mut removal = RemovalBatch::default();
    let added = removal.add_deployment(src, dst, recorded_mode, expected_copy_hash)?;
    removal.commit(|| Ok(()))?;
    Ok(added)
}

#[derive(Default)]
pub(crate) struct RemovalBatch {
    entries: Vec<StagedRemoval>,
}

#[derive(Debug, Serialize)]
pub struct RemovalRollback {
    pub paths: Vec<PathBuf>,
    details: Vec<String>,
}

impl std::fmt::Display for RemovalRollback {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Removal rollback could not restore files preserved at {}",
            self.details.join(", ")
        )
    }
}

enum RemovalTarget {
    Directory(DirectoryIdentity),
    Copy(DirectoryIdentity, String),
    Link(PathBuf),
}

impl RemovalTarget {
    fn matches(&self, path: &Path) -> bool {
        match self {
            Self::Directory(identity) => identity.matches_at(path),
            Self::Copy(identity, expected) => {
                identity.matches_at(path)
                    && managed_copy_fingerprint(path).is_ok_and(|actual| actual == *expected)
            }
            Self::Link(destination) => {
                read_removal_link(path).is_ok_and(|link| link == *destination)
            }
        }
    }
}

struct StagedRemoval {
    original: PathBuf,
    physical: PathBuf,
    parent: DirectoryIdentity,
    staging: tempfile::TempDir,
    target: RemovalTarget,
    moved: bool,
    replacement: Option<RemovalTarget>,
    installed: bool,
}

impl StagedRemoval {
    fn check_parent(&self) -> Result<()> {
        ensure!(
            physical_target_path(&self.original).as_ref() == Some(&self.physical)
                && self.parent.matches_at(self.physical.parent().unwrap()),
            "TARGET_CONFLICT: Removal parent changed: {}",
            self.original.display()
        );
        Ok(())
    }

    fn stage(&mut self) -> Result<()> {
        self.check_parent()?;
        ensure!(
            self.target.matches(&self.physical),
            "TARGET_CONFLICT: Removal target changed: {}",
            self.original.display()
        );
        let preserved = self.staging.path().join("original");
        rename_without_replacing(&self.physical, &preserved)?;
        self.moved = true;
        self.staging.disable_cleanup(true);
        ensure!(
            self.target.matches(&preserved),
            "TARGET_CONFLICT: Removal target changed: {}",
            self.original.display()
        );
        if let Some(replacement) = &self.replacement {
            let prepared = self.staging.path().join("replacement");
            ensure!(
                replacement.matches(&prepared),
                "Prepared replacement changed"
            );
            rename_without_replacing(&prepared, &self.physical)?;
            self.installed = true;
            ensure!(
                replacement.matches(&self.physical),
                "Installed replacement changed"
            );
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        self.check_parent()?;
        if self.installed {
            ensure!(
                self.replacement
                    .as_ref()
                    .is_some_and(|target| target.matches(&self.physical)),
                "TARGET_CONFLICT: Replacement changed during rollback: {}",
                self.original.display()
            );
            rename_without_replacing(&self.physical, &self.staging.path().join("replacement"))?;
        }
        rename_without_replacing(&self.staging.path().join("original"), &self.physical)?;
        self.staging.disable_cleanup(false);
        Ok(())
    }
}

impl RemovalBatch {
    pub(crate) fn add_deployment(
        &mut self,
        source: &Path,
        path: &Path,
        mode: DeployMode,
        recorded: Option<&str>,
    ) -> Result<bool> {
        let state = verify_managed_target(source, path, mode, recorded)?;
        if state == TargetState::Absent {
            return Ok(false);
        }
        ensure_dst_not_overlapping_src(source, path)?;
        let physical = physical_target_path(path).context("Missing deployment parent")?;
        let target = if state == TargetState::LinkToSource {
            RemovalTarget::Link(read_removal_link(&physical)?)
        } else {
            let expected = recorded
                .and_then(|hash| copy_baseline(source, hash))
                .context("TARGET_CONFLICT: No trustworthy managed copy baseline")?;
            RemovalTarget::Copy(
                DirectoryIdentity::read(&physical)?.context("Deployment disappeared")?,
                expected,
            )
        };
        ensure!(
            classify_target(source, &physical) == state && target.matches(&physical),
            "TARGET_CONFLICT: Removal target changed: {}",
            path.display()
        );
        self.add(path, physical, target)?;
        Ok(true)
    }

    pub(crate) fn add_directory(
        &mut self,
        path: &Path,
        fingerprint: Option<String>,
    ) -> Result<bool> {
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            result => result?,
        };
        let physical = physical_target_path(path).context("Missing removal parent")?;
        let identity = DirectoryIdentity::read(&physical)?.context("Removal target disappeared")?;
        let target = match fingerprint {
            Some(hash) => RemovalTarget::Copy(identity, hash),
            None => RemovalTarget::Directory(identity),
        };
        self.add(path, physical, target)?;
        Ok(true)
    }

    pub(crate) fn replace_last<T>(
        &mut self,
        target: &Path,
        build: impl FnOnce(&Path) -> Result<T>,
    ) -> Result<T> {
        let entry = self
            .entries
            .last_mut()
            .filter(|entry| entry.original == target)
            .context("Missing replacement target")?;
        let path = entry.staging.path().join("replacement");
        let result = build(&path)?;
        entry.replacement = Some(match classify_target(&path, &path) {
            TargetState::LinkToSource | TargetState::ForeignLink => {
                RemovalTarget::Link(read_removal_link(&path)?)
            }
            TargetState::RealDir => RemovalTarget::Copy(
                DirectoryIdentity::read(&path)?.context("Replacement disappeared")?,
                managed_copy_fingerprint(&path)?,
            ),
            _ => bail!("Invalid replacement at {}", path.display()),
        });
        Ok(result)
    }

    fn add(&mut self, original: &Path, physical: PathBuf, target: RemovalTarget) -> Result<()> {
        ensure!(
            self.entries
                .iter()
                .all(|entry| !paths_overlap(&entry.physical, &physical)),
            "TARGET_CONFLICT: Overlapping removal targets at {}",
            original.display()
        );
        let parent = physical.parent().context("Missing removal parent")?;
        let entry = StagedRemoval {
            original: original.to_owned(),
            parent: DirectoryIdentity::read(parent)?.context("Missing removal parent")?,
            staging: tempfile::Builder::new()
                .prefix(".unoskills-undeploy-")
                .tempdir_in(parent)
                .with_context(|| format!("Cannot stage removal of {}", original.display()))?,
            physical,
            target,
            moved: false,
            replacement: None,
            installed: false,
        };
        self.entries.push(entry);
        Ok(())
    }

    pub(crate) fn commit(mut self, persist: impl FnOnce() -> Result<()>) -> Result<()> {
        let result = self
            .entries
            .iter_mut()
            .try_for_each(|entry| {
                entry
                    .stage()
                    .map_err(|error| crate::RemovalProblem::at(&entry.original, error))
            })
            .and_then(|()| persist());
        if let Err(error) = result {
            let mut rollback = RemovalRollback {
                paths: Vec::new(),
                details: Vec::new(),
            };
            for entry in self.entries.iter_mut().rev().filter(|entry| entry.moved) {
                if let Err(restore) = entry.restore() {
                    let path = entry.staging.path().join("original");
                    rollback
                        .details
                        .push(format!("{} ({restore:#})", path.display()));
                    rollback.paths.push(path);
                }
            }
            if !rollback.paths.is_empty() {
                return Err(error).context(rollback);
            }
            return Err(error);
        }
        for entry in &mut self.entries {
            entry.staging.disable_cleanup(false);
        }
        Ok(())
    }
}

fn read_removal_link(path: &Path) -> std::io::Result<PathBuf> {
    #[cfg(windows)]
    if junction::exists(path).unwrap_or(false) {
        return junction::get_target(path);
    }
    fs::read_link(path)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct DirectoryIdentity {
    pub(crate) path: PathBuf,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    #[serde(default)]
    file_id: Option<(u64, [u8; 16])>,
}

impl DirectoryIdentity {
    pub(crate) fn read(path: &Path) -> Result<Option<Self>> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        ensure!(
            metadata.is_dir() && path.canonicalize()? == path,
            "TARGET_CONFLICT: Invalid directory {}",
            path.display()
        );
        Ok(Some(Self::from_metadata(path, &metadata)))
    }

    pub(crate) fn from_metadata(path: &Path, metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        #[cfg(windows)]
        let file_id = windows_directory_id(path);
        Self {
            path: path.into(),
            #[cfg(not(windows))]
            created: metadata.created().ok(),
            #[cfg(windows)]
            created: if file_id.is_none() {
                metadata.created().ok()
            } else {
                None
            },
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(windows)]
            file_id,
        }
    }

    pub(crate) fn matches_at(&self, path: &Path) -> bool {
        let mut expected = self.clone();
        expected.path = path.into();
        Self::read(path).is_ok_and(|value| value.as_ref() == Some(&expected))
    }
}

#[cfg(windows)]
fn windows_directory_id(path: &Path) -> Option<(u64, [u8; 16])> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_ID_INFO, FILE_READ_ATTRIBUTES, FileIdInfo, GetFileInformationByHandle,
        GetFileInformationByHandleEx,
    };

    let directory = fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .ok()?;
    let mut info = FILE_ID_INFO::default();
    // The directory handle and its correctly sized, writable output buffers remain live for both calls.
    if unsafe {
        GetFileInformationByHandleEx(
            directory.as_raw_handle(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            std::mem::size_of::<FILE_ID_INFO>() as u32,
        )
    } != 0
        && info.FileId.Identifier != [0; 16]
    {
        return Some((info.VolumeSerialNumber, info.FileId.Identifier));
    }

    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(directory.as_raw_handle(), &mut info) } == 0 {
        return None;
    }
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    (index != 0).then_some((
        u64::from(info.dwVolumeSerialNumber),
        u128::from(index).to_le_bytes(),
    ))
}

pub(crate) fn physical_target_path(path: &Path) -> Option<PathBuf> {
    Some(path.parent()?.canonicalize().ok()?.join(path.file_name()?))
}

pub(crate) fn paths_overlap(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        let left = PathBuf::from(left.to_string_lossy().to_lowercase());
        let right = PathBuf::from(right.to_string_lossy().to_lowercase());
        left.starts_with(&right) || right.starts_with(&left)
    } else {
        left.starts_with(right) || right.starts_with(left)
    }
}

pub(crate) fn rename_without_replacing(from: &Path, to: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
        let from = from
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();
        let to = to.as_os_str().encode_wide().chain([0]).collect::<Vec<_>>();
        // Both paths are NUL-terminated and live for the duration of the call.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        use std::io::ErrorKind;
        ensure!(
            fs::symlink_metadata(to).is_err_and(|error| error.kind() == ErrorKind::NotFound),
            "TARGET_CONFLICT: An entry already exists: {}",
            to.display()
        );
        fs::rename(from, to)?;
    }
    Ok(())
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

pub(crate) fn create_dir_link(src: &Path, dst: &Path) -> Result<()> {
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
    ensure_dst_not_overlapping_src(src, dst)?;
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

        let removed = remove_managed_target(&src, &dst, DeployMode::Symlink, None).unwrap();
        assert!(removed);
        assert!(!dst.exists());
        assert!(src.join("SKILL.md").exists());
    }

    #[test]
    fn rollback_leaves_a_new_target_and_reports_the_recoverable_original() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("deployment");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("SKILL.md"), "original").unwrap();
        let mut removal = RemovalBatch::default();
        removal.add_directory(&target, None).unwrap();
        let preserved = removal.entries[0].staging.path().join("original");
        let error = removal
            .commit(|| {
                fs::create_dir(&target)?;
                bail!("Simulated failure after a new target appeared");
            })
            .unwrap_err();

        assert!(error.to_string().contains(&preserved.display().to_string()));
        assert!(target.is_dir());
        assert!(fs::read_dir(&target).unwrap().next().is_none());
        assert_eq!(
            fs::read_to_string(preserved.join("SKILL.md")).unwrap(),
            "original"
        );
    }
}
