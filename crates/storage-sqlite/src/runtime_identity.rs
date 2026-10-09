use ed25519_dalek::SigningKey;
use keyring::Entry;
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};
use storage_core::{DeviceIdentityRecord, StoreError};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zeroize::{Zeroize, Zeroizing};

const KEYRING_SERVICE: &str = "com.litecowork.runtime-device-signing-key";
const SEED_BYTES: usize = 32;
static MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default)]
pub struct OsRuntimeDeviceIdentityProvider;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct StoredSigningIdentity {
    runtime_id: String,
    key_version: u32,
    private_seed_hex: String,
    issued_at: String,
}

impl Drop for StoredSigningIdentity {
    fn drop(&mut self) {
        self.private_seed_hex.zeroize();
    }
}

impl OsRuntimeDeviceIdentityProvider {
    /// Loads or creates a stable Ed25519 identity in the OS credential store. The
    /// account name contains only a digest of the canonical private data-directory
    /// path. A persisted key bound to another RuntimeId fails closed.
    pub fn load_or_create(
        &self,
        data_directory: &Path,
        runtime_id: &str,
        now: &str,
    ) -> Result<DeviceIdentityRecord, StoreError> {
        if runtime_id.trim().is_empty() {
            return Err(StoreError::Invalid("RuntimeId is required".to_owned()));
        }
        validate_timestamp(now)?;
        let canonical_directory = data_directory
            .canonicalize()
            .map_err(|_| key_store_error())?;
        let account = account_for(&canonical_directory);
        let entry = Entry::new(KEYRING_SERVICE, &account).map_err(|_| key_store_error())?;
        let _guard = mutation_lock().lock().map_err(|_| key_store_error())?;

        let mut stored = match entry.get_password() {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                serde_json::from_str::<StoredSigningIdentity>(&secret).map_err(|_| {
                    StoreError::Integrity("stored Runtime signing identity is malformed".to_owned())
                })?
            }
            Err(keyring::Error::NoEntry) => {
                let mut seed = Zeroizing::new([0_u8; SEED_BYTES]);
                getrandom::fill(seed.as_mut())
                    .map_err(|_| StoreError::Blob("secure random unavailable".to_owned()))?;
                let secret = StoredSigningIdentity {
                    runtime_id: runtime_id.to_owned(),
                    key_version: 1,
                    private_seed_hex: hex::encode(seed.as_ref()),
                    issued_at: now.to_owned(),
                };
                let encoded = Zeroizing::new(serde_json::to_string(&secret).map_err(|_| {
                    StoreError::Blob("could not encode Runtime signing identity".to_owned())
                })?);
                entry
                    .set_password(encoded.as_str())
                    .map_err(|_| key_store_error())?;
                secret
            }
            Err(_) => return Err(key_store_error()),
        };

        if stored.runtime_id != runtime_id || stored.key_version != 1 {
            return Err(StoreError::Integrity(
                "OS credential identity does not match the bootstrap Runtime identity".to_owned(),
            ));
        }
        validate_timestamp(&stored.issued_at)?;
        let seed_hex = Zeroizing::new(std::mem::take(&mut stored.private_seed_hex));
        let seed_bytes = Zeroizing::new(hex::decode(seed_hex.as_str()).map_err(|_| {
            StoreError::Integrity(
                "OS credential store contains an invalid Runtime signing key".to_owned(),
            )
        })?);
        let seed: Zeroizing<[u8; SEED_BYTES]> =
            Zeroizing::new(seed_bytes.as_slice().try_into().map_err(|_| {
                StoreError::Integrity(
                    "OS credential store contains an invalid Runtime signing key".to_owned(),
                )
            })?);
        // The `ed25519-dalek/zeroize` feature implements Drop for SigningKey and
        // zeroizes its private scalar. Limit its lifetime to public-key derivation.
        let public_key = {
            let signing_key = SigningKey::from_bytes(&*seed);
            signing_key.verifying_key().to_bytes()
        };
        let mut device_digest = Sha256::new();
        device_digest.update(public_key);

        Ok(DeviceIdentityRecord {
            device_id: format!("ed25519-sha256:{}", hex::encode(device_digest.finalize())),
            public_key: format!("ed25519:{}", hex::encode(public_key)),
            key_version: stored.key_version,
            issued_at: stored.issued_at.clone(),
            display_name: None,
        })
    }
}

fn account_for(path: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(b"litecowork-runtime-device-identity-v1\0");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        digest.update(path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in path.as_os_str().encode_wide() {
            digest.update(unit.to_le_bytes());
        }
    }
    #[cfg(not(any(unix, windows)))]
    digest.update(path.as_os_str().to_string_lossy().as_bytes());
    hex::encode(digest.finalize())
}

fn validate_timestamp(value: &str) -> Result<(), StoreError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(|_| ())
        .map_err(|_| StoreError::Invalid("Runtime identity timestamp is invalid".to_owned()))
}

fn mutation_lock() -> &'static Mutex<()> {
    MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

fn key_store_error() -> StoreError {
    StoreError::Blob("OS credential store is unavailable or rejected the operation".to_owned())
}
