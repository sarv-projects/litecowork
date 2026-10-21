//! E13-S01 — local desktop backup, verification, restore, and safe-upgrade checks.
//!
//! The implementation is deliberately conservative:
//!
//! * a backup is a single encrypted container file. The manifest, the filtered
//!   database snapshot and every referenced blob envelope are authenticated
//!   chunks inside that container;
//! * the container key is derived from an operator supplied passphrase with
//!   PBKDF2-HMAC-SHA256. The passphrase is never written to the container, to
//!   the archive name, or to a log line;
//! * the snapshot is a Workspace-filtered serialization. Runtime-local
//!   provider handles, local recovery registries and live authority (leases,
//!   locks, secret leases) are never carried;
//! * blob envelopes are copied verbatim. The backup records the `BlobRef`
//!   metadata each envelope's associated data is bound to, so a restore can
//!   prove that every blob still decrypts before it replaces anything;
//! * restore is all-or-nothing. Everything is verified and staged first; the
//!   destination is only replaced after every check has passed.

use crate::blob::WorkspaceBlobKeyProvider;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use fs2::FileExt;
use hmac::{Hmac, Mac};
use rusqlite::{Connection, OpenFlags, params};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use storage_core::{
    BACKUP_BLOB_ROOT, BACKUP_CONTAINER_MAGIC, BACKUP_CONTAINER_VERSION,
    BACKUP_CONTENT_ALGORITHM_XCHACHA20POLY1305, BACKUP_DATABASE_SNAPSHOT_PATH,
    BACKUP_EXCLUDED_BLOB_PURPOSES, BACKUP_EXCLUDED_TABLES, BACKUP_KDF_MINIMUM_ITERATIONS,
    BACKUP_KDF_PBKDF2_HMAC_SHA256, BACKUP_MAX_BLOB_DESCRIPTORS,
    BACKUP_MAX_CHUNK_BYTES, BACKUP_MAX_MANIFEST_BYTES, BACKUP_MAX_PAYLOAD_CHUNKS,
    BackupArchiveManifest, BackupBlobDescriptor, BackupCreateOutcome, BackupEventCursor,
    BackupKeyReference, BackupPayloadDescriptor, BackupRestoreOutcome, BackupVerification,
    BlobPurpose, SchemaCompatibility, StoreError, backup_integrity_payload,
    check_schema_compatibility,
};
use zeroize::Zeroizing;

/// The oldest schema version this Runtime can open.
///
/// The migration chain in `storage-sqlite` can take a v1 database forward, so
/// v1 is the oldest baseline a safe upgrade may be asked to perform.
pub const MINIMUM_SUPPORTED_SCHEMA_VERSION: i64 = 1;

/// Payload chunk size inside the container (1 MiB).
const CHUNK_SIZE: usize = 1024 * 1024;

/// Fixed container header size in bytes.
const HEADER_LEN: usize = 66;

/// Blob envelope magic bytes (`LCB1`).
const BLOB_ENVELOPE_MAGIC: &[u8; 4] = b"LCB1";

/// Blob envelope header size: magic, key version and nonce.
const BLOB_HEADER_LEN: usize = 32;

/// Nonce length used by XChaCha20-Poly1305.
const NONCE_LEN: usize = 24;

/// Authentication tag length appended by XChaCha20-Poly1305.
const TAG_LEN: usize = 16;

/// File name of the local SQLite database inside the state directory.
pub const DATABASE_FILE_NAME: &str = "litecowork.sqlite3";

/// Directory name of the local blob store inside the state directory.
pub const BLOB_DIRECTORY_NAME: &str = "blobs";

/// An operator supplied container passphrase.
///
/// The passphrase is never serialized, logged, or written to the archive. It is
/// zeroized when dropped.
#[derive(Clone)]
pub struct BackupPassphrase(Zeroizing<String>);

impl BackupPassphrase {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    fn expose(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Debug for BackupPassphrase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BackupPassphrase(<redacted>)")
    }
}

/// Request to create a backup container.
pub struct BackupCreateRequest {
    pub backup_id: String,
    pub workspace_id: String,
    pub passphrase: BackupPassphrase,
    pub kdf_iterations: u32,
    pub now: String,
}

/// A local backup/restore service bound to one installation state directory.
#[derive(Clone, Debug)]
pub struct LocalBackupService {
    state_directory: PathBuf,
    database_path: PathBuf,
    blob_root: PathBuf,
}

impl LocalBackupService {
    /// Binds the service to an installation state directory.
    pub fn new(state_directory: impl Into<PathBuf>) -> Self {
        let state_directory = state_directory.into();
        Self {
            database_path: state_directory.join(DATABASE_FILE_NAME),
            blob_root: state_directory.join(BLOB_DIRECTORY_NAME),
            state_directory,
        }
    }

    pub fn state_directory(&self) -> &Path {
        &self.state_directory
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn blob_root(&self) -> &Path {
        &self.blob_root
    }

    /// Reads the live schema version of the installation.
    pub fn current_schema_version(&self) -> Result<i64, StoreError> {
        let connection = open_database(&self.database_path)?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(map_database_error)?;
        Ok(version)
    }

    /// Reports whether the installation may be upgraded by this Runtime.
    ///
    /// A safe upgrade refuses to run on an unknown schema version. A database
    /// written by a newer Runtime is refused, and a database older than the
    /// oldest supported baseline is refused as well.
    pub fn check_upgrade_compatibility(&self) -> Result<SchemaCompatibility, StoreError> {
        let found = self.current_schema_version()?;
        Ok(check_schema_compatibility(
            found,
            MINIMUM_SUPPORTED_SCHEMA_VERSION,
            crate::SCHEMA_VERSION,
        ))
    }

    /// Creates, verifies and registers a backup container.
    pub fn create_backup(
        &self,
        keys: &dyn WorkspaceBlobKeyProvider,
        request: &BackupCreateRequest,
    ) -> Result<BackupCreateOutcome, StoreError> {
        validate_backup_id(&request.backup_id)?;
        if request.workspace_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "workspace ID is required for a backup".to_owned(),
            ));
        }
        if request.kdf_iterations < BACKUP_KDF_MINIMUM_ITERATIONS {
            return Err(StoreError::Invalid(format!(
                "backup key derivation requires at least {BACKUP_KDF_MINIMUM_ITERATIONS} iterations"
            )));
        }
        if !self.database_path.exists() {
            return Err(StoreError::NotFound);
        }

        let archive_path = self
            .state_directory
            .join("backups")
            .join(format!("{}.lcbbackup", request.backup_id));
        if archive_path.exists() {
            return Err(StoreError::Invalid(
                "a backup with this identifier already exists".to_owned(),
            ));
        }
        ensure_private_directory(
            archive_path
                .parent()
                .expect("the archive path always has a parent"),
        )?;

        let staging = self
            .state_directory
            .join(format!(".backup-staging-{}", request.backup_id));
        remove_directory_if_present(&staging)?;
        ensure_private_directory(&staging)?;

        let result = self.create_backup_inner(keys, request, &archive_path, &staging);
        // The staging area holds a plaintext snapshot. It is removed on every
        // exit path so excluded rows never linger next to the installation.
        let _ = remove_directory_if_present(&staging);
        result
    }

    fn create_backup_inner(
        &self,
        keys: &dyn WorkspaceBlobKeyProvider,
        request: &BackupCreateRequest,
        archive_path: &Path,
        staging: &Path,
    ) -> Result<BackupCreateOutcome, StoreError> {
        let snapshot_path = staging.join(DATABASE_FILE_NAME);
        let snapshot = build_filtered_snapshot(&self.database_path, &snapshot_path)?;

        let candidates = collect_blob_ref_candidates_from(&open_database(&self.database_path)?)?;
        let blobs = collect_backup_blobs(
            keys,
            &request.workspace_id,
            &self.blob_root,
            staging,
            &candidates,
        )?;

        let database_descriptor = BackupPayloadDescriptor {
            relative_path: BACKUP_DATABASE_SNAPSHOT_PATH.to_owned(),
            size_bytes: file_size(&snapshot_path)?,
            digest: file_digest(&snapshot_path)?,
        };

        let event_cursors = read_event_cursors(&self.database_path, &request.workspace_id)?;
        let excluded_tables: Vec<String> = BACKUP_EXCLUDED_TABLES
            .iter()
            .map(|table| (*table).to_owned())
            .collect();
        let key_references = derive_key_references(&request.workspace_id, &blobs);

        let integrity_payload = backup_integrity_payload(
            snapshot.schema_version,
            &database_descriptor,
            &blobs,
            &key_references,
            &event_cursors,
        );
        let integrity_digest = digest_bytes(&canonical_json(&integrity_payload)?);

        let mut manifest = BackupArchiveManifest {
            format_version: u32::from(BACKUP_CONTAINER_VERSION),
            backup_id: request.backup_id.clone(),
            workspace_id: request.workspace_id.clone(),
            created_at: request.now.clone(),
            verified_at: request.now.clone(),
            schema_version: snapshot.schema_version,
            source_runtime_id: snapshot.source_runtime_id.clone(),
            event_cursors,
            database_snapshot: database_descriptor,
            blobs,
            key_references,
            excluded_tables,
            integrity_digest: integrity_digest.clone(),
            manifest_authentication: String::new(),
            dropped_local_row_counts: snapshot.dropped_local_row_counts.clone(),
        };
        manifest.manifest_authentication = manifest_authentication(&manifest)?;

        let manifest_bytes = canonical_json(&manifest)?;
        let archive_size = write_container(
            archive_path,
            request.passphrase.expose(),
            request.kdf_iterations,
            &manifest_bytes,
            &staged_files(staging, &manifest)?,
        )?;

        // The archive is only advertised as restorable after it has been read
        // back and every digest re-verified.
        let verification = self.verify_backup(archive_path, &request.passphrase)?;
        if verification.backup_id != manifest.backup_id
            || verification.integrity_digest != manifest.integrity_digest
        {
            return Err(StoreError::Integrity(
                "the written backup did not verify against its own manifest".to_owned(),
            ));
        }

        let manifest_row_recorded =
            record_manifest_row(&self.database_path, &manifest, &request.now).is_ok();

        Ok(BackupCreateOutcome {
            backup_id: manifest.backup_id.clone(),
            workspace_id: manifest.workspace_id.clone(),
            schema_version: manifest.schema_version,
            archive_path: archive_path.display().to_string(),
            archive_size_bytes: archive_size,
            blob_count: manifest.blobs.len(),
            database_digest: manifest.database_snapshot.digest.clone(),
            integrity_digest,
            created_at: manifest.created_at.clone(),
            manifest_row_recorded,
        })
    }

    /// Verifies a backup container without mutating anything.
    pub fn verify_backup(
        &self,
        archive_path: &Path,
        passphrase: &BackupPassphrase,
    ) -> Result<BackupVerification, StoreError> {
        verify_container(archive_path, passphrase.expose())
    }

    /// Restores a backup into a fresh installation state directory.
    ///
    /// The restore is all-or-nothing: the container is fully verified, the
    /// payload is staged, the staged database is re-verified, live authority is
    /// revoked and only then is the destination replaced.
    pub fn restore_backup(
        &self,
        archive_path: &Path,
        passphrase: &BackupPassphrase,
        destination: &Path,
        keys: &dyn WorkspaceBlobKeyProvider,
        runtime_lock: Option<&Path>,
    ) -> Result<BackupRestoreOutcome, StoreError> {
        if let Some(lock) = runtime_lock {
            ensure_runtime_is_not_running(lock)?;
        }
        ensure_destination_is_available(destination)?;

        let manifest = read_container_manifest(archive_path, passphrase.expose())?;

        let staging = destination.with_extension(format!(
            "restore-staging-{}",
            sanitize_path_segment(&manifest.backup_id)
        ));
        remove_directory_if_present(&staging)?;
        ensure_private_directory(&staging)?;

        let outcome = self
            .restore_into_staging(archive_path, passphrase, &staging, &manifest, keys)
            .inspect_err(|_| {
                let _ = remove_directory_if_present(&staging);
            })?;

        // The destination must still be free at the moment of the swap.
        ensure_destination_is_available(destination)?;
        if destination.exists() {
            fs::remove_dir(destination).map_err(|error| {
                StoreError::Io(format!("could not clear the restore destination: {error}"))
            })?;
        }
        fs::rename(&staging, destination).map_err(|error| {
            StoreError::Io(format!("could not install the restored state directory: {error}"))
        })?;

        Ok(outcome)
    }

    fn restore_into_staging(
        &self,
        archive_path: &Path,
        passphrase: &BackupPassphrase,
        staging: &Path,
        manifest: &BackupArchiveManifest,
        keys: &dyn WorkspaceBlobKeyProvider,
    ) -> Result<BackupRestoreOutcome, StoreError> {
        extract_container(archive_path, passphrase.expose(), staging, manifest)?;

        let snapshot_path = staging.join(DATABASE_FILE_NAME);
        verify_restored_snapshot(&snapshot_path, manifest)?;

        let revoked_grant_count = revoke_live_authority(&snapshot_path)? as usize;
        let dropped_lease_count: usize = ["execution_leases", "environment_control_leases", "secret_leases"]
            .iter()
            .map(|table| {
                manifest
                    .dropped_local_row_counts
                    .get(*table)
                    .copied()
                    .unwrap_or(0) as usize
            })
            .sum();

        // Every restored blob must decrypt with the referenced key version
        // before the destination is replaced.
        verify_restored_blobs(keys, staging, manifest)?;

        Ok(BackupRestoreOutcome {
            backup_id: manifest.backup_id.clone(),
            workspace_id: manifest.workspace_id.clone(),
            schema_version: manifest.schema_version,
            restored_blob_count: manifest.blobs.len(),
            revoked_grant_count,
            dropped_lease_count,
            restored_at: manifest.verified_at.clone(),
            new_runtime_identity_required: true,
        })
    }
}

