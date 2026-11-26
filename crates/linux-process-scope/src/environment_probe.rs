//! Trusted, pre-admission observations made from inside the exact held Bubblewrap scope.
//!
//! The live probe reads the kernel-reported tmpfs capacity and checks the configured
//! writable aliases without filling the mount. Forcing `ENOSPC` by allocating an
//! Environment's entire scratch cap can exhaust that Attempt's memory limit; actual
//! exhaustion behavior belongs in a disposable, deliberately small integration scope.

use rustix::{
    fs::{AtFlags, Mode, OFlags, fstatfs, open, openat, unlinkat},
    io::Errno,
};
use std::{fs::File, os::unix::fs::MetadataExt};

const TMPFS_MAGIC: i64 = 0x0102_1994;
const ROOT_WRITE_PROBE: &str = ".litecowork-root-write-probe";
const INPUT_WRITE_PROBE: &str = ".litecowork-input-write-probe";
const OUTPUT_WRITE_PROBE: &str = ".litecowork-output-write-probe";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceProbeError {
    OpenFailed,
    NotTmpfs,
    InvalidFilesystemStats,
    CapacityMismatch,
    WritableMountMismatch,
    RootWriteNotDenied,
    InputWriteNotDenied,
    ScratchWriteFailed,
}

/// Check the actual `/workspace` filesystem and its writable aliases before the trusted
/// runner announces readiness or starts any native harness. No user process exists yet.
pub fn verify_workspace_tmpfs(expected_capacity_bytes: u64) -> Result<(), WorkspaceProbeError> {
    let root = open_directory("/")?;
    let workspace = open_directory_at(&root, "workspace")?;
    let workspace_metadata = workspace
        .metadata()
        .map_err(|_| WorkspaceProbeError::OpenFailed)?;
    let workspace_fs = fstatfs(&workspace).map_err(|_| WorkspaceProbeError::OpenFailed)?;
    validate_tmpfs_capacity(
        workspace_fs.f_type as i64,
        workspace_fs.f_bsize,
        workspace_fs.f_blocks,
        expected_capacity_bytes,
    )?;

    let outputs = open_directory_at(&workspace, "outputs")?;
    let home = open_directory_at(&outputs, "home")?;
    let scratch = open_directory_at(&workspace, "tmp")?;
    // `/tmp` intentionally resolves through the in-sandbox symlink to `/workspace/tmp`.
    let temporary = open_directory("/tmp")?;

    for directory in [&outputs, &home, &scratch, &temporary] {
        let metadata = directory
            .metadata()
            .map_err(|_| WorkspaceProbeError::OpenFailed)?;
        let filesystem = fstatfs(directory).map_err(|_| WorkspaceProbeError::OpenFailed)?;
        if metadata.dev() != workspace_metadata.dev() || filesystem.f_type as i64 != TMPFS_MAGIC {
            return Err(WorkspaceProbeError::WritableMountMismatch);
        }
    }

    // A write attempt against the immutable root and staged-input bind must be rejected
    // specifically by the read-only mount. EACCES or a missing path is not treated as
    // proof of a read-only filesystem.
    require_readonly_create(&root, ROOT_WRITE_PROBE)
        .map_err(|_| WorkspaceProbeError::RootWriteNotDenied)?;
    let inputs = open_directory_at(&workspace, "inputs")?;
    require_readonly_create(&inputs, INPUT_WRITE_PROBE)
        .map_err(|_| WorkspaceProbeError::InputWriteNotDenied)?;

    // Prove each writable directory and alias accepts a small transient file. The probe
    // never allocates in proportion to the Environment's configured storage limit.
    for directory in [&workspace, &outputs, &home, &scratch, &temporary] {
        create_and_remove_probe(directory, OUTPUT_WRITE_PROBE)?;
    }

    Ok(())
}

fn validate_tmpfs_capacity(
    filesystem_type: i64,
    block_size: i64,
    blocks: u64,
    expected_capacity_bytes: u64,
) -> Result<u64, WorkspaceProbeError> {
    if filesystem_type != TMPFS_MAGIC {
        return Err(WorkspaceProbeError::NotTmpfs);
    }
    let block_size = u64::try_from(block_size)
        .ok()
        .filter(|size| *size > 0)
        .ok_or(WorkspaceProbeError::InvalidFilesystemStats)?;
    let capacity = block_size
        .checked_mul(blocks)
        .filter(|capacity| *capacity > 0)
        .ok_or(WorkspaceProbeError::InvalidFilesystemStats)?;
    if capacity != expected_capacity_bytes {
        return Err(WorkspaceProbeError::CapacityMismatch);
    }
    Ok(capacity)
}

fn open_directory(path: &str) -> Result<File, WorkspaceProbeError> {
    open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| WorkspaceProbeError::OpenFailed)
}

fn open_directory_at(parent: &File, path: &str) -> Result<File, WorkspaceProbeError> {
    openat(
        parent,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| WorkspaceProbeError::OpenFailed)
}

fn require_readonly_create(parent: &File, name: &str) -> Result<(), Errno> {
    match openat(
        parent,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
        Mode::from_bits_retain(0o600),
    ) {
        Err(error) if error == Errno::ROFS => Ok(()),
        Ok(file) => {
            // An unexpected writable root must fail closed without leaving a probe file
            // behind in the very filesystem whose protection just failed.
            drop(file);
            let _ = unlinkat(parent, name, AtFlags::empty());
            Err(Errno::PERM)
        }
        Err(error) => Err(error),
    }
}

fn create_and_remove_probe(parent: &File, name: &str) -> Result<(), WorkspaceProbeError> {
    let file = openat(
        parent,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
        Mode::from_bits_retain(0o600),
    )
    .map(File::from)
    .map_err(|_| WorkspaceProbeError::ScratchWriteFailed)?;
    drop(file);
    unlinkat(parent, name, AtFlags::empty()).map_err(|_| WorkspaceProbeError::ScratchWriteFailed)
}

#[cfg(test)]
mod tests {
    use super::{
        ROOT_WRITE_PROBE, TMPFS_MAGIC, WorkspaceProbeError, require_readonly_create,
        validate_tmpfs_capacity,
    };

    #[test]
    fn accepts_only_exact_kernel_reported_tmpfs_capacity() {
        assert_eq!(
            validate_tmpfs_capacity(TMPFS_MAGIC, 4096, 4, 16 * 1024),
            Ok(16 * 1024)
        );
        assert_eq!(
            validate_tmpfs_capacity(TMPFS_MAGIC, 4096, 4, 12 * 1024),
            Err(WorkspaceProbeError::CapacityMismatch)
        );
        assert_eq!(
            validate_tmpfs_capacity(0, 4096, 4, 16 * 1024),
            Err(WorkspaceProbeError::NotTmpfs)
        );
        assert_eq!(
            validate_tmpfs_capacity(TMPFS_MAGIC, 0, 4, 16 * 1024),
            Err(WorkspaceProbeError::InvalidFilesystemStats)
        );
        assert_eq!(
            validate_tmpfs_capacity(TMPFS_MAGIC, 4096, u64::MAX, 16 * 1024),
            Err(WorkspaceProbeError::InvalidFilesystemStats)
        );
    }

    #[test]
    fn unexpected_writable_readonly_probe_is_removed_before_failure() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let handle = std::fs::File::open(directory.path()).expect("open temporary directory");

        assert_eq!(
            require_readonly_create(&handle, ROOT_WRITE_PROBE),
            Err(rustix::io::Errno::PERM)
        );
        assert!(!directory.path().join(ROOT_WRITE_PROBE).exists());
    }
}
