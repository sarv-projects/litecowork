//! Handle-confined, bounded file replacement for the first mediated Task write.
//!
//! This module owns filesystem mechanics only. It does not authorize an Effect, consume
//! an Approval, create a receipt, or prove that a provider is an isolation boundary.

use sha2::{Digest as _, Sha256};
use std::fmt;
#[cfg(test)]
use std::path::Path;

pub const BOUNDED_FILE_REPLACEMENT_MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Eq, PartialEq)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    pub fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn parse(value: &str) -> Result<Self, FileWriteError> {
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(FileWriteError::IntegrityFailure);
        };
        let decoded = hex::decode(hex).map_err(|_| FileWriteError::IntegrityFailure)?;
        let digest: [u8; 32] = decoded
            .try_into()
            .map_err(|_| FileWriteError::IntegrityFailure)?;
        Ok(Self(digest))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_wire(&self) -> String {
        format!("sha256:{}", hex::encode(self.0))
    }
}

impl fmt::Debug for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContentDigest([redacted])")
    }
}

pub struct BoundedBlob(Vec<u8>);

impl BoundedBlob {
    pub fn new(bytes: Vec<u8>) -> Result<Self, FileWriteError> {
        if bytes.len() > BOUNDED_FILE_REPLACEMENT_MAX_BYTES {
            return Err(FileWriteError::OverBound);
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for BoundedBlob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoundedBlob([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileWriteError {
    IntegrityFailure,
    PathEscape,
    OverBound,
    AmbiguousResult,
    UnsupportedMetadata,
    IoUnavailable,
}

impl fmt::Display for FileWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for FileWriteError {}

enum RootHandle {
    #[cfg(target_os = "linux")]
    Linux(rustix::fd::OwnedFd),
    #[cfg(windows)]
    Windows(windows_local_security::VerifiedDirectory),
    #[cfg(not(any(target_os = "linux", windows)))]
    Unsupported,
}

pub struct WorkspaceRootHandle(RootHandle);

impl WorkspaceRootHandle {
    /// Takes ownership of the directory descriptor that Runtime has already opened and
    /// identity-checked against the private WorkspaceRoot binding.
    ///
    /// This constructor intentionally accepts no path. The caller must perform the 003
    /// WorkspaceRoot identity check before transferring the descriptor.
    #[cfg(target_os = "linux")]
    pub fn from_runtime_verified_linux_directory(
        handle: rustix::fd::OwnedFd,
    ) -> Result<Self, FileWriteError> {
        use rustix::fs::fstat;
        let metadata = fstat(&handle).map_err(|_| FileWriteError::PathEscape)?;
        if metadata.st_mode & 0o170000 != 0o040000 {
            return Err(FileWriteError::PathEscape);
        }
        Ok(Self(RootHandle::Linux(handle)))
    }

    /// Takes ownership of the no-reparse directory handle that Runtime has already
    /// identity-checked against the private WorkspaceRoot binding.
    #[cfg(windows)]
    pub fn from_runtime_verified_windows_directory(
        handle: windows_local_security::VerifiedDirectory,
    ) -> Self {
        Self(RootHandle::Windows(handle))
    }

    #[cfg(test)]
    fn open_test_root(path: &Path) -> Result<Self, FileWriteError> {
        #[cfg(target_os = "linux")]
        {
            use rustix::fs::{Mode, OFlags, open};
            let handle = open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|_| FileWriteError::PathEscape)?;
            return Self::from_runtime_verified_linux_directory(handle);
        }
        #[cfg(windows)]
        {
            let handle = windows_local_security::open_directory_path_without_reparse(path)
                .map_err(|_| FileWriteError::PathEscape)?;
            return Ok(Self::from_runtime_verified_windows_directory(handle));
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            let _ = path;
            Err(FileWriteError::IoUnavailable)
        }
    }
}

pub struct WorkspaceConfinedPath {
    root: WorkspaceRootHandle,
    relative_path: String,
    effect_id: String,
}

impl WorkspaceConfinedPath {
    pub fn new(
        root: WorkspaceRootHandle,
        relative_path: &str,
        effect_id: &str,
    ) -> Result<Self, FileWriteError> {
        validate_relative_path(relative_path)?;
        if effect_id.is_empty()
            || effect_id.len() > 128
            || !effect_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(FileWriteError::PathEscape);
        }
        Ok(Self {
            root,
            relative_path: relative_path.to_owned(),
            effect_id: effect_id.to_owned(),
        })
    }
}

impl fmt::Debug for WorkspaceConfinedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkspaceConfinedPath([redacted])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceOutcome {
    pub content_digest: ContentDigest,
    pub bytes_written: u32,
}

pub fn replace_approved_file(
    target: &WorkspaceConfinedPath,
    base: &ContentDigest,
    new_content: &BoundedBlob,
) -> Result<ReplaceOutcome, FileWriteError> {
    if new_content.as_bytes().len() > BOUNDED_FILE_REPLACEMENT_MAX_BYTES {
        return Err(FileWriteError::OverBound);
    }
    #[cfg(target_os = "linux")]
    {
        return linux::replace(target, base, new_content);
    }
    #[cfg(windows)]
    {
        return windows::replace(target, base, new_content);
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (target, base);
        Err(FileWriteError::IoUnavailable)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{
        BOUNDED_FILE_REPLACEMENT_MAX_BYTES, BoundedBlob, ContentDigest, FileWriteError,
        ReplaceOutcome, RootHandle, WorkspaceConfinedPath, digest, effect_temp_leaf,
    };
    use rustix::{
        fd::AsFd,
        fs::{
            Gid, Mode, OFlags, ResolveFlags, Uid, XattrFlags, fchmod, fchown, flistxattr,
            fremovexattr, fsetxattr, fsync, openat2,
        },
        io::Errno,
    };
    use sha2::Digest as _;
    use std::{
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
        os::unix::fs::MetadataExt,
    };

    const RESOLVE: ResolveFlags = ResolveFlags::BENEATH
        .union(ResolveFlags::NO_SYMLINKS)
        .union(ResolveFlags::NO_MAGICLINKS)
        .union(ResolveFlags::NO_XDEV);
    const MAX_XATTR_LIST_BYTES: usize = 64 * 1024;
    const MAX_COPIED_XATTR_BYTES: usize = 1024 * 1024;
    const ACCESS_ACL_XATTR: &str = "system.posix_acl_access";

    pub(super) fn replace(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
    ) -> Result<ReplaceOutcome, FileWriteError> {
        replace_with_hook(target, base, new_content, || {})
    }

    #[cfg(test)]
    pub(super) fn replace_with_hook(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
        after_staging: impl FnOnce(),
    ) -> Result<ReplaceOutcome, FileWriteError> {
        replace_inner(target, base, new_content, after_staging)
    }

    #[cfg(not(test))]
    fn replace_with_hook(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
        after_staging: impl FnOnce(),
    ) -> Result<ReplaceOutcome, FileWriteError> {
        replace_inner(target, base, new_content, after_staging)
    }

    fn replace_inner(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
        after_staging: impl FnOnce(),
    ) -> Result<ReplaceOutcome, FileWriteError> {
        let RootHandle::Linux(root) = &target.root.0;
        if new_content.as_bytes().len() > BOUNDED_FILE_REPLACEMENT_MAX_BYTES {
            return Err(FileWriteError::OverBound);
        }

        let (parent_path, leaf) = target
            .relative_path
            .rsplit_once('/')
            .map_or((None, target.relative_path.as_str()), |(parent, leaf)| {
                (Some(parent), leaf)
            });
        let opened_parent = parent_path
            .map(|parent| {
                openat2(
                    root,
                    parent,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                    RESOLVE,
                )
                .map_err(map_resolution_error)
            })
            .transpose()?;
        let parent = opened_parent.as_ref().unwrap_or(root);
        let target_file = open_regular_file(parent, leaf)?;
        let target_metadata = target_file
            .metadata()
            .map_err(|_| FileWriteError::IoUnavailable)?;
        if !target_metadata.is_file() || target_metadata.nlink() != 1 {
            return Err(FileWriteError::PathEscape);
        }
        if digest_file(&target_file)? != *base {
            return Err(FileWriteError::IntegrityFailure);
        }

        let copied_xattrs = copyable_xattrs(&target_file)?;
        let temporary_leaf = effect_temp_leaf(&target.effect_id);
        let temporary_fd = openat2(
            parent,
            temporary_leaf.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
            RESOLVE,
        )
        .map_err(|error| {
            if error == Errno::EXIST {
                FileWriteError::AmbiguousResult
            } else {
                map_resolution_error(error)
            }
        })?;
        let mut temporary = File::from(temporary_fd);
        temporary
            .write_all(new_content.as_bytes())
            .map_err(|_| FileWriteError::IoUnavailable)?;

        fchown(
            &temporary,
            Some(Uid::from_raw(target_metadata.uid())),
            Some(Gid::from_raw(target_metadata.gid())),
        )
        .map_err(|_| FileWriteError::UnsupportedMetadata)?;
        // New files may inherit a default ACL from their parent. Remove that inherited ACL,
        // then copy the exact access ACL and user metadata from the approved target.
        clear_access_acl(&temporary)?;
        for (name, value) in &copied_xattrs {
            fsetxattr(
                &temporary,
                name.as_str(),
                value.as_slice(),
                XattrFlags::empty(),
            )
            .map_err(|_| FileWriteError::UnsupportedMetadata)?;
        }
        fchmod(
            &temporary,
            Mode::from_bits_retain(target_metadata.mode() & 0o7777),
        )
        .map_err(|_| FileWriteError::UnsupportedMetadata)?;
        temporary
            .sync_all()
            .map_err(|_| FileWriteError::IoUnavailable)?;
        after_staging();

        // This catches changes made while staging. LiteCowork lease fencing serializes
        // mediated writers; out-of-process writers do not participate in that fence, so
        // this check is not a filesystem compare-and-swap against a concurrent external edit.
        let latest = open_regular_file(parent, leaf)?;
        if digest_file(&latest)? != *base {
            return Err(FileWriteError::AmbiguousResult);
        }

        if rustix::fs::renameat(parent, temporary_leaf.as_str(), parent, leaf).is_err() {
            // A failed or interrupted rename is reconciled; never guess or automatically retry.
            return Err(FileWriteError::AmbiguousResult);
        }
        if fsync(parent).is_err() {
            return Err(FileWriteError::AmbiguousResult);
        }

        Ok(ReplaceOutcome {
            content_digest: digest(new_content.as_bytes()),
            bytes_written: new_content.as_bytes().len() as u32,
        })
    }

    fn open_regular_file(parent: &impl AsFd, leaf: &str) -> Result<File, FileWriteError> {
        let fd = openat2(
            parent,
            leaf,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
            RESOLVE,
        )
        .map_err(map_resolution_error)?;
        let file = File::from(fd);
        let metadata = file.metadata().map_err(|_| FileWriteError::IoUnavailable)?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(FileWriteError::PathEscape);
        }
        Ok(file)
    }

    fn digest_file(file: &File) -> Result<ContentDigest, FileWriteError> {
        let mut reader = file
            .try_clone()
            .map_err(|_| FileWriteError::IoUnavailable)?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| FileWriteError::IoUnavailable)?;
        let mut hasher = sha2::Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|_| FileWriteError::IoUnavailable)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(ContentDigest::from_sha256_bytes(hasher.finalize().into()))
    }

    fn copyable_xattrs(file: &File) -> Result<Vec<(String, Vec<u8>)>, FileWriteError> {
        let mut name_buffer = vec![0u8; MAX_XATTR_LIST_BYTES];
        let name_bytes = flistxattr(file, &mut name_buffer[..])
            .map_err(|_| FileWriteError::UnsupportedMetadata)?;
        let mut copied = Vec::new();
        let mut copied_bytes = 0usize;
        for raw_name in name_buffer[..name_bytes]
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let name =
                std::str::from_utf8(raw_name).map_err(|_| FileWriteError::UnsupportedMetadata)?;
            if is_supported_xattr(name) {
                let mut value = vec![0u8; MAX_COPIED_XATTR_BYTES];
                let value_bytes = rustix::fs::fgetxattr(file, name, &mut value[..])
                    .map_err(|_| FileWriteError::UnsupportedMetadata)?;
                value.truncate(value_bytes);
                copied_bytes = copied_bytes
                    .checked_add(name.len() + value.len())
                    .filter(|total| *total <= MAX_COPIED_XATTR_BYTES)
                    .ok_or(FileWriteError::UnsupportedMetadata)?;
                copied.push((name.to_owned(), value));
            } else {
                // Security labels, capabilities, trusted attributes and unknown system
                // metadata need an owner-defined preservation policy before replacement.
                return Err(FileWriteError::UnsupportedMetadata);
            }
        }
        Ok(copied)
    }