// ---------------------------------------------------------------------------
// Snapshot construction
// ---------------------------------------------------------------------------

/// Summary of the filtered snapshot that was produced.
struct SnapshotSummary {
    schema_version: i64,
    source_runtime_id: Option<String>,
    dropped_local_row_counts: BTreeMap<String, u64>,
}

/// Produces a Workspace-filtered snapshot of the local database.
///
/// The snapshot keeps the exact schema of the source (so the restored database
/// opens against the current migration ledger without rewriting it) and drops
/// the rows of every Runtime-local or live-authority table.
fn build_filtered_snapshot(
    source_path: &Path,
    destination_path: &Path,
) -> Result<SnapshotSummary, StoreError> {
    let source = open_database(source_path)?;
    let schema_version: i64 = source
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    let source_runtime_id = read_single_runtime_id(&source)?;

    if destination_path.exists() {
        fs::remove_file(destination_path).map_err(|error| {
            StoreError::Io(format!("could not clear the snapshot destination: {error}"))
        })?;
    }

    // `VACUUM INTO` produces a transactionally consistent copy of the live
    // database without blocking the writer for the whole copy.
    source
        .execute("VACUUM INTO ?1", params![destination_path.to_string_lossy()])
        .map_err(map_database_error)?;

    let snapshot = open_database(destination_path)?;

    let mut dropped_local_row_counts = BTreeMap::new();
    for table in BACKUP_EXCLUDED_TABLES {
        if !table_exists(&snapshot, table)? {
            continue;
        }
        let deleted = empty_table(&snapshot, table)?;
        if deleted > 0 {
            dropped_local_row_counts.insert((*table).to_owned(), deleted);
        }
    }

    // Reclaim the pages that held the excluded rows so the retained snapshot
    // cannot be mined for them.
    snapshot
        .execute_batch("VACUUM")
        .map_err(map_database_error)?;

    assert_snapshot_is_valid(&snapshot)?;

    Ok(SnapshotSummary {
        schema_version,
        source_runtime_id,
        dropped_local_row_counts,
    })
}

/// Deletes every row of `table` while preserving the table's delete guards.
///
/// The immutability triggers on the excluded tables abort a plain delete. The
/// snapshot is a throwaway artifact, so the guard is dropped for the delete
/// and re-created from the exact stored SQL afterwards. The resulting schema
/// is byte-identical to the source schema.
fn empty_table(connection: &Connection, table: &str) -> Result<u64, StoreError> {
    let triggers = delete_triggers_for_table(connection, table)?;
    let deleted = connection
        .execute(&format!("DELETE FROM \"{}\"", escape_identifier(table)), [])
        .map_err(map_database_error)?;
    for (name, sql) in triggers {
        connection
            .execute_batch(&sql)
            .map_err(|error| {
                StoreError::Integrity(format!(
                    "could not restore the delete guard {name} on {table}: {error}"
                ))
            })?;
    }
    Ok(deleted as u64)
}

fn delete_triggers_for_table(
    connection: &Connection,
    table: &str,
) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT name, COALESCE(sql, '') FROM sqlite_master WHERE type = 'trigger' AND tbl_name = ?1",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(params![table], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(map_database_error)?;
    let triggers = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    for (name, _) in &triggers {
        connection
            .execute(&format!("DROP TRIGGER \"{}\"", escape_identifier(name)), [])
            .map_err(map_database_error)?;
    }
    Ok(triggers)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, StoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    Ok(count > 0)
}

fn assert_snapshot_is_valid(connection: &Connection) -> Result<(), StoreError> {
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(map_database_error)?;
    if integrity != "ok" {
        return Err(StoreError::CorruptSchema(format!(
            "the filtered snapshot failed its integrity check: {integrity}"
        )));
    }
    let violations = foreign_key_violations(connection)?;
    if !violations.is_empty() {
        return Err(StoreError::CorruptSchema(format!(
            "the filtered snapshot references excluded rows: {}",
            violations.join(", ")
        )));
    }
    Ok(())
}

fn foreign_key_violations(connection: &Connection) -> Result<Vec<String>, StoreError> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([], |row| {
            let table: String = row.get(0)?;
            let rowid: Option<i64> = row.get(1)?;
            let parent: String = row.get(2)?;
            Ok(format!("{table}:{rowid:?}->{parent}"))
        })
        .map_err(map_database_error)?;
    let violations = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    Ok(violations)
}

fn read_event_cursors(
    database_path: &Path,
    workspace_id: &str,
) -> Result<Vec<BackupEventCursor>, StoreError> {
    let connection = open_database(database_path)?;
    if !table_exists(&connection, "workspace_origin_sequences")? {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT origin_runtime_id, last_sequence FROM workspace_origin_sequences WHERE workspace_id = ?1 ORDER BY origin_runtime_id",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(params![workspace_id], |row| {
            Ok(BackupEventCursor {
                origin_runtime_id: row.get(0)?,
                last_sequence: row.get(1)?,
            })
        })
        .map_err(map_database_error)?;
    let cursors = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    Ok(cursors)
}

