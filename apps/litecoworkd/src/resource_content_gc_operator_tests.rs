use super::{OperatorServer, RuntimeLifecycleBlocker};
use axum::http::StatusCode;
use domain_workspace::EventContext;
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    RequestFrame, RequestHeader, unix::UnixEndpoint,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use storage_core::{BlobPurpose, BlobRef, BlobStore, StoreError};
use storage_sqlite::{
    FileBlobStore, RuntimeOsPrincipalIdentity, SqliteConfig, SqliteWorkspaceStore,
    WorkspaceBlobKey, WorkspaceBlobKeyProvider,
};
use zeroize::Zeroizing;

const WORKSPACE_ID: &str = "resource-content-gc-workspace";
const OWNER_ID: &str = "resource-content-gc-owner";
const RUNTIME_ID: &str = "resource-content-gc-runtime";
const INCARNATION_ID: &str = "resource-content-gc-incarnation";
const RESOURCE_BYTES: &[u8] = b"registered Resource object retained for GC recovery";
const CHILD_MODE_ENV: &str = "LITECOWORK_RESOURCE_CONTENT_GC_CHILD_MODE";
const CHILD_DIRECTORY_ENV: &str = "LITECOWORK_RESOURCE_CONTENT_GC_CHILD_DIRECTORY";
const CHILD_READY_ENV: &str = "LITECOWORK_RESOURCE_CONTENT_GC_CHILD_READY";
const TEST_NAME: &str = "operator::resource_content_gc_operator_tests::resource_content_gc_periodic_sweep_retries_after_process_restart";

struct ChildGuard(std::process::Child);

impl ChildGuard {
    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.0.try_wait()
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()
    }

    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.0.wait()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

#[derive(Clone)]
struct GcTestKeys;

impl WorkspaceBlobKeyProvider for GcTestKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([0x42; 32]),
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

struct RemoveThenFailResourceBlobStore {
    inner: Arc<dyn BlobStore>,
}

