use crate::blob::{WorkspaceBlobKey, WorkspaceBlobKeyProvider};
use keyring::Entry;
use sha2::{Digest, Sha256};
use std::sync::{Mutex, OnceLock};
use storage_core::{BlobPurpose, StoreError};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "com.litecowork.workspace-blob-key";
const KEY_BYTES: usize = 32;
static KEYRING_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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
        let _guard = mutation_lock().lock().map_err(|_| key_store_error())?;
        let current_version = self.read_current_version(workspace_id, purpose)?;
        if current_version == 0 && self.version_exists(workspace_id, purpose, 1)? {
            return Err(key_state_recovery_error());
        }
        if current_version != 0 {
            // Never let rotation conceal a lost/corrupt active key. Existing blobs depend
            // on it even after the active pointer advances.
            drop(self.load_version(workspace_id, purpose, current_version)?);
        }
        let next_version = current_version
            .checked_add(1)
            .filter(|version| *version != 0)
            .ok_or_else(|| StoreError::Blob("workspace key version exhausted".to_owned()))?;

        let key = create_random_key()?;
        self.store_version(workspace_id, purpose, next_version, key.as_ref())?;
        self.store_current_version(workspace_id, purpose, next_version)?;

        Ok(WorkspaceBlobKey {
            version: next_version,
            bytes: key,
        })
    }

    fn read_current_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<u32, StoreError> {
        match read_secret(&entry_for(workspace_id, purpose, "active")?)? {
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
    fn version_exists(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<bool, StoreError> {
        match read_secret(&entry_for(workspace_id, purpose, &format!("key-{version}"))?)? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    fn store_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
        key: &[u8; KEY_BYTES],
    ) -> Result<(), StoreError> {
        let encoded = Zeroizing::new(hex::encode(key));
        entry_for(workspace_id, purpose, &format!("key-{version}"))?
            .set_password(encoded.as_str())
            .map_err(|_| key_store_error())
    }

    fn store_current_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<(), StoreError> {
        let encoded = Zeroizing::new(hex::encode(version.to_be_bytes()));
        entry_for(workspace_id, purpose, "active")?
            .set_password(encoded.as_str())
            .map_err(|_| key_store_error())
    }

    fn load_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version == 0 {
            return Err(StoreError::Blob(
                "workspace blob key version must be nonzero".to_owned(),
            ));
        }
        let Some(secret) = read_secret(&entry_for(
            workspace_id,
            purpose,
            &format!("key-{version}"),
        )?)?
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
}

impl WorkspaceBlobKeyProvider for OsWorkspaceBlobKeyProvider {
    fn current_key(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        validate_scope(workspace_id)?;
        let _guard = mutation_lock().lock().map_err(|_| key_store_error())?;
        let version = self.read_current_version(workspace_id, purpose)?;
        if version != 0 {
            return self.load_version(workspace_id, purpose, version);
        }
        if self.version_exists(workspace_id, purpose, 1)? {
            return Err(key_state_recovery_error());
        }

        let key = create_random_key()?;
        self.store_version(workspace_id, purpose, 1, key.as_ref())?;
        // The active pointer is written last. A crash before this point leaves only an
        // unreachable credential; it cannot make a partially initialized key active.
        self.store_current_version(workspace_id, purpose, 1)?;
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: key,
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        validate_scope(workspace_id)?;
        self.load_version(workspace_id, purpose, version)
    }
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

fn entry_for(workspace_id: &str, purpose: BlobPurpose, item: &str) -> Result<Entry, StoreError> {
    let mut digest = Sha256::new();
    digest.update(b"litecowork-os-blob-key-v1\0");
    digest.update(workspace_id.as_bytes());
    digest.update([0]);
    digest.update(purpose.as_str().as_bytes());
    let scope = hex::encode(digest.finalize());
    Entry::new(KEYRING_SERVICE, &format!("{scope}:{item}")).map_err(|_| key_store_error())
}

fn read_secret(entry: &Entry) -> Result<Option<Zeroizing<String>>, StoreError> {
    match entry.get_password() {
        Ok(secret) => Ok(Some(Zeroizing::new(secret))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(key_store_error()),
    }
}

fn mutation_lock() -> &'static Mutex<()> {
    KEYRING_MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

fn key_store_error() -> StoreError {
    StoreError::Blob("OS credential store is unavailable or rejected the operation".to_owned())
}

fn key_state_recovery_error() -> StoreError {
    StoreError::Blob(
        "OS credential store has a version-1 key without an active pointer; explicit key-state recovery is required".to_owned(),
    )
}