fn read_single_runtime_id(connection: &Connection) -> Result<Option<String>, StoreError> {
    if !table_exists(connection, "runtimes")? {
        return Ok(None);
    }
    let mut statement = connection
        .prepare("SELECT runtime_id FROM runtimes ORDER BY runtime_id LIMIT 1")
        .map_err(map_database_error)?;
    let mut rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_database_error)?;
    match rows.next() {
        Some(row) => Ok(Some(row.map_err(map_database_error)?)),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Blob collection
// ---------------------------------------------------------------------------

/// The `BlobRef` metadata a durable record stores for one blob.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BlobRefCandidate {
    digest: String,
    size_bytes: u64,
    media_type: String,
}

/// Copies every referenced blob envelope of a Workspace into the staging area
/// and proves that each one can be decrypted with the recorded `BlobRef`.
fn collect_backup_blobs(
    keys: &dyn WorkspaceBlobKeyProvider,
    workspace_id: &str,
    blob_root: &Path,
    staging: &Path,
    candidates: &[BlobRefCandidate],
) -> Result<Vec<BackupBlobDescriptor>, StoreError> {
    let scope_digest = hex_encode(&sha256(workspace_id.as_bytes()));
    let scope_directory = blob_root.join(&scope_digest);
    if !scope_directory.is_dir() {
        return Ok(Vec::new());
    }

    let mut descriptors = Vec::new();
    let mut purposes: Vec<_> = fs::read_dir(&scope_directory)
        .map_err(|error| StoreError::Io(format!("could not read the blob scope: {error}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| StoreError::Io(format!("could not read the blob scope: {error}")))?;
    purposes.sort_by_key(|entry| entry.file_name());

    for purpose_entry in purposes {
        let purpose_name = purpose_entry.file_name().to_string_lossy().to_string();
        let purpose_directory = purpose_entry.path();
        if !purpose_directory.is_dir() {
            continue;
        }
        let purpose = purpose_name.to_ascii_uppercase();
        if BACKUP_EXCLUDED_BLOB_PURPOSES.contains(&purpose.as_str()) {
            continue;
        }
        let mut objects: Vec<_> = fs::read_dir(&purpose_directory)
            .map_err(|error| {
                StoreError::Io(format!("could not read the blob purpose directory: {error}"))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                StoreError::Io(format!("could not read the blob purpose directory: {error}"))
            })?;
        objects.sort_by_key(|entry| entry.file_name());
        for object in objects {
            let object_path = object.path();
            if !object_path.is_file() {
                continue;
            }
            let file_name = object.file_name().to_string_lossy().to_string();
            let Some(digest_hex) = file_name.strip_suffix(".blob") else {
                continue;
            };
            if digest_hex.len() != 64 || !digest_hex.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }
            let blob_digest = format!("sha256:{digest_hex}");
            let envelope = fs::read(&object_path)
                .map_err(|error| StoreError::Io(format!("could not read a blob envelope: {error}")))?;
            let key_version = read_envelope_key_version(&envelope)?;
            let envelope_digest = digest_bytes(&envelope);
            let relative_path = format!("{BACKUP_BLOB_ROOT}/{scope_digest}/{purpose_name}/{file_name}");

            let mut resolved: Option<BlobRefCandidate> = None;
            for candidate in candidates
                .iter()
                .filter(|candidate| candidate.digest == blob_digest)
            {
                if let Some(found) = prove_blob_decryptable(
                    keys,
                    workspace_id,
                    &purpose,
                    &blob_digest,
                    key_version,
                    candidate,
                    &envelope,
                )? {
                    resolved = Some(found);
                    break;
                }
            }
            let Some(resolved) = resolved else {
                // The envelope cannot be tied back to a durable BlobRef, so it
                // is not restorable content. Fail closed instead of shipping a
                // blob that a restore could not authenticate.
                return Err(StoreError::Integrity(format!(
                    "blob {relative_path} has no verifiable BlobRef metadata and cannot be backed up"
                )));
            };

            let target = staging.join(&relative_path);
            if let Some(parent) = target.parent() {
                ensure_private_directory(parent)?;
            }
            fs::write(&target, &envelope)
                .map_err(|error| StoreError::Io(format!("could not stage a blob envelope: {error}")))?;

            descriptors.push(BackupBlobDescriptor {
                workspace_scope_digest: scope_digest.clone(),
                purpose: purpose.clone(),
                blob_digest,
                size_bytes: envelope.len() as u64,
                plaintext_size_bytes: resolved.size_bytes,
                media_type: resolved.media_type,
                envelope_digest,
                key_version,
                relative_path,
            });
            if descriptors.len() > BACKUP_MAX_BLOB_DESCRIPTORS {
                return Err(StoreError::Invalid(
                    "the backup carries more blobs than the contract allows".to_owned(),
                ));
            }
        }
    }

    descriptors.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(descriptors)
}

/// Gathers BlobRef candidates from every durable record that stores one.
///
/// The search is generic on purpose: any JSON column of an included table may
/// hold a `BlobRef` object, and the registry tables hold the `BlobRef` fields
/// in dedicated columns.
fn collect_blob_ref_candidates_from(
    connection: &Connection,
) -> Result<Vec<BlobRefCandidate>, StoreError> {
    let mut candidates: BTreeSet<BlobRefCandidate> = BTreeSet::new();

    if table_exists(connection, "resource_content_blob_registry")? {
        let mut statement = connection
            .prepare("SELECT digest, size_bytes, media_type FROM resource_content_blob_registry")
            .map_err(map_database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(BlobRefCandidate {
                    digest: row.get(0)?,
                    size_bytes: row.get::<_, i64>(1)? as u64,
                    media_type: row.get(2)?,
                })
            })
            .map_err(map_database_error)?;
        for row in rows {
            candidates.insert(row.map_err(map_database_error)?);
        }
    }

    if table_exists(connection, "resource_text_indexes")? {
        let mut statement = connection
            .prepare("SELECT extracted_blob_digest, extracted_size_bytes FROM resource_text_indexes")
            .map_err(map_database_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(BlobRefCandidate {
                    digest: row.get(0)?,
                    size_bytes: row.get::<_, i64>(1)? as u64,
                    media_type: crate::resource_index::INDEX_MEDIA_TYPE.to_owned(),
                })
            })
            .map_err(map_database_error)?;
        for row in rows {
            candidates.insert(row.map_err(map_database_error)?);
        }
    }

    for table in included_tables(connection)? {
        for column in text_json_columns(connection, &table)? {
            let sql = format!(
                "SELECT \"{}\" FROM \"{}\" WHERE \"{}\" IS NOT NULL",
                escape_identifier(&column),
                escape_identifier(&table),
                escape_identifier(&column)
            );
            let mut statement = match connection.prepare(&sql) {
                Ok(statement) => statement,
                Err(_) => continue,
            };
            let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
                continue;
            };
            for row in rows {
                let Ok(value) = row else { continue };
                let Ok(parsed) = serde_json::from_str::<Value>(&value) else {
                    continue;
                };
                collect_blob_refs_from_value(&parsed, &mut candidates);
            }
        }
    }

    Ok(candidates.into_iter().collect())
}

fn collect_blob_refs_from_value(value: &Value, candidates: &mut BTreeSet<BlobRefCandidate>) {
    match value {
        Value::Object(map) => {
            let digest = map.get("digest").and_then(Value::as_str);
            let size = map.get("size_bytes").and_then(Value::as_u64);
            let media = map.get("media_type").and_then(Value::as_str);
            if let (Some(digest), Some(size), Some(media)) = (digest, size, media) {
                if digest.starts_with("sha256:") && !media.is_empty() {
                    candidates.insert(BlobRefCandidate {
                        digest: digest.to_owned(),
                        size_bytes: size,
                        media_type: media.to_owned(),
                    });
                }
            }
            for nested in map.values() {
                collect_blob_refs_from_value(nested, candidates);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_blob_refs_from_value(item, candidates);
            }
        }
        _ => {}
    }
}

fn included_tables(connection: &Connection) -> Result<Vec<String>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_database_error)?;
    let tables = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    Ok(tables
        .into_iter()
        .filter(|table| !BACKUP_EXCLUDED_TABLES.contains(&table.as_str()))
        .collect())
}

fn text_json_columns(connection: &Connection, table: &str) -> Result<Vec<String>, StoreError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info(\"{}\")", escape_identifier(table)))
        .map_err(map_database_error)?;
    let rows = statement
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let declared: String = row.get(2)?;
            Ok((name, declared))
        })
        .map_err(map_database_error)?;
    let columns = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_database_error)?;
    Ok(columns
        .into_iter()
        .filter(|(name, declared)| {
            declared.eq_ignore_ascii_case("TEXT") && name.ends_with("_json")
        })
        .map(|(name, _)| name)
        .collect())
}

/// Proves that a blob envelope authenticates with the given `BlobRef`.
fn prove_blob_decryptable(
    keys: &dyn WorkspaceBlobKeyProvider,
    workspace_id: &str,
    purpose: &str,
    blob_digest: &str,
    key_version: u32,
    candidate: &BlobRefCandidate,
    envelope: &[u8],
) -> Result<Option<BlobRefCandidate>, StoreError> {
    let Ok(key) = keys.key_by_version(workspace_id, purpose_to_blob_purpose(purpose)?, key_version)
    else {
        return Ok(None);
    };
    if envelope.len() < BLOB_HEADER_LEN + TAG_LEN {
        return Ok(None);
    }
    let nonce = XNonce::from_slice(&envelope[8..BLOB_HEADER_LEN]);
    let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_ref())
        .map_err(|_| StoreError::Blob("invalid workspace encryption key".to_owned()))?;
    let aad = blob_associated_data(
        workspace_id,
        purpose,
        blob_digest,
        candidate.size_bytes,
        &candidate.media_type,
    );
    let Ok(plaintext) = cipher.decrypt(
        nonce,
        Payload {
            msg: &envelope[BLOB_HEADER_LEN..],
            aad: &aad,
        },
    ) else {
        return Ok(None);
    };
    if plaintext.len() as u64 != candidate.size_bytes || digest_bytes(&plaintext) != blob_digest {
        return Ok(None);
    }
    Ok(Some(candidate.clone()))
}