impl BlobStore for RemoveThenFailResourceBlobStore {
    fn put(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<BlobRef, StoreError> {
        self.inner.put(workspace_id, purpose, bytes, media_type)
    }

    fn get(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<Vec<u8>, StoreError> {
        self.inner.get(workspace_id, purpose, blob)
    }

    fn remove(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        blob: &BlobRef,
    ) -> Result<(), StoreError> {
        self.inner.remove(workspace_id, purpose, blob)?;
        if purpose == BlobPurpose::Resource {
            return Err(StoreError::Blob(
                "injected ambiguous Resource removal acknowledgment".to_owned(),
            ));
        }
        Ok(())
    }
}

fn open_store(directory: &Path, blobs: Arc<dyn BlobStore>) -> SqliteWorkspaceStore {
    SqliteWorkspaceStore::open(
        directory.join("state.sqlite3"),
        blobs,
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("real SQLite store opens")
}

fn file_blobs(directory: &Path) -> Arc<dyn BlobStore> {
    Arc::new(FileBlobStore::new(directory.join("blobs"), GcTestKeys))
}

fn registry_state(directory: &Path, blob: &BlobRef) -> Result<Option<String>, String> {
    let connection = Connection::open(directory.join("state.sqlite3"))
        .map_err(|_| "could not inspect Resource content registry")?;
    connection
        .query_row(
            "SELECT state FROM resource_content_blob_registry
             WHERE workspace_id = ?1 AND digest = ?2",
            params![WORKSPACE_ID, blob.digest],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| "could not read Resource content registry state".to_owned())
}

async fn wait_for_registry_state(
    directory: &Path,
    blob: &BlobRef,
    expected: Option<&str>,
) -> Result<bool, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if registry_state(directory, blob)?.as_deref() == expected {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_resource_blob_removal(
    blobs: &dyn BlobStore,
    blob: &BlobRef,
) -> Result<bool, StoreError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match blobs.get(WORKSPACE_ID, BlobPurpose::Resource, blob) {
            Err(StoreError::NotFound) => return Ok(true),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn check_operator_readiness(directory: &Path, request_id: &str) -> Result<(), String> {
    let endpoint = UnixEndpoint::derive(directory).map_err(|_| "could not derive IPC endpoint")?;
    let expected_uid = fs::metadata(directory)
        .map_err(|_| "could not inspect IPC data directory")?
        .uid();
    let mut client = endpoint
        .connect(|peer| peer.uid == expected_uid)
        .await
        .map_err(|_| "could not connect to authenticated local Operator")?;
    let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .map_err(|_| "could not create bounded IPC body budget")?;
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/operator/readiness".to_owned(),
            headers: Vec::<LogicalHeader>::new(),
            body_length: 0,
        },
        Vec::new(),
        &budget,
    )
    .map_err(|_| "could not create readiness request frame")?;
    client
        .write_request(&request)
        .await
        .map_err(|_| "could not write readiness request")?;
    let response = client
        .read_response(&budget)
        .await
        .map_err(|_| "could not read readiness response")?;
    if response.header.status != StatusCode::OK.as_u16() {
        return Err("Operator readiness returned a non-success status".to_owned());
    }
    let body: Value = serde_json::from_slice(response.body())
        .map_err(|_| "Operator readiness response was malformed")?;
    if body.get("operator_state") != Some(&json!("SERVING")) {
        return Err("Operator did not report SERVING".to_owned());
    }
    Ok(())
}

async fn run_failing_sweeper_child(directory: &Path, ready_marker: &Path) {
    let inner = file_blobs(directory);
    let store = open_store(
        directory,
        Arc::new(RemoveThenFailResourceBlobStore {
            inner: Arc::clone(&inner),
        }),
    );
    let server = OperatorServer::start(
        directory,
        store,
        OWNER_ID.to_owned(),
        RUNTIME_ID.to_owned(),
        INCARNATION_ID.to_owned(),
        Vec::<RuntimeLifecycleBlocker>::new(),
        RuntimeOsPrincipalIdentity::test_support(RUNTIME_ID)
            .expect("test-only authenticated local identity"),
    )
    .expect("actual OperatorServer starts");
    let blob = BlobRef {
        digest: digest(RESOURCE_BYTES),
        size_bytes: RESOURCE_BYTES.len() as u64,
        media_type: "text/plain".to_owned(),
    };
    assert!(
        wait_for_registry_state(directory, &blob, Some("DELETING"))
            .await
            .expect("poll durable delete fence"),
        "the periodic daemon sweep should durably fence the object before provider removal"
    );
    assert!(
        wait_for_resource_blob_removal(inner.as_ref(), &blob)
            .await
            .expect("poll provider removal after durable fence"),
        "the provider should remove the object after the durable delete fence"
    );
    fs::write(ready_marker, b"deletion fenced and object removed")
        .expect("signal parent after the ambiguous provider acknowledgment");
    let _server = server;
    std::future::pending::<()>().await;
}

fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)))
}