    fn is_supported_xattr(name: &str) -> bool {
        name == ACCESS_ACL_XATTR || name.starts_with("user.")
    }

    #[cfg(test)]
    pub(super) fn test_supported_xattr(name: &str) -> bool {
        is_supported_xattr(name)
    }

    fn clear_access_acl(file: &File) -> Result<(), FileWriteError> {
        match fremovexattr(file, ACCESS_ACL_XATTR) {
            Ok(()) | Err(Errno::NODATA) => Ok(()),
            Err(_) => Err(FileWriteError::UnsupportedMetadata),
        }
    }

    fn map_resolution_error(error: Errno) -> FileWriteError {
        if matches!(
            error,
            Errno::LOOP | Errno::XDEV | Errno::NOTDIR | Errno::NOENT
        ) {
            FileWriteError::PathEscape
        } else if matches!(error, Errno::NOSYS | Errno::INVAL | Errno::NOTSUP) {
            FileWriteError::IoUnavailable
        } else {
            FileWriteError::IoUnavailable
        }
    }
}

#[cfg(windows)]
mod windows {
    use super::{
        BOUNDED_FILE_REPLACEMENT_MAX_BYTES, BoundedBlob, ContentDigest, FileWriteError,
        ReplaceOutcome, RootHandle, WorkspaceConfinedPath, digest, effect_temp_leaf,
    };
    use sha2::Digest as _;
    use std::{
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
        path::Path,
    };

