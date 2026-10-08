//! Installation-local OS principal binding for authenticated local Operator IPC.
//!
//! Linux and macOS use the kernel-reported effective UID and a distinct random key in
//! the OS credential store to fingerprint it. The Runtime data directory must already
//! be a private, real directory owned by the same UID. Windows fails closed until its
//! logon-session identity mechanism has been qualified.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use keyring::Entry;
use sha2::{Digest, Sha256};
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::sync::{Mutex, OnceLock};
use storage_core::StoreError;
use zeroize::{Zeroize, Zeroizing};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const KEYRING_SERVICE: &str = "com.litecowork.runtime-os-principal-binding";
const KEY_BYTES: usize = 32;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const BINDING_VERSION: u32 = 1;
#[cfg(any(target_os = "linux", target_os = "macos"))]
static MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Platform recorded in a `RuntimeOsPrincipalBinding`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeOsPlatform {
    Linux,
    MacOs,
}

/// Kernel-authenticated Unix identity used to compare with accepted peer credentials.
///
/// This is the numeric effective UID exposed by Unix-domain-socket peer credentials.
/// It intentionally does not imply per-login-session isolation when sessions share a
/// UID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnixPrincipalId {
    uid: u32,
}

impl UnixPrincipalId {
    pub const fn uid(self) -> u32 {
        self.uid
    }
}

/// Non-secret, installation-local record for validating the expected IPC principal.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeOsPrincipalBinding {
    pub runtime_id: String,
    pub platform: RuntimeOsPlatform,
    /// Keyed digest; it does not contain the UID or credential-store key.
    pub principal_fingerprint: String,
    pub binding_version: u32,
}

/// Result of validating/creating the binding at trusted Runtime startup.
pub struct RuntimeOsPrincipalIdentity {
    pub binding: RuntimeOsPrincipalBinding,
    pub principal: UnixPrincipalId,
    pseudonym_key: Zeroizing<[u8; KEY_BYTES]>,
}

