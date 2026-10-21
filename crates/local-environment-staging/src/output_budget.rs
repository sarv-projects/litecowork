//! Bounded accounting for the staged writable-output root.
//!
//! The staged output root is the only writable surface a Task-scoped worker receives.
//! Its size is therefore part of the Environment's resource limits, and the Runtime
//! must be able to observe the consumed bytes before it releases a writer fence or
//! crosses a writable root into a new Attempt.
//!
//! Nothing in this module is an OS containment mechanism. A hostile same-user process
//! can still write into the output root between two observations, so the numbers here
//! are *observations*, not enforcement. Enforcement of an aggregate quota requires a
//! qualified provider (for example a size-bounded filesystem or a cgroup-backed
//! `storage` limit) and is reported separately through
//! [`crate::EnforcementStatus`].

use crate::StagingError;
use std::fs;
use std::path::Path;

/// Observed enforcement of one isolation dimension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnforcementStatus {
    /// The provider demonstrated the restriction and the Runtime observed it.
    Enforced,
    /// The provider demonstrated that the restriction is not in place.
    NotEnforced,
    /// Support could not be established. This never means "probably fine".
    Unknown,
}

impl EnforcementStatus {
    /// Only a positive observation may satisfy an admission requirement.
    pub fn is_enforced(self) -> bool {
        matches!(self, Self::Enforced)
    }
}

/// Host-enforced file-root mode requested for a consumer-visible root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileRootMode {
    /// The root may be read but not modified.
    ReadOnly,
    /// The root may be read and modified, including removal of its entries.
    ReadWrite,
    /// The root may be read and modified but existing entries may not be removed.
    ReadWriteNoDelete,
}

/// Whether the current host can actually enforce a requested file-root mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootModeSupport {
    /// The mode is enforced by the platform mechanism used here.
    Enforced,
    /// The platform mechanism used here cannot enforce this mode.
    NotEnforced,
    /// The platform is not qualified for any root-mode answer.
    Unknown,
}

impl RootModeSupport {
    pub fn is_enforced(self) -> bool {
        matches!(self, Self::Enforced)
    }
}

/// Report whether the requested file-root mode is enforceable on this host.
///
/// A root that permits creating new entries must also permit removing them on both
/// POSIX and Windows, because deletion permission is a property of the *parent*
/// directory rather than of the entry itself. LiteCowork has not qualified an
/// ACL-bearing or otherwise managed provider that could separate the two, so
/// [`FileRootMode::ReadWriteNoDelete`] is reported as `NotEnforced` instead of being
/// simulated with a weaker mode. A caller that requires it must fail closed or request
/// a qualified provider.
pub fn probe_root_mode_support(mode: FileRootMode) -> RootModeSupport {
    match mode {
        FileRootMode::ReadOnly => {
            if cfg!(any(unix, windows)) {
                RootModeSupport::Enforced
            } else {
                RootModeSupport::Unknown
            }
        }
        FileRootMode::ReadWrite => {
            if cfg!(any(unix, windows)) {
                RootModeSupport::Enforced
            } else {
                RootModeSupport::Unknown
            }
        }
        FileRootMode::ReadWriteNoDelete => {
            if cfg!(any(unix, windows)) {
                RootModeSupport::NotEnforced
            } else {
                RootModeSupport::Unknown
            }
        }
    }
}

/// Aggregate bound for the staged writable-output root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputQuota {
    pub max_total_bytes: u64,
    pub max_file_count: usize,
}

impl OutputQuota {
    /// A quota that records usage without ever rejecting it.
    pub fn unbounded() -> Self {
        Self {
            max_total_bytes: u64::MAX,
            max_file_count: usize::MAX,
        }
    }

    pub fn is_bounded(&self) -> bool {
        self.max_total_bytes != u64::MAX || self.max_file_count != usize::MAX
    }
}

/// Bytes and entries observed inside the staged writable-output root.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputUsage {
    pub total_bytes: u64,
    pub file_count: usize,
}

/// Bound on the number of directory entries a single usage observation may walk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputScanLimits {
    pub max_entries: usize,
}

impl OutputScanLimits {
    pub fn new(max_entries: usize) -> Self {
        Self { max_entries }
    }
}

/// Observe the current size of the staged output root.
///
/// The walk refuses symlinks and non-regular files instead of following or ignoring
/// them: a symlinked entry could point outside the output root, and silently skipping
/// it would under-report the bytes an escaped writer produced.
pub fn measure_output_root(
    output_root: &Path,
    limits: OutputScanLimits,
) -> Result<OutputUsage, StagingError> {
    if limits.max_entries == 0 {
        return Err(StagingError::OutputScanLimitExceeded);
    }
    let metadata = fs::symlink_metadata(output_root).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StagingError::InvalidParent);
    }
    let mut usage = OutputUsage::default();
    let mut pending = vec![output_root.to_path_buf()];
    let mut visited = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            visited += 1;
            if visited > limits.max_entries {
                return Err(StagingError::OutputScanLimitExceeded);
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
            if metadata.file_type().is_symlink() {
                return Err(StagingError::OutputRootSymlink(path.display().to_string()));
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(StagingError::OutputRootUnsupportedFile(
                    path.display().to_string(),
                ));
            }
            usage.total_bytes = usage
                .total_bytes
                .checked_add(metadata.len())
                .ok_or(StagingError::OutputQuotaExceeded {
                    limit_bytes: u64::MAX,
                    observed_bytes: u64::MAX,
                    limit_files: usize::MAX,
                    observed_files: usage.file_count.saturating_add(1),
                })?;
            usage.file_count = usage.file_count.saturating_add(1);
        }
    }
    Ok(usage)
}

/// Compare an observed usage against a quota.
pub fn check_output_quota(
    quota: &OutputQuota,
    usage: OutputUsage,
) -> Result<OutputUsage, StagingError> {
    if usage.total_bytes > quota.max_total_bytes || usage.file_count > quota.max_file_count {
        return Err(StagingError::OutputQuotaExceeded {
            limit_bytes: quota.max_total_bytes,
            observed_bytes: usage.total_bytes,
            limit_files: quota.max_file_count,
            observed_files: usage.file_count,
        });
    }
    Ok(usage)
}

fn io_error(error: std::io::Error) -> StagingError {
    if error.kind() == std::io::ErrorKind::Unsupported {
        StagingError::UnsupportedPlatform
    } else {
        StagingError::Io(error.to_string())
    }
}
