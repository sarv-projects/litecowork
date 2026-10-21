use crate::blob::{WorkspaceBlobKey, WorkspaceBlobKeyProvider};
use fs2::FileExt;
use keyring::Entry;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use storage_core::{BlobPurpose, StoreError};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "com.litecowork.workspace-blob-key";
const KEY_BYTES: usize = 32;
static KEYRING_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(windows)]
static WINDOWS_LOCK_DIRECTORY_READY: OnceLock<()> = OnceLock::new();
#[cfg(windows)]
static WINDOWS_LOCK_FILE_READY: OnceLock<()> = OnceLock::new();

trait CredentialStore {
    fn get_password(&self, account: &str) -> Result<Option<Zeroizing<String>>, StoreError>;
    fn set_password(&self, account: &str, password: &str) -> Result<(), StoreError>;
}

#[derive(Clone, Copy, Debug, Default)]
struct OsCredentialStore;

impl CredentialStore for OsCredentialStore {
    fn get_password(&self, account: &str) -> Result<Option<Zeroizing<String>>, StoreError> {
        let entry = platform_entry(account).map_err(|_| key_store_error())?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(key_store_error()),
        }
    }

    fn set_password(&self, account: &str, password: &str) -> Result<(), StoreError> {
        platform_entry(account)
            .map_err(|_| key_store_error())?
            .set_password(password)
            .map_err(|_| key_store_error())
    }
}

/// Stores encrypted-blob keys in the operating system's credential store.
///
/// Workspace identifiers are hashed before they are used as credential-account
/// metadata. The database and blob directory contain no key material. A missing OS
/// credential store is an error; this provider never falls back to a local key file.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsWorkspaceBlobKeyProvider;

impl OsWorkspaceBlobKeyProvider {
    /// Creates and activates the next key version for a Workspace/blob-purpose pair.
    /// Existing blobs remain decryptable because prior versions are retained.
    pub fn rotate_key(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        validate_scope(workspace_id)?;
        let _guard = mutation_lock()?;
        rotate_key_with(&OsCredentialStore, workspace_id, purpose)
    }
}

impl WorkspaceBlobKeyProvider for OsWorkspaceBlobKeyProvider {
    fn current_key(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        validate_scope(workspace_id)?;
        let _guard = mutation_lock()?;
        current_key_with(&OsCredentialStore, workspace_id, purpose)
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        validate_scope(workspace_id)?;
        load_version(&OsCredentialStore, workspace_id, purpose, version)
    }
}

fn rotate_key_with<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
) -> Result<WorkspaceBlobKey, StoreError> {
    validate_scope(workspace_id)?;
    let current_version = read_current_version(credentials, workspace_id, purpose)?;
    if current_version == 0 && version_exists(credentials, workspace_id, purpose, 1)? {
        return Err(key_state_recovery_error());
    }
    if current_version != 0 {
        // Never let rotation conceal a lost/corrupt active key. Existing blobs depend
        // on it even after the active pointer advances.
        drop(load_version(
            credentials,
            workspace_id,
            purpose,
            current_version,
        )?);
    }
    let next_version = current_version
        .checked_add(1)
        .filter(|version| *version != 0)
        .ok_or_else(|| StoreError::Blob("workspace key version exhausted".to_owned()))?;

    // A failed pointer write can leave a valid next-version credential behind. Reuse
    // that staged key after validating it; never replace retained key material.
    if version_exists(credentials, workspace_id, purpose, next_version)? {
        let staged_key = load_version(credentials, workspace_id, purpose, next_version)?;
        return activate_version(credentials, workspace_id, purpose, staged_key);
    }

    let key = create_random_key()?;
    store_version(credentials, workspace_id, purpose, next_version, &key)?;
    activate_version(
        credentials,
        workspace_id,
        purpose,
        WorkspaceBlobKey {
            version: next_version,
            bytes: key,
        },
    )
}