fn purpose_to_blob_purpose(purpose: &str) -> Result<BlobPurpose, StoreError> {
    match purpose {
        "AGGREGATE_STATE" => Ok(BlobPurpose::AggregateState),
        "ARTIFACT" => Ok(BlobPurpose::Artifact),
        "RESOURCE" => Ok(BlobPurpose::Resource),
        "RESOURCE_INDEX" => Ok(BlobPurpose::ResourceIndex),
        "CHECKPOINT" => Ok(BlobPurpose::Checkpoint),
        "RESOURCE_UPLOAD_CHUNK" => Ok(BlobPurpose::ResourceUploadChunk),
        "RICH_PRESENTATION" => Ok(BlobPurpose::RichPresentation),
        other => Err(StoreError::Invalid(format!(
            "unknown blob purpose {other}"
        ))),
    }
}

fn blob_associated_data(
    workspace_id: &str,
    purpose: &str,
    digest: &str,
    size_bytes: u64,
    media_type: &str,
) -> Vec<u8> {
    format!(
        "litecowork-blob-v1\0{workspace_id}\0{purpose}\0{digest}\0{size_bytes}\0{media_type}"
    )
    .into_bytes()
}

fn read_envelope_key_version(envelope: &[u8]) -> Result<u32, StoreError> {
    if envelope.len() < BLOB_HEADER_LEN + TAG_LEN || &envelope[..4] != BLOB_ENVELOPE_MAGIC {
        return Err(StoreError::Integrity(
            "a blob envelope is truncated or unknown".to_owned(),
        ));
    }
    let version = u32::from_be_bytes(
        envelope[4..8]
            .try_into()
            .map_err(|_| StoreError::Integrity("a blob key version is malformed".to_owned()))?,
    );
    if version == 0 {
        return Err(StoreError::Integrity(
            "a blob key version must be nonzero".to_owned(),
        ));
    }
    Ok(version)
}

fn derive_key_references(
    workspace_id: &str,
    blobs: &[BackupBlobDescriptor],
) -> Vec<BackupKeyReference> {
    let scope_digest = hex_encode(&sha256(workspace_id.as_bytes()));
    let mut references: BTreeSet<(String, u32)> = BTreeSet::new();
    for blob in blobs {
        references.insert((blob.purpose.clone(), blob.key_version));
    }
    references
        .into_iter()
        .map(|(purpose, key_version)| BackupKeyReference {
            workspace_id: workspace_id.to_owned(),
            workspace_scope_digest: scope_digest.clone(),
            purpose,
            key_version,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Container format
// ---------------------------------------------------------------------------

/// One payload file inside the container.
///
/// The order of the files is the container order: the database snapshot first,
/// then the blobs in the manifest's order. The relative path lives in the
/// manifest, not here, so the container and the manifest cannot disagree.
struct ContainerFile {
    source: PathBuf,
}

fn staged_files(
    staging: &Path,
    manifest: &BackupArchiveManifest,
) -> Result<Vec<ContainerFile>, StoreError> {
    let mut files = vec![ContainerFile {
        source: staging.join(DATABASE_FILE_NAME),
    }];
    for blob in &manifest.blobs {
        files.push(ContainerFile {
            source: staging.join(&blob.relative_path),
        });
    }
    Ok(files)
}

/// Computes the manifest's self-binding authentication value.
///
/// The value binds the manifest content to itself. It is computed over the
/// manifest with the authentication field emptied, so it can be embedded in
/// the manifest without being circular.
fn manifest_authentication(manifest: &BackupArchiveManifest) -> Result<String, StoreError> {
    let mut without = manifest.clone();
    without.manifest_authentication = String::new();
    Ok(digest_bytes(&canonical_json(&without)?))
}

/// Writes the encrypted container.
fn write_container(
    archive_path: &Path,
    passphrase: &str,
    iterations: u32,
    manifest_bytes: &[u8],
    files: &[ContainerFile],
) -> Result<u64, StoreError> {
    if manifest_bytes.len() as u64 > BACKUP_MAX_MANIFEST_BYTES {
        return Err(StoreError::Invalid(
            "the backup manifest is larger than the contract allows".to_owned(),
        ));
    }
    let mut salt = [0_u8; 16];
    getrandom::fill(&mut salt).map_err(|error| StoreError::Blob(error.to_string()))?;
    let mut manifest_nonce = [0_u8; NONCE_LEN];
    getrandom::fill(&mut manifest_nonce).map_err(|error| StoreError::Blob(error.to_string()))?;
    let key = derive_container_key(passphrase, &salt, iterations)?;

    let chunk_plan = plan_chunks(files)?;
    let header = build_header(
        iterations,
        &salt,
        &manifest_nonce,
        files.len() as u32,
        manifest_bytes.len() as u32,
    );
    let directory = build_directory(&chunk_plan);
    let aad_prefix = [header.as_slice(), directory.as_slice()].concat();

    let manifest_ciphertext = encrypt_chunk(
        &key,
        &manifest_nonce,
        &aad_prefix,
        b"manifest",
        0,
        manifest_bytes,
    )?;

    if let Some(parent) = archive_path.parent() {
        ensure_private_directory(parent)?;
    }
    let temporary = archive_path.with_extension("lcbbackup.partial");
    let mut file = fs::File::create(&temporary)
        .map_err(|error| StoreError::Io(format!("could not create the backup archive: {error}")))?;
    file.write_all(&header)
        .and_then(|()| file.write_all(&directory))
        .and_then(|()| file.write_all(&manifest_ciphertext))
        .map_err(|error| StoreError::Io(format!("could not write the backup archive: {error}")))?;

    let mut chunk_index: u32 = 0;
    for (file_index, container_file) in files.iter().enumerate() {
        for chunk in 0..chunk_plan[file_index].len() {
            let plaintext = read_chunk(&container_file.source, chunk)?;
            let mut nonce = [0_u8; NONCE_LEN];
            getrandom::fill(&mut nonce).map_err(|error| StoreError::Blob(error.to_string()))?;
            let ciphertext = encrypt_chunk(
                &key,
                &nonce,
                &aad_prefix,
                b"chunk",
                chunk_index,
                &plaintext,
            )?;
            file.write_all(&nonce)
                .and_then(|()| file.write_all(&ciphertext))
                .map_err(|error| {
                    StoreError::Io(format!("could not write the backup archive: {error}"))
                })?;
            chunk_index += 1;
        }
    }
    file.sync_all()
        .map_err(|error| StoreError::Io(format!("could not flush the backup archive: {error}")))?;
    drop(file);
    fs::rename(&temporary, archive_path).map_err(|error| {
        StoreError::Io(format!("could not publish the backup archive: {error}"))
    })?;
    file_size(archive_path)
}

/// Plans the chunk layout of every payload file.
fn plan_chunks(files: &[ContainerFile]) -> Result<Vec<Vec<u32>>, StoreError> {
    let mut plan = Vec::with_capacity(files.len());
    let mut total: u64 = 0;
    for container_file in files {
        let size = file_size(&container_file.source)?;
        let count = if size == 0 {
            1_u64
        } else {
            size.div_ceil(CHUNK_SIZE as u64)
        };
        let mut chunks = Vec::with_capacity(count as usize);
        for index in 0..count {
            let offset = index * CHUNK_SIZE as u64;
            let remaining = size.saturating_sub(offset);
            // Each chunk stores its own nonce next to its ciphertext.
            let length =
                remaining.min(CHUNK_SIZE as u64) as u32 + TAG_LEN as u32 + NONCE_LEN as u32;
            chunks.push(length);
        }
        total += count;
        if total > u64::from(BACKUP_MAX_PAYLOAD_CHUNKS) {
            return Err(StoreError::Invalid(
                "the backup payload exceeds the contract chunk limit".to_owned(),
            ));
        }
        plan.push(chunks);
    }
    Ok(plan)
}

fn build_header(
    iterations: u32,
    salt: &[u8; 16],
    manifest_nonce: &[u8; NONCE_LEN],
    file_count: u32,
    manifest_len: u32,
) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(BACKUP_CONTAINER_MAGIC);
    header.push(BACKUP_CONTAINER_VERSION);
    header.push(BACKUP_CONTENT_ALGORITHM_XCHACHA20POLY1305);
    header.push(BACKUP_KDF_PBKDF2_HMAC_SHA256);
    header.push(0);
    header.extend_from_slice(&iterations.to_be_bytes());
    header.extend_from_slice(salt);
    header.extend_from_slice(manifest_nonce);
    header.extend_from_slice(&file_count.to_be_bytes());
    header.extend_from_slice(&(CHUNK_SIZE as u32).to_be_bytes());
    header.extend_from_slice(&manifest_len.to_be_bytes());
    debug_assert_eq!(header.len(), HEADER_LEN);
    header
}

fn build_directory(chunk_plan: &[Vec<u32>]) -> Vec<u8> {
    let mut directory = Vec::new();
    for chunks in chunk_plan {
        directory.extend_from_slice(&(chunks.len() as u32).to_be_bytes());
        for length in chunks {
            directory.extend_from_slice(&length.to_be_bytes());
        }
    }
    directory
}

fn derive_container_key(
    passphrase: &str,
    salt: &[u8; 16],
    iterations: u32,
) -> Result<Zeroizing<[u8; 32]>, StoreError> {
    if passphrase.is_empty() {
        return Err(StoreError::Invalid(
            "a backup passphrase is required".to_owned(),
        ));
    }
    let mut key = Zeroizing::new([0_u8; 32]);
    pbkdf2_hmac_sha256(passphrase.as_bytes(), salt, iterations, key.as_mut());
    Ok(key)
}

fn encrypt_chunk(
    key: &[u8; 32],
    nonce: &[u8; NONCE_LEN],
    aad_prefix: &[u8],
    label: &[u8],
    index: u32,
    plaintext: &[u8],
) -> Result<Vec<u8>, StoreError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| StoreError::Blob("invalid backup container key".to_owned()))?;
    let mut aad = aad_prefix.to_vec();
    aad.extend_from_slice(label);
    aad.extend_from_slice(&index.to_be_bytes());
    cipher
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| StoreError::Blob("backup chunk encryption failed".to_owned()))
}

