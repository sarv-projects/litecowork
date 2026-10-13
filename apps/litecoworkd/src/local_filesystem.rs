//! Native local filesystem primitives used by WorkspaceRoot providers.
//!
//! The current Tauri-only selection operation calls this module after a native folder
//! chooser, and the private IPC path never enters WebView state. The selected path remains
//! untrusted input; the returned handle and identity are required before a root is stored.
//! A path string alone is never a root grant.

use std::fmt;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::{Component, Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
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

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct RawDirectoryIdentity {
    filesystem_instance_id: u64,
    file_id: u64,
    platform_kind: &'static str,
}

impl RawDirectoryIdentity {
    pub(crate) fn matches_binding(&self, binding: &LocalFileIdentityBindingRecord) -> bool {
        binding.raw_filesystem_instance_id == self.filesystem_instance_id.to_string()
            && binding.raw_volume_id.is_none()
            && binding.raw_file_id == self.file_id.to_string()
            && binding.raw_generation.is_none()
            && binding.platform_kind == self.platform_kind
    }

    pub(crate) fn keyed_projection(
        &self,
        runtime_identity: &impl RuntimeIdentityKey,
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    canonical_path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    private_locator: String,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
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
    // Keep the fail-closed error for targets without qualified native handle identity.
    #[allow(dead_code)]
    UnsupportedPlatform,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    InvalidSelection,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    NotDirectory,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    OpenFailed,
    #[allow(dead_code)]
    IdentityChanged,
}

impl fmt::Display for LocalDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "persistent folder access is unsupported on this platform",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::InvalidSelection => "selected folder is invalid",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            Self::NotDirectory => "selected item is not a folder",
            #[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn path_bytes(path: &Path) -> Result<&[u8], LocalDirectoryError> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes())
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::{LocalDirectoryError, open_selected_directory};
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
}