    pub(super) fn replace(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
    ) -> Result<ReplaceOutcome, FileWriteError> {
        replace_with_hook(target, base, new_content, || {})
    }

    #[cfg(test)]
    pub(super) fn replace_with_hook(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
        after_staging: impl FnOnce(),
    ) -> Result<ReplaceOutcome, FileWriteError> {
        replace_inner(target, base, new_content, after_staging)
    }

    #[cfg(not(test))]
    fn replace_with_hook(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
        after_staging: impl FnOnce(),
    ) -> Result<ReplaceOutcome, FileWriteError> {
        replace_inner(target, base, new_content, after_staging)
    }

    fn replace_inner(
        target: &WorkspaceConfinedPath,
        base: &ContentDigest,
        new_content: &BoundedBlob,
        after_staging: impl FnOnce(),
    ) -> Result<ReplaceOutcome, FileWriteError> {
        let RootHandle::Windows(root) = &target.root.0;
        if new_content.as_bytes().len() > BOUNDED_FILE_REPLACEMENT_MAX_BYTES {
            return Err(FileWriteError::OverBound);
        }
        if !root
            .supports_durable_handle_relative_replace()
            .map_err(map_io_error)?
        {
            return Err(FileWriteError::IoUnavailable);
        }

        let relative_path = Path::new(&target.relative_path);
        let target_file = root
            .open_regular_file_beneath(relative_path)
            .map_err(map_io_error)?;
        if digest_file(&target_file)? != *base {
            return Err(FileWriteError::IntegrityFailure);
        }

        let parent = relative_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty());
        let temporary_path = parent.map_or_else(
            || Path::new(&effect_temp_leaf(&target.effect_id)).to_path_buf(),
            |parent| parent.join(effect_temp_leaf(&target.effect_id)),
        );
        let mut temporary = root
            .create_new_replacement_file_beneath(&temporary_path)
            .map_err(map_io_error)?;
        root.copy_replacement_metadata(&target_file, &temporary)
            .map_err(|_| FileWriteError::UnsupportedMetadata)?;
        temporary
            .write_all(new_content.as_bytes())
            .map_err(|_| FileWriteError::IoUnavailable)?;
        temporary
            .sync_all()
            .map_err(|_| FileWriteError::IoUnavailable)?;
        after_staging();