fn current_key_with<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
) -> Result<WorkspaceBlobKey, StoreError> {
    validate_scope(workspace_id)?;
    let version = read_current_version(credentials, workspace_id, purpose)?;
    if version != 0 {
        return load_version(credentials, workspace_id, purpose, version);
    }
    if version_exists(credentials, workspace_id, purpose, 1)? {
        return Err(key_state_recovery_error());
    }

    let key = create_random_key()?;
    store_version(credentials, workspace_id, purpose, 1, &key)?;
    // The active pointer is written last. A crash before this point leaves only an
    // unreachable credential; it cannot make a partially initialized key active.
    activate_version(
        credentials,
        workspace_id,
        purpose,
        WorkspaceBlobKey {
            version: 1,
            bytes: key,
        },
    )
}

fn activate_version<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
    key: WorkspaceBlobKey,
) -> Result<WorkspaceBlobKey, StoreError> {
    match store_current_version(credentials, workspace_id, purpose, key.version) {
        Ok(()) => Ok(key),
        Err(write_error) => match read_current_version(credentials, workspace_id, purpose) {
            Ok(active_version) if active_version == key.version => {
                let observed = load_version(credentials, workspace_id, purpose, key.version)?;
                if observed.bytes.as_ref() != key.bytes.as_ref() {
                    return Err(StoreError::Integrity(
                        "OS key store changed a workspace key during activation".to_owned(),
                    ));
                }
                Ok(observed)
            }
            Ok(_) => Err(write_error),
            Err(reconcile_error) => Err(reconcile_error),
        },
    }
}

fn read_current_version<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
) -> Result<u32, StoreError> {
    match read_secret(credentials, workspace_id, purpose, "active")? {
        None => Ok(0),
        Some(secret) if secret.len() == 8 => {
            let bytes: [u8; 4] = hex::decode(secret.as_str())
                .map_err(|_| {
                    StoreError::Integrity(
                        "OS key store contains a malformed active blob-key version".to_owned(),
                    )
                })?
                .try_into()
                .map_err(|_| {
                    StoreError::Integrity(
                        "OS key store contains a malformed active blob-key version".to_owned(),
                    )
                })?;
            let version = u32::from_be_bytes(bytes);
            if version == 0 {
                return Err(StoreError::Integrity(
                    "OS key store contains a zero active blob-key version".to_owned(),
                ));
            }
            Ok(version)
        }
        Some(_) => Err(StoreError::Integrity(
            "OS key store contains a malformed active blob-key version".to_owned(),
        )),
    }
}

/// A version credential without an active pointer can represent a crash during
/// first provisioning or loss of the active pointer. Never overwrite that key with
/// freshly generated material under the same version number; that would make
/// existing encrypted blobs and ResourceIndex tokens ambiguous.
fn version_exists<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
    version: u32,
) -> Result<bool, StoreError> {
    Ok(read_secret(
        credentials,
        workspace_id,
        purpose,
        &format!("key-{version}"),
    )?
    .is_some())
}

fn store_version<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
    version: u32,
    key: &[u8; KEY_BYTES],
) -> Result<(), StoreError> {
    let encoded = Zeroizing::new(hex::encode(key));
    credentials.set_password(
        &credential_account(workspace_id, purpose, &format!("key-{version}")),
        encoded.as_str(),
    )
}

fn store_current_version<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
    version: u32,
) -> Result<(), StoreError> {
    let encoded = Zeroizing::new(hex::encode(version.to_be_bytes()));
    credentials.set_password(
        &credential_account(workspace_id, purpose, "active"),
        encoded.as_str(),
    )
}

