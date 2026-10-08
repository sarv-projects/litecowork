use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use storage_core::{BlobPurpose, BlobRef, BlobStore, StoreError};
use tempfile::NamedTempFile;
use zeroize::Zeroizing;

const BLOB_HEADER: &[u8; 4] = b"LCB1";
const NONCE_LEN: usize = 24;
const HEADER_LEN: usize = BLOB_HEADER.len() + 4 + NONCE_LEN;
#[cfg(test)]
const MEDIA_TYPE: &str = "application/octet-stream";

pub struct WorkspaceBlobKey {
    pub version: u32,
    pub bytes: Zeroizing<[u8; 32]>,
}

pub trait WorkspaceBlobKeyProvider: Send + Sync {
    fn current_key(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError>;

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError>;
}

pub struct FileBlobStore<K> {
    root: PathBuf,
    keys: K,
}

impl<K> FileBlobStore<K> {
    pub fn new(root: impl Into<PathBuf>, keys: K) -> Self {
        Self {
            root: root.into(),
            keys,
        }
    }

    fn object_path(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        digest: &str,
    ) -> Result<PathBuf, StoreError> {
        let digest_bytes = parse_digest(digest)?;
        let workspace_hash = Sha256::digest(workspace_id.as_bytes());
        let workspace_dir = self
            .root
            .join(hex::encode(workspace_hash))
            .join(purpose.as_str().to_ascii_lowercase());
        Ok(workspace_dir.join(format!("{}.blob", hex::encode(digest_bytes))))
    }
}

impl<K: WorkspaceBlobKeyProvider> BlobStore for FileBlobStore<K> {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        if workspace_id.trim().is_empty() || media_type.trim().is_empty() {
            return Err(StoreError::Invalid(
                "workspace and media type are required for blobs".to_owned(),
            ));
        }

        let digest = digest(bytes);
        let blob = BlobRef {
            digest,
            size_bytes: bytes.len() as u64,
            media_type: media_type.to_owned(),
        };
        let path = self.object_path(workspace_id, purpose, &blob.digest)?;
        if path.exists() {
            let existing = self.get(workspace_id, purpose, &blob)?;
            if existing != bytes {
                return Err(StoreError::Integrity(
                    "existing content-addressed object differs from requested bytes".to_owned(),
                ));
            }
            return Ok(blob);
        }

