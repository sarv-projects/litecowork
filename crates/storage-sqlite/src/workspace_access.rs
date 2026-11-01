//! One authority for Workspace ownership and archive policy in storage.
//!
//! The contract distinguishes reading from mutating an ARCHIVED Workspace:
//! archived Workspaces remain readable and keep authorized downloads, but reject
//! every domain mutation (`docs/API.md`, "Archive ..."). A write refused for that
//! reason reports `WORKSPACE_ARCHIVED`, not `NOT_FOUND`.
//!
//! Before this module that distinction lived in hand-written SQL at each call
//! site, so every new feature re-decided it. Two failure modes had already
//! appeared:
//!
//! * mutations that inlined `status='ACTIVE'` and reported `NotFound`, losing the
//!   contractual `WORKSPACE_ARCHIVED` code; and
//! * reads that inlined `status='ACTIVE'` and therefore refused an archived
//!   Workspace that the contract keeps readable.
//!
//! Callers now state which fact they need — [`require_read_access`] or
//! [`require_write_access`] — instead of copying a predicate and hoping it
//! matches their operation. Non-ownership stays non-disclosing: a caller who
//! does not own the Workspace always receives [`StoreError::NotFound`], never a
//! distinction between "missing" and "archived".

use super::*;

/// Stable error code for a mutation refused because the Workspace is archived.
///
/// Operator route modules match on `StoreError::Invalid` with this message, so
/// the code travels with the storage error rather than being re-derived per
/// feature.
pub(crate) const WORKSPACE_ARCHIVED: &str = "WORKSPACE_ARCHIVED";

/// Refuse when the caller does not own the Workspace.
///
/// An owned ARCHIVED Workspace is accepted: archive is a read-only transition,
/// not a deletion.
pub(crate) fn require_read_access(
    connection: &Connection,
    workspace_id: &str,
    principal_id: &str,
) -> Result<(), StoreError> {
    match workspace_status(connection, workspace_id, principal_id)? {
        Some(_) => Ok(()),
        None => Err(StoreError::NotFound),
    }
}

/// Refuse when the caller does not own the Workspace, or owns an ARCHIVED one.
///
/// Returns `WORKSPACE_ARCHIVED` only to a caller who already proved ownership, so
/// the archive state is never disclosed to a non-owner.
pub(crate) fn require_write_access(
    connection: &Connection,
    workspace_id: &str,
    principal_id: &str,
) -> Result<(), StoreError> {
    match workspace_status(connection, workspace_id, principal_id)? {
        Some(status) if status == "ACTIVE" => Ok(()),
        Some(status) if status == "ARCHIVED" => Err(StoreError::Invalid(WORKSPACE_ARCHIVED.to_owned())),
        Some(_) => Err(StoreError::Invalid(WORKSPACE_ARCHIVED.to_owned())),
        None => Err(StoreError::NotFound),
    }
}

/// Read the status of a Workspace the caller owns.
///
/// Returns `None` both when the Workspace does not exist and when it exists but
/// the caller does not own it, so callers cannot accidentally disclose which.
fn workspace_status(
    connection: &Connection,
    workspace_id: &str,
    principal_id: &str,
) -> Result<Option<String>, StoreError> {
    if workspace_id.trim().is_empty() || principal_id.trim().is_empty() {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT status FROM workspaces WHERE workspace_id = ?1 AND owner_principal_id = ?2",
            params![workspace_id, principal_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(map_database_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BlobPurpose, FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };
    use std::sync::Arc;
    use std::time::Duration;

    const OWNER: &str = "owner-access";
    const OTHER_OWNER: &str = "owner-elsewhere";
    const WORKSPACE: &str = "workspace-access";
    const TIMESTAMP: &str = "2026-10-21T09:00:00Z";

    #[derive(Clone)]
    struct TestKeys {
        key: [u8; 32],
    }

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new(self.key),
            })
        }

        fn key_by_version(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if version != 1 {
                return Err(StoreError::Blob("unknown test key version".to_owned()));
            }
            self.current_key("", BlobPurpose::AggregateState)
        }
    }

    fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
        directory.path().join("state").join("state.sqlite3")
    }

    fn test_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
        let blob_store = Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys { key: [31_u8; 32] },
        ));
        SqliteWorkspaceStore::open(
            state_database(directory),
            blob_store,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(5),
            },
        )
        .expect("SQLite store opens")
    }

    fn create_workspace(store: &SqliteWorkspaceStore) {
        let event = domain_workspace::EventContext {
            event_id: "event-workspace-access".to_owned(),
            origin_runtime_id: "runtime-local".to_owned(),
            hlc_timestamp: TIMESTAMP.to_owned(),
            correlation_id: "correlation-access".to_owned(),
            causation_id: None,
            recorded_at: TIMESTAMP.to_owned(),
        };
        domain_workspace::WorkspaceService::new(store.clone())
            .create(domain_workspace::CreateWorkspace {
                workspace_id: WORKSPACE.to_owned(),
                name: "Access workspace".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event,
            })
            .expect("Workspace creation commits");
    }

    fn archive(directory: &tempfile::TempDir) {
        Connection::open(state_database(directory))
            .expect("open inspection connection")
            .execute(
                "UPDATE workspaces SET status = 'ARCHIVED', version = version + 1 WHERE workspace_id = ?1",
                params![WORKSPACE],
            )
            .expect("archive Workspace");
    }

    #[test]
    fn an_active_workspace_allows_reads_and_writes_for_its_owner() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = test_store(&directory);
        create_workspace(&store);
        let connection = Connection::open(state_database(&directory)).expect("connection");
        require_read_access(&connection, WORKSPACE, OWNER).expect("owner reads active Workspace");
        require_write_access(&connection, WORKSPACE, OWNER).expect("owner writes active Workspace");
    }

    #[test]
    fn an_archived_workspace_stays_readable_but_rejects_writes_with_the_contract_code() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = test_store(&directory);
        create_workspace(&store);
        archive(&directory);
        let connection = Connection::open(state_database(&directory)).expect("connection");
        require_read_access(&connection, WORKSPACE, OWNER)
            .expect("archived Workspace remains readable");
        match require_write_access(&connection, WORKSPACE, OWNER) {
            Err(StoreError::Invalid(code)) => assert_eq!(code, WORKSPACE_ARCHIVED),
            other => panic!("archived write must report WORKSPACE_ARCHIVED, got {other:?}"),
        }
    }

    #[test]
    fn a_non_owner_is_not_told_whether_the_workspace_exists_or_is_archived() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = test_store(&directory);
        create_workspace(&store);
        let connection = Connection::open(state_database(&directory)).expect("connection");
        assert!(matches!(
            require_read_access(&connection, WORKSPACE, OTHER_OWNER),
            Err(StoreError::NotFound)
        ));
        archive(&directory);
        assert!(matches!(
            require_write_access(&connection, WORKSPACE, OTHER_OWNER),
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            require_write_access(&connection, "workspace-absent", OWNER),
            Err(StoreError::NotFound)
        ));
    }

    #[test]
    fn blank_identifiers_are_refused_without_touching_the_database() {
        let directory = tempfile::tempdir().expect("tempdir");
        let _store = test_store(&directory);
        let connection = Connection::open(state_database(&directory)).expect("connection");
        assert!(matches!(
            require_read_access(&connection, "", OWNER),
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            require_write_access(&connection, WORKSPACE, "  "),
            Err(StoreError::NotFound)
        ));
    }
}