fn load_version<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
    version: u32,
) -> Result<WorkspaceBlobKey, StoreError> {
    if version == 0 {
        return Err(StoreError::Blob(
            "workspace blob key version must be nonzero".to_owned(),
        ));
    }
    let Some(secret) = read_secret(
        credentials,
        workspace_id,
        purpose,
        &format!("key-{version}"),
    )?
    else {
        return Err(StoreError::Blob(
            "workspace blob key is unavailable in the OS credential store".to_owned(),
        ));
    };
    let key_bytes_vec = Zeroizing::new(hex::decode(secret.as_str()).map_err(|_| {
        StoreError::Integrity("OS key store contains a malformed workspace key".to_owned())
    })?);
    let key_bytes: [u8; KEY_BYTES] = key_bytes_vec.as_slice().try_into().map_err(|_| {
        StoreError::Integrity("OS key store contains a malformed workspace key".to_owned())
    })?;
    Ok(WorkspaceBlobKey {
        version,
        bytes: Zeroizing::new(key_bytes),
    })
}

fn read_secret<C: CredentialStore>(
    credentials: &C,
    workspace_id: &str,
    purpose: BlobPurpose,
    item: &str,
) -> Result<Option<Zeroizing<String>>, StoreError> {
    credentials.get_password(&credential_account(workspace_id, purpose, item))
}

fn create_random_key() -> Result<Zeroizing<[u8; KEY_BYTES]>, StoreError> {
    let mut key = Zeroizing::new([0_u8; KEY_BYTES]);
    getrandom::fill(key.as_mut())
        .map_err(|_| StoreError::Blob("secure random unavailable".to_owned()))?;
    Ok(key)
}

fn validate_scope(workspace_id: &str) -> Result<(), StoreError> {
    if workspace_id.trim().is_empty() {
        return Err(StoreError::Invalid(
            "workspace ID is required for blob keys".to_owned(),
        ));
    }
    Ok(())
}

fn credential_account(workspace_id: &str, purpose: BlobPurpose, item: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"litecowork-os-blob-key-v1\0");
    digest.update(workspace_id.as_bytes());
    digest.update([0]);
    digest.update(purpose.as_str().as_bytes());
    let scope = hex::encode(digest.finalize());
    format!("{scope}:{item}")
}

struct KeyringMutationGuard {
    _process_guard: MutexGuard<'static, ()>,
    _file: File,
}

fn mutation_lock() -> Result<KeyringMutationGuard, StoreError> {
    let process_guard = KEYRING_MUTATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| key_store_error())?;
    let path = mutation_lock_path()?;
    let file = open_mutation_lock_file(&path)?;
    #[cfg(windows)]
    if WINDOWS_LOCK_FILE_READY.get().is_none() {
        windows_local_security::secure_current_user_file(&path).map_err(|_| key_store_error())?;
        let _ = WINDOWS_LOCK_FILE_READY.set(());
    }
    file.lock_exclusive().map_err(|_| key_store_error())?;
    Ok(KeyringMutationGuard {
        _process_guard: process_guard,
        _file: file,
    })
}

#[cfg(unix)]
fn mutation_lock_path() -> Result<PathBuf, StoreError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let uid = rustix::process::geteuid().as_raw();
    let directory = Path::new("/tmp").join(format!("litecowork-keyring-{uid}"));
    match fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(key_store_error()),
    }

    let metadata = fs::symlink_metadata(&directory).map_err(|_| key_store_error())?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != uid as u32
        || metadata.mode() & 0o077 != 0
        || metadata.mode() & 0o7000 != 0
    {
        return Err(key_store_error());
    }
    if metadata.mode() & 0o700 != 0o700 {
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| key_store_error())?;
    }
    Ok(directory.join("mutations.lock"))
}

