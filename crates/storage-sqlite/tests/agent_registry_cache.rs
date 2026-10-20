use std::sync::Arc;

use serde_json::json;
use storage_core::{
    AgentRegistryCacheEntry, AgentRegistryCacheSnapshot, AgentRegistryCacheStore, BlobPurpose,
    StoreError,
};
use storage_sqlite::{
    FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use zeroize::Zeroizing;

struct TestKeys;

impl WorkspaceBlobKeyProvider for TestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([11; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob("unknown test key version".to_owned()));
        }
        self.current_key(workspace_id, purpose)
    }
}

fn snapshot(revision: &str, expires_at: &str, ids: &[&str]) -> AgentRegistryCacheSnapshot {
    AgentRegistryCacheSnapshot {
        source_registry: "https://registry.example.test/index.json".to_owned(),
        source_revision: Some(revision.to_owned()),
        observed_at: "2026-10-19T10:00:00Z".to_owned(),
        expires_at: Some(expires_at.to_owned()),
        entries: ids
            .iter()
            .map(|id| AgentRegistryCacheEntry {
                registry_agent_id: (*id).to_owned(),
                metadata: json!({"name":id,"distribution":{"npx":{"package":"safe-package"}}}),
            })
            .collect(),
    }
}

fn test_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
    let state = directory.path().join("state");
    std::fs::create_dir(&state).expect("create private state directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700))
            .expect("restrict state directory permissions");
    }
    let blobs = Arc::new(FileBlobStore::new(state.join("blobs"), TestKeys));
    SqliteWorkspaceStore::open(
        state.join("litecowork.sqlite3"),
        blobs,
        SqliteConfig::default(),
    )
    .expect("SQLite store opens")
}

#[test]
fn registry_snapshot_replaces_atomically_and_expired_rows_remain_available_as_stale() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    let old = snapshot(
        "old-revision",
        "2026-10-19T10:15:00Z",
        &["agent-a", "agent-b"],
    );
    store
        .replace_agent_registry_snapshot(old.clone())
        .expect("first snapshot persists");

    // Reads intentionally do not filter expires_at: the Operator decides whether to
    // label this snapshot stale when the network is unavailable.
    assert_eq!(
        store
            .read_agent_registry_snapshot(&old.source_registry)
            .expect("expired cache reads")
            .expect("snapshot exists"),
        old
    );

    let replacement = snapshot("new-revision", "2026-10-19T10:30:00Z", &["agent-c"]);
    store
        .replace_agent_registry_snapshot(replacement.clone())
        .expect("replacement commits");
    assert_eq!(
        store
            .read_agent_registry_snapshot(&replacement.source_registry)
            .expect("replacement reads"),
        Some(replacement.clone())
    );

    let empty = AgentRegistryCacheSnapshot {
        entries: Vec::new(),
        ..replacement.clone()
    };
    store
        .replace_agent_registry_snapshot(empty)
        .expect("empty source snapshot clears prior rows");
    assert_eq!(
        store
            .read_agent_registry_snapshot(&replacement.source_registry)
            .expect("empty cache read"),
        None
    );
}

#[test]
fn malformed_snapshot_is_rejected_without_replacing_the_last_good_cache() {
    let directory = tempfile::tempdir().expect("temporary store directory");
    let store = test_store(&directory);
    let original = snapshot("r1", "2026-10-19T10:15:00Z", &["agent-a"]);
    store
        .replace_agent_registry_snapshot(original.clone())
        .expect("original snapshot persists");
    let malformed = snapshot("r2", "2026-10-19T10:30:00Z", &["agent-a", "agent-a"]);
    assert!(matches!(
        store.replace_agent_registry_snapshot(malformed),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        store
            .read_agent_registry_snapshot(&original.source_registry)
            .expect("last good snapshot remains readable"),
        Some(original)
    );
}