fn read_chunk(path: &Path, chunk: usize) -> Result<Vec<u8>, StoreError> {
    use std::io::Seek;
    let mut file = fs::File::open(path)
        .map_err(|error| StoreError::Io(format!("could not read a backup payload: {error}")))?;
    let offset = chunk as u64 * CHUNK_SIZE as u64;
    file.seek(std::io::SeekFrom::Start(offset))
        .map_err(|error| StoreError::Io(format!("could not seek a backup payload: {error}")))?;
    let mut buffer = vec![0_u8; CHUNK_SIZE];
    let mut filled = 0;
    while filled < CHUNK_SIZE {
        let read = file
            .read(&mut buffer[filled..])
            .map_err(|error| StoreError::Io(format!("could not read a backup payload: {error}")))?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    buffer.truncate(filled);
    Ok(buffer)
}

/// Reads and authenticates the manifest of a container.
fn read_container_manifest(
    archive_path: &Path,
    passphrase: &str,
) -> Result<BackupArchiveManifest, StoreError> {
    let (manifest, _header, _files) = read_container(archive_path, passphrase)?;
    Ok(manifest)
}

/// Verifies a container end to end without mutating anything.
fn verify_container(
    archive_path: &Path,
    passphrase: &str,
) -> Result<BackupVerification, StoreError> {
    let (manifest, _header, _files) = read_container(archive_path, passphrase)?;
    let compatibility = check_schema_compatibility(
        manifest.schema_version,
        MINIMUM_SUPPORTED_SCHEMA_VERSION,
        crate::SCHEMA_VERSION,
    );
    Ok(BackupVerification {
        backup_id: manifest.backup_id.clone(),
        workspace_id: manifest.workspace_id.clone(),
        schema_version: manifest.schema_version,
        database_digest: manifest.database_snapshot.digest.clone(),
        blob_count: manifest.blobs.len(),
        key_reference_count: manifest.key_references.len(),
        excluded_table_count: manifest.excluded_tables.len(),
        verified_at: manifest.verified_at.clone(),
        compatibility,
        integrity_digest: manifest.integrity_digest.clone(),
    })
}

/// Reads a container, authenticates every chunk and re-computes the digests.
fn read_container(
    archive_path: &Path,
    passphrase: &str,
) -> Result<(BackupArchiveManifest, Vec<u8>, Vec<Vec<u8>>), StoreError> {
    let bytes = fs::read(archive_path)
        .map_err(|error| StoreError::Io(format!("could not read the backup archive: {error}")))?;
    if bytes.len() < HEADER_LEN {
        return Err(StoreError::Integrity(
            "the backup archive is truncated".to_owned(),
        ));
    }
    if &bytes[..6] != BACKUP_CONTAINER_MAGIC {
        return Err(StoreError::Integrity(
            "the file is not a LiteCowork backup archive".to_owned(),
        ));
    }
    if bytes[6] != BACKUP_CONTAINER_VERSION {
        return Err(StoreError::UnsupportedSchema(i64::from(bytes[6])));
    }
    if bytes[7] != BACKUP_CONTENT_ALGORITHM_XCHACHA20POLY1305
        || bytes[8] != BACKUP_KDF_PBKDF2_HMAC_SHA256
    {
        return Err(StoreError::Integrity(
            "the backup archive uses an unsupported container algorithm".to_owned(),
        ));
    }
    let iterations = u32::from_be_bytes(bytes[10..14].try_into().map_err(|_| {
        StoreError::Integrity("the backup header is malformed".to_owned())
    })?);
    if iterations < BACKUP_KDF_MINIMUM_ITERATIONS {
        return Err(StoreError::Integrity(
            "the backup archive uses a weakened key derivation".to_owned(),
        ));
    }
    let mut salt = [0_u8; 16];
    salt.copy_from_slice(&bytes[14..30]);
    let mut manifest_nonce = [0_u8; NONCE_LEN];
    manifest_nonce.copy_from_slice(&bytes[30..54]);
    let file_count = u32::from_be_bytes(bytes[54..58].try_into().map_err(|_| {
        StoreError::Integrity("the backup header is malformed".to_owned())
    })?);
    let chunk_size = u32::from_be_bytes(bytes[58..62].try_into().map_err(|_| {
        StoreError::Integrity("the backup header is malformed".to_owned())
    })?);
    let manifest_len = u32::from_be_bytes(bytes[62..66].try_into().map_err(|_| {
        StoreError::Integrity("the backup header is malformed".to_owned())
    })?);
    if chunk_size as usize != CHUNK_SIZE {
        return Err(StoreError::Integrity(
            "the backup archive uses an unsupported chunk size".to_owned(),
        ));
    }
    if u64::from(manifest_len) > BACKUP_MAX_MANIFEST_BYTES {
        return Err(StoreError::Integrity(
            "the backup manifest is larger than the contract allows".to_owned(),
        ));
    }
    if file_count == 0 || file_count > BACKUP_MAX_PAYLOAD_CHUNKS {
        return Err(StoreError::Integrity(
            "the backup archive declares an invalid file count".to_owned(),
        ));
    }

    let header = bytes[..HEADER_LEN].to_vec();
    let mut offset = HEADER_LEN;
    let mut chunk_plan: Vec<Vec<u32>> = Vec::with_capacity(file_count as usize);
    for _ in 0..file_count {
        if bytes.len() < offset + 4 {
            return Err(StoreError::Integrity(
                "the backup chunk directory is truncated".to_owned(),
            ));
        }
        let count = u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| {
            StoreError::Integrity("the backup chunk directory is malformed".to_owned())
        })?);
        offset += 4;
        if count == 0 || count > BACKUP_MAX_PAYLOAD_CHUNKS {
            return Err(StoreError::Integrity(
                "the backup chunk directory is malformed".to_owned(),
            ));
        }
        let mut chunks = Vec::with_capacity(count as usize);
        for _ in 0..count {
            if bytes.len() < offset + 4 {
                return Err(StoreError::Integrity(
                    "the backup chunk directory is truncated".to_owned(),
                ));
            }
            let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| {
                StoreError::Integrity("the backup chunk directory is malformed".to_owned())
            })?);
            offset += 4;
            if u64::from(length) > BACKUP_MAX_CHUNK_BYTES + TAG_LEN as u64 + NONCE_LEN as u64 {
                return Err(StoreError::Integrity(
                    "the backup chunk directory is malformed".to_owned(),
                ));
            }
            chunks.push(length);
        }
        chunk_plan.push(chunks);
    }
    let directory = bytes[HEADER_LEN..offset].to_vec();

    let key = derive_container_key(passphrase, &salt, iterations)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_ref())
        .map_err(|_| StoreError::Blob("invalid backup container key".to_owned()))?;

    let manifest_ciphertext_len = manifest_len as usize + TAG_LEN;
    if bytes.len() < offset + manifest_ciphertext_len {
        return Err(StoreError::Integrity(
            "the backup archive is truncated".to_owned(),
        ));
    }
    let mut manifest_aad = header.clone();
    manifest_aad.extend_from_slice(&directory);
    manifest_aad.extend_from_slice(b"manifest");
    manifest_aad.extend_from_slice(&0_u32.to_be_bytes());
    let manifest_plaintext = cipher
        .decrypt(
            XNonce::from_slice(&manifest_nonce),
            Payload {
                msg: &bytes[offset..offset + manifest_ciphertext_len],
                aad: &manifest_aad,
            },
        )
        .map_err(|_| {
            StoreError::Integrity(
                "the backup manifest failed authentication; the archive is corrupt or the passphrase is wrong"
                    .to_owned(),
            )
        })?;
    offset += manifest_ciphertext_len;

    let manifest: BackupArchiveManifest = serde_json::from_slice(&manifest_plaintext)
        .map_err(|_| StoreError::Integrity("the backup manifest is malformed".to_owned()))?;
    if manifest.format_version != u32::from(BACKUP_CONTAINER_VERSION) {
        return Err(StoreError::UnsupportedSchema(i64::from(manifest.format_version)));
    }
    if manifest_authentication(&manifest)? != manifest.manifest_authentication {
        return Err(StoreError::Integrity(
            "the backup manifest failed its own authentication binding".to_owned(),
        ));
    }

    let aad_prefix = [header.as_slice(), directory.as_slice()].concat();
    let mut global_index: u32 = 0;
    let mut files: Vec<Vec<u8>> = Vec::with_capacity(chunk_plan.len());
    for chunks in &chunk_plan {
        let mut file_bytes = Vec::new();
        for length in chunks {
            let length = *length as usize;
            if bytes.len() < offset + length {
                return Err(StoreError::Integrity(
                    "the backup archive is truncated".to_owned(),
                ));
            }
            let mut nonce = [0_u8; NONCE_LEN];
            nonce.copy_from_slice(&bytes[offset..offset + NONCE_LEN]);
            let mut chunk_aad = aad_prefix.clone();
            chunk_aad.extend_from_slice(b"chunk");
            chunk_aad.extend_from_slice(&global_index.to_be_bytes());
            let plaintext = cipher
                .decrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: &bytes[offset + NONCE_LEN..offset + length],
                        aad: &chunk_aad,
                    },
                )
                .map_err(|_| {
                    StoreError::Integrity(
                        "a backup payload failed authentication; the archive is corrupt or the passphrase is wrong"
                            .to_owned(),
                    )
                })?;
            file_bytes.extend_from_slice(&plaintext);
            offset += length;
            global_index += 1;
        }
        files.push(file_bytes);
    }
    if offset != bytes.len() {
        return Err(StoreError::Integrity(
            "the backup archive carries unexpected trailing bytes".to_owned(),
        ));
    }

    if files.len() != manifest.blobs.len() + 1 {
        return Err(StoreError::Integrity(
            "the backup archive does not match its manifest".to_owned(),
        ));
    }
    if digest_bytes(&files[0]) != manifest.database_snapshot.digest
        || files[0].len() as u64 != manifest.database_snapshot.size_bytes
    {
        return Err(StoreError::Integrity(
            "the backup database snapshot does not match its manifest digest".to_owned(),
        ));
    }
    for (index, blob) in manifest.blobs.iter().enumerate() {
        let payload = &files[index + 1];
        if digest_bytes(payload) != blob.envelope_digest || payload.len() as u64 != blob.size_bytes {
            return Err(StoreError::Integrity(format!(
                "the backup blob {} does not match its manifest digest",
                blob.relative_path
            )));
        }
    }
    let integrity_payload = backup_integrity_payload(
        manifest.schema_version,
        &manifest.database_snapshot,
        &manifest.blobs,
        &manifest.key_references,
        &manifest.event_cursors,
    );
    if digest_bytes(&canonical_json(&integrity_payload)?) != manifest.integrity_digest {
        return Err(StoreError::Integrity(
            "the backup manifest integrity digest does not match its contents".to_owned(),
        ));
    }

    Ok((manifest, header, files))
}