        let key = self.keys.current_key(workspace_id, purpose)?;
        if key.version == 0 {
            return Err(StoreError::Blob(
                "workspace blob key version must be nonzero".to_owned(),
            ));
        }
        let mut nonce = [0_u8; NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|error| StoreError::Blob(error.to_string()))?;
        let aad = associated_data(workspace_id, purpose, &blob);
        let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_ref())
            .map_err(|_| StoreError::Blob("invalid workspace encryption key".to_owned()))?;
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: bytes,
                    aad: &aad,
                },
            )
            .map_err(|_| StoreError::Blob("blob encryption failed".to_owned()))?;

        let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        envelope.extend_from_slice(BLOB_HEADER);
        envelope.extend_from_slice(&key.version.to_be_bytes());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);

        let parent = path
            .parent()
            .ok_or_else(|| StoreError::Invalid("blob path has no parent".to_owned()))?;
        ensure_private_directory(parent)?;
        let mut temp = NamedTempFile::new_in(parent).map_err(io_error)?;
        temp.write_all(&envelope).map_err(io_error)?;
        temp.as_file().sync_all().map_err(io_error)?;
        match temp.persist_noclobber(&path) {
            Ok(file) => {
                file.sync_all().map_err(io_error)?;
                sync_directory(parent)?;
            }
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = self.get(workspace_id, purpose, &blob)?;
                if existing != bytes {
                    return Err(StoreError::Integrity(
                        "concurrent content-addressed object differs".to_owned(),
                    ));
                }
            }
            Err(error) => return Err(io_error(error.error)),
        }
        Ok(blob)
    }

    fn get(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<Vec<u8>, StoreError> {
        let expected_digest = parse_digest(&blob.digest)?;
        let path = self.object_path(workspace_id, purpose, &blob.digest)?;
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::Invalid("blob path has no parent".to_owned()))?;
        validate_private_directory(parent)?;
        let path_metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::NotFound
            } else {
                io_error(error)
            }
        })?;
        if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
            return Err(StoreError::Integrity(
                "blob object is not a regular file".to_owned(),
            ));
        }
        let mut file = File::open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::NotFound
            } else {
                io_error(error)
            }
        })?;
        let expected_envelope_len = blob
            .size_bytes
            .checked_add((HEADER_LEN + 16) as u64)
            .ok_or_else(|| StoreError::Integrity("blob envelope length overflowed".to_owned()))?;
        let metadata = file.metadata().map_err(io_error)?;
        if !metadata.is_file() || metadata.len() != expected_envelope_len {
            return Err(StoreError::Integrity(
                "blob envelope size does not match its BlobRef".to_owned(),
            ));
        }
        let allocation_len = usize::try_from(expected_envelope_len).map_err(|_| {
            StoreError::Integrity("blob envelope is too large for this Runtime".to_owned())
        })?;
        let mut envelope = vec![0_u8; allocation_len];
        file.read_exact(&mut envelope).map_err(io_error)?;
        if envelope.len() < HEADER_LEN + 16 || &envelope[..4] != BLOB_HEADER {
            return Err(StoreError::Integrity(
                "blob envelope is truncated or unknown".to_owned(),
            ));
        }

        let key_version = u32::from_be_bytes(
            envelope[4..8]
                .try_into()
                .map_err(|_| StoreError::Integrity("blob key version is malformed".to_owned()))?,
        );
        if key_version == 0 {
            return Err(StoreError::Integrity(
                "blob key version must be nonzero".to_owned(),
            ));
        }
        let nonce = XNonce::from_slice(&envelope[8..HEADER_LEN]);
        let key = self
            .keys
            .key_by_version(workspace_id, purpose, key_version)?;
        if key.version != key_version {
            return Err(StoreError::Integrity(
                "key provider returned a mismatched key version".to_owned(),
            ));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_ref())
            .map_err(|_| StoreError::Blob("invalid workspace encryption key".to_owned()))?;
        let aad = associated_data(workspace_id, purpose, blob);
        let plaintext = cipher
            .decrypt(
                nonce,
                Payload {
                    msg: &envelope[HEADER_LEN..],
                    aad: &aad,
                },
            )
            .map_err(|_| StoreError::Integrity("blob authentication failed".to_owned()))?;

        if plaintext.len() as u64 != blob.size_bytes
            || Sha256::digest(&plaintext).as_slice() != expected_digest
        {
            return Err(StoreError::Integrity(
                "blob plaintext size or digest does not match its BlobRef".to_owned(),
            ));
        }
        Ok(plaintext)
    }

    fn remove(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<(), StoreError> {
        let path = self.object_path(workspace_id, purpose, &blob.digest)?;
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::Invalid("blob path has no parent".to_owned()))?;
        match validate_private_directory(parent) {
            Ok(()) => {}
            Err(StoreError::NotFound) => return Ok(()),
            Err(error) => return Err(error),
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(StoreError::Integrity(
                "refusing to remove a non-regular blob object".to_owned(),
            ));
        }
        fs::remove_file(&path).map_err(io_error)?;
        sync_directory(parent)
    }

    fn resource_index_token(
        &self,
        workspace_id: &str,
        key_version: Option<u32>,
        normalized_term: &str,
    ) -> Result<(u32, String), StoreError> {
        let (version, mut tokens) = self.resource_index_tokens(workspace_id, key_version, &[normalized_term.to_owned()])?;
        let token = tokens.pop().ok_or_else(|| StoreError::Integrity("Resource index token is missing".to_owned()))?;
        Ok((version, token))
    }

    fn resource_index_tokens(
        &self,
        workspace_id: &str,
        key_version: Option<u32>,
        normalized_terms: &[String],
    ) -> Result<(u32, Vec<String>), StoreError> {
        if workspace_id.trim().is_empty()
            || normalized_terms.is_empty()
            || normalized_terms.iter().any(|term| term.is_empty() || term.len() > 512 || term.contains('\0'))
        {
            return Err(StoreError::Invalid("Resource index terms are invalid".to_owned()));
        }
        let key = match key_version {
            Some(version) if version > 0 => self
                .keys
                .key_by_version(workspace_id, BlobPurpose::ResourceIndex, version)?,
            Some(_) => return Err(StoreError::Invalid("Resource index key version must be nonzero".to_owned())),
            None => self.keys.current_key(workspace_id, BlobPurpose::ResourceIndex)?,
        };
        if key.version == 0 {
            return Err(StoreError::Blob("Resource index key version must be nonzero".to_owned()));
        }
        let hkdf = Hkdf::<Sha256>::new(Some(b"LiteCowork.ResourceIndex.key.v1"), key.bytes.as_ref());
        let mut mac_key = Zeroizing::new([0_u8; 32]);
        hkdf.expand(b"term-token-hmac-sha256", mac_key.as_mut())
            .map_err(|_| StoreError::Blob("Resource index key derivation failed".to_owned()))?;
        type HmacSha256 = Hmac<Sha256>;
        let mut tokens = Vec::with_capacity(normalized_terms.len());
        for term in normalized_terms {
            let mut mac = HmacSha256::new_from_slice(mac_key.as_ref())
                .map_err(|_| StoreError::Blob("invalid Resource index key".to_owned()))?;
            mac.update(b"LiteCowork.ResourceIndex.term.v1\0");
            mac.update(term.as_bytes());
            tokens.push(hex::encode(mac.finalize().into_bytes()));
        }
        Ok((key.version, tokens))
    }
}

