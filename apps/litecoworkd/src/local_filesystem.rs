//! Native local filesystem primitives used by WorkspaceRoot providers.
//!
//! The current Tauri-only selection operation calls this module after a native folder
//! chooser, and the private IPC path never enters WebView state. The selected path remains
//! untrusted input; the returned handle and identity are required before a root is stored.
//! A path string alone is never a root grant.

use std::fmt;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::Component;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
use storage_core::LocalFileIdentityBindingRecord;
use storage_sqlite::RuntimeOsPrincipalIdentity;

pub(crate) trait RuntimeIdentityKey {
    fn keyed_pseudonym(&self, namespace: &str, material: &[u8]) -> String;
}

impl RuntimeIdentityKey for RuntimeOsPrincipalIdentity {
    fn keyed_pseudonym(&self, namespace: &str, material: &[u8]) -> String {
        RuntimeOsPrincipalIdentity::keyed_pseudonym(self, namespace, material)
    }
}

impl<T: RuntimeIdentityKey + ?Sized> RuntimeIdentityKey for std::sync::Arc<T> {
    fn keyed_pseudonym(&self, namespace: &str, material: &[u8]) -> String {
        self.as_ref().keyed_pseudonym(namespace, material)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_SELECTION_PATH_BYTES: usize = 32 * 1024;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const DIRECTORY_TYPE_MASK: u32 = 0o170000;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const DIRECTORY_TYPE: u32 = 0o040000;
#[cfg(target_os = "linux")]
// The compatibility collector retains one O_PATH descriptor per file and remains capped.
const MAX_RETAINED_SCAN_ENTRIES: usize = 64;
#[cfg(target_os = "linux")]
// Streaming traversal keeps one directory stream and one directory handle per level.
// A hard depth limit bounds both descriptor use and recursive stack depth.
const MAX_SCAN_DEPTH: usize = 64;
#[cfg(target_os = "linux")]
// Server-owned ceiling for streaming callers. Individual routes may choose lower limits,
// but no caller can turn a metadata traversal into an unbounded directory walk.
const MAX_SCAN_ENTRIES: usize = 10_000;
#[cfg(target_os = "linux")]
const MAX_SCAN_LOCATOR_BYTES: usize = 16 * 1024 * 1024;

// Windows entries marked as reparse points (symlinks, junctions, mount points, and other
// providers) are never eligible for traversal. This predicate is intentionally separate
// from a Windows scanner: traversal remains unsupported until the daemon has a qualified
// handle-relative open API that can enforce this check without a path-replacement race.
#[cfg(any(target_os = "windows", test))]
const WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

#[cfg(any(target_os = "windows", test))]
fn windows_directory_entry_is_traversable(file_attributes: u32) -> bool {
    const DIRECTORY: u32 = 0x0000_0010;
    file_attributes & DIRECTORY != 0 && file_attributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT == 0
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum NativeDirectoryIdentity {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    Unix {
        filesystem_instance_id: u64,
        file_id: u64,
        platform_kind: &'static str,
    },
    #[cfg(target_os = "windows")]
    Windows {
        volume_serial_number: u64,
        file_id: [u8; 16],
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RawDirectoryIdentity {
    native: NativeDirectoryIdentity,
}

impl RawDirectoryIdentity {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    const fn unix(filesystem_instance_id: u64, file_id: u64, platform_kind: &'static str) -> Self {
        Self {
            native: NativeDirectoryIdentity::Unix {
                filesystem_instance_id,
                file_id,
                platform_kind,
            },
        }
    }

    #[cfg(target_os = "windows")]
    fn windows(identity: windows_local_security::DirectoryIdentity) -> Self {
        Self {
            native: NativeDirectoryIdentity::Windows {
                volume_serial_number: identity.volume_serial_number(),
                file_id: identity.file_id(),
            },
        }
    }

    fn platform_kind(&self) -> &'static str {
        match self.native {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            NativeDirectoryIdentity::Unix { platform_kind, .. } => platform_kind,
            #[cfg(target_os = "windows")]
            NativeDirectoryIdentity::Windows { .. } => "WINDOWS_VOLUME_FILE_ID_128",
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    const fn unix_ids(&self) -> (u64, u64) {
        match self.native {
            NativeDirectoryIdentity::Unix {
                filesystem_instance_id,
                file_id,
                ..
            } => (filesystem_instance_id, file_id),
        }
    }

    pub(crate) fn matches_binding(&self, binding: &LocalFileIdentityBindingRecord) -> bool {
        match self.native {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            NativeDirectoryIdentity::Unix {
                filesystem_instance_id,
                file_id,
                platform_kind,
            } => {
                binding.raw_filesystem_instance_id == filesystem_instance_id.to_string()
                    && binding.raw_volume_id.is_none()
                    && binding.raw_file_id == file_id.to_string()
                    && binding.raw_generation.is_none()
                    && binding.platform_kind == platform_kind
            }
            #[cfg(target_os = "windows")]
            NativeDirectoryIdentity::Windows {
                volume_serial_number,
                file_id,
            } => {
                binding.raw_filesystem_instance_id == format!("{volume_serial_number:016x}")
                    && binding.raw_volume_id.is_none()
                    && binding.raw_file_id == hex::encode(file_id)
                    && binding.raw_generation.is_none()
                    && binding.platform_kind == self.platform_kind()
            }
        }
    }

    pub(crate) fn keyed_projection(
        &self,
        runtime_identity: &impl RuntimeIdentityKey,
    ) -> (String, serde_json::Value) {
        let (filesystem_bytes, file_bytes): ([u8; 8], Vec<u8>) = match self.native {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            NativeDirectoryIdentity::Unix {
                filesystem_instance_id,
                file_id,
                ..
            } => (
                filesystem_instance_id.to_be_bytes(),
                file_id.to_be_bytes().to_vec(),
            ),
            #[cfg(target_os = "windows")]
            NativeDirectoryIdentity::Windows {
                volume_serial_number,
                file_id,
            } => (volume_serial_number.to_be_bytes(), file_id.to_vec()),
        };
        let mut file_material = Vec::with_capacity(filesystem_bytes.len() + file_bytes.len());
        file_material.extend_from_slice(&filesystem_bytes);
        file_material.extend_from_slice(&file_bytes);
        let digest = format!(
            "sha256:{}",
            runtime_identity.keyed_pseudonym(
                "litecowork.local-filesystem.workspace-root.v1",
                &file_material,
            )
        );
        let filesystem_pseudonym = runtime_identity.keyed_pseudonym(
            "litecowork.local-filesystem.filesystem.v1",
            &filesystem_bytes,
        );
        let file_pseudonym =
            runtime_identity.keyed_pseudonym("litecowork.local-filesystem.file.v1", &file_material);
        let identity = serde_json::json!({
            "filesystem_instance_id": format!("sha256:{filesystem_pseudonym}"),
            "volume_id": null,
            "file_id": format!("sha256:{file_pseudonym}"),
            "generation": null,
            "platform_kind": self.platform_kind(),
        });
        (digest, identity)
    }

    pub(crate) fn binding_record(
        &self,
        location_id: String,
        runtime_id: String,
        runtime_incarnation_id: String,
        observed_at: String,
    ) -> LocalFileIdentityBindingRecord {
        let (raw_filesystem_instance_id, raw_file_id) = match self.native {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            NativeDirectoryIdentity::Unix {
                filesystem_instance_id,
                file_id,
                ..
            } => (filesystem_instance_id.to_string(), file_id.to_string()),
            #[cfg(target_os = "windows")]
            NativeDirectoryIdentity::Windows {
                volume_serial_number,
                file_id,
            } => (format!("{volume_serial_number:016x}"), hex::encode(file_id)),
        };
        LocalFileIdentityBindingRecord {
            location_id,
            runtime_id,
            runtime_incarnation_id,
            raw_filesystem_instance_id,
            raw_volume_id: None,
            raw_file_id,
            raw_generation: None,
            platform_kind: self.platform_kind().to_owned(),
            observed_at,
        }
    }
}

/// An open, verified directory selection. It is not serializable and its Debug output
/// intentionally reveals neither the locator nor operating-system identity.
pub(crate) struct OpenedDirectorySelection {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    handle: rustix::fd::OwnedFd,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    canonical_path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    private_locator: String,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    display_name: String,
    #[cfg(target_os = "windows")]
    handle: windows_local_security::VerifiedDirectory,
    #[cfg(target_os = "windows")]
    canonical_path: PathBuf,
    #[cfg(target_os = "windows")]
    private_locator: String,
    #[cfg(target_os = "windows")]
    display_name: String,
    identity: RawDirectoryIdentity,
}

impl fmt::Debug for OpenedDirectorySelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenedDirectorySelection")
            .field("display_name", &"<private>")
            .field("identity", &"<private>")
            .field("private_locator", &"<private>")
            .finish_non_exhaustive()
    }
}

impl OpenedDirectorySelection {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn private_locator(&self) -> &str {
        &self.private_locator
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn private_locator(&self) -> &str {
        &self.private_locator
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }

    pub(crate) const fn identity(&self) -> RawDirectoryIdentity {
        self.identity
    }

    /// Reopens the exact private locator without following path-component symlinks and
    /// compares it with the retained handle. Each call detects replacement at that
    /// identity checkpoint; it does not establish identity after process restart.
    pub(crate) fn revalidate(&self) -> Result<(), LocalDirectoryError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.revalidate_with_cancel(|| false)
        }
        #[cfg(target_os = "windows")]
        {
            self.handle
                .revalidate_path(&self.canonical_path)
                .map_err(|_| LocalDirectoryError::IdentityChanged)?;
            if RawDirectoryIdentity::windows(self.handle.identity()) != self.identity {
                return Err(LocalDirectoryError::IdentityChanged);
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Err(LocalDirectoryError::UnsupportedPlatform)
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn revalidate_with_cancel<C>(
        &self,
        mut is_cancelled: C,
    ) -> Result<(), LocalDirectoryError>
    where
        C: FnMut() -> bool,
    {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let reopened = open_absolute_directory_without_symlinks_with_cancel(
                &self.canonical_path,
                &mut is_cancelled,
            )?;
            let current = rustix::fs::fstat(&reopened)
                .map_err(|_| map_failed_directory_io(&mut is_cancelled))?;
            let retained = rustix::fs::fstat(&self.handle)
                .map_err(|_| map_failed_directory_io(&mut is_cancelled))?;
            check_directory_reopen_deadline(&mut is_cancelled)?;
            if current.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
                || current.st_dev != retained.st_dev
                || current.st_ino != retained.st_ino
            {
                return Err(LocalDirectoryError::IdentityChanged);
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalDirectoryError {
    // Keep the fail-closed error for targets without qualified native handle identity.
    #[allow(dead_code)]
    UnsupportedPlatform,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    InvalidSelection,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    NotDirectory,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    OpenFailed,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    ScanDeadlineExceeded,
    #[allow(dead_code)]
    IdentityChanged,
}

impl fmt::Display for LocalDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "persistent folder access is unsupported on this platform",
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            Self::InvalidSelection => "selected folder is invalid",
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            Self::NotDirectory => "selected item is not a folder",
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            Self::OpenFailed => "selected folder could not be opened safely",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::ScanDeadlineExceeded => "folder scan deadline elapsed during identity checking",
            Self::IdentityChanged => "selected folder no longer matches its opened identity",
        })
    }
}

impl std::error::Error for LocalDirectoryError {}

/// Opens a native folder selection and captures its identity from the opened handle.
///
/// On Unix, every resolved path component is opened relative to the previous directory
/// handle with `O_NOFOLLOW | O_DIRECTORY`. The final open handle stays alive in the
/// returned value. Other platforms fail closed until their native identity and handle
/// implementations are qualified.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn open_selected_directory(
    selected_path: &Path,
) -> Result<OpenedDirectorySelection, LocalDirectoryError> {
    validate_selection_path(selected_path)?;
    let selected_metadata =
        std::fs::symlink_metadata(selected_path).map_err(|_| LocalDirectoryError::OpenFailed)?;
    if selected_metadata.file_type().is_symlink() {
        return Err(LocalDirectoryError::InvalidSelection);
    }
    if !selected_metadata.is_dir() {
        return Err(LocalDirectoryError::NotDirectory);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Open the selected spelling before canonicalizing it. Canonicalization follows
        // symlinks, so doing it first would erase evidence of an intermediate symlink
        // before the no-follow component walk gets a chance to reject it.
        let handle = open_absolute_directory_without_symlinks(selected_path)?;
        let metadata = rustix::fs::fstat(&handle).map_err(|_| LocalDirectoryError::OpenFailed)?;
        if metadata.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE {
            return Err(LocalDirectoryError::NotDirectory);
        }
        let canonical_path = selected_path
            .canonicalize()
            .map_err(|_| LocalDirectoryError::OpenFailed)?;
        validate_selection_path(&canonical_path)?;
        let canonical_handle = open_absolute_directory_without_symlinks(&canonical_path)?;
        let canonical_metadata =
            rustix::fs::fstat(&canonical_handle).map_err(|_| LocalDirectoryError::OpenFailed)?;
        if metadata.st_dev != canonical_metadata.st_dev
            || metadata.st_ino != canonical_metadata.st_ino
        {
            return Err(LocalDirectoryError::IdentityChanged);
        }
        let display_name = canonical_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Filesystem root".to_owned());
        if display_name.chars().any(char::is_control) || display_name.len() > 255 {
            return Err(LocalDirectoryError::InvalidSelection);
        }
        let encoded_path = URL_SAFE_NO_PAD.encode(path_bytes(&canonical_path)?);
        let private_locator = format!("unix-path-b64:{encoded_path}");
        Ok(OpenedDirectorySelection {
            handle,
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            canonical_path,
            private_locator,
            display_name,
            identity: RawDirectoryIdentity::unix(
                metadata.st_dev as u64,
                metadata.st_ino as u64,
                if cfg!(target_os = "macos") {
                    "MACOS_DEVICE_INODE"
                } else {
                    "LINUX_DEVICE_INODE"
                },
            ),
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(LocalDirectoryError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn open_selected_directory(
    selected_path: &Path,
) -> Result<OpenedDirectorySelection, LocalDirectoryError> {
    let handle = windows_local_security::open_directory_path_without_reparse(selected_path)
        .map_err(|_| LocalDirectoryError::OpenFailed)?;
    let canonical_path = selected_path.to_path_buf();
    let wide: Vec<u16> = canonical_path.as_os_str().encode_wide().collect();
    validate_windows_path_units(&wide)?;
    let display_name = canonical_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Filesystem root".to_owned());
    validate_display_name(&display_name)?;
    let path_bytes = utf16le_bytes(&wide);
    let private_locator = format!(
        "windows-path-u16le-b64:{}",
        URL_SAFE_NO_PAD.encode(path_bytes)
    );
    handle
        .revalidate_path(&canonical_path)
        .map_err(|_| LocalDirectoryError::IdentityChanged)?;
    let identity = RawDirectoryIdentity::windows(handle.identity());
    Ok(OpenedDirectorySelection {
        handle,
        canonical_path,
        private_locator,
        display_name,
        identity,
    })
}

/// Reopens a persisted Runtime-local locator without canonicalizing or following any
/// component. The returned handle is not trusted until its raw and keyed identity match
/// the prior bindings and Resource identity.
pub(crate) fn reopen_saved_directory(
    private_locator: &str,
    display_name: &str,
) -> Result<OpenedDirectorySelection, LocalDirectoryError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        reopen_saved_directory_with_cancel(private_locator, display_name, || false)
    }
    #[cfg(target_os = "windows")]
    {
        let encoded = private_locator
            .strip_prefix("windows-path-u16le-b64:")
            .ok_or(LocalDirectoryError::InvalidSelection)?;
        if encoded.is_empty() || encoded.len() > 45_056 {
            return Err(LocalDirectoryError::InvalidSelection);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| LocalDirectoryError::InvalidSelection)?;
        if bytes.is_empty()
            || bytes.len() > 33_792
            || bytes.len() % 2 != 0
            || URL_SAFE_NO_PAD.encode(&bytes) != encoded
        {
            return Err(LocalDirectoryError::InvalidSelection);
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
            .collect();
        validate_windows_path_units(&units)?;
        let path_string =
            String::from_utf16(&units).map_err(|_| LocalDirectoryError::InvalidSelection)?;
        let path = PathBuf::from(path_string);
        validate_display_name(display_name)?;
        let handle = windows_local_security::open_directory_path_without_reparse(&path)
            .map_err(|_| LocalDirectoryError::OpenFailed)?;
        handle
            .revalidate_path(&path)
            .map_err(|_| LocalDirectoryError::IdentityChanged)?;
        let identity = RawDirectoryIdentity::windows(handle.identity());
        Ok(OpenedDirectorySelection {
            handle,
            canonical_path: path,
            private_locator: private_locator.to_owned(),
            display_name: display_name.to_owned(),
            identity,
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (private_locator, display_name);
        Err(LocalDirectoryError::UnsupportedPlatform)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn reopen_saved_directory_with_cancel<C>(
    private_locator: &str,
    display_name: &str,
    mut is_cancelled: C,
) -> Result<OpenedDirectorySelection, LocalDirectoryError>
where
    C: FnMut() -> bool,
{
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        check_directory_reopen_deadline(&mut is_cancelled)?;
        let encoded = private_locator
            .strip_prefix("unix-path-b64:")
            .ok_or(LocalDirectoryError::InvalidSelection)?;
        if encoded.is_empty() || encoded.len() > 44 * 1024 {
            return Err(LocalDirectoryError::InvalidSelection);
        }
        let path_bytes = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| LocalDirectoryError::InvalidSelection)?;
        if path_bytes.is_empty()
            || path_bytes.len() > MAX_SELECTION_PATH_BYTES
            || path_bytes.contains(&0)
            || URL_SAFE_NO_PAD.encode(&path_bytes) != encoded
        {
            return Err(LocalDirectoryError::InvalidSelection);
        }
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(path_bytes));
        validate_selection_path(&path)?;
        let handle =
            open_absolute_directory_without_symlinks_with_cancel(&path, &mut is_cancelled)?;
        check_directory_reopen_deadline(&mut is_cancelled)?;
        let metadata =
            rustix::fs::fstat(&handle).map_err(|_| map_failed_directory_io(&mut is_cancelled))?;
        if metadata.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE {
            return Err(LocalDirectoryError::NotDirectory);
        }
        if display_name.trim().is_empty()
            || display_name.chars().any(char::is_control)
            || display_name.len() > 255
        {
            return Err(LocalDirectoryError::InvalidSelection);
        }
        Ok(OpenedDirectorySelection {
            handle,
            canonical_path: path,
            private_locator: private_locator.to_owned(),
            display_name: display_name.to_owned(),
            identity: RawDirectoryIdentity::unix(
                metadata.st_dev as u64,
                metadata.st_ino as u64,
                if cfg!(target_os = "macos") {
                    "MACOS_DEVICE_INODE"
                } else {
                    "LINUX_DEVICE_INODE"
                },
            ),
        })
    }
}

#[cfg(target_os = "windows")]
fn validate_windows_path_units(units: &[u16]) -> Result<(), LocalDirectoryError> {
    if units.is_empty()
        || units.len() > 16_896
        || units.contains(&0)
        || String::from_utf16(units).is_err()
    {
        return Err(LocalDirectoryError::InvalidSelection);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn utf16le_bytes(units: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(units.len() * 2);
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
fn validate_display_name(display_name: &str) -> Result<(), LocalDirectoryError> {
    if display_name.trim().is_empty()
        || display_name.chars().any(char::is_control)
        || display_name.len() > 255
    {
        return Err(LocalDirectoryError::InvalidSelection);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_selection_path(path: &Path) -> Result<(), LocalDirectoryError> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path.as_os_str().len() > MAX_SELECTION_PATH_BYTES
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(LocalDirectoryError::InvalidSelection);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_absolute_directory_without_symlinks(
    path: &Path,
) -> Result<rustix::fd::OwnedFd, LocalDirectoryError> {
    open_absolute_directory_without_symlinks_with_cancel(path, &mut || false)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_absolute_directory_without_symlinks_with_cancel(
    path: &Path,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<rustix::fd::OwnedFd, LocalDirectoryError> {
    use rustix::fs::{Mode, OFlags, open, openat};

    check_directory_reopen_deadline(is_cancelled)?;
    let mut current = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|_| map_failed_directory_io(is_cancelled))?;
    let mut saw_component = false;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                // A saved path may have many components. Check between each no-follow
                // open so an expired scan never walks the rest of the path unbounded.
                check_directory_reopen_deadline(is_cancelled)?;
                let next = openat(
                    &current,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                    Mode::empty(),
                )
                .map_err(|_| map_failed_directory_io(is_cancelled))?;
                current = next;
                saw_component = true;
            }
            _ => return Err(LocalDirectoryError::InvalidSelection),
        }
    }
    check_directory_reopen_deadline(is_cancelled)?;
    if !saw_component {
        return Err(LocalDirectoryError::InvalidSelection);
    }
    Ok(current)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn check_directory_reopen_deadline(
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), LocalDirectoryError> {
    if is_cancelled() {
        Err(LocalDirectoryError::ScanDeadlineExceeded)
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn map_failed_directory_io(is_cancelled: &mut impl FnMut() -> bool) -> LocalDirectoryError {
    if is_cancelled() {
        LocalDirectoryError::ScanDeadlineExceeded
    } else {
        LocalDirectoryError::OpenFailed
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn path_bytes(path: &Path) -> Result<&[u8], LocalDirectoryError> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RawLocalFileIdentity {
    device_id: u64,
    file_id: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[expect(
    dead_code,
    reason = "scanner accessors are intentionally awaiting the durable WorldIndexer integration"
)]
impl RawLocalFileIdentity {
    pub(crate) const fn device_id(&self) -> u64 {
        self.device_id
    }

    pub(crate) const fn file_id(&self) -> u64 {
        self.file_id
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "scanner primitives are intentionally isolated until the durable WorldIndexer port is integrated"
    )
)]
pub(crate) struct LocalFileScanEntry {
    relative_locator: Vec<u8>,
    display_basename: String,
    identity: RawLocalFileIdentity,
    mount_id: u64,
    size_bytes: u64,
    modified_at: Option<String>,
    #[cfg(target_os = "linux")]
    file_handle: rustix::fd::OwnedFd,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[expect(
    dead_code,
    reason = "scanner accessors are intentionally awaiting the durable WorldIndexer integration"
)]
impl LocalFileScanEntry {
    pub(crate) fn relative_locator(&self) -> &[u8] {
        &self.relative_locator
    }

    pub(crate) fn display_basename(&self) -> &str {
        &self.display_basename
    }

    pub(crate) const fn identity(&self) -> RawLocalFileIdentity {
        self.identity
    }

    pub(crate) const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub(crate) fn modified_at(&self) -> Option<&str> {
        self.modified_at.as_deref()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for LocalFileScanEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalFileScanEntry")
            .field("relative_locator", &"<private>")
            .field("display_basename", &"<private>")
            .field("identity", &"<private>")
            .field("size_bytes", &self.size_bytes)
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(
    all(not(test), target_os = "macos"),
    expect(
        dead_code,
        reason = "streaming metadata scanning is not qualified on macOS"
    )
)]
pub(crate) struct LocalFileScanLimits {
    max_depth: usize,
    max_entries: usize,
    max_locator_bytes: usize,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(
    all(not(test), target_os = "macos"),
    expect(
        dead_code,
        reason = "streaming metadata scanning is not qualified on macOS"
    )
)]
impl LocalFileScanLimits {
    pub(crate) const fn new(
        max_depth: usize,
        max_entries: usize,
        max_locator_bytes: usize,
    ) -> Self {
        Self {
            max_depth,
            max_entries,
            max_locator_bytes,
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(
    all(not(test), target_os = "macos"),
    expect(
        dead_code,
        reason = "streaming metadata scanning is not qualified on macOS"
    )
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalFileScanError {
    InvalidLimits,
    Cancelled,
    RootChanged,
    OpenFailed,
    EntryChanged,
    MountBoundary,
    MountIdentityUnavailable,
    ReadFailed,
    BudgetExceeded,
    #[cfg(target_os = "macos")]
    UnsupportedPlatform,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[cfg_attr(
    all(not(test), target_os = "macos"),
    expect(
        dead_code,
        reason = "streaming metadata scanning is not qualified on macOS"
    )
)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LocalFileScanSummary {
    pub(crate) entries_visited: usize,
    pub(crate) files_visited: usize,
}

#[cfg(target_os = "linux")]
fn file_metadata_open_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW
}

/// Enumerates regular files using only handles opened relative to the selected root.
/// This compatibility collector retains each file handle and is therefore capped at 64.
#[cfg(target_os = "linux")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "scanner primitives are intentionally isolated until the durable WorldIndexer port is integrated"
    )
)]
pub(crate) fn scan_regular_files(
    selection: &OpenedDirectorySelection,
    limits: LocalFileScanLimits,
) -> Result<Vec<LocalFileScanEntry>, LocalFileScanError> {
    if limits.max_entries > MAX_RETAINED_SCAN_ENTRIES {
        return Err(LocalFileScanError::InvalidLimits);
    }
    let mut entries = Vec::new();
    scan_regular_files_inner(
        selection,
        limits,
        &mut || false,
        &mut |_| {},
        &mut || {},
        &mut |entry| {
            entries.push(entry);
            Ok(())
        },
    )?;
    Ok(entries)
}

/// Visits regular files without retaining prior file handles. The visitor receives one
/// pinned metadata entry at a time; its descriptor is dropped before the next regular
/// file is opened. Directory handles/streams are retained only along the active path,
/// whose depth is hard-capped at 64. The cancellation callback is checked between every
/// directory entry; cancellation and every other error return `Err`, so callers cannot
/// mistake an incomplete pass for completion. `Ok` means traversal completed, but does
/// not promise an atomic tree snapshot and must not alone be used to infer absence.
///
/// `read_bounded_file` remains the content path and enforces its caller-provided
/// per-file byte limit. This metadata traversal independently enforces server-capped
/// entry, depth, aggregate-locator-byte, and cancellation budgets. The visitor may have
/// received positive entries before a later error; callers must stage side effects until
/// this function returns `Ok` and still must not infer absence from the pass.
#[cfg(target_os = "linux")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "streaming scanner awaits the durable WorldIndexer integration"
    )
)]
pub(crate) fn visit_regular_files<F, C>(
    selection: &OpenedDirectorySelection,
    limits: LocalFileScanLimits,
    is_cancelled: C,
    visitor: F,
) -> Result<LocalFileScanSummary, LocalFileScanError>
where
    F: FnMut(&LocalFileScanEntry) -> Result<(), LocalFileScanError>,
    C: FnMut() -> bool,
{
    visit_regular_files_with_progress(selection, limits, is_cancelled, visitor, |_| {})
}

/// Streaming variant that exposes only monotonic counters so callers can report a
/// truthful PARTIAL summary if bounded traversal stops after observing entries.
#[cfg(target_os = "linux")]
pub(crate) fn visit_regular_files_with_progress<F, C, P>(
    selection: &OpenedDirectorySelection,
    limits: LocalFileScanLimits,
    mut is_cancelled: C,
    mut visitor: F,
    mut progress: P,
) -> Result<LocalFileScanSummary, LocalFileScanError>
where
    F: FnMut(&LocalFileScanEntry) -> Result<(), LocalFileScanError>,
    C: FnMut() -> bool,
    P: FnMut(LocalFileScanSummary),
{
    let summary = scan_regular_files_inner(
        selection,
        limits,
        &mut is_cancelled,
        &mut progress,
        &mut || {},
        &mut |entry| visitor(&entry),
    )?;
    Ok(LocalFileScanSummary {
        entries_visited: summary.entries_visited,
        files_visited: summary.files_visited,
    })
}

#[cfg(target_os = "macos")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "streaming scanner remains fail-closed until a qualified macOS file-handle path exists"
    )
)]
pub(crate) fn visit_regular_files<F, C>(
    _selection: &OpenedDirectorySelection,
    _limits: LocalFileScanLimits,
    _is_cancelled: C,
    _visitor: F,
) -> Result<LocalFileScanSummary, LocalFileScanError>
where
    F: FnMut(&LocalFileScanEntry) -> Result<(), LocalFileScanError>,
    C: FnMut() -> bool,
{
    Err(LocalFileScanError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn scan_regular_files_inner<F, C>(
    selection: &OpenedDirectorySelection,
    limits: LocalFileScanLimits,
    is_cancelled: &mut C,
    progress: &mut impl FnMut(LocalFileScanSummary),
    before_entry_open: &mut impl FnMut(),
    visitor: &mut F,
) -> Result<LocalFileScanSummary, LocalFileScanError>
where
    F: FnMut(LocalFileScanEntry) -> Result<(), LocalFileScanError>,
    C: FnMut() -> bool,
{
    use rustix::fs::{AtFlags, Mode, OFlags, fstat, openat};

    if limits.max_depth == 0
        || limits.max_depth > MAX_SCAN_DEPTH
        || limits.max_entries == 0
        || limits.max_entries > MAX_SCAN_ENTRIES
        || limits.max_locator_bytes == 0
        || limits.max_locator_bytes > MAX_SCAN_LOCATOR_BYTES
    {
        return Err(LocalFileScanError::InvalidLimits);
    }
    if is_cancelled() {
        return Err(LocalFileScanError::Cancelled);
    }
    selection
        .revalidate_with_cancel(|| is_cancelled())
        .map_err(|error| match error {
            LocalDirectoryError::ScanDeadlineExceeded => LocalFileScanError::Cancelled,
            _ => LocalFileScanError::RootChanged,
        })?;

    let root_handle = openat(
        &selection.handle,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|_| LocalFileScanError::OpenFailed)?;
    let root_stat = fstat(&root_handle).map_err(|_| LocalFileScanError::OpenFailed)?;
    let (root_filesystem_instance_id, root_file_id) = selection.identity.unix_ids();
    if root_stat.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
        || root_stat.st_dev as u64 != root_filesystem_instance_id
        || root_stat.st_ino as u64 != root_file_id
    {
        return Err(LocalFileScanError::RootChanged);
    }
    let root_mount_id = mount_id_at(&root_handle, "", AtFlags::EMPTY_PATH)?;

    let mut summary = LocalFileScanSummary {
        entries_visited: 0,
        files_visited: 0,
    };
    let mut visited_path_bytes = 0usize;
    let mut entries_considered = 0usize;
    visit_directory_recursively(
        selection,
        &root_handle,
        &[],
        0,
        root_filesystem_instance_id,
        root_mount_id,
        &limits,
        is_cancelled,
        progress,
        before_entry_open,
        visitor,
        &mut summary,
        &mut visited_path_bytes,
        &mut entries_considered,
    )?;
    if is_cancelled() {
        return Err(LocalFileScanError::Cancelled);
    }

    // Detect replacement or disappearance of the selected path during traversal. A
    // successful metadata pass is still not a tree snapshot: callers must fence the
    // root version at commit and must not infer absence from one pass alone.
    selection
        .revalidate_with_cancel(|| is_cancelled())
        .map_err(|error| match error {
            LocalDirectoryError::ScanDeadlineExceeded => LocalFileScanError::Cancelled,
            _ => LocalFileScanError::RootChanged,
        })?;
    if is_cancelled() {
        return Err(LocalFileScanError::Cancelled);
    }
    Ok(summary)
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn visit_directory_recursively<F, C>(
    selection: &OpenedDirectorySelection,
    directory_handle: &rustix::fd::OwnedFd,
    directory_locator: &[u8],
    directory_depth: usize,
    root_device_id: u64,
    root_mount_id: u64,
    limits: &LocalFileScanLimits,
    is_cancelled: &mut C,
    progress: &mut impl FnMut(LocalFileScanSummary),
    before_entry_open: &mut impl FnMut(),
    visitor: &mut F,
    summary: &mut LocalFileScanSummary,
    visited_path_bytes: &mut usize,
    entries_considered: &mut usize,
) -> Result<(), LocalFileScanError>
where
    F: FnMut(LocalFileScanEntry) -> Result<(), LocalFileScanError>,
    C: FnMut() -> bool,
{
    use rustix::fs::{AtFlags, Dir, Mode, OFlags, ResolveFlags, fstat, openat2};
    use std::os::unix::ffi::OsStrExt;

    verify_directory_remains_beneath_root(
        selection,
        directory_locator,
        directory_handle,
        root_mount_id,
    )?;
    let mut listing =
        Dir::read_from(directory_handle).map_err(|_| LocalFileScanError::ReadFailed)?;
    while let Some(entry) = listing.read() {
        if is_cancelled() {
            return Err(LocalFileScanError::Cancelled);
        }
        verify_directory_remains_beneath_root(
            selection,
            directory_locator,
            directory_handle,
            root_mount_id,
        )?;
        let entry = entry.map_err(|_| LocalFileScanError::ReadFailed)?;
        let name = entry.file_name();
        let name_bytes = name.to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }

        *entries_considered = entries_considered
            .checked_add(1)
            .ok_or(LocalFileScanError::BudgetExceeded)?;
        if *entries_considered > limits.max_entries {
            return Err(LocalFileScanError::BudgetExceeded);
        }

        let depth = directory_depth
            .checked_add(1)
            .ok_or(LocalFileScanError::BudgetExceeded)?;
        if depth > limits.max_depth || depth > MAX_SCAN_DEPTH {
            return Err(LocalFileScanError::BudgetExceeded);
        }
        let mut relative_locator = directory_locator.to_vec();
        if !relative_locator.is_empty() {
            relative_locator.push(b'/');
        }
        relative_locator.extend_from_slice(name_bytes);
        let next_path_bytes = visited_path_bytes
            .checked_add(relative_locator.len())
            .ok_or(LocalFileScanError::BudgetExceeded)?;
        if next_path_bytes > limits.max_locator_bytes {
            return Err(LocalFileScanError::BudgetExceeded);
        }
        *visited_path_bytes = next_path_bytes;
        // Prune known private/generated paths before resolving or opening them. They
        // never contribute metadata observations or visible progress counts.
        if excluded_root_entry(name_bytes) {
            continue;
        }

        // Resolve the complete candidate path from the selected root. The retained
        // directory stream may have been detached after the point-in-time check above;
        // never use it as authority to stat or open an entry.
        before_entry_open();
        let inspected = openat2(
            &selection.handle,
            std::ffi::OsStr::from_bytes(&relative_locator),
            file_metadata_open_flags(),
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_XDEV,
        )
        .map_err(|error| {
            if error == rustix::io::Errno::XDEV {
                LocalFileScanError::MountBoundary
            } else {
                LocalFileScanError::EntryChanged
            }
        })?;
        let metadata = fstat(&inspected).map_err(|_| LocalFileScanError::EntryChanged)?;
        let entry_mount_id = mount_id_at(&inspected, "", AtFlags::EMPTY_PATH)?;
        if entry_mount_id != root_mount_id {
            return Err(LocalFileScanError::MountBoundary);
        }

        summary.entries_visited = summary
            .entries_visited
            .checked_add(1)
            .ok_or(LocalFileScanError::BudgetExceeded)?;
        progress(*summary);

        let file_type = metadata.st_mode & DIRECTORY_TYPE_MASK;
        if file_type == 0o120000 {
            // Directory entries are untrusted and may be symlinks to arbitrary
            // locations. They are never opened or followed.
            continue;
        }
        if file_type == DIRECTORY_TYPE {
            if metadata.st_dev as u64 != root_device_id {
                return Err(LocalFileScanError::MountBoundary);
            }
            let child = openat2(
                &selection.handle,
                std::ffi::OsStr::from_bytes(&relative_locator),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
                ResolveFlags::BENEATH
                    | ResolveFlags::NO_SYMLINKS
                    | ResolveFlags::NO_MAGICLINKS
                    | ResolveFlags::NO_XDEV,
            )
            .map_err(|error| {
                if error == rustix::io::Errno::XDEV {
                    LocalFileScanError::MountBoundary
                } else {
                    LocalFileScanError::EntryChanged
                }
            })?;
            let after_open = fstat(&child).map_err(|_| LocalFileScanError::EntryChanged)?;
            let opened_mount_id = mount_id_at(&child, "", AtFlags::EMPTY_PATH)?;
            if after_open.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
                || after_open.st_dev != metadata.st_dev
                || after_open.st_ino != metadata.st_ino
            {
                return Err(LocalFileScanError::EntryChanged);
            }
            if after_open.st_dev as u64 != root_device_id {
                return Err(LocalFileScanError::MountBoundary);
            }
            if opened_mount_id != root_mount_id {
                return Err(LocalFileScanError::MountBoundary);
            }
            visit_directory_recursively(
                selection,
                &child,
                &relative_locator,
                depth,
                root_device_id,
                root_mount_id,
                limits,
                is_cancelled,
                progress,
                before_entry_open,
                visitor,
                summary,
                visited_path_bytes,
                entries_considered,
            )?;
            continue;
        }
        if file_type != 0o100000 {
            // Sockets, FIFOs, devices, and other special entries are not files
            // the local indexer may observe.
            continue;
        }
        if metadata.st_dev as u64 != root_device_id {
            return Err(LocalFileScanError::MountBoundary);
        }

        if metadata.st_size < 0 {
            return Err(LocalFileScanError::EntryChanged);
        }

        summary.files_visited = summary
            .files_visited
            .checked_add(1)
            .ok_or(LocalFileScanError::BudgetExceeded)?;
        progress(*summary);
        visitor(LocalFileScanEntry {
            relative_locator,
            display_basename: display_basename(name_bytes),
            identity: RawLocalFileIdentity {
                device_id: metadata.st_dev as u64,
                file_id: metadata.st_ino as u64,
            },
            mount_id: entry_mount_id,
            size_bytes: metadata.st_size as u64,
            modified_at: file_modified_at(metadata.st_mtime, metadata.st_mtime_nsec),
            file_handle: inspected,
        })?;
    }
    verify_directory_remains_beneath_root(
        selection,
        directory_locator,
        directory_handle,
        root_mount_id,
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn file_modified_at(seconds: i64, nanoseconds: u64) -> Option<String> {
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    if nanoseconds >= 1_000_000_000 {
        return None;
    }
    let timestamp = i128::from(seconds) * 1_000_000_000 + i128::from(nanoseconds);
    OffsetDateTime::from_unix_timestamp_nanos(timestamp)
        .ok()?
        .format(&Rfc3339)
        .ok()
}

/// Re-resolves each directory from the selected root with Linux `openat2` resolution
/// constraints. A retained child descriptor is not sufficient authority after its name
/// has been moved outside the selected tree. This is a point-in-time check; a later move
/// can still race the next filesystem operation, so visitors must stage results until the
/// whole traversal returns successfully.
#[cfg(target_os = "linux")]
fn verify_directory_remains_beneath_root(
    selection: &OpenedDirectorySelection,
    relative_locator: &[u8],
    directory_handle: &rustix::fd::OwnedFd,
    root_mount_id: u64,
) -> Result<(), LocalFileScanError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags, fstat, openat2};
    use std::os::unix::ffi::OsStrExt;

    if relative_locator.contains(&0) {
        return Err(LocalFileScanError::EntryChanged);
    }
    let relative_path = if relative_locator.is_empty() {
        std::ffi::OsStr::new(".")
    } else {
        std::ffi::OsStr::from_bytes(relative_locator)
    };
    let reopened = openat2(
        &selection.handle,
        relative_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    )
    .map_err(|_| LocalFileScanError::EntryChanged)?;
    let reopened_stat = fstat(&reopened).map_err(|_| LocalFileScanError::EntryChanged)?;
    let retained_stat = fstat(directory_handle).map_err(|_| LocalFileScanError::EntryChanged)?;
    if reopened_stat.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
        || retained_stat.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
        || reopened_stat.st_dev != retained_stat.st_dev
        || reopened_stat.st_ino != retained_stat.st_ino
        || mount_id_at(&reopened, "", rustix::fs::AtFlags::EMPTY_PATH)? != root_mount_id
    {
        return Err(LocalFileScanError::EntryChanged);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn mount_id_at<Fd, Path>(
    directory: Fd,
    path: Path,
    flags: rustix::fs::AtFlags,
) -> Result<u64, LocalFileScanError>
where
    Fd: rustix::fd::AsFd,
    Path: rustix::path::Arg,
{
    use rustix::fs::{StatxFlags, statx};

    let observation = statx(directory, path, flags, StatxFlags::MNT_ID)
        .map_err(|_| LocalFileScanError::MountIdentityUnavailable)?;
    if observation.stx_mask & StatxFlags::MNT_ID.bits() == 0 {
        return Err(LocalFileScanError::MountIdentityUnavailable);
    }
    Ok(observation.stx_mnt_id)
}

/// Reads bytes from the exact regular file handle retained by the Linux metadata scan.
/// Candidate names are opened only with O_PATH during scanning; readable access is
/// obtained by reopening the held descriptor through a verified procfs fd directory.
/// On success, the returned bytes are the exact observation to hash; callers must not
/// substitute a later path read. The pre/post size and timestamp comparison is only a
/// best-effort mutation detector, not an atomic filesystem snapshot and not a guarantee
/// that every in-place write will be detected. `EntryChanged` requires a fresh scan before
/// retrying so the current name and identity are pinned again.
#[cfg(target_os = "linux")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "bounded reader awaits the durable WorldIndexer integration"
    )
)]
pub(crate) fn read_bounded_file(
    selection: &OpenedDirectorySelection,
    entry: &LocalFileScanEntry,
    max_bytes: usize,
) -> Result<Vec<u8>, LocalFileScanError> {
    read_bounded_file_inner(selection, entry, max_bytes, || {}, || {}, || false)
}

/// Reads one pinned file while checking cancellation between bounded 8 KiB reads.
/// Cancellation cannot interrupt an individual kernel read already in progress, but it
/// prevents continuing the remainder of an oversized/slow file after that read returns.
#[cfg(target_os = "linux")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "cancellation-aware reader awaits durable WorldIndexer integration"
    )
)]
pub(crate) fn read_bounded_file_with_cancel<C>(
    selection: &OpenedDirectorySelection,
    entry: &LocalFileScanEntry,
    max_bytes: usize,
    is_cancelled: C,
) -> Result<Vec<u8>, LocalFileScanError>
where
    C: FnMut() -> bool,
{
    read_bounded_file_inner(selection, entry, max_bytes, || {}, || {}, is_cancelled)
}

#[cfg(all(target_os = "linux", test))]
fn read_bounded_file_with_hooks(
    selection: &OpenedDirectorySelection,
    entry: &LocalFileScanEntry,
    max_bytes: usize,
    before_reopen: impl FnOnce(),
    after_read: impl FnOnce(),
) -> Result<Vec<u8>, LocalFileScanError> {
    read_bounded_file_inner(
        selection,
        entry,
        max_bytes,
        before_reopen,
        after_read,
        || false,
    )
}

#[cfg(target_os = "linux")]
fn read_bounded_file_inner(
    selection: &OpenedDirectorySelection,
    entry: &LocalFileScanEntry,
    max_bytes: usize,
    before_reopen: impl FnOnce(),
    after_read: impl FnOnce(),
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, LocalFileScanError> {
    use rustix::fs::{AtFlags, Mode, OFlags, fstat, open, openat};
    use std::io::Read;
    use std::os::fd::AsRawFd;

    let max_read = max_bytes
        .checked_add(1)
        .ok_or(LocalFileScanError::BudgetExceeded)?;
    if is_cancelled() {
        return Err(LocalFileScanError::Cancelled);
    }
    selection
        .revalidate()
        .map_err(|_| LocalFileScanError::RootChanged)?;
    let pinned_stat = fstat(&entry.file_handle).map_err(|_| LocalFileScanError::EntryChanged)?;
    if !matches_entry_identity(&pinned_stat, entry)
        || pinned_stat.st_mode & DIRECTORY_TYPE_MASK != 0o100000
    {
        return Err(LocalFileScanError::EntryChanged);
    }
    if mount_id_at(&entry.file_handle, "", AtFlags::EMPTY_PATH)? != entry.mount_id {
        return Err(LocalFileScanError::MountBoundary);
    }

    // Tests use this seam to place a replacement precisely between metadata pinning
    // and the procfs reopen. Production always supplies a no-op.
    before_reopen();
    let (parent, leaf) = open_entry_parent(selection, entry)?;
    ensure_entry_name_matches(&parent, &leaf, entry)?;

    // /proc/self is a procfs magic link, so open it relative to a verified procfs root.
    // Retain and verify the final fd directory before using the numeric held-fd name.
    const PROC_SUPER_MAGIC: i64 = 0x9fa0;
    let proc_root = open(
        "/proc",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|_| LocalFileScanError::ReadFailed)?;
    verify_procfs_directory(&proc_root, PROC_SUPER_MAGIC)?;
    let proc_self = openat(
        &proc_root,
        "self",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| LocalFileScanError::ReadFailed)?;
    verify_procfs_directory(&proc_self, PROC_SUPER_MAGIC)?;
    let proc_fds = openat(
        &proc_self,
        "fd",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|_| LocalFileScanError::ReadFailed)?;
    verify_procfs_directory(&proc_fds, PROC_SUPER_MAGIC)?;

    // This is the sole O_RDONLY open of the content. The path is the decimal descriptor
    // opened with O_PATH|O_NOFOLLOW during enumeration, never a WorkspaceRoot name.
    let readable = openat(
        &proc_fds,
        entry.file_handle.as_raw_fd().to_string(),
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| LocalFileScanError::ReadFailed)?;
    let before_read = fstat(&readable).map_err(|_| LocalFileScanError::EntryChanged)?;
    if !matches_entry_identity(&before_read, entry)
        || before_read.st_mode & DIRECTORY_TYPE_MASK != 0o100000
    {
        return Err(LocalFileScanError::EntryChanged);
    }
    if mount_id_at(&readable, "", AtFlags::EMPTY_PATH)? != entry.mount_id {
        return Err(LocalFileScanError::MountBoundary);
    }

    let mut content = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut readable = std::fs::File::from(readable);
    while content.len() < max_read {
        if is_cancelled() {
            return Err(LocalFileScanError::Cancelled);
        }
        let count = (max_read - content.len()).min(buffer.len());
        let read = readable
            .read(&mut buffer[..count])
            .map_err(|_| LocalFileScanError::ReadFailed)?;
        if read == 0 {
            break;
        }
        content.extend_from_slice(&buffer[..read]);
    }

    after_read();
    let after_read_stat = fstat(&readable).map_err(|_| LocalFileScanError::EntryChanged)?;
    if !same_file_observation(&before_read, &after_read_stat, entry)
        || after_read_stat.st_mode & DIRECTORY_TYPE_MASK != 0o100000
    {
        return Err(LocalFileScanError::EntryChanged);
    }
    let (current_parent, current_leaf) = open_entry_parent(selection, entry)?;
    let original_parent = fstat(&parent).map_err(|_| LocalFileScanError::EntryChanged)?;
    let current_parent_stat =
        fstat(&current_parent).map_err(|_| LocalFileScanError::EntryChanged)?;
    if original_parent.st_dev != current_parent_stat.st_dev
        || original_parent.st_ino != current_parent_stat.st_ino
    {
        return Err(LocalFileScanError::EntryChanged);
    }
    ensure_entry_name_matches(&current_parent, &current_leaf, entry)?;
    selection
        .revalidate()
        .map_err(|_| LocalFileScanError::RootChanged)?;
    if content.len() > max_bytes {
        return Err(LocalFileScanError::BudgetExceeded);
    }
    Ok(content)
}

#[cfg(target_os = "linux")]
fn excluded_root_entry(name: &[u8]) -> bool {
    let normalized = name.iter().map(u8::to_ascii_lowercase).collect::<Vec<_>>();
    const EXCLUDED_DIRECTORIES: [&[u8]; 11] = [
        b".git",
        b".svn",
        b".hg",
        b"node_modules",
        b"target",
        b"dist",
        b"coverage",
        b".venv",
        b".ssh",
        b".aws",
        b"credentials",
    ];
    if EXCLUDED_DIRECTORIES.contains(&normalized.as_slice()) {
        return true;
    }
    normalized == b".env"
        || normalized.starts_with(b".env.")
        || normalized.ends_with(b".pem")
        || normalized.ends_with(b".p12")
        || normalized.ends_with(b".pfx")
        || normalized == b"id_rsa"
        || normalized == b"id_ed25519"
        || normalized.ends_with(b".key")
        || normalized
            .windows(b"credentials".len())
            .any(|part| part == b"credentials")
}

#[cfg(target_os = "linux")]
fn verify_procfs_directory(
    directory: &impl rustix::fd::AsFd,
    proc_super_magic: i64,
) -> Result<(), LocalFileScanError> {
    use rustix::fs::{fstat, fstatfs};

    let metadata = fstat(directory).map_err(|_| LocalFileScanError::ReadFailed)?;
    let filesystem = fstatfs(directory).map_err(|_| LocalFileScanError::ReadFailed)?;
    if metadata.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
        || filesystem.f_type as i64 != proc_super_magic
    {
        return Err(LocalFileScanError::ReadFailed);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_entry_parent(
    selection: &OpenedDirectorySelection,
    entry: &LocalFileScanEntry,
) -> Result<(rustix::fd::OwnedFd, std::ffi::OsString), LocalFileScanError> {
    use rustix::fs::{AtFlags, Mode, OFlags, fstat, openat, statat};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let mut components = entry.relative_locator.split(|byte| *byte == b'/');
    let first = components.next().ok_or(LocalFileScanError::EntryChanged)?;
    let mut current = openat(
        &selection.handle,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|_| LocalFileScanError::EntryChanged)?;
    let root_stat = fstat(&current).map_err(|_| LocalFileScanError::RootChanged)?;
    let (root_filesystem_instance_id, root_file_id) = selection.identity.unix_ids();
    if root_stat.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
        || root_stat.st_dev as u64 != root_filesystem_instance_id
        || root_stat.st_ino as u64 != root_file_id
    {
        return Err(LocalFileScanError::RootChanged);
    }
    if mount_id_at(&current, "", AtFlags::EMPTY_PATH)? != entry.mount_id {
        return Err(LocalFileScanError::MountBoundary);
    }

    let mut parts = vec![first];
    parts.extend(components);
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == b"." || *part == b".." || part.contains(&0))
    {
        return Err(LocalFileScanError::EntryChanged);
    }
    let leaf = parts.pop().ok_or(LocalFileScanError::EntryChanged)?;
    for component in parts {
        if mount_id_at(
            &current,
            std::ffi::OsStr::from_bytes(component),
            AtFlags::SYMLINK_NOFOLLOW,
        )? != entry.mount_id
        {
            return Err(LocalFileScanError::MountBoundary);
        }
        let before = statat(
            &current,
            std::ffi::OsStr::from_bytes(component),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| LocalFileScanError::EntryChanged)?;
        if before.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE {
            return Err(LocalFileScanError::EntryChanged);
        }
        let next = openat(
            &current,
            std::ffi::OsStr::from_bytes(component),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|_| LocalFileScanError::EntryChanged)?;
        let opened = fstat(&next).map_err(|_| LocalFileScanError::EntryChanged)?;
        if mount_id_at(&next, "", AtFlags::EMPTY_PATH)? != entry.mount_id {
            return Err(LocalFileScanError::MountBoundary);
        }
        let still_named = statat(
            &current,
            std::ffi::OsStr::from_bytes(component),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| LocalFileScanError::EntryChanged)?;
        if opened.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
            || opened.st_dev != before.st_dev
            || opened.st_ino != before.st_ino
            || still_named.st_dev != opened.st_dev
            || still_named.st_ino != opened.st_ino
        {
            return Err(LocalFileScanError::EntryChanged);
        }
        if mount_id_at(
            &current,
            std::ffi::OsStr::from_bytes(component),
            AtFlags::SYMLINK_NOFOLLOW,
        )? != entry.mount_id
        {
            return Err(LocalFileScanError::MountBoundary);
        }
        current = next;
    }
    Ok((current, std::ffi::OsString::from_vec(leaf.to_vec())))
}

#[cfg(target_os = "linux")]
fn ensure_entry_name_matches(
    parent: &rustix::fd::OwnedFd,
    leaf: &std::ffi::OsStr,
    entry: &LocalFileScanEntry,
) -> Result<(), LocalFileScanError> {
    use rustix::fs::{AtFlags, statat};

    let named = statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| LocalFileScanError::EntryChanged)?;
    if mount_id_at(parent, leaf, AtFlags::SYMLINK_NOFOLLOW)? != entry.mount_id {
        return Err(LocalFileScanError::MountBoundary);
    }
    if !matches_entry_identity(&named, entry) || named.st_mode & DIRECTORY_TYPE_MASK != 0o100000 {
        return Err(LocalFileScanError::EntryChanged);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn matches_entry_identity(metadata: &rustix::fs::Stat, entry: &LocalFileScanEntry) -> bool {
    metadata.st_dev == entry.identity.device_id && metadata.st_ino == entry.identity.file_id
}

#[cfg(target_os = "linux")]
fn same_file_observation(
    before: &rustix::fs::Stat,
    after: &rustix::fs::Stat,
    entry: &LocalFileScanEntry,
) -> bool {
    matches_entry_identity(after, entry)
        && before.st_size == after.st_size
        && before.st_mtime == after.st_mtime
        && before.st_mtime_nsec == after.st_mtime_nsec
        && before.st_ctime == after.st_ctime
        && before.st_ctime_nsec == after.st_ctime_nsec
}

#[cfg(target_os = "macos")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "scanner primitives are intentionally isolated until the durable WorldIndexer port is integrated"
    )
)]
pub(crate) fn scan_regular_files(
    _selection: &OpenedDirectorySelection,
    _limits: LocalFileScanLimits,
) -> Result<Vec<LocalFileScanEntry>, LocalFileScanError> {
    // macOS currently has no qualified metadata-only handle path in this adapter.
    // Fail closed rather than opening an attacker-swapped device or special file.
    Err(LocalFileScanError::UnsupportedPlatform)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn display_basename(name_bytes: &[u8]) -> String {
    use std::os::unix::ffi::OsStrExt;

    std::ffi::OsStr::from_bytes(name_bytes)
        .to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    #[cfg(target_os = "linux")]
    use super::file_metadata_open_flags;
    use super::{
        LocalDirectoryError, LocalFileScanError, LocalFileScanLimits, open_selected_directory,
        read_bounded_file, read_bounded_file_with_cancel, read_bounded_file_with_hooks,
        scan_regular_files, visit_regular_files,
    };
    use std::{fs, path::Path};
    use tempfile::tempdir;

    #[test]
    fn selected_directory_opens_and_revalidates_the_same_handle_identity() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");

        let opened = open_selected_directory(&selected).expect("open selected directory");
        opened
            .revalidate()
            .expect("same directory identity remains");
        assert_eq!(opened.display_name(), "selected");
        assert!(format!("{opened:?}").contains("<private>"));
    }

    #[test]
    fn selected_directory_rejects_relative_parent_and_filesystem_root_paths() {
        let relative = Path::new("relative");
        assert_eq!(
            open_selected_directory(relative).unwrap_err(),
            LocalDirectoryError::InvalidSelection
        );

        let root = tempdir().expect("temporary directory");
        let traversal = root.path().join("child/../child");
        fs::create_dir(root.path().join("child")).expect("create child");
        assert_eq!(
            open_selected_directory(&traversal).unwrap_err(),
            LocalDirectoryError::InvalidSelection
        );
        assert_eq!(
            open_selected_directory(Path::new("/")).unwrap_err(),
            LocalDirectoryError::InvalidSelection
        );
    }

    #[cfg(unix)]
    #[test]
    fn selected_directory_rejects_final_and_intermediate_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("temporary directory");
        let target = root.path().join("target");
        fs::create_dir(&target).expect("create target");
        let final_link = root.path().join("final-link");
        symlink(&target, &final_link).expect("create final symlink");
        assert_eq!(
            open_selected_directory(&final_link).unwrap_err(),
            LocalDirectoryError::InvalidSelection
        );

        let intermediate = root.path().join("intermediate");
        symlink(&root, &intermediate).expect("create intermediate symlink");
        let through_link = intermediate.join("target");
        assert_eq!(
            open_selected_directory(&through_link).unwrap_err(),
            LocalDirectoryError::OpenFailed
        );
    }

    #[test]
    fn selected_directory_revalidation_detects_replacement_at_saved_path() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        let moved = root.path().join("moved");
        fs::rename(&selected, &moved).expect("move original directory");
        fs::create_dir(&selected).expect("replace selected path");

        assert_eq!(
            opened.revalidate(),
            Err(LocalDirectoryError::IdentityChanged)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn saved_directory_reopen_stops_when_deadline_expires_between_components() {
        use std::cell::Cell;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let checks = Cell::new(0);

        let result = super::reopen_saved_directory_with_cancel(
            opened.private_locator(),
            opened.display_name(),
            || {
                let count = checks.get() + 1;
                checks.set(count);
                count == 4
            },
        );

        assert_eq!(
            result.unwrap_err(),
            LocalDirectoryError::ScanDeadlineExceeded
        );
        assert_eq!(checks.get(), 4, "the walker stops at the expired component");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn failed_directory_io_maps_to_deadline_only_after_expiration() {
        let mut active = || false;
        assert_eq!(
            super::map_failed_directory_io(&mut active),
            LocalDirectoryError::OpenFailed
        );

        let mut expired = || true;
        assert_eq!(
            super::map_failed_directory_io(&mut expired),
            LocalDirectoryError::ScanDeadlineExceeded
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_lists_nested_regular_files_without_exposing_private_locators_in_debug() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::create_dir(selected.join("nested")).expect("create nested directory");
        fs::write(selected.join("top.txt"), b"top").expect("write top file");
        fs::write(selected.join("nested/secret-name.txt"), b"nested").expect("write nested file");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(4, 20, 4096))
            .expect("scan selected directory");
        let mut locators = entries
            .iter()
            .map(|entry| entry.relative_locator.as_slice())
            .collect::<Vec<_>>();
        locators.sort_unstable();

        assert_eq!(
            locators,
            vec![b"nested/secret-name.txt".as_slice(), b"top.txt"]
        );
        let nested = entries
            .iter()
            .find(|entry| entry.display_basename == "secret-name.txt")
            .expect("nested file entry");
        assert_eq!(nested.size_bytes, 6);
        assert!(!format!("{nested:?}").contains("secret-name.txt"));
    }

    #[cfg(all(target_os = "linux", unix))]
    #[test]
    fn scan_skips_symlinked_files_and_directories_that_point_outside_the_root() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside");
        fs::create_dir(&selected).expect("create selected directory");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(selected.join("inside.txt"), b"inside").expect("write in-root file");
        fs::write(outside.join("secret.txt"), b"outside secret").expect("write outside file");
        symlink(outside.join("secret.txt"), selected.join("linked-file.txt"))
            .expect("create file symlink");
        symlink(&outside, selected.join("linked-directory")).expect("create directory symlink");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(4, 20, 4096))
            .expect("scan selected directory");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative_locator, b"inside.txt");
        assert_eq!(entries[0].display_basename, "inside.txt");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn metadata_scan_preserves_file_modification_time_without_reading_content() {
        use std::time::UNIX_EPOCH;
        use time::{OffsetDateTime, format_description::well_known::Rfc3339};

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        let file_path = selected.join("observed.txt");
        fs::write(&file_path, b"metadata only").expect("write test file");
        let metadata = fs::metadata(&file_path).expect("read filesystem metadata");
        let duration = metadata
            .modified()
            .expect("file modification time")
            .duration_since(UNIX_EPOCH)
            .expect("temporary file timestamp is after the epoch");
        let expected = OffsetDateTime::from_unix_timestamp_nanos(duration.as_nanos() as i128)
            .expect("timestamp fits the supported range")
            .format(&Rfc3339)
            .expect("timestamp serializes");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let mut observed = None;
        super::visit_regular_files(
            &opened,
            LocalFileScanLimits::new(4, 20, 4096),
            || false,
            |entry| {
                observed = Some((entry.size_bytes(), entry.modified_at().map(str::to_owned)));
                Ok(())
            },
        )
        .expect("scan selected directory");

        assert_eq!(
            observed,
            Some((b"metadata only".len() as u64, Some(expected)))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn file_modified_at_checks_nanosecond_range_and_handles_pre_epoch_times() {
        assert_eq!(
            super::file_modified_at(-1, 500_000_000).as_deref(),
            Some("1969-12-31T23:59:59.5Z")
        );
        assert_eq!(
            super::file_modified_at(1, 999_999_999).as_deref(),
            Some("1970-01-01T00:00:01.999999999Z")
        );
        assert_eq!(super::file_modified_at(1, 1_000_000_000), None);
        assert_eq!(super::file_modified_at(i64::MAX, 0), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn streaming_scan_stops_when_an_open_descendant_directory_moves_outside_root() {
        use std::cell::Cell;

        let parent = tempdir().expect("temporary directory");
        let selected = parent.path().join("selected");
        let outside = parent.path().join("outside");
        let nested = selected.join("nested");
        fs::create_dir(&selected).expect("create selected directory");
        fs::create_dir(&outside).expect("create sibling outside directory");
        fs::create_dir(&nested).expect("create nested directory");
        fs::write(nested.join("first.txt"), b"first").expect("write first file");
        fs::write(nested.join("second.txt"), b"second").expect("write second file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let observed = Cell::new(0);
        let detached = Cell::new(false);

        let result = super::visit_regular_files(
            &opened,
            LocalFileScanLimits::new(4, 20, 4096),
            || {
                if observed.get() > 0 && !detached.get() {
                    fs::rename(&nested, outside.join("detached"))
                        .expect("move open descendant outside the selected root");
                    detached.set(true);
                }
                false
            },
            |_| {
                observed.set(observed.get() + 1);
                Ok(())
            },
        );

        assert!(detached.get(), "the deterministic race must occur");
        assert_eq!(result, Err(LocalFileScanError::EntryChanged));
        assert_eq!(observed.get(), 1, "no file is visited after detachment");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_does_not_count_or_visit_entries_detached_after_parent_verification() {
        use std::cell::Cell;

        let parent = tempdir().expect("temporary directory");
        let selected = parent.path().join("selected");
        let outside = parent.path().join("outside");
        let nested = selected.join("nested");
        fs::create_dir(&selected).expect("create selected directory");
        fs::create_dir(&outside).expect("create sibling outside directory");
        fs::create_dir(&nested).expect("create nested directory");
        fs::write(nested.join("private-outside.txt"), b"outside").expect("write nested file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let progress = Cell::new(super::LocalFileScanSummary::default());
        let hook_calls = Cell::new(0);
        let detached = Cell::new(false);
        let observed_files = Cell::new(0);
        let mut cancelled = || false;
        let mut report_progress = |snapshot| progress.set(snapshot);
        let mut detach_after_parent_check = || {
            let call = hook_calls.get();
            hook_calls.set(call + 1);
            if call == 1 {
                fs::rename(&nested, outside.join("detached"))
                    .expect("move the verified descendant outside the selected root");
                detached.set(true);
            }
        };

        let result = super::scan_regular_files_inner(
            &opened,
            LocalFileScanLimits::new(4, 20, 4096),
            &mut cancelled,
            &mut report_progress,
            &mut detach_after_parent_check,
            &mut |_| {
                observed_files.set(observed_files.get() + 1);
                Ok(())
            },
        );

        assert!(detached.get(), "the deterministic race must occur");
        assert_eq!(result, Err(LocalFileScanError::EntryChanged));
        assert_eq!(observed_files.get(), 0, "off-tree file is never visited");
        assert_eq!(
            progress.get(),
            super::LocalFileScanSummary {
                entries_visited: 1,
                files_visited: 0,
            },
            "off-tree entries do not inflate partial progress",
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn streaming_scan_reports_monotonic_counts_when_cancelled() {
        use std::cell::Cell;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("first.txt"), b"first").expect("write first file");
        fs::write(selected.join("second.txt"), b"second").expect("write second file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let progress = Cell::new(super::LocalFileScanSummary::default());

        let result = super::visit_regular_files_with_progress(
            &opened,
            LocalFileScanLimits::new(4, 20, 4096),
            || progress.get().files_visited >= 1,
            |_| Ok(()),
            |snapshot| progress.set(snapshot),
        );

        assert_eq!(result, Err(LocalFileScanError::Cancelled));
        assert_eq!(progress.get().entries_visited, 1);
        assert_eq!(progress.get().files_visited, 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_prunes_sensitive_and_generated_paths_before_opening_entries() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        for excluded in [
            ".git",
            ".venv",
            "node_modules",
            "target",
            "dist",
            ".ssh",
            ".aws",
        ] {
            fs::create_dir_all(selected.join(excluded)).expect("create excluded directory");
            fs::write(selected.join(excluded).join("payload.txt"), b"private")
                .expect("write excluded payload");
        }
        for excluded in [
            ".env",
            ".env.local",
            "id_rsa",
            "client.pem",
            "bundle.p12",
            "identity.pfx",
            "signing.key",
            "cloud-credentials.json",
        ] {
            fs::write(selected.join(excluded), b"private").expect("write excluded sensitive file");
        }
        fs::write(selected.join("README.txt"), b"ordinary file").expect("write ordinary file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let mut visited = Vec::new();

        let summary = super::visit_regular_files(
            &opened,
            LocalFileScanLimits::new(8, 128, 16 * 1024),
            || false,
            |entry| {
                visited.push(entry.relative_locator().to_vec());
                Ok(())
            },
        )
        .expect("complete bounded traversal");

        assert_eq!(visited, vec![b"README.txt".to_vec()]);
        assert_eq!(summary.files_visited, 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_counts_resolved_entry_types_but_only_observes_regular_files() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::create_dir(selected.join("nested")).expect("create nested directory");
        fs::write(selected.join("nested/inside.txt"), b"inside")
            .expect("write nested regular file");
        fs::write(selected.join("regular.txt"), b"regular").expect("write regular file");

        let outside_target = root.path().join("outside-secret.txt");
        fs::write(&outside_target, b"must not be followed").expect("write outside target");
        symlink(&outside_target, selected.join("outside-link"))
            .expect("create final-component symlink");

        let selected_handle = rustix::fs::open(
            &selected,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .expect("open selected directory for nonblocking FIFO creation");
        rustix::fs::mkfifoat(&selected_handle, "pipe", rustix::fs::Mode::RWXU)
            .expect("create special FIFO entry");

        fs::create_dir(selected.join(".git")).expect("create excluded generated directory");
        fs::write(selected.join(".git/payload.txt"), b"excluded")
            .expect("write excluded generated payload");
        fs::write(selected.join(".env"), b"excluded secret").expect("write excluded secret file");

        let opened = open_selected_directory(&selected).expect("open selected directory");
        let mut visited = Vec::new();
        let summary = visit_regular_files(
            &opened,
            LocalFileScanLimits::new(8, 7, 4096),
            || false,
            |entry| {
                visited.push(entry.relative_locator().to_vec());
                Ok(())
            },
        )
        .expect("scan resolves directory, symlink object, FIFO, and regular files safely");

        visited.sort_unstable();
        assert_eq!(
            visited,
            vec![b"nested/inside.txt".to_vec(), b"regular.txt".to_vec()],
            "only regular files reach the file visitor"
        );
        assert_eq!(summary.entries_visited, 5);
        assert_eq!(summary.files_visited, 2);

        let budget_result = visit_regular_files(
            &opened,
            LocalFileScanLimits::new(8, 6, 4096),
            || false,
            |_| Ok(()),
        );
        assert_eq!(
            budget_result,
            Err(LocalFileScanError::BudgetExceeded),
            "the excluded .git and .env names consume the internal seven-entry budget"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_fails_closed_when_depth_entry_or_path_budgets_are_exceeded() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::create_dir_all(selected.join("one/two")).expect("create nested directories");
        fs::write(selected.join("one/two/file.txt"), b"file").expect("write nested file");
        fs::write(selected.join("short.txt"), b"short").expect("write root file");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        let depth_error = scan_regular_files(&opened, LocalFileScanLimits::new(1, 20, 4096))
            .expect_err("depth limit should reject an incomplete scan");
        assert_eq!(depth_error, LocalFileScanError::BudgetExceeded);

        let entry_error = scan_regular_files(&opened, LocalFileScanLimits::new(4, 1, 4096))
            .expect_err("entry limit should reject an incomplete scan");
        assert_eq!(entry_error, LocalFileScanError::BudgetExceeded);

        let path_error = scan_regular_files(&opened, LocalFileScanLimits::new(4, 20, 4))
            .expect_err("path limit should reject an incomplete scan");
        assert_eq!(path_error, LocalFileScanError::BudgetExceeded);

        assert!(matches!(
            scan_regular_files(&opened, LocalFileScanLimits::new(0, 20, 4096)),
            Err(LocalFileScanError::InvalidLimits)
        ));
        assert!(matches!(
            scan_regular_files(&opened, LocalFileScanLimits::new(65, 20, 4096)),
            Err(LocalFileScanError::InvalidLimits)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_rejects_entry_limit_above_retained_descriptor_cap() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        assert!(matches!(
            scan_regular_files(&opened, LocalFileScanLimits::new(4, 65, 4096)),
            Err(LocalFileScanError::InvalidLimits)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn streaming_scan_rejects_entry_and_locator_budgets_above_hard_bounds() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        assert_eq!(
            super::visit_regular_files(
                &opened,
                LocalFileScanLimits::new(super::MAX_SCAN_DEPTH, super::MAX_SCAN_ENTRIES + 1, 1),
                || false,
                |_| Ok(()),
            ),
            Err(LocalFileScanError::InvalidLimits)
        );
        assert_eq!(
            super::visit_regular_files(
                &opened,
                LocalFileScanLimits::new(
                    super::MAX_SCAN_DEPTH,
                    1,
                    super::MAX_SCAN_LOCATOR_BYTES + 1,
                ),
                || false,
                |_| Ok(()),
            ),
            Err(LocalFileScanError::InvalidLimits)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn streaming_scan_processes_more_than_sixty_four_files_with_bounded_descriptors() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        for index in 0..96 {
            let directory = selected.join(format!("directory-{index:03}"));
            fs::create_dir(&directory).expect("create child directory");
            fs::write(directory.join("file.txt"), b"data").expect("write file");
        }
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let selected = selected.canonicalize().expect("canonicalize selected root");
        let selected_descriptors = || {
            fs::read_dir("/proc/self/fd")
                .expect("list descriptors")
                .filter_map(Result::ok)
                .filter_map(|entry| fs::read_link(entry.path()).ok())
                .filter(|target| target.starts_with(&selected))
                .count()
        };
        let baseline_fds = selected_descriptors();
        let mut peak_fds = baseline_fds;
        let mut processed = 0;

        let summary = super::visit_regular_files(
            &opened,
            LocalFileScanLimits::new(4, 512, 32 * 1024),
            || false,
            |entry| {
                let observed_fds = selected_descriptors();
                peak_fds = peak_fds.max(observed_fds);
                assert_eq!(
                    read_bounded_file(&opened, entry, 16).expect("read pinned file"),
                    b"data"
                );
                processed += 1;
                Ok(())
            },
        )
        .expect("complete streaming scan");

        assert_eq!(processed, 96);
        assert_eq!(summary.files_visited, 96);
        assert_eq!(summary.entries_visited, 192);
        assert!(
            peak_fds <= baseline_fds + 16,
            "selected-root descriptor count grew from {baseline_fds} to {peak_fds}"
        );
        let after_fds = selected_descriptors();
        assert_eq!(after_fds, baseline_fds);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn streaming_scan_cancellation_is_an_explicit_incomplete_result() {
        use std::cell::Cell;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        for index in 0..4 {
            fs::write(selected.join(format!("file-{index}.txt")), b"data").expect("write file");
        }
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let processed = Cell::new(0);

        let result = super::visit_regular_files(
            &opened,
            LocalFileScanLimits::new(4, 16, 4096),
            || processed.get() == 1,
            |_| {
                processed.set(processed.get() + 1);
                Ok(())
            },
        );

        assert_eq!(result, Err(LocalFileScanError::Cancelled));
        assert_eq!(processed.get(), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_preserves_file_identity_when_the_file_is_renamed_within_the_root() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        let original = selected.join("before.txt");
        let renamed = selected.join("after.txt");
        fs::write(&original, b"same file").expect("write file");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        let before = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan before rename");
        fs::rename(&original, &renamed).expect("rename file");
        let after = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan after rename");

        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), 1);
        assert_ne!(before[0].relative_locator, after[0].relative_locator);
        assert!(before[0].identity == after[0].identity);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn file_identity_open_uses_linux_path_only_flags() {
        let flags = file_metadata_open_flags();
        assert_eq!(
            flags,
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bounded_read_returns_regular_file_bytes_and_rejects_over_limit() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("file.txt"), b"bounded content").expect("write file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan selected directory");

        assert_eq!(
            read_bounded_file(&opened, &entries[0], 64).expect("read pinned file"),
            b"bounded content"
        );
        assert_eq!(
            read_bounded_file(&opened, &entries[0], 4),
            Err(LocalFileScanError::BudgetExceeded)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bounded_read_stops_after_cancellation_between_chunks() {
        use std::cell::Cell;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("large.txt"), vec![b'x'; 32 * 1024])
            .expect("write large test file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan selected directory");
        let checks = Cell::new(0);

        let result = read_bounded_file_with_cancel(&opened, &entries[0], 32 * 1024, || {
            checks.set(checks.get() + 1);
            checks.get() == 3
        });

        assert_eq!(result, Err(LocalFileScanError::Cancelled));
        assert_eq!(checks.get(), 3, "no later chunk is read after cancellation");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bounded_read_rejects_name_replaced_by_symlink_or_fifo_before_procfs_reopen() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        let outside = root.path().join("outside.txt");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("file.txt"), b"pinned content").expect("write file");
        fs::write(&outside, b"outside content").expect("write outside file");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        for replacement in ["symlink", "fifo"] {
            fs::remove_file(selected.join("file.txt")).expect("remove scanned file");
            fs::write(selected.join("file.txt"), b"pinned content").expect("restore file");
            let entries = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
                .expect("scan selected directory");

            let result = read_bounded_file_with_hooks(
                &opened,
                &entries[0],
                64,
                || {
                    fs::remove_file(selected.join("file.txt")).expect("remove scanned name");
                    if replacement == "symlink" {
                        symlink(&outside, selected.join("file.txt")).expect("replace with symlink");
                    } else {
                        use rustix::fs::{Mode, OFlags, open};
                        let parent = open(
                            &selected,
                            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                            Mode::empty(),
                        )
                        .expect("open selected directory");
                        rustix::fs::mkfifoat(&parent, "file.txt", Mode::RWXU)
                            .expect("replace with fifo");
                    }
                },
                || {},
            );

            assert_eq!(result, Err(LocalFileScanError::EntryChanged));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bounded_read_rejects_name_replaced_after_content_read() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("file.txt"), b"pinned content").expect("write file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan selected directory");

        let result = read_bounded_file_with_hooks(
            &opened,
            &entries[0],
            64,
            || {},
            || {
                fs::remove_file(selected.join("file.txt")).expect("remove scanned name");
                fs::write(selected.join("file.txt"), b"replacement").expect("replace scanned name");
            },
        );

        assert_eq!(result, Err(LocalFileScanError::EntryChanged));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bounded_read_rejects_detectable_in_place_mutation_during_read() {
        use std::os::unix::fs::MetadataExt;
        use std::time::{Duration, SystemTime};

        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        let file = selected.join("file.txt");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(&file, b"original").expect("write file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan selected directory");

        let result = read_bounded_file_with_hooks(
            &opened,
            &entries[0],
            64,
            || {},
            || {
                let before = fs::metadata(&file).expect("read original metadata");
                fs::write(&file, b"mutated!").expect("mutate same file in place");
                let content_file = fs::File::open(&file).expect("open mutated file");
                let distinct_modified_time =
                    SystemTime::UNIX_EPOCH + Duration::from_secs(4_102_444_800);
                content_file
                    .set_times(fs::FileTimes::new().set_modified(distinct_modified_time))
                    .expect("set a distinct modification time");
                let after = fs::metadata(&file).expect("read mutated metadata");
                assert_eq!(before.dev(), after.dev());
                assert_eq!(before.ino(), after.ino());
                assert_eq!(before.len(), after.len());
                assert_ne!(
                    before.modified().expect("original modified time"),
                    after.modified().expect("mutated modified time")
                );
            },
        );

        assert_eq!(result, Err(LocalFileScanError::EntryChanged));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bounded_read_rejects_root_replacement_during_read() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("file.txt"), b"pinned content").expect("write file");
        let opened = open_selected_directory(&selected).expect("open selected directory");
        let entries = scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024))
            .expect("scan selected directory");

        let moved = root.path().join("moved");
        let result = read_bounded_file_with_hooks(
            &opened,
            &entries[0],
            64,
            || {},
            || {
                fs::rename(&selected, &moved).expect("move selected root");
                fs::create_dir(&selected).expect("replace selected root");
            },
        );

        assert_eq!(result, Err(LocalFileScanError::RootChanged));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn scan_fails_closed_without_a_safe_metadata_only_file_handle() {
        let root = tempdir().expect("temporary directory");
        let selected = root.path().join("selected");
        fs::create_dir(&selected).expect("create selected directory");
        fs::write(selected.join("file.txt"), b"file").expect("write file");
        let opened = open_selected_directory(&selected).expect("open selected directory");

        assert!(matches!(
            scan_regular_files(&opened, LocalFileScanLimits::new(2, 10, 1024)),
            Err(LocalFileScanError::UnsupportedPlatform)
        ));
    }
}

#[cfg(test)]
mod windows_reparse_guard_tests {
    use super::windows_directory_entry_is_traversable;

    #[test]
    fn windows_root_walker_never_traverses_reparse_points() {
        const DIRECTORY: u32 = 0x0000_0010;
        const REPARSE_POINT: u32 = 0x0000_0400;

        assert!(windows_directory_entry_is_traversable(DIRECTORY));
        assert!(!windows_directory_entry_is_traversable(
            DIRECTORY | REPARSE_POINT
        ));
        assert!(!windows_directory_entry_is_traversable(REPARSE_POINT));
        assert!(!windows_directory_entry_is_traversable(0));
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_workspace_root_revalidation_tests {
    use super::{open_selected_directory, reopen_saved_directory};
    use base64::Engine as _;
    use std::fs;
    use std::os::windows::ffi::OsStrExt;

    #[test]
    fn windows_workspace_root_selection_and_restart_reopen_preserve_identity() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let selected = temporary.path().join("selected");
        fs::create_dir(&selected).expect("create selected folder");

        let opened = open_selected_directory(&selected)
            .expect("open a local Windows folder through a verified handle");
        let identity = opened.identity();
        assert_eq!(identity.platform_kind(), "WINDOWS_VOLUME_FILE_ID_128");
        let binding = identity.binding_record(
            "root-1".to_owned(),
            "runtime-1".to_owned(),
            "incarnation-1".to_owned(),
            "2026-10-17T00:00:00Z".to_owned(),
        );
        assert_eq!(binding.platform_kind, "WINDOWS_VOLUME_FILE_ID_128");
        assert_eq!(binding.raw_filesystem_instance_id.len(), 16);
        assert!(
            binding
                .raw_filesystem_instance_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        assert_eq!(binding.raw_file_id.len(), 32);
        assert!(
            binding
                .raw_file_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        assert!(binding.raw_volume_id.is_none());
        assert!(binding.raw_generation.is_none());
        assert!(identity.matches_binding(&binding));
        let encoded_path = opened
            .private_locator()
            .strip_prefix("windows-path-u16le-b64:")
            .expect("Windows locator prefix");
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded_path)
            .expect("canonical base64url locator");
        let expected: Vec<u8> = selected
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decoded, expected);
        opened
            .revalidate()
            .expect("the selected folder still resolves to the open identity");
        let reopened = reopen_saved_directory(opened.private_locator(), opened.display_name())
            .expect("restart reopen uses the private no-reparse locator");
        assert!(reopened.identity() == identity);
        assert!(reopened.identity().matches_binding(&binding));
    }

    #[test]
    fn windows_workspace_root_revalidation_rejects_path_replacement() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let selected = temporary.path().join("selected");
        let moved = temporary.path().join("moved");
        fs::create_dir(&selected).expect("create selected folder");
        let opened = open_selected_directory(&selected).expect("open selected folder");
        fs::rename(&selected, &moved).expect("move the original directory");
        fs::create_dir(&selected).expect("replace selected path");
        assert!(opened.revalidate().is_err());
        let replacement = open_selected_directory(&selected).expect("open replacement folder");
        assert!(opened.identity() != replacement.identity());
    }

    #[test]
    fn windows_workspace_root_locator_rejects_noncanonical_and_invalid_utf16() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let selected = temporary.path().join("selected");
        fs::create_dir(&selected).expect("create selected folder");
        let opened = open_selected_directory(&selected).expect("open selected folder");
        let name = opened.display_name();
        for locator in [
            "windows-path-u16le-b64:AA==", // padded/noncanonical base64url
            "windows-path-u16le-b64:AA",   // odd UTF-16 byte count
            "windows-path-u16le-b64:AAE",  // malformed UTF-16 path
        ] {
            assert!(reopen_saved_directory(locator, name).is_err());
        }
    }
}