/// Writes the decrypted container payload into the staging directory.
fn extract_container(
    archive_path: &Path,
    passphrase: &str,
    staging: &Path,
    manifest: &BackupArchiveManifest,
) -> Result<(), StoreError> {
    let (read_manifest, _header, files) = read_container(archive_path, passphrase)?;
    if read_manifest.backup_id != manifest.backup_id {
        return Err(StoreError::Integrity(
            "the backup archive does not match the expected manifest".to_owned(),
        ));
    }
    let snapshot_path = staging.join(DATABASE_FILE_NAME);
    fs::write(&snapshot_path, &files[0])
        .map_err(|error| StoreError::Io(format!("could not stage the snapshot: {error}")))?;
    for (index, blob) in manifest.blobs.iter().enumerate() {
        let target = staging.join(&blob.relative_path);
        if let Some(parent) = target.parent() {
            ensure_private_directory(parent)?;
        }
        fs::write(&target, &files[index + 1]).map_err(|error| {
            StoreError::Io(format!("could not stage a blob envelope: {error}"))
        })?;
    }
    Ok(())
}

/// Re-verifies the staged snapshot before it replaces the destination.
fn verify_restored_snapshot(
    snapshot_path: &Path,
    manifest: &BackupArchiveManifest,
) -> Result<(), StoreError> {
    let connection = open_database(snapshot_path)?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(map_database_error)?;
    if version != manifest.schema_version {
        return Err(StoreError::CorruptSchema(format!(
            "the restored snapshot declares schema version {version} but the manifest declares {}",
            manifest.schema_version
        )));
    }
    let compatibility = check_schema_compatibility(
        version,
        MINIMUM_SUPPORTED_SCHEMA_VERSION,
        crate::SCHEMA_VERSION,
    );
    if !compatibility.is_compatible() {
        return Err(StoreError::UnsupportedSchema(version));
    }
    assert_snapshot_is_valid(&connection)?;
    Ok(())
}

/// Revokes every restored capability grant and fences the restored runtimes.
fn revoke_live_authority(snapshot_path: &Path) -> Result<u64, StoreError> {
    let connection = open_database(snapshot_path)?;
    let mut revoked = 0_u64;
    if table_exists(&connection, "capability_grants")? {
        revoked = connection
            .execute(
                "UPDATE capability_grants SET status = 'REVOKED' WHERE status = 'ACTIVE'",
                [],
            )
            .map_err(map_database_error)? as u64;
    }
    if table_exists(&connection, "runtime_incarnations")? {
        connection
            .execute(
                "UPDATE runtime_incarnations SET recovery_state = 'STOPPED', stopped_at = COALESCE(stopped_at, strftime('%Y-%m-%dT%H:%M:%fZ','now')) WHERE recovery_state <> 'STOPPED'",
                [],
            )
            .map_err(map_database_error)?;
    }
    if table_exists(&connection, "runtimes")? {
        connection
            .execute(
                "UPDATE runtimes SET current_incarnation_id = NULL, availability = 'OFFLINE'",
                [],
            )
            .map_err(map_database_error)?;
    }
    Ok(revoked)
}

/// Proves that every restored blob still decrypts with the referenced key.
fn verify_restored_blobs(
    keys: &dyn WorkspaceBlobKeyProvider,
    staging: &Path,
    manifest: &BackupArchiveManifest,
) -> Result<(), StoreError> {
    for blob in &manifest.blobs {
        let path = staging.join(&blob.relative_path);
        let envelope = fs::read(&path)
            .map_err(|error| StoreError::Io(format!("could not read a restored blob: {error}")))?;
        let key_version = read_envelope_key_version(&envelope)?;
        if key_version != blob.key_version {
            return Err(StoreError::Integrity(format!(
                "the restored blob {} changed its key version",
                blob.relative_path
            )));
        }
        let purpose = purpose_to_blob_purpose(&blob.purpose)?;
        let key = keys
            .key_by_version(&manifest.workspace_id, purpose, blob.key_version)
            .map_err(|_| {
                StoreError::Blob(format!(
                    "the encryption key for {} version {} is unavailable; the backup cannot be restored",
                    blob.purpose, blob.key_version
                ))
            })?;
        if envelope.len() < BLOB_HEADER_LEN + TAG_LEN {
            return Err(StoreError::Integrity(
                "a restored blob envelope is truncated".to_owned(),
            ));
        }
        let nonce = XNonce::from_slice(&envelope[8..BLOB_HEADER_LEN]);
        let cipher = XChaCha20Poly1305::new_from_slice(key.bytes.as_ref())
            .map_err(|_| StoreError::Blob("invalid workspace encryption key".to_owned()))?;
        let aad = blob_associated_data(
            &manifest.workspace_id,
            &blob.purpose,
            &blob.blob_digest,
            blob.plaintext_size_bytes,
            &blob.media_type,
        );
        match cipher.decrypt(
            nonce,
            Payload {
                msg: &envelope[BLOB_HEADER_LEN..],
                aad: &aad,
            },
        ) {
            Ok(plaintext) => {
                if digest_bytes(&plaintext) != blob.blob_digest
                    || plaintext.len() as u64 != blob.plaintext_size_bytes
                {
                    return Err(StoreError::Integrity(format!(
                        "the restored blob {} does not match its recorded digest",
                        blob.relative_path
                    )));
                }
            }
            Err(_) => {
                return Err(StoreError::Integrity(format!(
                    "the restored blob {} failed authentication",
                    blob.relative_path
                )));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Manifest persistence
// ---------------------------------------------------------------------------

fn record_manifest_row(
    database_path: &Path,
    manifest: &BackupArchiveManifest,
    now: &str,
) -> Result<(), StoreError> {
    let connection = open_database(database_path)?;
    if !table_exists(&connection, "workspace_backup_manifests")? {
        return Err(StoreError::Invalid(
            "the installed schema has no workspace backup manifest table".to_owned(),
        ));
    }
    let blob_manifest = serde_json::to_value(&manifest.blobs).map_err(|error| {
        StoreError::Invalid(format!("could not encode the blob manifest: {error}"))
    })?;
    let blob_manifest_digest = digest_bytes(&canonical_json(&blob_manifest)?);
    let database_snapshot_ref = serde_json::to_value(&manifest.database_snapshot).map_err(|error| {
        StoreError::Invalid(format!("could not encode the snapshot reference: {error}"))
    })?;
    let encryption_key_ref = serde_json::to_value(&manifest.key_references).map_err(|error| {
        StoreError::Invalid(format!("could not encode the key reference: {error}"))
    })?;
    let event_cursors = serde_json::to_value(&manifest.event_cursors).map_err(|error| {
        StoreError::Invalid(format!("could not encode the event cursors: {error}"))
    })?;
    connection
        .execute(
            "INSERT INTO workspace_backup_manifests (backup_id, workspace_id, schema_version, event_cursors_json, blob_manifest_ref_json, blob_manifest_digest, database_snapshot_ref_json, encryption_key_ref, integrity_digest, manifest_authentication, created_at, verified_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                manifest.backup_id,
                manifest.workspace_id,
                manifest.schema_version,
                event_cursors.to_string(),
                blob_manifest.to_string(),
                blob_manifest_digest,
                database_snapshot_ref.to_string(),
                encryption_key_ref.to_string(),
                manifest.integrity_digest,
                manifest.manifest_authentication,
                manifest.created_at,
                if now.is_empty() { manifest.verified_at.clone() } else { now.to_owned() },
            ],
        )
        .map_err(map_database_error)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn open_database(path: &Path) -> Result<Connection, StoreError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(map_database_error)?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(map_database_error)?;
    Ok(connection)
}

fn ensure_private_directory(path: &Path) -> Result<(), StoreError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| StoreError::Io(format!("could not inspect a directory: {error}")))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(StoreError::Io(
                "the backup path must be a real directory".to_owned(),
            ));
        }
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).map_err(|error| {
            StoreError::Io(format!("could not create a private directory: {error}"))
        })?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path)
        .map_err(|error| StoreError::Io(format!("could not create a directory: {error}")))?;
    Ok(())
}