#[cfg(windows)]
fn mutation_lock_path() -> Result<PathBuf, StoreError> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    let profile_data = std::env::var_os("LOCALAPPDATA").ok_or_else(key_store_error)?;
    let profile_data = PathBuf::from(profile_data);
    let profile_metadata = fs::symlink_metadata(&profile_data).map_err(|_| key_store_error())?;
    if !profile_metadata.file_type().is_dir()
        || profile_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(key_store_error());
    }

    let app_directory = profile_data.join("LiteCowork");
    ensure_windows_lock_directory(&app_directory)?;
    let directory = app_directory.join("keyring-locks");
    ensure_windows_lock_directory(&directory)?;
    if WINDOWS_LOCK_DIRECTORY_READY.get().is_none() {
        windows_local_security::secure_current_user_directory(&directory)
            .map_err(|_| key_store_error())?;
        let _ = WINDOWS_LOCK_DIRECTORY_READY.set(());
    }
    let directory_metadata = fs::symlink_metadata(&directory).map_err(|_| key_store_error())?;
    if !directory_metadata.file_type().is_dir()
        || directory_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(key_store_error());
    }
    Ok(directory.join("mutations.lock"))
}

#[cfg(windows)]
fn ensure_windows_lock_directory(path: &Path) -> Result<(), StoreError> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(key_store_error()),
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| key_store_error())?;
    if !metadata.file_type().is_dir()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(key_store_error());
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn mutation_lock_path() -> Result<PathBuf, StoreError> {
    let directory = std::env::temp_dir()
        .join("LiteCowork")
        .join("keyring-locks");
    fs::create_dir_all(&directory).map_err(|_| key_store_error())?;
    let metadata = fs::symlink_metadata(&directory).map_err(|_| key_store_error())?;
    if !metadata.file_type().is_dir() {
        return Err(key_store_error());
    }
    Ok(directory.join("mutations.lock"))
}

fn open_mutation_lock_file(path: &Path) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

        options
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        let file = options.open(path).map_err(|_| key_store_error())?;
        let metadata = file.metadata().map_err(|_| key_store_error())?;
        if !metadata.file_type().is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw() as u32
            || metadata.mode() & 0o077 != 0
            || metadata.mode() & 0o7000 != 0
        {
            return Err(key_store_error());
        }
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        #[cfg(windows)]
        {
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
            let file = options.open(path).map_err(|_| key_store_error())?;
            let metadata = file.metadata().map_err(|_| key_store_error())?;
            if !metadata.file_type().is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(key_store_error());
            }
            Ok(file)
        }
        #[cfg(not(windows))]
        {
            let file = options.open(path).map_err(|_| key_store_error())?;
            if !file
                .metadata()
                .map_err(|_| key_store_error())?
                .file_type()
                .is_file()
            {
                return Err(key_store_error());
            }
            Ok(file)
        }
    }
}

fn platform_entry(account: &str) -> Result<Entry, keyring::Error> {
    #[cfg(target_os = "linux")]
    {
        // The keyutils-persistent builder also exposes a process cache. Use its
        // persistent Secret Service credential directly so rotation recovery never
        // promotes a cache-only key after an interrupted dual-store write.
        let credential =
            keyring::secret_service::SsCredential::new_with_target(None, KEYRING_SERVICE, account)?;
        Ok(Entry::new_with_credential(Box::new(credential)))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Entry::new(KEYRING_SERVICE, account)
    }
}

fn key_store_error() -> StoreError {
    StoreError::Blob("OS credential store is unavailable or rejected the operation".to_owned())
}