        // LiteCowork's lease fences serialize mediated writers. Out-of-process file editors
        // are not fenced; this recheck catches changes made while staging but is not a
        // filesystem compare-and-swap against a concurrent external writer.
        let latest = root
            .open_regular_file_beneath(relative_path)
            .map_err(|_| FileWriteError::AmbiguousResult)?;
        if digest_file(&latest)? != *base {
            return Err(FileWriteError::AmbiguousResult);
        }
        // Windows rejects replacement while the old file still has any open handle. Close
        // both Runtime-owned read handles after the final digest check and before the rename.
        // External writers remain outside LiteCowork's lease fence; this is not a filesystem
        // compare-and-swap and a racing external writer still requires reconciliation.
        drop(latest);
        drop(target_file);

        if root
            .replace_open_file_beneath(&temporary, relative_path)
            .is_err()
        {
            return Err(FileWriteError::AmbiguousResult);
        }
        Ok(ReplaceOutcome {
            content_digest: digest(new_content.as_bytes()),
            bytes_written: new_content.as_bytes().len() as u32,
        })
    }

    fn digest_file(file: &File) -> Result<ContentDigest, FileWriteError> {
        let mut reader = file
            .try_clone()
            .map_err(|_| FileWriteError::IoUnavailable)?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| FileWriteError::IoUnavailable)?;
        let mut hasher = sha2::Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|_| FileWriteError::IoUnavailable)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(ContentDigest::from_sha256_bytes(hasher.finalize().into()))
    }

    fn map_io_error(error: std::io::Error) -> FileWriteError {
        match error.kind() {
            std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::InvalidData
            | std::io::ErrorKind::NotFound => FileWriteError::PathEscape,
            std::io::ErrorKind::Unsupported => FileWriteError::UnsupportedMetadata,
            std::io::ErrorKind::AlreadyExists => FileWriteError::AmbiguousResult,
            _ => FileWriteError::IoUnavailable,
        }
    }
}