impl RuntimeOsPrincipalIdentity {
    /// Returns a domain-separated pseudonym without exposing the Runtime identity key.
    /// The key remains in zeroizing memory and is never serializable or Debug-formatted.
    pub fn keyed_pseudonym(&self, namespace: &str, material: &[u8]) -> String {
        let mut message = Vec::with_capacity(namespace.len() + material.len() + 48);
        message.extend_from_slice(b"litecow-runtime-keyed-pseudonym-v1\0");
        message.extend_from_slice(namespace.as_bytes());
        message.push(0);
        message.extend_from_slice(material);
        hex::encode(hmac_sha256(self.pseudonym_key.as_ref(), &message))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OsRuntimePrincipalBindingProvider;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct StoredBinding {
    runtime_id: String,
    platform: RuntimeOsPlatform,
    principal_fingerprint: String,
    binding_version: u32,
    fingerprint_key_hex: String,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for StoredBinding {
    fn drop(&mut self) {
        self.fingerprint_key_hex.zeroize();
    }
}

impl OsRuntimePrincipalBindingProvider {
    /// Loads or creates this Runtime installation's OS-principal binding.
    ///
    /// Call only after the Runtime's single-instance lock is held. The credential
    /// account is scoped to the canonical private data directory. Reusing an account
    /// with another RuntimeId, changing the OS principal, or losing access to the
    /// credential store fails closed. `allow_create` must be true only during the
    /// explicit first-install bootstrap. Existing installations pass false so a lost
    /// credential entry (including after moving the data directory) requires recovery
    /// instead of silently adopting a new OS principal binding.
    pub fn load_or_create(
        &self,
        data_directory: &Path,
        runtime_id: &str,
        allow_create: bool,
    ) -> Result<RuntimeOsPrincipalIdentity, StoreError> {
        if runtime_id.trim().is_empty() {
            return Err(StoreError::Invalid("RuntimeId is required".to_owned()));
        }

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.load_or_create_unix(data_directory, runtime_id, allow_create)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (data_directory, runtime_id, allow_create);
            Err(StoreError::Blob(
                "OS principal binding is not qualified on this platform".to_owned(),
            ))
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn load_or_create_unix(
        &self,
        data_directory: &Path,
        runtime_id: &str,
        allow_create: bool,
    ) -> Result<RuntimeOsPrincipalIdentity, StoreError> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let platform = current_unix_platform();
        let principal = unix_process_principal()?;
        let path_metadata = std::fs::symlink_metadata(data_directory)
            .map_err(|_| unsafe_directory_error())?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.is_dir()
            || path_metadata.uid() != principal.uid()
            || path_metadata.permissions().mode() & 0o777 != 0o700
        {
            return Err(unsafe_directory_error());
        }
        let canonical_directory = data_directory
            .canonicalize()
            .map_err(|_| unsafe_directory_error())?;
        let canonical_metadata = std::fs::symlink_metadata(&canonical_directory)
            .map_err(|_| unsafe_directory_error())?;
        if canonical_metadata.file_type().is_symlink()
            || !canonical_metadata.is_dir()
            || canonical_metadata.uid() != principal.uid()
            || canonical_metadata.permissions().mode() & 0o777 != 0o700
        {
            return Err(unsafe_directory_error());
        }

        let account = account_for(&canonical_directory);
        let entry = Entry::new(KEYRING_SERVICE, &account).map_err(|_| credential_store_error())?;
        let _guard = mutation_lock().lock().map_err(|_| credential_store_error())?;

        let stored = match entry.get_password() {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                serde_json::from_str::<StoredBinding>(&secret).map_err(|_| {
                    StoreError::Integrity("stored OS principal binding is malformed".to_owned())
                })?
            }
            Err(keyring::Error::NoEntry) => {
                if !allow_create {
                    return Err(StoreError::Integrity(
                        "OS principal binding is missing; explicit local recovery is required"
                            .to_owned(),
                    ));
                }
                let mut key = Zeroizing::new([0_u8; KEY_BYTES]);
                getrandom::fill(key.as_mut())
                    .map_err(|_| StoreError::Blob("secure random unavailable".to_owned()))?;
                let fingerprint = fingerprint(key.as_ref(), platform, principal);
                let stored = StoredBinding {
                    runtime_id: runtime_id.to_owned(),
                    platform,
                    principal_fingerprint: fingerprint,
                    binding_version: BINDING_VERSION,
                    fingerprint_key_hex: hex::encode(key.as_ref()),
                };
                let encoded = Zeroizing::new(serde_json::to_string(&stored).map_err(|_| {
                    StoreError::Blob("could not encode OS principal binding".to_owned())
                })?);
                entry
                    .set_password(encoded.as_str())
                    .map_err(|_| credential_store_error())?;
                stored
            }
            Err(_) => return Err(credential_store_error()),
        };

        if stored.runtime_id != runtime_id
            || stored.platform != platform
            || stored.binding_version != BINDING_VERSION
        {
            return Err(StoreError::Integrity(
                "OS principal binding does not match this Runtime installation".to_owned(),
            ));
        }

        let key_hex = Zeroizing::new(stored.fingerprint_key_hex.clone());
        let key_bytes = Zeroizing::new(hex::decode(key_hex.as_str()).map_err(|_| {
            StoreError::Integrity("OS principal fingerprint key is malformed".to_owned())
        })?);
        let key: Zeroizing<[u8; KEY_BYTES]> = Zeroizing::new(
            key_bytes.as_slice().try_into().map_err(|_| {
                StoreError::Integrity("OS principal fingerprint key has an invalid size".to_owned())
            })?,
        );
        let expected_fingerprint = fingerprint(key.as_ref(), platform, principal);
        if stored.principal_fingerprint != expected_fingerprint {
            return Err(StoreError::Integrity(
                "OS principal changed for this Runtime installation".to_owned(),
            ));
        }

        Ok(RuntimeOsPrincipalIdentity {
            binding: RuntimeOsPrincipalBinding {
                runtime_id: stored.runtime_id.clone(),
                platform: stored.platform,
                principal_fingerprint: stored.principal_fingerprint.clone(),
                binding_version: stored.binding_version,
            },
            principal,
            pseudonym_key: key,
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn current_unix_platform() -> RuntimeOsPlatform {
    #[cfg(target_os = "linux")]
    {
        RuntimeOsPlatform::Linux
    }
    #[cfg(target_os = "macos")]
    {
        RuntimeOsPlatform::MacOs
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn unix_process_principal() -> Result<UnixPrincipalId, StoreError> {
    // rustix exposes the kernel effective UID without unsafe code. Failure to obtain
    // it disables IPC principal binding rather than guessing from environment data.
    Ok(UnixPrincipalId {
        uid: rustix::process::geteuid().as_raw(),
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn fingerprint(
    key: &[u8; KEY_BYTES],
    platform: RuntimeOsPlatform,
    principal: UnixPrincipalId,
) -> String {
    let platform_tag = match platform {
        RuntimeOsPlatform::Linux => b"linux-uid".as_slice(),
        RuntimeOsPlatform::MacOs => b"macos-uid".as_slice(),
    };
    let mut message = Vec::with_capacity(48);
    message.extend_from_slice(b"litecowork-runtime-os-principal-v1\0");
    message.extend_from_slice(platform_tag);
    message.push(0);
    message.extend_from_slice(&principal.uid().to_be_bytes());
    format!("hmac-sha256:{}", hex::encode(hmac_sha256(key, &message)))
}

/// RFC 2104 HMAC-SHA-256, kept local to avoid broadening the storage crate's
/// dependency surface for this single fixed-size fingerprint operation.
fn hmac_sha256(key: &[u8; KEY_BYTES], message: &[u8]) -> [u8; 32] {
    const BLOCK_BYTES: usize = 64;
    let mut inner_pad = [0x36_u8; BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; BLOCK_BYTES];
    for (index, byte) in key.iter().copied().enumerate() {
        inner_pad[index] ^= byte;
        outer_pad[index] ^= byte;
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    let digest = outer.finalize();

    inner_pad.zeroize();
    outer_pad.zeroize();
    digest.into()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn account_for(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    let mut digest = Sha256::new();
    digest.update(b"litecowork-runtime-os-principal-binding-account-v1\0");
    digest.update(path.as_os_str().as_bytes());
    hex::encode(digest.finalize())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn mutation_lock() -> &'static Mutex<()> {
    MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn unsafe_directory_error() -> StoreError {
    StoreError::Blob("Runtime state directory is not private and Runtime-owned".to_owned())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn credential_store_error() -> StoreError {
    StoreError::Blob("OS credential store is unavailable or rejected the operation".to_owned())
}
