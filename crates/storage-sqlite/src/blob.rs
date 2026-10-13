use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
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
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::Invalid("blob path has no parent".to_owned()))?;
        match validate_private_directory_tree(&self.root, parent) {
            Ok(()) | Err(StoreError::NotFound) => {}
            Err(error) => return Err(error),
        }
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

        ensure_private_directory_tree(&self.root, parent)?;
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
        validate_private_directory_tree(&self.root, parent)?;
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
        match validate_private_directory_tree(&self.root, parent) {
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
        let (version, mut tokens) =
            self.resource_index_tokens(workspace_id, key_version, &[normalized_term.to_owned()])?;
        let token = tokens
            .pop()
            .ok_or_else(|| StoreError::Integrity("Resource index token is missing".to_owned()))?;
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
            || normalized_terms
                .iter()
                .any(|term| term.is_empty() || term.len() > 512 || term.contains('\0'))
        {
            return Err(StoreError::Invalid(
                "Resource index terms are invalid".to_owned(),
            ));
        }
        let key = match key_version {
            Some(version) if version > 0 => {
                self.keys
                    .key_by_version(workspace_id, BlobPurpose::ResourceIndex, version)?
            }
            Some(_) => {
                return Err(StoreError::Invalid(
                    "Resource index key version must be nonzero".to_owned(),
                ));
            }
            None => self
                .keys
                .current_key(workspace_id, BlobPurpose::ResourceIndex)?,
        };
        if key.version == 0 {
            return Err(StoreError::Blob(
                "Resource index key version must be nonzero".to_owned(),
            ));
        }
        let hkdf =
            Hkdf::<Sha256>::new(Some(b"LiteCowork.ResourceIndex.key.v1"), key.bytes.as_ref());
        let mut mac_key = Zeroizing::new([0_u8; 32]);
        hkdf.expand(b"term-token-hmac-sha256", mac_key.as_mut())
            .map_err(|_| StoreError::Blob("Resource index key derivation failed".to_owned()))?;
        type HmacSha256 = Hmac<Sha256>;
        let mut tokens = Vec::with_capacity(normalized_terms.len());
        for term in normalized_terms {
            let mut mac = <HmacSha256 as Mac>::new_from_slice(mac_key.as_ref())
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

fn ensure_private_directory_tree(root: &Path, target: &Path) -> Result<(), StoreError> {
    let relative = target.strip_prefix(root).map_err(|_| {
        StoreError::Invalid("blob directory is outside the configured root".to_owned())
    })?;
    ensure_private_directory(root)?;

    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(StoreError::Invalid(
                "blob directory path is not normalized beneath its root".to_owned(),
            ));
        };
        current.push(name);
        ensure_private_directory(&current)?;
    }
    Ok(())
}