fn validate_relative_path(path: &str) -> Result<(), FileWriteError> {
    if path.is_empty()
        || path.len() > 4096
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(FileWriteError::PathEscape);
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> ContentDigest {
    ContentDigest::from_sha256_bytes(Sha256::digest(bytes).into())
}

fn effect_temp_leaf(effect_id: &str) -> String {
    let digest = Sha256::digest(effect_id.as_bytes());
    format!(".litecowork-{}.tmp", hex::encode(digest))
}

#[cfg(test)]
fn test_target(root: &Path, relative_path: &str, effect_id: &str) -> WorkspaceConfinedPath {
    WorkspaceConfinedPath::new(
        WorkspaceRootHandle::open_test_root(root).expect("open test root"),
        relative_path,
        effect_id,
    )
    .expect("construct confined test target")
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", windows))]
    use super::effect_temp_leaf;
    use super::{
        BoundedBlob, ContentDigest, FileWriteError, digest, replace_approved_file, test_target,
    };
    use std::{fs, path::Path};
    use tempfile::TempDir;

    #[test]
    fn replaces_one_file_and_returns_the_digest_of_the_approved_bytes() {
        let workspace = TempDir::new().expect("create test workspace");
        fs::write(workspace.path().join("answer.txt"), b"before").expect("write base file");
        let target = test_target(workspace.path(), "answer.txt", "effect-1");
        let content = BoundedBlob::new(b"approved replacement".to_vec()).unwrap();

        let outcome = replace_approved_file(&target, &digest(b"before"), &content)
            .expect("replace the approved file");

        assert_eq!(outcome.content_digest, digest(b"approved replacement"));
        assert_eq!(outcome.bytes_written, b"approved replacement".len() as u32);
        assert_eq!(
            fs::read(workspace.path().join("answer.txt")).unwrap(),
            b"approved replacement"
        );
    }

    #[test]
    fn digest_wire_form_is_exact_sha256_hex() {
        let parsed = ContentDigest::parse(&digest(b"known").to_wire()).unwrap();
        assert_eq!(parsed, digest(b"known"));
    }

    #[test]
    fn bounded_blob_rejects_one_byte_over_the_limit() {
        assert_eq!(
            BoundedBlob::new(vec![0; 1024 * 1024 + 1]).unwrap_err(),
            FileWriteError::OverBound
        );
    }

    #[test]
    fn root_relative_path_validation_rejects_absolute_and_traversal_forms() {
        for path in [
            "",
            "/absolute.txt",
            "C:/absolute.txt",
            "../outside.txt",
            "nested/../../outside.txt",
            "nested//empty.txt",
            "nested/./dot.txt",
            "nested\\windows-separator.txt",
        ] {
            assert_eq!(
                super::validate_relative_path(path),
                Err(FileWriteError::PathEscape),
                "accepted unsafe path {path:?}"
            );
        }
        assert!(super::validate_relative_path("nested/answer.txt").is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unsupported_xattr_names_fail_the_preservation_policy() {
        assert!(super::linux::test_supported_xattr("user.note"));
        assert!(super::linux::test_supported_xattr(
            "system.posix_acl_access"
        ));
        assert!(!super::linux::test_supported_xattr("security.selinux"));
        assert!(!super::linux::test_supported_xattr("trusted.example"));
        assert!(!super::linux::test_supported_xattr("system.unknown"));
    }

    #[test]
    fn stale_base_refuses_without_changing_target_or_creating_a_temp() {
        let workspace = TempDir::new().expect("create test workspace");
        fs::write(workspace.path().join("answer.txt"), b"current").expect("write current file");
        let target = test_target(workspace.path(), "answer.txt", "effect-stale");
        let content = BoundedBlob::new(b"proposed".to_vec()).unwrap();

        assert_eq!(
            replace_approved_file(&target, &digest(b"old base"), &content),
            Err(FileWriteError::IntegrityFailure)
        );
        assert_eq!(
            fs::read(workspace.path().join("answer.txt")).unwrap(),
            b"current"
        );
        assert_no_effect_temp(workspace.path());
    }

    #[cfg(windows)]
    #[test]
    fn missing_handle_relative_replace_capability_refuses_before_staging() {
        use std::path::PathBuf;

        let workspace = TempDir::new().expect("create test workspace");
        let parent = windows_local_security::open_directory_path_without_reparse(workspace.path())
            .expect("open safe parent");
        let nested_path = PathBuf::from("nested");
        let nested = parent
            .create_new_directory(&nested_path)
            .expect("create a child directory with traversal-only handle");
        let target_path = workspace.path().join("nested").join("answer.txt");
        fs::write(&target_path, b"before").expect("write base file");

        let root = super::WorkspaceRootHandle::from_runtime_verified_windows_directory(nested);
        let target = super::WorkspaceConfinedPath::new(root, "answer.txt", "effect-no-capability")
            .expect("create confined target");
        let content = BoundedBlob::new(b"approved".to_vec()).unwrap();

        assert_eq!(
            replace_approved_file(&target, &digest(b"before"), &content),
            Err(FileWriteError::IoUnavailable)
        );
        assert_eq!(fs::read(&target_path).unwrap(), b"before");
        assert_no_effect_temp(&target_path.parent().unwrap());
    }

    #[test]
    fn hard_linked_target_is_refused_before_staging() {
        let workspace = TempDir::new().expect("create test workspace");
        fs::write(workspace.path().join("answer.txt"), b"before").expect("write base file");
        fs::hard_link(
            workspace.path().join("answer.txt"),
            workspace.path().join("alias.txt"),
        )
        .expect("create second hard link");
        let target = test_target(workspace.path(), "answer.txt", "effect-hardlink");
        let content = BoundedBlob::new(b"approved".to_vec()).unwrap();

        assert_eq!(
            replace_approved_file(&target, &digest(b"before"), &content),
            Err(FileWriteError::PathEscape)
        );
        assert_eq!(
            fs::read(workspace.path().join("alias.txt")).unwrap(),
            b"before"
        );
        assert_no_effect_temp(workspace.path());
    }

    #[cfg(windows)]
    #[test]
    fn reparse_component_is_refused_without_writing_outside_the_root() {
        use std::process::Command;

        let workspace = TempDir::new().expect("create test workspace");
        let outside = TempDir::new().expect("create outside directory");
        fs::write(outside.path().join("answer.txt"), b"outside").expect("write outside target");
        let junction = workspace.path().join("escape");
        let result = Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(outside.path())
            .output()
            .expect("create Windows junction");
        assert!(result.status.success(), "junction setup failed");
        let target = test_target(workspace.path(), "escape/answer.txt", "effect-junction");
        let content = BoundedBlob::new(b"approved".to_vec()).unwrap();

        assert_eq!(
            replace_approved_file(&target, &digest(b"outside"), &content),
            Err(FileWriteError::PathEscape)
        );
        assert_eq!(
            fs::read(outside.path().join("answer.txt")).unwrap(),
            b"outside"
        );
        assert_no_effect_temp(workspace.path());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_component_is_refused_without_writing_outside_the_root() {
        use std::os::unix::fs::symlink;

        let workspace = TempDir::new().expect("create test workspace");
        let outside = TempDir::new().expect("create outside directory");
        fs::write(outside.path().join("answer.txt"), b"outside").expect("write outside target");
        symlink(outside.path(), workspace.path().join("escape")).expect("create symlink");
        let target = test_target(workspace.path(), "escape/answer.txt", "effect-symlink");
        let content = BoundedBlob::new(b"approved".to_vec()).unwrap();

        assert_eq!(
            replace_approved_file(&target, &digest(b"outside"), &content),
            Err(FileWriteError::PathEscape)
        );
        assert_eq!(
            fs::read(outside.path().join("answer.txt")).unwrap(),
            b"outside"
        );
        assert_no_effect_temp(workspace.path());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn replacement_preserves_posix_mode_and_access_acl_when_supported() {
        use rustix::{
            fs::{XattrFlags, fgetxattr, fsetxattr},
            io::Errno,
        };
        use std::{
            fs::OpenOptions,
            os::unix::fs::{MetadataExt, PermissionsExt},
        };

        let workspace = TempDir::new().expect("create test workspace");
        let file_path = workspace.path().join("answer.txt");
        fs::write(&file_path, b"before").expect("write base file");
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o640))
            .expect("set target mode");
        let original_metadata = fs::metadata(&file_path).expect("read base metadata");
        let expected_uid = original_metadata.uid();
        let expected_gid = original_metadata.gid();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .expect("open target for ACL setup");
        let user_xattr_supported = match fsetxattr(
            &file,
            "user.litecowork.test",
            b"preserved user metadata",
            XattrFlags::empty(),
        ) {
            Ok(()) => true,
            Err(error) if error == Errno::NOTSUP || error == Errno::OPNOTSUPP => false,
            Err(error) => panic!("unexpected failure setting test user xattr: {error}"),
        };
        // POSIX ACL v2: user::rw-, group::r--, mask::r--, other::---.
        let acl = [
            0x02, 0x00, 0x01, 0x00, 0x06, 0x00, 0xff, 0xff, 0xff, 0xff, 0x04, 0x00, 0x04, 0x00,
            0xff, 0xff, 0xff, 0xff, 0x10, 0x00, 0x04, 0x00, 0xff, 0xff, 0xff, 0xff, 0x20, 0x00,
            0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
        ];
        let acl_supported =
            match fsetxattr(&file, "system.posix_acl_access", &acl, XattrFlags::empty()) {
                Ok(()) => true,
                Err(error) if error == Errno::NOTSUP || error == Errno::OPNOTSUPP => false,
                Err(error) => panic!("unexpected failure setting test ACL: {error}"),
            };
        if std::env::var_os("LITECOWORK_REQUIRE_ACL_XATTR_TEST").is_some() {
            assert!(
                acl_supported,
                "the selected test filesystem did not support POSIX access ACL xattrs"
            );
        }
        drop(file);
        let target = test_target(workspace.path(), "answer.txt", "effect-acl");
        let content = BoundedBlob::new(b"approved".to_vec()).unwrap();

        replace_approved_file(&target, &digest(b"before"), &content)
            .expect("replace file while preserving access metadata");

        let metadata = fs::metadata(&file_path).expect("read replacement metadata");
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o640);
        assert_eq!(metadata.uid(), expected_uid);
        assert_eq!(metadata.gid(), expected_gid);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .expect("open replacement for ACL verification");
        if acl_supported {
            let mut actual_acl = vec![0u8; 128];
            let acl_bytes = fgetxattr(&file, "system.posix_acl_access", &mut actual_acl[..])
                .expect("read replacement ACL");
            actual_acl.truncate(acl_bytes);
            assert_eq!(actual_acl, acl);
        }
        if user_xattr_supported {
            let mut actual_user_xattr = vec![0u8; 64];
            let actual_bytes = fgetxattr(&file, "user.litecowork.test", &mut actual_user_xattr[..])
                .expect("read replacement user xattr");
            actual_user_xattr.truncate(actual_bytes);
            assert_eq!(actual_user_xattr, b"preserved user metadata");
        }
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn process_kill_after_temp_flush_leaves_base_and_effect_temp_for_recovery() {
        use std::{
            process::{Command, Stdio},
            thread,
            time::{Duration, Instant},
        };

        const CHILD_TEST: &str =
            "file_replace::tests::crash_child_pauses_after_temp_flush_for_parent_kill";
        let workspace = TempDir::new().expect("create crash-test workspace");
        fs::write(workspace.path().join("answer.txt"), b"before").expect("write base file");
        let marker = workspace.path().join("staging-ready");
        let mut child = Command::new(std::env::current_exe().expect("test executable path"))
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env("LITECOWORK_REPLACE_CRASH_ROOT", workspace.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn crash-test child");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            if let Some(status) = child.try_wait().expect("check child") {
                panic!("crash-test child exited before staging: {status}");
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !marker.exists() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not report durable temp staging before deadline");
        }

        child.kill().expect("kill child between staging and rename");
        child.wait().expect("reap killed child");

        assert_eq!(
            fs::read(workspace.path().join("answer.txt")).unwrap(),
            b"before"
        );
        assert_eq!(
            fs::read(workspace.path().join(effect_temp_leaf("effect-crash"))).unwrap(),
            b"approved replacement"
        );
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn crash_child_pauses_after_temp_flush_for_parent_kill() {
        use std::{path::PathBuf, thread, time::Duration};

        let Ok(root) = std::env::var("LITECOWORK_REPLACE_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let target = test_target(&root, "answer.txt", "effect-crash");
        let content = BoundedBlob::new(b"approved replacement".to_vec()).unwrap();
        let after_staging = || {
            fs::write(root.join("staging-ready"), b"ready").expect("signal parent");
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        };
        #[cfg(target_os = "linux")]
        let result =
            super::linux::replace_with_hook(&target, &digest(b"before"), &content, after_staging);
        #[cfg(windows)]
        let result =
            super::windows::replace_with_hook(&target, &digest(b"before"), &content, after_staging);
        panic!("crash-test child returned before parent kill: {result:?}");
    }

    fn assert_no_effect_temp(directory: &Path) {
        let entries = fs::read_dir(directory).expect("list test workspace");
        assert!(
            entries.filter_map(Result::ok).all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".litecowork-")),
            "an Effect-keyed temp remained after a pre-staging refusal"
        );
    }
}