#[tokio::test]
async fn resource_content_gc_periodic_sweep_retries_after_process_restart() {
    if std::env::var(CHILD_MODE_ENV).as_deref() == Ok("1") {
        let directory = std::env::var_os(CHILD_DIRECTORY_ENV)
            .expect("child mode includes its private Runtime directory");
        let ready_marker = std::env::var_os(CHILD_READY_ENV)
            .expect("child mode includes its private readiness marker");
        run_failing_sweeper_child(Path::new(&directory), Path::new(&ready_marker)).await;
        return;
    }

    let directory = tempfile::tempdir().expect("temporary Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private Runtime data directory");
    let initial_blobs = file_blobs(directory.path());
    let initial_store = open_store(directory.path(), Arc::clone(&initial_blobs));
    let now = "2026-10-15T00:00:00.000000000Z";
    domain_workspace::WorkspaceService::new(initial_store.clone())
        .create(domain_workspace::CreateWorkspace {
            workspace_id: WORKSPACE_ID.to_owned(),
            name: "Resource content GC recovery test".to_owned(),
            owner_principal_id: OWNER_ID.to_owned(),
            event: EventContext {
                event_id: "resource-content-gc-workspace-created".to_owned(),
                origin_runtime_id: RUNTIME_ID.to_owned(),
                hlc_timestamp: now.to_owned(),
                correlation_id: "resource-content-gc-workspace-create".to_owned(),
                causation_id: None,
                recorded_at: now.to_owned(),
            },
        })
        .expect("Workspace fixture commits");
    let blob = initial_blobs
        .put(
            WORKSPACE_ID,
            BlobPurpose::Resource,
            RESOURCE_BYTES,
            "text/plain",
        )
        .expect("actual encrypted Resource object publishes");
    drop(initial_store);
    Connection::open(directory.path().join("state.sqlite3"))
        .expect("open registry fixture connection")
        .execute(
            "INSERT INTO resource_content_blob_registry(
               workspace_id, digest, size_bytes, media_type, created_at,
               expires_at, state, claimed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'TRACKED', NULL)",
            params![
                WORKSPACE_ID,
                blob.digest,
                blob.size_bytes as i64,
                blob.media_type,
                now,
            ],
        )
        .expect("seed an unreferenced but registered local object");
    drop(initial_blobs);

    let ready_marker = directory.path().join("gc-child-ready");
    let child_stderr_path = directory.path().join("gc-child-stderr.log");
    let child_stderr =
        fs::File::create(&child_stderr_path).expect("create private child diagnostic log");
    let executable = std::env::current_exe().expect("current daemon test executable");
    let mut child = ChildGuard(
        Command::new(executable)
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(CHILD_MODE_ENV, "1")
            .env(CHILD_DIRECTORY_ENV, directory.path())
            .env(CHILD_READY_ENV, &ready_marker)
            .stdout(Stdio::null())
            .stderr(Stdio::from(child_stderr))
            .spawn()
            .expect("spawn the first OperatorServer process"),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut child_ready = false;
    let mut child_exit = None;
    while Instant::now() < deadline {
        if ready_marker.exists() {
            child_ready = true;
            break;
        }
        if let Some(status) = child.try_wait().expect("check sweeper child process") {
            child_exit = Some(status);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let child_state = registry_state(directory.path(), &blob).expect("read child registry state");
    let object_missing = matches!(
        file_blobs(directory.path()).get(WORKSPACE_ID, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    );
    let child_readiness = if child_ready {
        check_operator_readiness(directory.path(), "resource-content-gc-first-server").await
    } else {
        Err("first OperatorServer process did not signal its sweep result".to_owned())
    };
    if child
        .try_wait()
        .expect("check child before simulated crash")
        .is_none()
        && let Err(kill_error) = child.kill()
    {
        assert!(
            child
                .try_wait()
                .expect("recheck child after kill race")
                .is_some(),
            "could not terminate first daemon process: {kill_error}"
        );
    }
    let crash_status = child.wait().expect("reap first daemon process");
    let child_stderr = if child_ready && child_readiness.is_ok() {
        String::new()
    } else {
        fs::read_to_string(&child_stderr_path).unwrap_or_default()
    };

    assert!(
        child_ready,
        "first daemon should run its initial periodic sweep; child status: {child_exit:?}; child stderr: {child_stderr}"
    );
    assert_eq!(child_state.as_deref(), Some("DELETING"));
    assert!(
        object_missing,
        "the provider removed the object before its error acknowledgment"
    );
    assert!(
        child_readiness.is_ok(),
        "server remained available: {child_readiness:?}; child stderr: {child_stderr}"
    );
    assert!(
        !crash_status.success(),
        "the first server process was terminated abruptly"
    );

    let recovery_blobs = file_blobs(directory.path());
    let recovery_store = open_store(directory.path(), Arc::clone(&recovery_blobs));
    let server = OperatorServer::start(
        directory.path(),
        recovery_store.clone(),
        OWNER_ID.to_owned(),
        RUNTIME_ID.to_owned(),
        INCARNATION_ID.to_owned(),
        Vec::<RuntimeLifecycleBlocker>::new(),
        RuntimeOsPrincipalIdentity::test_support(RUNTIME_ID)
            .expect("test-only authenticated local identity"),
    )
    .expect("restarted OperatorServer starts");
    assert!(
        wait_for_registry_state(directory.path(), &blob, None)
            .await
            .expect("poll recovered registry"),
        "the restarted daemon's first sweep should retry the missing object and clear its fence"
    );
    assert!(matches!(
        recovery_blobs.get(WORKSPACE_ID, BlobPurpose::Resource, &blob),
        Err(StoreError::NotFound)
    ));
    check_operator_readiness(directory.path(), "resource-content-gc-restarted-server")
        .await
        .expect("restarted OperatorServer serves authenticated IPC");
    server
        .stop()
        .expect("restarted OperatorServer stops cleanly");
}