fn remove_directory_if_present(path: &Path) -> Result<(), StoreError> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(path)
        .map_err(|error| StoreError::Io(format!("could not remove a directory: {error}")))
}

fn ensure_destination_is_available(destination: &Path) -> Result<(), StoreError> {
    if !destination.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(destination)
        .map_err(|error| StoreError::Io(format!("could not inspect the destination: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(StoreError::Invalid(
            "the restore destination must be a real directory".to_owned(),
        ));
    }
    let mut entries = fs::read_dir(destination)
        .map_err(|error| StoreError::Io(format!("could not read the destination: {error}")))?;
    if entries.next().is_some() {
        return Err(StoreError::Invalid(
            "the restore destination is not empty; restore only targets a fresh installation"
                .to_owned(),
        ));
    }
    Ok(())
}

fn ensure_runtime_is_not_running(lock_path: &Path) -> Result<(), StoreError> {
    if !lock_path.exists() {
        return Ok(());
    }
    let file = fs::File::open(lock_path)
        .map_err(|error| StoreError::Io(format!("could not open the Runtime lock: {error}")))?;
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            Ok(())
        }
        Err(_) => Err(StoreError::Busy),
    }
}

fn file_size(path: &Path) -> Result<u64, StoreError> {
    let metadata = fs::metadata(path)
        .map_err(|error| StoreError::Io(format!("could not inspect a file: {error}")))?;
    Ok(metadata.len())
}

fn file_digest(path: &Path) -> Result<String, StoreError> {
    let mut file = fs::File::open(path)
        .map_err(|error| StoreError::Io(format!("could not read a file: {error}")))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| StoreError::Io(format!("could not read a file: {error}")))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("sha256:{}", hex_encode(&hasher.finalize())))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex_encode(&sha256(bytes)))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn canonical_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let value = serde_json::to_value(value)
        .map_err(|error| StoreError::Invalid(format!("value is not valid JSON: {error}")))?;
    serde_json_canonicalizer::to_vec(&value)
        .map_err(|error| StoreError::Invalid(format!("value is not canonical JSON: {error}")))
}

fn escape_identifier(value: &str) -> String {
    value.replace('"', "\"\"")
}

fn validate_backup_id(backup_id: &str) -> Result<(), StoreError> {
    if backup_id.is_empty() || backup_id.len() > 64 {
        return Err(StoreError::Invalid(
            "a backup identifier must be 1..=64 characters".to_owned(),
        ));
    }
    if !backup_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(StoreError::Invalid(
            "a backup identifier may only contain letters, digits, '-', '_' and '.'".to_owned(),
        ));
    }
    Ok(())
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn map_database_error(error: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(code, _) = &error {
        match code.code {
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                return StoreError::Busy;
            }
            rusqlite::ErrorCode::DiskFull => {
                return StoreError::Io("database disk is full".to_owned());
            }
            _ => {}
        }
    }
    StoreError::Database(error.to_string())
}