fn associated_data(workspace_id: &str, purpose: BlobPurpose, blob: &BlobRef) -> Vec<u8> {
    format!(
        "litecowork-blob-v1\0{workspace_id}\0{}\0{}\0{}\0{}",
        purpose.as_str(),
        blob.digest,
        blob.size_bytes,
        blob.media_type
    )
    .into_bytes()
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn parse_digest(digest: &str) -> Result<[u8; 32], StoreError> {
    let Some(hex_digest) = digest.strip_prefix("sha256:") else {
        return Err(StoreError::Invalid("BlobRef must use SHA-256".to_owned()));
    };
    let decoded = hex::decode(hex_digest).map_err(|_| {
        StoreError::Invalid("BlobRef digest is not lowercase hexadecimal".to_owned())
    })?;
    if hex_digest.len() != 64
        || hex_digest != hex_digest.to_ascii_lowercase()
        || decoded.len() != 32
    {
        return Err(StoreError::Invalid(
            "BlobRef has an invalid SHA-256 digest".to_owned(),
        ));
    }
    decoded
        .try_into()
        .map_err(|_| StoreError::Invalid("BlobRef digest has an invalid size".to_owned()))
}

fn ensure_private_directory(path: &Path) -> Result<(), StoreError> {
    match validate_private_directory(path) {
        Ok(()) => return Ok(()),
        Err(StoreError::NotFound) => {}
        Err(error) => return Err(error),
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).map_err(io_error)?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path).map_err(io_error)?;

    validate_private_directory(path)
}