fn validate_private_directory_tree(root: &Path, target: &Path) -> Result<(), StoreError> {
    let relative = target.strip_prefix(root).map_err(|_| {
        StoreError::Invalid("blob directory is outside the configured root".to_owned())
    })?;
    validate_private_directory(root)?;

    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(StoreError::Invalid(
                "blob directory path is not normalized beneath its root".to_owned(),
            ));
        };
        current.push(name);
        validate_private_directory(&current)?;
    }
    Ok(())
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

    fn private_tempdir() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary blob directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
                .expect("restrict temporary blob root");
        }
        directory
    }

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
        let directory = private_tempdir();
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
        let directory = private_tempdir();
        let store = FileBlobStore::new(directory.path(), MissingKeys);
        assert!(matches!(
            store.put("workspace-a", BlobPurpose::Artifact, b"secret", MEDIA_TYPE),
            Err(StoreError::Blob(_))
        ));
        assert_eq!(
            fs::read_dir(directory.path()).expect("blob root").count(),
            0
        );

        let encrypted_store = FileBlobStore::new(directory.path(), FixedKeys);
        let blob = encrypted_store
            .put("workspace-a", BlobPurpose::Artifact, b"secret", MEDIA_TYPE)
            .expect("write ciphertext while key is available");
        let unavailable_store = FileBlobStore::new(directory.path(), MissingKeys);
        assert!(matches!(
            unavailable_store.get("workspace-a", BlobPurpose::Artifact, &blob),
            Err(StoreError::Blob(_))
        ));
    }

    #[test]
    fn resource_index_tokens_are_stable_and_workspace_scoped() {
        let directory = private_tempdir();
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
        let directory = private_tempdir();
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

    #[test]
    fn rejects_valid_ciphertext_transplanted_across_workspace_or_purpose() {
        let directory = private_tempdir();
        let store = FileBlobStore::new(directory.path(), FixedKeys);
        let bytes = b"same plaintext and BlobRef across scopes";
        let aggregate = store
            .put(
                "workspace-a",
                BlobPurpose::AggregateState,
                bytes,
                MEDIA_TYPE,
            )
            .expect("aggregate blob committed");
        let other_workspace = store
            .put(
                "workspace-b",
                BlobPurpose::AggregateState,
                bytes,
                MEDIA_TYPE,
            )
            .expect("other Workspace blob committed");
        let other_purpose = store
            .put("workspace-a", BlobPurpose::Artifact, bytes, MEDIA_TYPE)
            .expect("other purpose blob committed");
        assert_eq!(aggregate, other_workspace);
        assert_eq!(aggregate, other_purpose);

        let aggregate_path = store
            .object_path(
                "workspace-a",
                BlobPurpose::AggregateState,
                &aggregate.digest,
            )
            .expect("aggregate object path");
        let aggregate_envelope = fs::read(aggregate_path).expect("aggregate envelope");

        let other_workspace_path = store
            .object_path(
                "workspace-b",
                BlobPurpose::AggregateState,
                &other_workspace.digest,
            )
            .expect("other Workspace object path");
        fs::write(&other_workspace_path, &aggregate_envelope)
            .expect("transplant envelope to another Workspace");
        assert!(matches!(
            store.get(
                "workspace-b",
                BlobPurpose::AggregateState,
                &other_workspace
            ),
            Err(StoreError::Integrity(message)) if message == "blob authentication failed"
        ));

        let other_purpose_path = store
            .object_path("workspace-a", BlobPurpose::Artifact, &other_purpose.digest)
            .expect("other purpose object path");
        fs::write(&other_purpose_path, &aggregate_envelope)
            .expect("transplant envelope to another purpose");
        assert!(matches!(
            store.get("workspace-a", BlobPurpose::Artifact, &other_purpose),
            Err(StoreError::Integrity(message)) if message == "blob authentication failed"
        ));
    }

    #[test]
    fn remove_is_idempotent_for_missing_blob_objects() {
        let directory = private_tempdir();
        let store = FileBlobStore::new(directory.path(), FixedKeys);
        let blob = store
            .put(
                "workspace-a",
                BlobPurpose::Artifact,
                b"remove me",
                MEDIA_TYPE,
            )
            .expect("blob committed");
        let path = store
            .object_path("workspace-a", BlobPurpose::Artifact, &blob.digest)
            .expect("object path");

        store
            .remove("workspace-a", BlobPurpose::Artifact, &blob)
            .expect("first deletion succeeds");
        assert!(!path.exists(), "the exact object was removed");
        store
            .remove("workspace-a", BlobPurpose::Artifact, &blob)
            .expect("deleting an absent object is idempotent");
        store
            .remove("workspace-a", BlobPurpose::Artifact, &blob)
            .expect("repeated deletion remains idempotent");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_remove_unsafe_blob_objects() {
        use std::os::unix::fs::symlink;

        let directory = private_tempdir();
        let sentinel = tempfile::NamedTempFile::new().expect("outside sentinel");
        fs::write(sentinel.path(), b"must remain untouched").expect("sentinel contents");
        let store = FileBlobStore::new(directory.path(), FixedKeys);
        let blob = store
            .put(
                "workspace-a",
                BlobPurpose::Artifact,
                b"replace me",
                MEDIA_TYPE,
            )
            .expect("blob committed");
        let path = store
            .object_path("workspace-a", BlobPurpose::Artifact, &blob.digest)
            .expect("object path");

        fs::remove_file(&path).expect("remove original object for adversarial setup");
        symlink(sentinel.path(), &path).expect("replace object with symlink");
        assert!(matches!(
            store.remove("workspace-a", BlobPurpose::Artifact, &blob),
            Err(StoreError::Integrity(_))
        ));
        assert!(
            fs::symlink_metadata(&path)
                .expect("unsafe object remains for diagnosis")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read(sentinel.path()).expect("sentinel remains readable"),
            b"must remain untouched"
        );

        fs::remove_file(&path).expect("remove symlink before second case");
        fs::create_dir(&path).expect("replace object with directory");
        assert!(matches!(
            store.remove("workspace-a", BlobPurpose::Artifact, &blob),
            Err(StoreError::Integrity(_))
        ));
        assert!(path.is_dir(), "unsafe directory remains untouched");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_blob_directory_ancestors() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temporary = tempfile::tempdir().expect("temporary blob test root");
        let external_root = tempfile::tempdir().expect("external target directory");
        fs::set_permissions(external_root.path(), fs::Permissions::from_mode(0o700))
            .expect("restrict external target directory");
        let linked_root = temporary.path().join("linked-root");
        symlink(external_root.path(), &linked_root).expect("link configured blob root");
        let linked_root_store = FileBlobStore::new(&linked_root, FixedKeys);

        assert!(matches!(
            linked_root_store.put(
                "workspace-a",
                BlobPurpose::Artifact,
                b"must stay in configured root",
                MEDIA_TYPE,
            ),
            Err(StoreError::Io(_))
        ));
        assert_eq!(
            fs::read_dir(external_root.path())
                .expect("external root remains readable")
                .count(),
            0,
            "a symlinked configured root must not receive blob directories"
        );

        let configured_root = temporary.path().join("blob-root");
        fs::create_dir(&configured_root).expect("create real configured root");
        fs::set_permissions(&configured_root, fs::Permissions::from_mode(0o700))
            .expect("restrict real configured root");
        let workspace_directory = hex::encode(Sha256::digest(b"workspace-a"));
        let linked_workspace = configured_root.join(workspace_directory);
        symlink(external_root.path(), &linked_workspace).expect("link Workspace directory");
        let workspace_link_store = FileBlobStore::new(&configured_root, FixedKeys);

        assert!(matches!(
            workspace_link_store.put(
                "workspace-a",
                BlobPurpose::Artifact,
                b"must stay in workspace scope",
                MEDIA_TYPE,
            ),
            Err(StoreError::Io(_))
        ));
        assert_eq!(
            fs::read_dir(external_root.path())
                .expect("external target remains readable")
                .count(),
            0,
            "a symlinked Workspace directory must not receive purpose/object files"
        );

        let external_store = FileBlobStore::new(external_root.path(), FixedKeys);
        let outside_blob = external_store
            .put(
                "workspace-a",
                BlobPurpose::Artifact,
                b"preserve external object",
                MEDIA_TYPE,
            )
            .expect("create an object outside the configured root");
        let outside_path = external_store
            .object_path("workspace-a", BlobPurpose::Artifact, &outside_blob.digest)
            .expect("external object path");
        assert!(matches!(
            workspace_link_store.get("workspace-a", BlobPurpose::Artifact, &outside_blob),
            Err(StoreError::Io(_))
        ));
        assert!(matches!(
            workspace_link_store.remove("workspace-a", BlobPurpose::Artifact, &outside_blob),
            Err(StoreError::Io(_))
        ));
        assert_eq!(
            external_store
                .get("workspace-a", BlobPurpose::Artifact, &outside_blob)
                .expect("external object remains readable"),
            b"preserve external object"
        );
        assert!(outside_path.is_file(), "external object remains present");

        let purpose_link_root = temporary.path().join("purpose-link-root");
        fs::create_dir(&purpose_link_root).expect("create purpose-link root");
        fs::set_permissions(&purpose_link_root, fs::Permissions::from_mode(0o700))
            .expect("restrict purpose-link root");
        let purpose_workspace = purpose_link_root.join(hex::encode(Sha256::digest(b"workspace-a")));
        fs::create_dir(&purpose_workspace).expect("create real Workspace directory");
        fs::set_permissions(&purpose_workspace, fs::Permissions::from_mode(0o700))
            .expect("restrict Workspace directory");
        let purpose_path = purpose_workspace.join("artifact");
        symlink(external_root.path(), &purpose_path).expect("link purpose directory");
        let purpose_link_store = FileBlobStore::new(&purpose_link_root, FixedKeys);
        let purpose_object = purpose_link_store
            .object_path("workspace-a", BlobPurpose::Artifact, &outside_blob.digest)
            .expect("purpose-scoped object path");
        let purpose_object_name = purpose_object.file_name().expect("object filename");
        let external_bytes = fs::read(&outside_path).expect("read external ciphertext");
        let purpose_external_object = external_root.path().join(purpose_object_name);
        fs::write(&purpose_external_object, &external_bytes)
            .expect("place sentinel at redirected object path");

        assert!(matches!(
            purpose_link_store.put(
                "workspace-a",
                BlobPurpose::Artifact,
                b"must stay in purpose scope",
                MEDIA_TYPE,
            ),
            Err(StoreError::Io(_))
        ));
        assert!(matches!(
            purpose_link_store.get("workspace-a", BlobPurpose::Artifact, &outside_blob),
            Err(StoreError::Io(_))
        ));
        assert!(matches!(
            purpose_link_store.remove("workspace-a", BlobPurpose::Artifact, &outside_blob),
            Err(StoreError::Io(_))
        ));
        assert_eq!(
            fs::read(&purpose_external_object).expect("redirected sentinel remains"),
            external_bytes
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_preexisting_blob_directory_with_broad_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = private_tempdir();
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

        let directory = private_tempdir();
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