/// PBKDF2-HMAC-SHA256 (RFC 8018) over the supplied password and salt.
fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32, output: &mut [u8]) {
    type HmacSha256 = Hmac<Sha256>;
    let mut block_index: u32 = 1;
    let mut offset = 0_usize;
    while offset < output.len() {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(password)
            .expect("HMAC accepts a key of any length");
        mac.update(salt);
        mac.update(&block_index.to_be_bytes());
        let mut previous = mac.finalize().into_bytes();
        let mut accumulator = previous;
        for _ in 1..iterations {
            let mut mac = <HmacSha256 as Mac>::new_from_slice(password)
                .expect("HMAC accepts a key of any length");
            mac.update(&previous);
            previous = mac.finalize().into_bytes();
            for (target, byte) in accumulator.iter_mut().zip(previous.iter()) {
                *target ^= byte;
            }
        }
        let take = std::cmp::min(32, output.len() - offset);
        output[offset..offset + take].copy_from_slice(&accumulator[..take]);
        offset += take;
        block_index += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileBlobStore, SqliteConfig, SqliteWorkspaceStore};
    use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
    use std::sync::Arc;

    const WORKSPACE: &str = "workspace-backup";
    const PASSPHRASE: &str = "correct horse battery staple";

    /// A deterministic Workspace-scoped key provider.
    ///
    /// The production provider keeps the key material in the OS credential
    /// store. The tests use a deterministic key so the backup/restore proofs
    /// can run without a platform credential service.
    struct FixedKeys;

    impl WorkspaceBlobKeyProvider for FixedKeys {
        fn current_key(
            &self,
            workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<crate::WorkspaceBlobKey, StoreError> {
            Ok(crate::WorkspaceBlobKey {
                version: 1,
                bytes: Zeroizing::new(sha256(workspace_id.as_bytes())),
            })
        }

        fn key_by_version(
            &self,
            workspace_id: &str,
            _purpose: BlobPurpose,
            version: u32,
        ) -> Result<crate::WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown key version".to_owned()));
            }
            self.current_key(workspace_id, BlobPurpose::AggregateState)
        }
    }

    struct Installation {
        _directory: tempfile::TempDir,
        store: SqliteWorkspaceStore,
        blobs: Arc<dyn storage_core::BlobStore>,
    }

    fn install() -> Installation {
        let directory = tempfile::tempdir().expect("temporary installation directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                directory.path(),
                fs::Permissions::from_mode(0o700),
            )
            .expect("restrict the installation directory");
        }
        let blobs: Arc<dyn storage_core::BlobStore> =
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), FixedKeys));
        let store = SqliteWorkspaceStore::open(
            directory.path().join(DATABASE_FILE_NAME),
            Arc::clone(&blobs),
            SqliteConfig::default(),
        )
        .expect("the local store opens");
        Installation {
            _directory: directory,
            store,
            blobs,
        }
    }

    fn context(event_id: &str) -> EventContext {
        EventContext {
            event_id: event_id.to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            hlc_timestamp: "2026-10-20T10:00:00Z".to_owned(),
            correlation_id: "correlation-backup".to_owned(),
            causation_id: None,
            recorded_at: "2026-10-20T10:00:00Z".to_owned(),
        }
    }

    fn create_workspace(store: &SqliteWorkspaceStore) {
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Backup workspace".to_owned(),
                owner_principal_id: "owner-local".to_owned(),
                event: context("event-backup-create"),
            })
            .expect("the workspace commits");
    }

    /// Writes an aggregate blob and records the durable `BlobRef` that names it.
    fn write_blob(installation: &Installation, bytes: &[u8]) -> storage_core::BlobRef {
        let blob = installation
            .blobs
            .put(
                WORKSPACE,
                BlobPurpose::AggregateState,
                bytes,
                "application/json",
            )
            .expect("the aggregate blob commits");
        let connection = rusqlite::Connection::open(
            installation
                ._directory
                .path()
                .join(DATABASE_FILE_NAME),
        )
        .expect("inspect the store");
        connection
            .execute(
                "INSERT INTO aggregate_snapshots (workspace_id, entity_type, entity_id, through_revision, projection_schema_version, state_digest, blob_ref_json, created_at) VALUES (?1, 'Task', 'task-backup', 1, 1, ?2, ?3, '2026-10-20T10:00:00Z')",
                params![
                    WORKSPACE,
                    digest_bytes(bytes),
                    serde_json::to_string(&serde_json::json!({
                        "digest": blob.digest,
                        "size_bytes": blob.size_bytes,
                        "media_type": blob.media_type,
                    }))
                    .expect("encode the blob reference"),
                ],
            )
            .expect("record the aggregate snapshot");
        blob
    }

    fn request(backup_id: &str) -> BackupCreateRequest {
        BackupCreateRequest {
            backup_id: backup_id.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
            passphrase: BackupPassphrase::new(PASSPHRASE),
            kdf_iterations: BACKUP_KDF_MINIMUM_ITERATIONS,
            now: "2026-10-20T10:00:00Z".to_owned(),
        }
    }

    #[test]
    fn backup_and_restore_round_trip_preserves_the_workspace_and_its_blobs() {
        let installation = install();
        create_workspace(&installation.store);
        let blob = write_blob(&installation, b"{\"value\":42}");

        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("round-trip"))
            .expect("the backup is created");
        assert!(outcome.blob_count >= 1);
        assert!(outcome.manifest_row_recorded);
        assert_eq!(outcome.schema_version, crate::SCHEMA_VERSION);

        let verification = service
            .verify_backup(
                Path::new(&outcome.archive_path),
                &BackupPassphrase::new(PASSPHRASE),
            )
            .expect("the backup verifies");
        assert_eq!(verification.backup_id, "round-trip");
        assert!(verification.compatibility.is_compatible());

        let destination = installation._directory.path().join("restored");
        let restored = service
            .restore_backup(
                Path::new(&outcome.archive_path),
                &BackupPassphrase::new(PASSPHRASE),
                &destination,
                &FixedKeys,
                None,
            )
            .expect("the backup restores");
        assert_eq!(restored.restored_blob_count, outcome.blob_count);
        assert!(restored.new_runtime_identity_required);

        let restored_database = destination.join(DATABASE_FILE_NAME);
        assert!(restored_database.is_file());
        let restored_blobs = FileBlobStore::new(destination.join(BLOB_DIRECTORY_NAME), FixedKeys);
        assert_eq!(
            storage_core::BlobStore::get(
                &restored_blobs,
                WORKSPACE,
                BlobPurpose::AggregateState,
                &blob,
            )
            .expect("the restored blob decrypts"),
            b"{\"value\":42}"
        );

        let connection = rusqlite::Connection::open(&restored_database).expect("inspect the restore");
        let workspaces: i64 = connection
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .expect("count workspaces");
        assert_eq!(workspaces, 1);
        let grants: i64 = connection
            .query_row("SELECT COUNT(*) FROM capability_grants", [], |row| row.get(0))
            .expect("count grants");
        assert_eq!(grants, 0, "live authority is never restored");
    }

    #[test]
    fn the_filtered_snapshot_drops_runtime_local_tables_and_keeps_workspace_rows() {
        let installation = install();
        create_workspace(&installation.store);
        let connection = rusqlite::Connection::open(
            installation._directory.path().join(DATABASE_FILE_NAME),
        )
        .expect("inspect the store");
        connection
            .execute(
                "INSERT INTO runtimes (runtime_id, device_identity_json, runtime_version, platform, architecture, roles_json, trust_zone, availability, startup_policy, resource_capacity_json, last_seen, version) VALUES ('runtime-backup', '{}', '0.1.0', 'linux', 'x86_64', '[]', 'LOCAL', 'ONLINE', 'MANUAL', '{}', '2026-10-20T10:00:00Z', 1)",
                [],
            )
            .expect("insert the runtime row");
        connection
            .execute(
                "INSERT INTO provider_circuit_states (runtime_id, provider_kind, provider_ref, status, consecutive_failures, updated_at, version) VALUES ('runtime-backup', 'CAPABILITY', 'provider-backup', 'CLOSED', 0, '2026-10-20T10:00:00Z', 1)",
                [],
            )
            .expect("insert the local circuit state");

        let snapshot_path = installation._directory.path().join("snapshot.sqlite3");
        let summary = build_filtered_snapshot(
            &installation._directory.path().join(DATABASE_FILE_NAME),
            &snapshot_path,
        )
        .expect("the filtered snapshot is built");
        assert_eq!(summary.schema_version, crate::SCHEMA_VERSION);
        assert_eq!(
            summary.dropped_local_row_counts.get("provider_circuit_states"),
            Some(&1)
        );

        let snapshot = rusqlite::Connection::open(&snapshot_path).expect("open the snapshot");
        let circuits: i64 = snapshot
            .query_row("SELECT COUNT(*) FROM provider_circuit_states", [], |row| {
                row.get(0)
            })
            .expect("count circuits");
        assert_eq!(circuits, 0, "local provider state is not carried");
        let workspaces: i64 = snapshot
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .expect("count workspaces");
        assert_eq!(workspaces, 1, "workspace rows are carried");
        let triggers: i64 = snapshot
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = 'immutable_workspace_backup_manifest_update'",
                [],
                |row| row.get(0),
            )
            .expect("count triggers");
        assert_eq!(triggers, 1, "the snapshot keeps the source schema guards");
    }

    #[test]
    fn verification_rejects_a_corrupt_archive() {
        let installation = install();
        create_workspace(&installation.store);
        write_blob(&installation, b"payload");
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("corrupt"))
            .expect("the backup is created");
        let archive = PathBuf::from(&outcome.archive_path);
        let mut bytes = fs::read(&archive).expect("read the archive");
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        fs::write(&archive, &bytes).expect("corrupt the archive");
        assert!(matches!(
            service.verify_backup(&archive, &BackupPassphrase::new(PASSPHRASE)),
            Err(StoreError::Integrity(_))
        ));
    }

    #[test]
    fn verification_rejects_the_wrong_passphrase() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("wrong-key"))
            .expect("the backup is created");
        assert!(matches!(
            service.verify_backup(
                Path::new(&outcome.archive_path),
                &BackupPassphrase::new("not the passphrase")
            ),
            Err(StoreError::Integrity(_))
        ));
    }

    #[test]
    fn a_failed_restore_leaves_the_destination_untouched() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("partial"))
            .expect("the backup is created");
        let archive = PathBuf::from(&outcome.archive_path);
        let mut bytes = fs::read(&archive).expect("read the archive");
        bytes.truncate(bytes.len() / 2);
        fs::write(&archive, &bytes).expect("truncate the archive");

        let destination = installation._directory.path().join("restored-partial");
        assert!(matches!(
            service.restore_backup(
                &archive,
                &BackupPassphrase::new(PASSPHRASE),
                &destination,
                &FixedKeys,
                None,
            ),
            Err(StoreError::Integrity(_))
        ));
        assert!(
            !destination.exists(),
            "a rejected restore never creates the destination"
        );
    }

    #[test]
    fn a_restore_refuses_a_non_empty_destination() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("occupied"))
            .expect("the backup is created");
        let destination = installation._directory.path().join("occupied");
        fs::create_dir(&destination).expect("create the destination");
        fs::write(destination.join("owner.txt"), b"owner data").expect("owner file");

        assert!(matches!(
            service.restore_backup(
                Path::new(&outcome.archive_path),
                &BackupPassphrase::new(PASSPHRASE),
                &destination,
                &FixedKeys,
                None,
            ),
            Err(StoreError::Invalid(_))
        ));
        assert_eq!(
            fs::read(destination.join("owner.txt")).expect("owner file survives"),
            b"owner data"
        );
    }

    #[test]
    fn a_restore_refuses_a_live_runtime_lock() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("live-daemon"))
            .expect("the backup is created");
        let destination = installation._directory.path().join("live");
        fs::create_dir(&destination).expect("create the destination");
        let lock_path = destination.join("runtime.lock");
        let lock = fs::File::create(&lock_path).expect("create the lock file");
        lock.lock_exclusive().expect("hold the runtime lock");

        assert!(matches!(
            service.restore_backup(
                Path::new(&outcome.archive_path),
                &BackupPassphrase::new(PASSPHRASE),
                &destination,
                &FixedKeys,
                Some(&lock_path),
            ),
            Err(StoreError::Busy)
        ));
        assert!(
            !destination.join(DATABASE_FILE_NAME).exists(),
            "a refused restore never writes the destination"
        );
    }

    #[test]
    fn the_upgrade_check_refuses_an_unknown_schema_version() {
        let installation = install();
        let service = LocalBackupService::new(installation._directory.path());
        assert!(service.check_upgrade_compatibility().expect("version").is_compatible());

        let connection = rusqlite::Connection::open(
            installation._directory.path().join(DATABASE_FILE_NAME),
        )
        .expect("inspect the store");
        connection
            .pragma_update(None, "user_version", crate::SCHEMA_VERSION + 1)
            .expect("raise the schema version");
        let compatibility = service
            .check_upgrade_compatibility()
            .expect("the compatibility verdict");
        assert!(!compatibility.is_compatible());
        assert!(compatibility.reason().contains("newer"));
    }

    #[test]
    fn a_restore_refuses_a_manifest_that_declares_a_foreign_schema() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("foreign-schema"))
            .expect("the backup is created");
        let archive = PathBuf::from(&outcome.archive_path);
        let mut manifest = read_container_manifest(&archive, PASSPHRASE).expect("read the manifest");
        manifest.schema_version = crate::SCHEMA_VERSION + 1;
        let snapshot = installation._directory.path().join("foreign.sqlite3");
        fs::copy(
            installation._directory.path().join(DATABASE_FILE_NAME),
            &snapshot,
        )
        .expect("copy the database");
        assert!(matches!(
            verify_restored_snapshot(&snapshot, &manifest),
            Err(StoreError::CorruptSchema(_))
        ));
    }

    #[test]
    fn the_passphrase_never_appears_in_the_archive_or_its_name() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let outcome = service
            .create_backup(&FixedKeys, &request("no-secret"))
            .expect("the backup is created");
        assert!(!outcome.archive_path.contains(PASSPHRASE));
        let bytes = fs::read(&outcome.archive_path).expect("read the archive");
        assert!(
            !contains_bytes(&bytes, PASSPHRASE.as_bytes()),
            "the passphrase is never stored in the archive"
        );
        let key_material = sha256(WORKSPACE.as_bytes());
        assert!(
            !contains_bytes(&bytes, &key_material),
            "the workspace key material is never stored in the archive"
        );
    }

    #[test]
    fn a_failed_backup_leaves_no_usable_archive() {
        let installation = install();
        create_workspace(&installation.store);
        let service = LocalBackupService::new(installation._directory.path());
        let backup_directory = installation._directory.path().join("backups");
        fs::create_dir_all(&backup_directory).expect("create the backup directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&backup_directory, fs::Permissions::from_mode(0o500))
                .expect("make the backup directory read only");
        }
        let result = service.create_backup(&FixedKeys, &request("unwritable"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&backup_directory, fs::Permissions::from_mode(0o700))
                .expect("restore the backup directory permissions");
        }
        assert!(result.is_err(), "an unwritable backup directory fails");
        assert!(
            !backup_directory.join("unwritable.lcbbackup").exists(),
            "a failed backup leaves no archive"
        );
        assert!(
            !installation
                ._directory
                .path()
                .join(".backup-staging-unwritable")
                .exists(),
            "a failed backup leaves no staging area"
        );
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() || haystack.len() < needle.len() {
            return false;
        }
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }
}