fn validate_private_directory(path: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StoreError::NotFound
        } else {
            io_error(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StoreError::Io(
            "blob workspace path must be a real directory".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(StoreError::Io(
                "blob directory grants group or other access".to_owned(),
            ));
        }
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn io_error(error: std::io::Error) -> StoreError {
    StoreError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::sync::Arc;

    #[derive(Clone)]
    struct FixedKeys;

    impl WorkspaceBlobKeyProvider for FixedKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new([7_u8; 32]),
            })
        }

        fn key_by_version(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown key version".to_owned()));
            }
            self.current_key("workspace", BlobPurpose::AggregateState)
        }
    }

    struct MissingKeys;

    impl WorkspaceBlobKeyProvider for MissingKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Err(StoreError::Blob("workspace key unavailable".to_owned()))
        }

        fn key_by_version(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            _version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Err(StoreError::Blob("workspace key unavailable".to_owned()))
        }
    }

    struct WorkspaceScopedKeys;

    impl WorkspaceBlobKeyProvider for WorkspaceScopedKeys {
        fn current_key(
            &self,
            workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new(Sha256::digest(workspace_id.as_bytes()).into()),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown key version".to_owned()));
            }
            self.current_key(workspace_id, purpose)
        }
    }

    #[test]
    fn encrypted_blobs_round_trip_and_are_workspace_scoped() {
        let directory = tempfile::tempdir().expect("temporary blob directory");
        let store = Arc::new(FileBlobStore::new(directory.path(), FixedKeys));
        let blob = store
            .put(
                "workspace-a",
                BlobPurpose::AggregateState,
                br#"{"value":1}"#,
                "application/json",
            )
            .expect("blob committed");

        assert_eq!(
            store
                .get("workspace-a", BlobPurpose::AggregateState, &blob)
                .expect("blob read"),
            br#"{"value":1}"#
        );
        assert!(matches!(
            store.get("workspace-b", BlobPurpose::AggregateState, &blob),
            Err(StoreError::NotFound)
        ));
        assert_eq!(blob.digest, digest(br#"{"value":1}"#));
    }

    #[test]
    fn unavailable_key_provider_fails_closed_without_plaintext_blob() {
        let directory = tempfile::tempdir().expect("temporary blob directory");
        let store = FileBlobStore::new(directory.path(), MissingKeys);
        assert!(matches!(
            store.put("workspace-a", BlobPurpose::Artifact, b"secret", MEDIA_TYPE),
            Err(StoreError::Blob(_))
        ));
        assert_eq!(
            fs::read_dir(directory.path()).expect("blob root").count(),
            0
        );
    }

    #[test]
    fn resource_index_tokens_are_stable_and_workspace_scoped() {
        let directory = tempfile::tempdir().expect("temporary blob directory");
        let store = FileBlobStore::new(directory.path(), WorkspaceScopedKeys);
        let (version, token) = store
            .resource_index_token("workspace-a", None, "architecture")
            .expect("token from OS-style workspace key provider");
        let (same_version, same_token) = store
            .resource_index_token("workspace-a", Some(version), "architecture")
            .expect("token remains stable for the key version");
        let (_, other_workspace_token) = store
            .resource_index_token("workspace-b", None, "architecture")
            .expect("different Workspace has a different scoped key");

        assert_eq!(version, same_version);
        assert_eq!(token, same_token);
        assert_ne!(token, other_workspace_token);
        assert_eq!(token.len(), 64);
    }

    #[test]
    fn rejects_wrong_purpose_and_corrupted_ciphertext() {
        let directory = tempfile::tempdir().expect("temporary blob directory");
        let store = FileBlobStore::new(directory.path(), FixedKeys);
        let blob = store
            .put(
                "workspace-a",
                BlobPurpose::AggregateState,
                b"state",
                MEDIA_TYPE,
            )
            .expect("blob committed");

        let wrong_purpose = store.get("workspace-a", BlobPurpose::Artifact, &blob);
        assert!(matches!(wrong_purpose, Err(StoreError::NotFound)));
        let unused_purpose_path = store
            .object_path("workspace-a", BlobPurpose::Artifact, &blob.digest)
            .expect("unused-purpose path");
        assert!(
            !unused_purpose_path
                .parent()
                .expect("unused-purpose parent")
                .exists()
        );

        let path = store
            .object_path("workspace-a", BlobPurpose::AggregateState, &blob.digest)
            .expect("path");
        let mut contents = fs::read(&path).expect("blob bytes");
        let last = contents.len() - 1;
        contents[last] ^= 0x01;
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .expect("open blob")
            .write_all(&contents)
            .expect("corrupt blob");

        assert!(matches!(
            store.get("workspace-a", BlobPurpose::AggregateState, &blob),
            Err(StoreError::Integrity(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_preexisting_blob_directory_with_broad_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary blob directory");
        let store = FileBlobStore::new(directory.path(), FixedKeys);
        let path = store
            .object_path("workspace-a", BlobPurpose::Artifact, &digest(b"private"))
            .expect("object path");
        let parent = path.parent().expect("object parent");
        fs::create_dir_all(parent).expect("create broad directory");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o755))
            .expect("set broad directory permissions");

        assert!(matches!(
            store.put("workspace-a", BlobPurpose::Artifact, b"private", MEDIA_TYPE,),
            Err(StoreError::Io(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_read_blob_from_directory_with_broad_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary blob directory");
        let store = FileBlobStore::new(directory.path(), FixedKeys);
        let blob = store
            .put("workspace-a", BlobPurpose::Artifact, b"private", MEDIA_TYPE)
            .expect("blob committed");
        let path = store
            .object_path("workspace-a", BlobPurpose::Artifact, &blob.digest)
            .expect("object path");
        fs::set_permissions(
            path.parent().expect("object parent"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("broaden object directory permissions");

        assert!(matches!(
            store.get("workspace-a", BlobPurpose::Artifact, &blob),
            Err(StoreError::Io(_))
        ));
    }
}
