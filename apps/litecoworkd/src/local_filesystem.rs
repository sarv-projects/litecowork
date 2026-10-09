//! Native local filesystem primitives used by WorkspaceRoot providers.
//!
//! The current Tauri-only selection operation calls this module after a native folder
//! chooser, and the private IPC path never enters WebView state. The selected path remains
//! untrusted input; the returned handle and identity are required before a root is stored.
//! A path string alone is never a root grant.

use std::{
    fmt,
    path::{Component, Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use storage_core::LocalFileIdentityBindingRecord;
use storage_sqlite::RuntimeOsPrincipalIdentity;

const MAX_SELECTION_PATH_BYTES: usize = 32 * 1024;
const DIRECTORY_TYPE_MASK: u32 = 0o170000;
const DIRECTORY_TYPE: u32 = 0o040000;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RawDirectoryIdentity {
    filesystem_instance_id: u64,
    file_id: u64,
    platform_kind: &'static str,
}

impl RawDirectoryIdentity {
    pub(crate) fn filesystem_instance_id(&self) -> String {
        self.filesystem_instance_id.to_string()
    }

    pub(crate) fn file_id(&self) -> String {
        self.file_id.to_string()
    }

    pub(crate) const fn platform_kind(&self) -> &'static str {
        self.platform_kind
    }

    pub(crate) fn matches_binding(&self, binding: &LocalFileIdentityBindingRecord) -> bool {
        binding.raw_filesystem_instance_id == self.filesystem_instance_id.to_string()
            && binding.raw_volume_id.is_none()
            && binding.raw_file_id == self.file_id.to_string()
            && binding.raw_generation.is_none()
            && binding.platform_kind == self.platform_kind
    }

    pub(crate) fn keyed_projection(
        &self,
        runtime_identity: &RuntimeOsPrincipalIdentity,
    ) -> (String, serde_json::Value) {
        let filesystem_bytes = self.filesystem_instance_id.to_be_bytes();
        let mut file_material = Vec::with_capacity(16);
        file_material.extend_from_slice(&filesystem_bytes);
        file_material.extend_from_slice(&self.file_id.to_be_bytes());
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
            "platform_kind": self.platform_kind,
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
        LocalFileIdentityBindingRecord {
            location_id,
            runtime_id,
            runtime_incarnation_id,
            raw_filesystem_instance_id: self.filesystem_instance_id.to_string(),
            raw_volume_id: None,
            raw_file_id: self.file_id.to_string(),
            raw_generation: None,
            platform_kind: self.platform_kind.to_owned(),
            observed_at,
        }
    }
}

/// An open, verified directory selection. It is not serializable and its Debug output
/// intentionally reveals neither the locator nor operating-system identity.
pub(crate) struct OpenedDirectorySelection {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    handle: rustix::fd::OwnedFd,
    canonical_path: PathBuf,
    private_locator: String,
    display_name: String,
    identity: RawDirectoryIdentity,
}

impl fmt::Debug for OpenedDirectorySelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenedDirectorySelection")
            .field("display_name", &self.display_name)
            .field("identity", &"<private>")
            .field("private_locator", &"<private>")
            .finish_non_exhaustive()
    }
}

impl OpenedDirectorySelection {
    pub(crate) fn private_locator(&self) -> &str {
        &self.private_locator
    }

    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }

    pub(crate) const fn identity(&self) -> RawDirectoryIdentity {
        self.identity
    }

    /// Reopens the exact private locator without following path-component symlinks and
    /// compares it with the retained handle. This detects replacement before admission;
    /// it does not establish identity after process restart.
    pub(crate) fn revalidate(&self) -> Result<(), LocalDirectoryError> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let reopened = open_absolute_directory_without_symlinks(&self.canonical_path)?;
            let current =
                rustix::fs::fstat(&reopened).map_err(|_| LocalDirectoryError::OpenFailed)?;
            let retained =
                rustix::fs::fstat(&self.handle).map_err(|_| LocalDirectoryError::OpenFailed)?;
            if current.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE
                || current.st_dev != retained.st_dev
                || current.st_ino != retained.st_ino
            {
                return Err(LocalDirectoryError::IdentityChanged);
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(LocalDirectoryError::UnsupportedPlatform)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalDirectoryError {
    UnsupportedPlatform,
    InvalidSelection,
    NotDirectory,
    OpenFailed,
    IdentityChanged,
}

impl fmt::Display for LocalDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "persistent folder access is unsupported on this platform",
            Self::InvalidSelection => "selected folder is invalid",
            Self::NotDirectory => "selected item is not a folder",
            Self::OpenFailed => "selected folder could not be opened safely",
            Self::IdentityChanged => "selected folder changed before it could be admitted",
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
    let canonical_path = selected_path
        .canonicalize()
        .map_err(|_| LocalDirectoryError::OpenFailed)?;
    validate_selection_path(&canonical_path)?;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let handle = open_absolute_directory_without_symlinks(&canonical_path)?;
        let metadata = rustix::fs::fstat(&handle).map_err(|_| LocalDirectoryError::OpenFailed)?;
        if metadata.st_mode & DIRECTORY_TYPE_MASK != DIRECTORY_TYPE {
            return Err(LocalDirectoryError::NotDirectory);
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
            canonical_path,
            private_locator,
            display_name,
            identity: RawDirectoryIdentity {
                filesystem_instance_id: metadata.st_dev as u64,
                file_id: metadata.st_ino as u64,
                platform_kind: if cfg!(target_os = "macos") {
                    "MACOS_DEVICE_INODE"
                } else {
                    "LINUX_DEVICE_INODE"
                },
            },
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = canonical_path;
        Err(LocalDirectoryError::UnsupportedPlatform)
    }
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
        let handle = open_absolute_directory_without_symlinks(&path)?;
        let metadata = rustix::fs::fstat(&handle).map_err(|_| LocalDirectoryError::OpenFailed)?;
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
            identity: RawDirectoryIdentity {
                filesystem_instance_id: metadata.st_dev as u64,
                file_id: metadata.st_ino as u64,
                platform_kind: if cfg!(target_os = "macos") {
                    "MACOS_DEVICE_INODE"
                } else {
                    "LINUX_DEVICE_INODE"
                },
            },
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (private_locator, display_name);
        Err(LocalDirectoryError::UnsupportedPlatform)
    }
}

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
    use rustix::fs::{Mode, OFlags, open, openat};

    let mut current = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|_| LocalDirectoryError::OpenFailed)?;
    let mut saw_component = false;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let next = openat(
                    &current,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                    Mode::empty(),
                )
                .map_err(|_| LocalDirectoryError::OpenFailed)?;
                current = next;
                saw_component = true;
            }
            _ => return Err(LocalDirectoryError::InvalidSelection),
        }
    }
    if !saw_component {
        return Err(LocalDirectoryError::InvalidSelection);
    }
    Ok(current)
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Result<&[u8], LocalDirectoryError> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn path_bytes(_path: &Path) -> Result<&[u8], LocalDirectoryError> {
    Err(LocalDirectoryError::UnsupportedPlatform)
}