fn key_state_recovery_error() -> StoreError {
    StoreError::Blob(
        "OS credential store has a staged key without a matching active pointer; explicit key-state recovery is required".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::process::Command;

    #[derive(Clone, Copy)]
    enum ActiveWriteFailure {
        BeforeWrite,
        AfterWrite,
    }

    #[derive(Default)]
    struct MemoryCredentialStore {
        entries: Mutex<HashMap<String, String>>,
        active_write_failure: Mutex<Option<ActiveWriteFailure>>,
        fail_next_key_write: Mutex<bool>,
        corrupt_staged_key_after_active_write: Mutex<bool>,
        fail_reconciliation_read_after_active_write: Mutex<bool>,
        fail_next_active_read: Mutex<bool>,
    }

    impl MemoryCredentialStore {
        fn fail_next_active_write(&self, failure: ActiveWriteFailure) {
            *self.active_write_failure.lock().expect("failure lock") = Some(failure);
        }

        fn fail_next_key_write(&self) {
            *self.fail_next_key_write.lock().expect("key failure lock") = true;
        }

        fn corrupt_staged_key_after_active_write(&self) {
            *self
                .corrupt_staged_key_after_active_write
                .lock()
                .expect("staged key failure lock") = true;
        }

        fn fail_reconciliation_read_after_active_write(&self) {
            *self
                .fail_reconciliation_read_after_active_write
                .lock()
                .expect("reconciliation failure lock") = true;
        }

        fn read(&self, workspace_id: &str, purpose: BlobPurpose, item: &str) -> String {
            self.entries
                .lock()
                .expect("entry lock")
                .get(&credential_account(workspace_id, purpose, item))
                .expect("credential exists")
                .clone()
        }
    }

    impl CredentialStore for MemoryCredentialStore {
        fn get_password(&self, account: &str) -> Result<Option<Zeroizing<String>>, StoreError> {
            if account.ends_with(":active") {
                let mut fail_next_active_read = self
                    .fail_next_active_read
                    .lock()
                    .map_err(|_| key_store_error())?;
                if std::mem::take(&mut *fail_next_active_read) {
                    return Err(key_store_error());
                }
            }
            let password = self
                .entries
                .lock()
                .map_err(|_| key_store_error())?
                .get(account)
                .cloned();
            Ok(password.map(Zeroizing::new))
        }

        fn set_password(&self, account: &str, password: &str) -> Result<(), StoreError> {
            let is_active = account.ends_with(":active");
            if !is_active && account.contains(":key-") {
                let mut fail_key_write = self
                    .fail_next_key_write
                    .lock()
                    .map_err(|_| key_store_error())?;
                if std::mem::take(&mut *fail_key_write) {
                    return Err(key_store_error());
                }
            }
            let failure = if is_active {
                self.active_write_failure
                    .lock()
                    .map_err(|_| key_store_error())?
                    .take()
            } else {
                None
            };
            if matches!(failure, Some(ActiveWriteFailure::BeforeWrite)) {
                return Err(key_store_error());
            }

            let mut entries = self.entries.lock().map_err(|_| key_store_error())?;
            entries.insert(account.to_owned(), password.to_owned());

            if matches!(failure, Some(ActiveWriteFailure::AfterWrite)) {
                let mut corrupt_staged_key = self
                    .corrupt_staged_key_after_active_write
                    .lock()
                    .map_err(|_| key_store_error())?;
                let corrupt_staged_key = std::mem::take(&mut *corrupt_staged_key);
                if corrupt_staged_key {
                    let prefix = account.strip_suffix(":active").expect("active account");
                    let version = u32::from_be_bytes(
                        hex::decode(password)
                            .map_err(|_| key_store_error())?
                            .try_into()
                            .map_err(|_| key_store_error())?,
                    );
                    entries.insert(
                        format!("{prefix}:key-{version}"),
                        hex::encode([0xA5_u8; KEY_BYTES]),
                    );
                }
                let mut fail_reconciliation_read = self
                    .fail_reconciliation_read_after_active_write
                    .lock()
                    .map_err(|_| key_store_error())?;
                if std::mem::take(&mut *fail_reconciliation_read) {
                    *self
                        .fail_next_active_read
                        .lock()
                        .map_err(|_| key_store_error())? = true;
                }
                return Err(key_store_error());
            }
            Ok(())
        }
    }

    #[test]
    fn retry_after_pointer_write_failure_preserves_the_staged_key_and_old_versions() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-rotation-retry";
        let purpose = BlobPurpose::ResourceIndex;
        let original = current_key_with(&credentials, workspace_id, purpose).unwrap();

        credentials.fail_next_active_write(ActiveWriteFailure::BeforeWrite);
        assert!(rotate_key_with(&credentials, workspace_id, purpose).is_err());
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            1
        );
        let staged = load_version(&credentials, workspace_id, purpose, 2).unwrap();
        let staged_secret = credentials.read(workspace_id, purpose, "key-2");

        let retried = rotate_key_with(&credentials, workspace_id, purpose).unwrap();
        assert_eq!(retried.version, 2);
        assert_eq!(retried.bytes.as_ref(), staged.bytes.as_ref());
        assert_eq!(
            credentials.read(workspace_id, purpose, "key-2"),
            staged_secret
        );
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            2
        );

        let retained = load_version(&credentials, workspace_id, purpose, 1).unwrap();
        assert_eq!(retained.bytes.as_ref(), original.bytes.as_ref());
        assert_eq!(
            current_key_with(&credentials, workspace_id, purpose)
                .unwrap()
                .bytes
                .as_ref(),
            staged.bytes.as_ref()
        );
    }

    #[test]
    fn ambiguous_pointer_write_is_reconciled_from_the_credential_store() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-rotation-ambiguous";
        let purpose = BlobPurpose::Artifact;
        current_key_with(&credentials, workspace_id, purpose).unwrap();

        credentials.fail_next_active_write(ActiveWriteFailure::AfterWrite);
        let rotated = rotate_key_with(&credentials, workspace_id, purpose).unwrap();

        assert_eq!(rotated.version, 2);
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            2
        );
        assert_eq!(
            load_version(&credentials, workspace_id, purpose, 2)
                .unwrap()
                .bytes
                .as_ref(),
            rotated.bytes.as_ref()
        );
    }

    #[test]
    fn reconciliation_read_failure_returns_an_error_without_replacing_old_keys() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-rotation-read-failure";
        let purpose = BlobPurpose::Artifact;
        let original = current_key_with(&credentials, workspace_id, purpose).unwrap();

        credentials.fail_next_active_write(ActiveWriteFailure::AfterWrite);
        credentials.fail_reconciliation_read_after_active_write();
        assert!(rotate_key_with(&credentials, workspace_id, purpose).is_err());

        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            2
        );
        assert_eq!(
            load_version(&credentials, workspace_id, purpose, 1)
                .unwrap()
                .bytes
                .as_ref(),
            original.bytes.as_ref()
        );
        assert!(load_version(&credentials, workspace_id, purpose, 2).is_ok());
    }

    #[test]
    fn reconciliation_rejects_a_changed_staged_key() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-rotation-changed-stage";
        let purpose = BlobPurpose::Artifact;
        let original = current_key_with(&credentials, workspace_id, purpose).unwrap();

        credentials.fail_next_active_write(ActiveWriteFailure::AfterWrite);
        credentials.corrupt_staged_key_after_active_write();
        assert!(matches!(
            rotate_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Integrity(_))
        ));
        assert_eq!(
            load_version(&credentials, workspace_id, purpose, 1)
                .unwrap()
                .bytes
                .as_ref(),
            original.bytes.as_ref()
        );
        assert_eq!(
            load_version(&credentials, workspace_id, purpose, 2)
                .unwrap()
                .bytes
                .as_ref(),
            &[0xA5_u8; KEY_BYTES]
        );
    }

    #[test]
    fn malformed_staged_key_is_not_overwritten() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-rotation-malformed-stage";
        let purpose = BlobPurpose::Resource;
        current_key_with(&credentials, workspace_id, purpose).unwrap();
        let staged_account = credential_account(workspace_id, purpose, "key-2");
        credentials
            .entries
            .lock()
            .unwrap()
            .insert(staged_account.clone(), "not-a-32-byte-key".to_owned());

        assert!(matches!(
            rotate_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Integrity(_))
        ));
        assert_eq!(
            credentials.entries.lock().unwrap().get(&staged_account),
            Some(&"not-a-32-byte-key".to_owned())
        );
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            1
        );
    }

    #[test]
    fn orphaned_first_key_requires_recovery_without_replacement() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-orphaned-first-key";
        let purpose = BlobPurpose::Resource;
        let original_key = [0x5A_u8; KEY_BYTES];
        credentials
            .set_password(
                &credential_account(workspace_id, purpose, "key-1"),
                &hex::encode(original_key),
            )
            .unwrap();

        assert!(matches!(
            current_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Blob(_))
        ));
        assert!(matches!(
            rotate_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Blob(_))
        ));
        assert_eq!(
            credentials.read(workspace_id, purpose, "key-1"),
            hex::encode(original_key)
        );
        assert!(
            credentials
                .get_password(&credential_account(workspace_id, purpose, "active"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn malformed_active_pointer_fails_closed_without_staging_a_key() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-malformed-active";
        let purpose = BlobPurpose::Checkpoint;
        let active_account = credential_account(workspace_id, purpose, "active");
        credentials
            .set_password(&active_account, "invalid")
            .unwrap();

        assert!(matches!(
            current_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Integrity(_))
        ));
        assert!(matches!(
            rotate_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Integrity(_))
        ));
        assert_eq!(
            credentials.entries.lock().unwrap().get(&active_account),
            Some(&"invalid".to_owned())
        );
        assert!(
            credentials
                .get_password(&credential_account(workspace_id, purpose, "key-1"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn staged_key_write_failure_keeps_the_active_key_unchanged() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-staged-write-failure";
        let purpose = BlobPurpose::ResourceIndex;
        let original = current_key_with(&credentials, workspace_id, purpose).unwrap();
        credentials.fail_next_key_write();

        assert!(rotate_key_with(&credentials, workspace_id, purpose).is_err());
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            1
        );
        assert_eq!(
            load_version(&credentials, workspace_id, purpose, 1)
                .unwrap()
                .bytes
                .as_ref(),
            original.bytes.as_ref()
        );
        assert!(!version_exists(&credentials, workspace_id, purpose, 2).unwrap());
    }

    #[test]
    fn rotation_fails_closed_when_the_version_counter_is_exhausted() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-version-exhausted";
        let purpose = BlobPurpose::Artifact;
        credentials
            .set_password(
                &credential_account(workspace_id, purpose, "active"),
                "ffffffff",
            )
            .unwrap();
        credentials
            .set_password(
                &credential_account(workspace_id, purpose, "key-4294967295"),
                &hex::encode([0x3C_u8; KEY_BYTES]),
            )
            .unwrap();

        assert!(matches!(
            rotate_key_with(&credentials, workspace_id, purpose),
            Err(StoreError::Blob(_))
        ));
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            u32::MAX
        );
        assert_eq!(
            credentials.read(workspace_id, purpose, "key-4294967295"),
            hex::encode([0x3C_u8; KEY_BYTES])
        );
    }

    #[test]
    fn rotation_fails_closed_when_the_active_key_is_missing() {
        let credentials = MemoryCredentialStore::default();
        let workspace_id = "workspace-rotation-missing-active";
        let purpose = BlobPurpose::Checkpoint;
        credentials
            .set_password(
                &credential_account(workspace_id, purpose, "active"),
                "00000001",
            )
            .unwrap();

        assert!(load_version(&credentials, workspace_id, purpose, 1).is_err());
        assert!(rotate_key_with(&credentials, workspace_id, purpose).is_err());
        assert!(
            credentials
                .get_password(&credential_account(workspace_id, purpose, "key-2"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            read_current_version(&credentials, workspace_id, purpose).unwrap(),
            1
        );
    }

    #[test]
    fn key_versions_are_scoped_to_workspace_and_blob_purpose() {
        let credentials = MemoryCredentialStore::default();
        let workspace_key = current_key_with(
            &credentials,
            "workspace-key-scope-a",
            BlobPurpose::ResourceIndex,
        )
        .unwrap();
        let other_workspace_key = current_key_with(
            &credentials,
            "workspace-key-scope-b",
            BlobPurpose::ResourceIndex,
        )
        .unwrap();
        let other_purpose_key =
            current_key_with(&credentials, "workspace-key-scope-a", BlobPurpose::Artifact).unwrap();

        assert_eq!(workspace_key.version, 1);
        assert_eq!(other_workspace_key.version, 1);
        assert_eq!(other_purpose_key.version, 1);
        assert!(
            load_version(
                &credentials,
                "workspace-key-scope-b",
                BlobPurpose::Artifact,
                1
            )
            .is_err()
        );
        assert!(
            load_version(
                &credentials,
                "workspace-key-scope-a",
                BlobPurpose::ResourceIndex,
                1
            )
            .unwrap()
            .bytes
            .as_ref()
                == workspace_key.bytes.as_ref()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_provider_uses_the_persistent_secret_service_credential() {
        let entry = platform_entry("backend-contract-probe").unwrap();
        assert!(
            entry
                .get_credential()
                .is::<keyring::secret_service::SsCredential>()
        );
    }

    #[test]
    fn mutation_lock_is_exclusive_across_processes() {
        const PROBE_PATH: &str = "LITECOWORK_KEYRING_LOCK_PROBE_PATH";
        if let Some(path) = std::env::var_os(PROBE_PATH) {
            let file = open_mutation_lock_file(Path::new(&path)).unwrap();
            assert!(file.try_lock_exclusive().is_err());
            return;
        }

        let path = mutation_lock_path().unwrap();
        let _guard = mutation_lock().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "os_key_provider::tests::mutation_lock_is_exclusive_across_processes",
                "--nocapture",
            ])
            .env(PROBE_PATH, &path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child test failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Runs a child test process with a hard time limit so a credential-service
    /// probe can never hang the suite.
    #[cfg(target_os = "linux")]
    fn run_child_with_timeout(
        mut command: Command,
        limit: std::time::Duration,
    ) -> Result<std::process::ExitStatus, String> {
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start the child probe: {error}"))?;
        let deadline = std::time::Instant::now() + limit;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err("the child probe exceeded its time limit".to_owned());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("the child probe failed: {error}")),
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unreachable_credential_service_fails_closed_without_a_local_fallback() {
        const PROBE_MARKER: &str = "LITECOWORK_UNREACHABLE_CREDENTIAL_SERVICE_PROBE";
        if std::env::var_os(PROBE_MARKER).is_some() {
            // The probe runs with an unreachable session bus. The provider must
            // report the missing credential service instead of inventing a
            // local key or an empty key.
            let result = OsWorkspaceBlobKeyProvider.current_key(
                "workspace-unreachable-credential-service",
                BlobPurpose::Artifact,
            );
            match result {
                Err(StoreError::Blob(message)) => {
                    assert!(
                        message.contains("OS credential store"),
                        "unexpected credential-store failure: {message}"
                    );
                }
                Err(other) => panic!("unexpected error kind: {other:?}"),
                Ok(_) => panic!("an unreachable credential service must fail closed"),
            }
            return;
        }

        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "os_key_provider::tests::unreachable_credential_service_fails_closed_without_a_local_fallback",
                "--nocapture",
            ])
            .env(PROBE_MARKER, "1")
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/litecowork-no-such-credential-service-bus",
            )
            .env_remove("XDG_RUNTIME_DIR");
        let status = run_child_with_timeout(command, std::time::Duration::from_secs(60))
            .expect("the unreachable-credential-service probe completes");
        assert!(status.success(), "the child probe must pass");
    }

    // Platform qualification against a live Linux Secret Service lives in
    // `crates/storage-sqlite/tests/linux_secret_service.rs`. It uses only the
    // public provider surface and is run explicitly by
    // `scripts/qualify-os-credential-store.sh`.
}
