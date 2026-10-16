use super::{
    RuntimeState, STATE_FILE, read_state, runtime_process_tests::assert_degraded_lifecycle,
};
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    RequestFrame, RequestHeader, ResponseFrame, unix::UnixEndpoint,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use storage_core::{BlobPurpose, BlobStore, StoreError};
use storage_sqlite::{
    FileBlobStore, LocalWorkspaceStorage, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
    WorkspaceBlobKeyProvider,
};
use zeroize::Zeroizing;

const CHILD_DATA_DIRECTORY: &str = "LITECOWORK_TEST_WORKSPACE_REPLAY_DATA_DIRECTORY";
const CHILD_ENTRY_TEST: &str =
    "runtime::runtime_workspace_replay_process_tests::spawned_workspace_replay_process_probe";
const WORKSPACE_CREATE_KEY: &str = "spawned-runtime-workspace-create-lost-ack-v1";
const CHILD_START_DEADLINE: Duration = Duration::from_secs(10);
const RECEIPT_DEADLINE: Duration = Duration::from_secs(15);
const CHILD_STOP_DEADLINE: Duration = Duration::from_secs(10);

struct ReplayTestBlobKeys;

impl WorkspaceBlobKeyProvider for ReplayTestBlobKeys {
    fn current_key(
        &self,
        _workspace_id: &str,
        _purpose: BlobPurpose,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        Ok(WorkspaceBlobKey {
            version: 1,
            bytes: Zeroizing::new([53_u8; 32]),
        })
    }

    fn key_by_version(
        &self,
        workspace_id: &str,
        purpose: BlobPurpose,
        version: u32,
    ) -> Result<WorkspaceBlobKey, StoreError> {
        if version != 1 {
            return Err(StoreError::Blob(
                "unknown replay test key version".to_owned(),
            ));
        }
        self.current_key(workspace_id, purpose)
    }
}

pub(super) fn open_test_storage(
    data_directory: &Path,
    config: SqliteConfig,
) -> Result<LocalWorkspaceStorage, StoreError> {
    let blobs: Arc<dyn BlobStore> = Arc::new(FileBlobStore::new(
        data_directory.join("blobs"),
        ReplayTestBlobKeys,
    ));
    let store = SqliteWorkspaceStore::open(
        data_directory.join("litecowork.sqlite3"),
        Arc::clone(&blobs),
        config,
    )?;
    Ok(LocalWorkspaceStorage { store, blobs })
}

struct RuntimeChild(Child);

impl RuntimeChild {
    fn spawn(data_directory: &Path) -> Self {
        let child = Command::new(std::env::current_exe().expect("current test executable"))
            .args(["--exact", CHILD_ENTRY_TEST, "--nocapture"])
            .env(CHILD_DATA_DIRECTORY, data_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn Runtime process test child");
        Self(child)
    }

    fn child(&mut self) -> &mut Child {
        &mut self.0
    }

    fn terminate_and_wait(&mut self) -> (ExitStatus, String, String) {
        if self.0.try_wait().expect("check Runtime child").is_none() {
            let pid = rustix::process::Pid::from_raw(self.0.id() as rustix::process::RawPid)
                .expect("child process ID is valid");
            rustix::process::kill_process(pid, rustix::process::Signal::TERM)
                .expect("send graceful Runtime shutdown signal");
        }
        wait_for_exit(&mut self.0, CHILD_STOP_DEADLINE)
    }
}

impl Drop for RuntimeChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[derive(Debug, PartialEq)]
struct DurableWorkspaceCreate {
    owner_principal_id: String,
    request_digest: String,
    response_digest: String,
    receipt: Value,
    workspace_count: i64,
    created_event_count: i64,
    workspace_event_count: i64,
    request_receipt_count: i64,
}

struct LostAckWorkspaceCreate {
    receipt: DurableWorkspaceCreate,
    _connection: operator_ipc::unix::AuthenticatedUnixConnection,
    _runtime: tokio::runtime::Runtime,
}

#[derive(Debug, PartialEq)]
struct DurableInstructionRevision {
    owner_principal_id: String,
    request_digest: String,
    response_digest: String,
    response: Value,
    workspace_version: i64,
    current_instruction_revision: Option<i64>,
    revision_count: i64,
    event_count: i64,
    request_receipt_count: i64,
    event_payload_json: String,
}

struct LostAckInstructionRevision {
    receipt: DurableInstructionRevision,
    _connection: operator_ipc::unix::AuthenticatedUnixConnection,
    _runtime: tokio::runtime::Runtime,
}

struct LostAckOperatorRequest {
    _connection: operator_ipc::unix::AuthenticatedUnixConnection,
    _runtime: tokio::runtime::Runtime,
}

#[derive(Debug, PartialEq)]
struct DurableContextDocumentPurge {
    principal_id: String,
    request_digest: String,
    response_json: String,
    response_digest: String,
    manifest_digest: String,
    target_count: i64,
    targets_json: String,
    receipts: Vec<DurableContextDocumentPurgeReceipt>,
}

#[derive(Debug, PartialEq)]
struct DurableContextDocumentPurgeReceipt {
    replica_ref: String,
    replica_kind: String,
    status: String,
    runtime_id: Option<String>,
    runtime_incarnation_id: Option<String>,
    target_revision_ids_json: String,
    target_upload_chunk_refs_json: String,
    receipt_digest: Option<String>,
    acknowledged_at: Option<String>,
}

#[test]
fn spawned_workspace_replay_process_probe() {
    let Some(directory) = std::env::var_os(CHILD_DATA_DIRECTORY) else {
        return;
    };
    super::run_with_test_identity_providers(Path::new(&directory))
        .expect("spawned Runtime exits after its supervisor signal");
}

#[test]
fn spawned_daemon_replays_workspace_create_after_lost_ack_and_restart() {
    let directory = tempfile::tempdir().expect("private Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict Runtime directory");

    let mut first_child = RuntimeChild::spawn(directory.path());
    let first_readiness = wait_for_operator_readiness(first_child.child(), directory.path());
    assert_operator_readiness(&first_readiness);
    let runtime_id = first_readiness["runtime_id"]
        .as_str()
        .expect("first readiness exposes RuntimeId")
        .to_owned();
    let first_incarnation = first_readiness["local_incarnation_id"]
        .as_str()
        .expect("first readiness exposes local incarnation")
        .to_owned();
    let first_lifecycle = operator_get_request(
        directory.path(),
        "/v1/runtime-lifecycle",
        "workspace-replay-lifecycle-before-create",
    )
    .expect("authenticated lifecycle projection is available after readiness");
    assert_degraded_lifecycle(&first_lifecycle, &runtime_id, &first_incarnation);
    let local_principal_id = read_state(&directory.path().join(STATE_FILE))
        .expect("read first Runtime state")
        .expect("first Runtime state exists")
        .local_principal_id
        .expect("test Runtime has a local principal");

    // Write the actual authenticated IPC request, keep its response unread, and wait
    // until SQLite exposes the committed receipt. Keep the connection open through
    // server drain so peer closure cannot race the response write; closing it after
    // the daemon exits models an ACK the caller never observed.
    let lost_ack = match submit_create_without_reading_response(
        directory.path(),
        WORKSPACE_CREATE_KEY,
        "workspace-create-before-lost-ack",
        br#"{"name":"Workspace lost ACK fixture"}"#,
        &local_principal_id,
    ) {
        Ok(lost_ack) => lost_ack,
        Err(error) => {
            let (status, stdout, stderr) = first_child.terminate_and_wait();
            panic!("{error}; Runtime status={status}; stdout={stdout}; stderr={stderr}");
        }
    };
    let LostAckWorkspaceCreate {
        receipt: committed_before_restart,
        _connection: unread_response_connection,
        _runtime: ipc_runtime,
    } = lost_ack;
    assert_single_workspace_commit(
        &committed_before_restart,
        &local_principal_id,
        WORKSPACE_CREATE_KEY,
    );

    let (first_status, first_stdout, first_stderr) = first_child.terminate_and_wait();
    assert!(
        first_status.success(),
        "first Runtime shutdown failed: stdout={first_stdout} stderr={first_stderr}"
    );
    let stopped_state = read_state(&directory.path().join(STATE_FILE))
        .expect("read stopped Runtime state")
        .expect("Runtime state persists");
    assert_eq!(stopped_state.recovery_state, RuntimeState::Stopped);
    assert!(stopped_state.last_shutdown_clean);
    assert_eq!(
        stopped_state.local_principal_id.as_deref(),
        Some(local_principal_id.as_str())
    );
    // Keep the unread response connection alive until the daemon has fully drained.
    // This makes the lost ACK an unread successful response rather than a peer-reset
    // racing the server's response write.
    drop(unread_response_connection);
    drop(ipc_runtime);

    let mut restarted_child = RuntimeChild::spawn(directory.path());
    let restarted_readiness =
        wait_for_operator_readiness(restarted_child.child(), directory.path());
    assert_operator_readiness(&restarted_readiness);
    assert_eq!(
        restarted_readiness["runtime_id"].as_str(),
        Some(runtime_id.as_str())
    );
    let restarted_incarnation = restarted_readiness["local_incarnation_id"]
        .as_str()
        .expect("restarted readiness exposes local incarnation");
    assert_ne!(restarted_incarnation, first_incarnation);
    let restarted_lifecycle = operator_get_request(
        directory.path(),
        "/v1/runtime-lifecycle",
        "workspace-replay-lifecycle-after-restart",
    )
    .expect("authenticated lifecycle projection is available after restarted readiness");
    assert_degraded_lifecycle(&restarted_lifecycle, &runtime_id, restarted_incarnation);

    let replay = send_workspace_create(
        directory.path(),
        "workspace-create-after-restart-retry",
        WORKSPACE_CREATE_KEY,
        br#"{"name":"Workspace lost ACK fixture"}"#,
    );
    assert_eq!(replay.header.status, 201, "exact retry returns CREATED");
    assert_eq!(
        replay.header.request_id, "workspace-create-after-restart-retry",
        "the new frame ID is only response correlation"
    );
    let replay_body: Value =
        serde_json::from_slice(replay.body()).expect("replayed Workspace body is JSON");
    assert_eq!(
        replay_body, committed_before_restart.receipt["workspace"],
        "replay returns the Workspace projection committed before restart"
    );
    let correlation_id = replay
        .header
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("x-correlation-id"))
        .map(|header| header.value.as_str());
    assert_eq!(
        correlation_id,
        committed_before_restart.receipt["event"]["correlation_id"].as_str(),
        "replay retains the committed event correlation ID"
    );

    let changed_payload = send_workspace_create(
        directory.path(),
        "workspace-create-after-restart-conflict",
        WORKSPACE_CREATE_KEY,
        br#"{"name":"Changed payload must conflict"}"#,
    );
    assert_eq!(changed_payload.header.status, 409);
    assert_eq!(
        changed_payload.header.request_id,
        "workspace-create-after-restart-conflict"
    );
    let conflict: Value =
        serde_json::from_slice(changed_payload.body()).expect("conflict response is JSON");
    assert_eq!(conflict["code"], "CONFLICT");

    let after_retry =
        read_durable_create(directory.path(), WORKSPACE_CREATE_KEY, &local_principal_id)
            .expect("Workspace retry inspection succeeds")
            .expect("committed Workspace receipt remains present after retries");
    assert_eq!(after_retry, committed_before_restart);
    assert_single_workspace_commit(&after_retry, &local_principal_id, WORKSPACE_CREATE_KEY);

    let (restart_status, restart_stdout, restart_stderr) = restarted_child.terminate_and_wait();
    assert!(
        restart_status.success(),
        "restarted Runtime shutdown failed: stdout={restart_stdout} stderr={restart_stderr}"
    );
}

#[test]
fn spawned_daemon_recovers_resource_upload_create_chunk_and_commit_after_lost_ack() {
    const CHUNK_SIZE: usize = 4 * 1024 * 1024;
    const CREATE_KEY: &str = "resource-upload-create-lost-ack-v1";
    const FIRST_CHUNK_KEY: &str = "resource-upload-chunk-zero-lost-ack-v1";
    const COMMIT_KEY: &str = "resource-upload-commit-lost-ack-v1";

    let directory = tempfile::tempdir().expect("private Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict Runtime directory");
    let mut first_child = RuntimeChild::spawn(directory.path());
    let first_readiness = wait_for_operator_readiness(first_child.child(), directory.path());
    assert_operator_readiness(&first_readiness);
    let principal_id = read_state(&directory.path().join(STATE_FILE))
        .expect("read first Runtime state")
        .expect("first Runtime state exists")
        .local_principal_id
        .expect("test Runtime has a local principal");

    let workspace_response = send_workspace_create(
        directory.path(),
        "resource-upload-create-workspace",
        "resource-upload-workspace-v1",
        br#"{"name":"Resource upload recovery fixture"}"#,
    );
    assert_eq!(workspace_response.header.status, 201);
    let workspace_body: Value =
        serde_json::from_slice(workspace_response.body()).expect("Workspace response is JSON");
    let workspace_id = workspace_body["workspace_id"]
        .as_str()
        .expect("Workspace response contains its ID")
        .to_owned();

    let mut content = vec![b'X'; CHUNK_SIZE + 1];
    let marker = b"LITECOWORK-PRIVATE-UPLOAD-FIXTURE";
    content[..marker.len()].copy_from_slice(marker);
    let content_digest = format!("sha256:{:x}", Sha256::digest(&content));
    let create_body = serde_json::to_vec(&serde_json::json!({
        "workspace_id": workspace_id,
        "display_name": "restart-upload.bin",
        "media_type": "application/octet-stream",
        "size_bytes": content.len(),
        "expected_digest": content_digest,
        "context_document": null,
        "folder_import": null,
    }))
    .expect("Resource upload create request serializes");
    let lost_create_ack = submit_operator_request_without_reading_response(
        directory.path(),
        "resource-upload-create-before-lost-ack",
        "POST",
        "/v1/resources/uploads",
        vec![
            ("content-type", "application/json"),
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", CREATE_KEY),
        ],
        &create_body,
    );
    let original_session =
        wait_for_durable_upload_create(directory.path(), CREATE_KEY, &principal_id).unwrap_or_else(
            |error| {
                let (status, stdout, stderr) = first_child.terminate_and_wait();
                panic!("{error}; Runtime status={status}; stdout={stdout}; stderr={stderr}");
            },
        );
    let upload_id = original_session["upload_id"]
        .as_str()
        .expect("durable create receipt contains upload ID")
        .to_owned();
    assert_eq!(original_session["expected_digest"], content_digest);
    assert_eq!(original_session["state"], "OPEN");
    assert_upload_create_has_one_durable_commit(directory.path(), &upload_id, CREATE_KEY);

    let (first_status, first_stdout, first_stderr) = first_child.terminate_and_wait();
    assert!(
        first_status.success(),
        "first Runtime shutdown failed: stdout={first_stdout} stderr={first_stderr}"
    );
    drop(lost_create_ack);

    let mut second_child = RuntimeChild::spawn(directory.path());
    let restarted_readiness = wait_for_operator_readiness(second_child.child(), directory.path());
    assert_operator_readiness(&restarted_readiness);
    let create_retry = send_operator_request(
        directory.path(),
        "resource-upload-create-after-restart-retry",
        "POST",
        "/v1/resources/uploads",
        vec![
            ("content-type", "application/json"),
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", CREATE_KEY),
        ],
        &create_body,
    );
    assert_eq!(create_retry.header.status, 201);
    let retried_session: Value =
        serde_json::from_slice(create_retry.body()).expect("replayed session is JSON");
    assert_eq!(retried_session, original_session);
    assert_upload_create_has_one_durable_commit(directory.path(), &upload_id, CREATE_KEY);

    let chunk_digest = format!("{:x}", Sha256::digest(&content[..CHUNK_SIZE]));
    let first_chunk_path = format!("/v1/resources/uploads/{upload_id}/chunks/0");
    let lost_chunk_ack = submit_operator_request_without_reading_response(
        directory.path(),
        "resource-upload-chunk-zero-before-lost-ack",
        "PUT",
        &first_chunk_path,
        vec![
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", FIRST_CHUNK_KEY),
            ("content-range", "bytes 0-4194303/4194305"),
            ("x-chunk-sha256", chunk_digest.as_str()),
            ("content-type", "application/octet-stream"),
        ],
        &content[..CHUNK_SIZE],
    );
    wait_for_durable_upload_chunk(directory.path(), &upload_id, FIRST_CHUNK_KEY, 1).unwrap_or_else(
        |error| {
            let (status, stdout, stderr) = second_child.terminate_and_wait();
            panic!("{error}; Runtime status={status}; stdout={stdout}; stderr={stderr}");
        },
    );
    let chunk_request_digest_before_restart =
        upload_chunk_request_digest(directory.path(), &upload_id, FIRST_CHUNK_KEY)
            .expect("accepted chunk and its request receipt are durable before restart");
    let (second_status, second_stdout, second_stderr) = second_child.terminate_and_wait();
    assert!(
        second_status.success(),
        "second Runtime shutdown failed: stdout={second_stdout} stderr={second_stderr}"
    );
    drop(lost_chunk_ack);

    let mut third_child = RuntimeChild::spawn(directory.path());
    let third_readiness = wait_for_operator_readiness(third_child.child(), directory.path());
    assert_operator_readiness(&third_readiness);
    let session_after_chunk_restart = send_operator_request(
        directory.path(),
        "resource-upload-progress-after-chunk-restart",
        "GET",
        &format!("/v1/resources/uploads/{upload_id}"),
        vec![("x-workspace-id", workspace_id.as_str())],
        &[],
    );
    assert_eq!(session_after_chunk_restart.header.status, 200);
    let progress: Value =
        serde_json::from_slice(session_after_chunk_restart.body()).expect("progress is JSON");
    assert_eq!(progress["state"], "OPEN");
    assert_eq!(progress["next_missing_offset"], CHUNK_SIZE);
    assert_eq!(
        progress["received_ranges"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(progress["received_ranges"][0]["start_offset"], 0);
    assert_eq!(
        progress["received_ranges"][0]["end_offset_inclusive"],
        CHUNK_SIZE - 1
    );

    let exact_chunk_replay = send_operator_request(
        directory.path(),
        "resource-upload-chunk-zero-after-restart-retry",
        "PUT",
        &first_chunk_path,
        vec![
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", FIRST_CHUNK_KEY),
            ("content-range", "bytes 0-4194303/4194305"),
            ("x-chunk-sha256", chunk_digest.as_str()),
            ("content-type", "application/octet-stream"),
        ],
        &content[..CHUNK_SIZE],
    );
    assert_eq!(exact_chunk_replay.header.status, 204);
    assert_upload_chunk_counts(directory.path(), &upload_id, 1, 1);
    assert_eq!(
        upload_chunk_request_digest(directory.path(), &upload_id, FIRST_CHUNK_KEY).as_deref(),
        Some(chunk_request_digest_before_restart.as_str()),
        "exact chunk retry preserves the original request receipt"
    );

    let tail_digest = format!("{:x}", Sha256::digest(&content[CHUNK_SIZE..]));
    let tail_response = send_operator_request(
        directory.path(),
        "resource-upload-chunk-one-final",
        "PUT",
        &format!("/v1/resources/uploads/{upload_id}/chunks/1"),
        vec![
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", "resource-upload-chunk-one-final-v1"),
            ("content-range", "bytes 4194304-4194304/4194305"),
            ("x-chunk-sha256", tail_digest.as_str()),
            ("content-type", "application/octet-stream"),
        ],
        &content[CHUNK_SIZE..],
    );
    assert_eq!(tail_response.header.status, 204);
    assert_upload_chunk_counts(directory.path(), &upload_id, 2, 2);

    let commit_path = format!("/v1/resources/uploads/{upload_id}/commit");
    let lost_commit_ack = submit_operator_request_without_reading_response(
        directory.path(),
        "resource-upload-commit-before-lost-ack",
        "POST",
        &commit_path,
        vec![
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", COMMIT_KEY),
        ],
        &[],
    );
    let committed_before_restart =
        wait_for_durable_upload_commit(directory.path(), &upload_id, &principal_id).unwrap_or_else(
            |error| {
                let (status, stdout, stderr) = third_child.terminate_and_wait();
                panic!("{error}; Runtime status={status}; stdout={stdout}; stderr={stderr}");
            },
        );
    let resource_id = committed_before_restart["resource"]["resource_id"]
        .as_str()
        .expect("commit receipt contains Resource ID")
        .to_owned();
    assert_eq!(
        committed_before_restart["resource"]["display_name"],
        "restart-upload.bin"
    );
    assert_eq!(
        committed_before_restart["revision"]["content_digest"],
        content_digest
    );

    let (third_status, third_stdout, third_stderr) = third_child.terminate_and_wait();
    assert!(
        third_status.success(),
        "third Runtime shutdown failed: stdout={third_stdout} stderr={third_stderr}"
    );
    drop(lost_commit_ack);

    let mut fourth_child = RuntimeChild::spawn(directory.path());
    let fourth_readiness = wait_for_operator_readiness(fourth_child.child(), directory.path());
    assert_operator_readiness(&fourth_readiness);
    let commit_retry = send_operator_request(
        directory.path(),
        "resource-upload-commit-after-restart-retry",
        "POST",
        &commit_path,
        vec![
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", COMMIT_KEY),
        ],
        &[],
    );
    assert_eq!(commit_retry.header.status, 201);
    let replayed_commit: Value =
        serde_json::from_slice(commit_retry.body()).expect("replayed commit is JSON");
    assert_eq!(
        replayed_commit["resource"],
        committed_before_restart["resource"]
    );
    assert_eq!(
        replayed_commit["revision"],
        committed_before_restart["revision"]
    );
    assert_eq!(replayed_commit["event"], committed_before_restart["event"]);
    let replay_correlation = commit_retry
        .header
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("x-correlation-id"))
        .map(|header| header.value.as_str());
    assert_eq!(
        replay_correlation,
        committed_before_restart["event"]["correlation_id"].as_str()
    );
    let revision_id = replayed_commit["revision"]["resource_revision_id"]
        .as_str()
        .expect("commit response contains Resource revision ID");
    let content_after_restart = send_operator_request(
        directory.path(),
        "resource-content-after-upload-commit-restart",
        "GET",
        &format!(
            "/v1/resources/{resource_id}/content?revision_id={revision_id}&max_bytes={}",
            content.len()
        ),
        vec![("x-workspace-id", workspace_id.as_str())],
        &[],
    );
    assert_eq!(content_after_restart.header.status, 200);
    assert_eq!(
        content_after_restart.body(),
        content.as_slice(),
        "committed Resource bytes remain readable and exact after daemon restart"
    );
    assert_resource_upload_commit_is_single_and_private(
        directory.path(),
        &upload_id,
        &resource_id,
        marker,
    );

    let changed_create_body = serde_json::to_vec(&serde_json::json!({
        "workspace_id": workspace_id,
        "display_name": "changed-upload.bin",
        "media_type": "application/octet-stream",
        "size_bytes": content.len(),
        "expected_digest": content_digest,
        "context_document": null,
        "folder_import": null,
    }))
    .expect("changed Resource upload request serializes");
    let changed_create = send_operator_request(
        directory.path(),
        "resource-upload-create-changed-payload",
        "POST",
        "/v1/resources/uploads",
        vec![
            ("content-type", "application/json"),
            ("x-workspace-id", workspace_id.as_str()),
            ("idempotency-key", CREATE_KEY),
        ],
        &changed_create_body,
    );
    assert_eq!(changed_create.header.status, 409);
    let conflict: Value = serde_json::from_slice(changed_create.body()).expect("conflict is JSON");
    assert_eq!(conflict["code"], "CONFLICT");
    assert_upload_create_has_one_durable_commit(directory.path(), &upload_id, CREATE_KEY);

    let (fourth_status, fourth_stdout, fourth_stderr) = fourth_child.terminate_and_wait();
    assert!(
        fourth_status.success(),
        "fourth Runtime shutdown failed: stdout={fourth_stdout} stderr={fourth_stderr}"
    );
}

fn submit_operator_request_without_reading_response(
    data_directory: &Path,
    frame_request_id: &str,
    method: &str,
    path: &str,
    headers: Vec<(&str, &str)>,
    body: &[u8],
) -> LostAckOperatorRequest {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build upload IPC client runtime");
    let connection = runtime.block_on(async {
        let mut connection = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("authenticate the real daemon Unix peer");
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("upload client in-flight body budget");
        let request =
            operator_request_frame(frame_request_id, method, path, headers, body, &budget);
        connection
            .write_request(&request)
            .await
            .expect("send real upload Operator request");
        connection
    });
    LostAckOperatorRequest {
        _connection: connection,
        _runtime: runtime,
    }
}

fn send_operator_request(
    data_directory: &Path,
    frame_request_id: &str,
    method: &str,
    path: &str,
    headers: Vec<(&str, &str)>,
    body: &[u8],
) -> ResponseFrame {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build upload IPC client runtime");
    runtime.block_on(async {
        let mut connection = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("authenticate the real daemon Unix peer");
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("upload client in-flight body budget");
        let request =
            operator_request_frame(frame_request_id, method, path, headers, body, &budget);
        connection
            .write_request(&request)
            .await
            .expect("write upload Operator request");
        let response = connection
            .read_response(&budget)
            .await
            .expect("read correlated upload Operator response");
        assert_eq!(response.header.request_id, frame_request_id);
        response
    })
}

fn operator_request_frame(
    frame_request_id: &str,
    method: &str,
    path: &str,
    headers: Vec<(&str, &str)>,
    body: &[u8],
    budget: &std::sync::Arc<InFlightBodyBudget>,
) -> RequestFrame {
    RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: frame_request_id.to_owned(),
            method: method.to_owned(),
            path_and_query: path.to_owned(),
            headers: headers
                .into_iter()
                .map(|(name, value)| LogicalHeader {
                    name: name.to_owned(),
                    value: value.to_owned(),
                })
                .collect(),
            body_length: body.len() as u64,
        },
        body.to_vec(),
        budget,
    )
    .expect("bounded Resource upload Operator frame")
}

fn wait_for_durable_upload_create(
    data_directory: &Path,
    idempotency_key: &str,
    principal_id: &str,
) -> Result<Value, String> {
    let deadline = Instant::now() + RECEIPT_DEADLINE;
    loop {
        match read_durable_upload_create(data_directory, idempotency_key, principal_id) {
            Ok(Some(receipt)) => return Ok(receipt),
            Ok(None) => {}
            Err(error) if Instant::now() < deadline => {
                let _ = error;
            }
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(
                "Resource upload create receipt was not durable before the deadline".to_owned(),
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_durable_upload_create(
    data_directory: &Path,
    idempotency_key: &str,
    principal_id: &str,
) -> Result<Option<Value>, String> {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .map_err(|error| error.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(1))
        .map_err(|error| error.to_string())?;
    let row: Option<(String, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT principal_id, response_json, response_digest FROM request_dedup WHERE request_id = ?1",
            [idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((receipt_principal, response_json, response_digest)) = row else {
        return Ok(None);
    };
    if receipt_principal != principal_id {
        return Err("Resource upload create receipt belongs to a different Principal".to_owned());
    }
    let response_json =
        response_json.ok_or_else(|| "Resource upload create receipt is incomplete".to_owned())?;
    let response_digest = response_digest
        .ok_or_else(|| "Resource upload create receipt digest is absent".to_owned())?;
    if response_digest != format!("sha256:{:x}", Sha256::digest(response_json.as_bytes())) {
        return Err("Resource upload create receipt digest is invalid".to_owned());
    }
    serde_json::from_str(&response_json)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn assert_upload_create_has_one_durable_commit(
    data_directory: &Path,
    upload_id: &str,
    create_key: &str,
) {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("open durable upload database");
    let session_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_sessions WHERE upload_id = ?1",
            [upload_id],
            |row| row.get(0),
        )
        .expect("count upload session");
    let event_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE type = 'resource.upload.created.v1' AND entity_id = ?1",
            [upload_id],
            |row| row.get(0),
        )
        .expect("count upload create events");
    let receipt_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = ?1",
            [create_key],
            |row| row.get(0),
        )
        .expect("count upload create receipts");
    assert_eq!(session_count, 1, "one upload session is durable");
    assert_eq!(event_count, 1, "one upload-created event is durable");
    assert_eq!(receipt_count, 1, "one create receipt is durable");
}

fn wait_for_durable_upload_chunk(
    data_directory: &Path,
    upload_id: &str,
    request_id: &str,
    expected_chunk_count: i64,
) -> Result<(), String> {
    let deadline = Instant::now() + RECEIPT_DEADLINE;
    loop {
        let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
            .map_err(|error| error.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(1))
            .map_err(|error| error.to_string())?;
        let chunk_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
                [upload_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let request_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM resource_upload_chunk_requests WHERE upload_id = ?1 AND request_id = ?2 AND chunk_index = 0",
                rusqlite::params![upload_id, request_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if chunk_count == expected_chunk_count && request_count == 1 {
            return Ok(());
        }
        if chunk_count > expected_chunk_count || request_count > 1 || Instant::now() >= deadline {
            return Err(format!(
                "Resource upload chunk/request counts were {chunk_count}/{request_count}, expected {expected_chunk_count}/1"
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn upload_chunk_request_digest(
    data_directory: &Path,
    upload_id: &str,
    request_id: &str,
) -> Option<String> {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("open durable upload database");
    connection
        .query_row(
            "SELECT request_payload_digest FROM resource_upload_chunk_requests WHERE upload_id = ?1 AND request_id = ?2 AND chunk_index = 0",
            rusqlite::params![upload_id, request_id],
            |row| row.get(0),
        )
        .optional()
        .expect("read upload chunk request receipt")
}

fn assert_upload_chunk_counts(
    data_directory: &Path,
    upload_id: &str,
    expected_chunks: i64,
    expected_requests: i64,
) {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("open durable upload database");
    let chunk_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunks WHERE upload_id = ?1",
            [upload_id],
            |row| row.get(0),
        )
        .expect("count accepted upload chunks");
    let request_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM resource_upload_chunk_requests WHERE upload_id = ?1",
            [upload_id],
            |row| row.get(0),
        )
        .expect("count upload chunk request receipts");
    assert_eq!(chunk_count, expected_chunks);
    assert_eq!(request_count, expected_requests);
}

fn wait_for_durable_upload_commit(
    data_directory: &Path,
    upload_id: &str,
    principal_id: &str,
) -> Result<Value, String> {
    let request_id = format!("resource-upload-commit:{upload_id}");
    let deadline = Instant::now() + RECEIPT_DEADLINE;
    loop {
        let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
            .map_err(|error| error.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(1))
            .map_err(|error| error.to_string())?;
        let row: Option<(String, Option<String>, Option<String>)> = connection
            .query_row(
                "SELECT principal_id, response_json, response_digest FROM request_dedup WHERE request_id = ?1",
                [&request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some((owner, response_json, response_digest)) = row {
            if owner != principal_id {
                return Err(
                    "Resource upload commit receipt belongs to a different Principal".to_owned(),
                );
            }
            let response_json = response_json
                .ok_or_else(|| "Resource upload commit receipt is incomplete".to_owned())?;
            let response_digest = response_digest
                .ok_or_else(|| "Resource upload commit receipt digest is absent".to_owned())?;
            if response_digest != format!("sha256:{:x}", Sha256::digest(response_json.as_bytes())) {
                return Err("Resource upload commit receipt digest is invalid".to_owned());
            }
            return serde_json::from_str(&response_json).map_err(|error| error.to_string());
        }
        if Instant::now() >= deadline {
            return Err(
                "Resource upload commit receipt was not durable before the deadline".to_owned(),
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn assert_resource_upload_commit_is_single_and_private(
    data_directory: &Path,
    upload_id: &str,
    resource_id: &str,
    private_marker: &[u8],
) {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("open committed upload database");
    let count = |sql: &str, parameter: &str| {
        connection
            .query_row(sql, [parameter], |row| row.get::<_, i64>(0))
            .expect("count committed upload records")
    };
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM resource_upload_sessions WHERE upload_id = ?1 AND state = 'COMMITTED'",
            upload_id
        ),
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM resources WHERE resource_id = ?1",
            resource_id
        ),
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM resource_revisions WHERE resource_id = ?1",
            resource_id
        ),
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM resource_locations WHERE resource_id = ?1",
            resource_id
        ),
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM domain_events WHERE type = 'resource.created.v1' AND entity_id = ?1",
            resource_id
        ),
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM domain_events WHERE type = 'resource.upload.created.v1' AND entity_id = ?1",
            upload_id
        ),
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM domain_events WHERE type = 'resource.upload.status.changed.v1' AND entity_id = ?1",
            upload_id
        ),
        2
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = ?1",
            &format!("resource-upload-commit:{upload_id}")
        ),
        1
    );
    let mut statement = connection
        .prepare("SELECT payload_json FROM domain_events WHERE entity_id IN (?1, ?2)")
        .expect("prepare upload event privacy inspection");
    let payloads = statement
        .query_map(rusqlite::params![upload_id, resource_id], |row| {
            row.get::<_, String>(0)
        })
        .expect("query upload event payloads")
        .collect::<Result<Vec<_>, _>>()
        .expect("read upload event payloads");
    let marker = String::from_utf8_lossy(private_marker);
    assert!(
        payloads
            .iter()
            .all(|payload| !payload.contains(marker.as_ref())),
        "ordinary Resource and upload events must not contain uploaded bytes"
    );
}

#[test]
fn spawned_daemon_replays_workspace_instruction_revision_after_lost_ack_and_restart() {
    const WORKSPACE_CREATE_KEY: &str = "spawned-instruction-workspace-create-v1";
    const RESOURCE_ID: &str = "spawned-instruction-resource";
    const RESOURCE_REVISION_ID: &str = "spawned-instruction-resource-revision-1";
    const INSTRUCTION_KEY: &str = "spawned-instruction-create-revision-1-v1";
    const CONTENT: &[u8] = b"Keep local instructions private and immutable.\n";

    let directory = tempfile::tempdir().expect("private Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict Runtime directory");
    let mut first_child = RuntimeChild::spawn(directory.path());
    let readiness = wait_for_operator_readiness(first_child.child(), directory.path());
    assert_operator_readiness(&readiness);
    let local_principal_id = read_state(&directory.path().join(STATE_FILE))
        .expect("read Runtime state")
        .expect("Runtime state exists")
        .local_principal_id
        .expect("test Runtime has a local principal");

    let workspace_response = send_workspace_create(
        directory.path(),
        "spawned-instruction-workspace-create-frame",
        WORKSPACE_CREATE_KEY,
        br#"{"name":"Instruction restart fixture"}"#,
    );
    assert_eq!(workspace_response.header.status, 201);
    let workspace: Value =
        serde_json::from_slice(workspace_response.body()).expect("Workspace response is JSON");
    let workspace_id = workspace["workspace_id"]
        .as_str()
        .expect("created Workspace ID is returned")
        .to_owned();
    let content_digest = seed_instruction_resource(
        directory.path(),
        &workspace_id,
        RESOURCE_ID,
        RESOURCE_REVISION_ID,
        CONTENT,
    );
    let body = workspace_instruction_revision_body(
        &workspace_id,
        RESOURCE_ID,
        RESOURCE_REVISION_ID,
        &content_digest,
    );

    let lost_ack = match submit_instruction_revision_without_reading_response(
        directory.path(),
        &workspace_id,
        INSTRUCTION_KEY,
        "spawned-instruction-frame-before-restart",
        "1",
        &body,
        &local_principal_id,
    ) {
        Ok(lost_ack) => lost_ack,
        Err(error) => {
            let (status, stdout, stderr) = first_child.terminate_and_wait();
            panic!("{error}; Runtime status={status}; stdout={stdout}; stderr={stderr}");
        }
    };
    assert_single_instruction_revision(
        &lost_ack.receipt,
        &workspace_id,
        &local_principal_id,
        INSTRUCTION_KEY,
    );
    assert_eq!(
        lost_ack.receipt.response["instruction_revision"]["revision"],
        1
    );

    let (first_status, first_stdout, first_stderr) = first_child.terminate_and_wait();
    assert!(
        first_status.success(),
        "first Runtime shutdown failed: stdout={first_stdout} stderr={first_stderr}"
    );
    drop(lost_ack._connection);
    drop(lost_ack._runtime);

    let mut restarted_child = RuntimeChild::spawn(directory.path());
    let restarted_readiness =
        wait_for_operator_readiness(restarted_child.child(), directory.path());
    assert_operator_readiness(&restarted_readiness);
    assert_eq!(restarted_readiness["runtime_id"], readiness["runtime_id"]);
    assert_ne!(
        restarted_readiness["local_incarnation_id"],
        readiness["local_incarnation_id"]
    );

    let replay = send_workspace_instruction_revision(
        directory.path(),
        &workspace_id,
        INSTRUCTION_KEY,
        "spawned-instruction-frame-after-restart",
        "1",
        &body,
    );
    assert_eq!(replay.header.status, 201);
    assert_eq!(
        replay.header.request_id,
        "spawned-instruction-frame-after-restart"
    );
    let replay_body: Value =
        serde_json::from_slice(replay.body()).expect("replayed instruction response is JSON");
    assert_eq!(
        replay_body, lost_ack.receipt.response["instruction_revision"],
        "restart retry returns the exact committed instruction revision"
    );
    assert_eq!(
        replay
            .header
            .headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case("x-correlation-id"))
            .map(|header| header.value.as_str()),
        lost_ack.receipt.response["event"]["correlation_id"].as_str()
    );

    let changed_body = workspace_instruction_revision_body_with_parents(
        &workspace_id,
        RESOURCE_ID,
        RESOURCE_REVISION_ID,
        &content_digest,
        &[1],
    );
    let conflict = send_workspace_instruction_revision(
        directory.path(),
        &workspace_id,
        INSTRUCTION_KEY,
        "spawned-instruction-frame-changed-payload",
        "1",
        &changed_body,
    );
    assert_eq!(conflict.header.status, 409);
    let conflict_body: Value =
        serde_json::from_slice(conflict.body()).expect("conflict response is JSON");
    assert_eq!(conflict_body["code"], "CONFLICT");

    let stale = send_workspace_instruction_revision(
        directory.path(),
        &workspace_id,
        "spawned-instruction-stale-version-v1",
        "spawned-instruction-frame-stale-version",
        "1",
        &changed_body,
    );
    assert_eq!(stale.header.status, 409);
    let stale_body: Value =
        serde_json::from_slice(stale.body()).expect("stale-version response is JSON");
    assert_eq!(stale_body["code"], "STALE_WORKSPACE_VERSION");
    assert!(
        read_durable_instruction_revision(
            directory.path(),
            &workspace_id,
            "spawned-instruction-stale-version-v1",
            &local_principal_id,
        )
        .expect("stale request receipt inspection succeeds")
        .is_none()
    );
    let current_body = workspace_instruction_revision_body(
        &workspace_id,
        RESOURCE_ID,
        RESOURCE_REVISION_ID,
        &content_digest,
    );
    let stale_version = send_workspace_instruction_revision(
        directory.path(),
        &workspace_id,
        "spawned-instruction-stale-if-match-v1",
        "spawned-instruction-frame-stale-if-match",
        "1",
        &current_body,
    );
    assert_eq!(stale_version.header.status, 409);
    let stale_version_body: Value =
        serde_json::from_slice(stale_version.body()).expect("stale If-Match response is JSON");
    assert_eq!(stale_version_body["code"], "STALE_WORKSPACE_VERSION");
    assert!(
        read_durable_instruction_revision(
            directory.path(),
            &workspace_id,
            "spawned-instruction-stale-if-match-v1",
            &local_principal_id,
        )
        .expect("stale If-Match receipt inspection succeeds")
        .is_none()
    );

    let after_retry = read_durable_instruction_revision(
        directory.path(),
        &workspace_id,
        INSTRUCTION_KEY,
        &local_principal_id,
    )
    .expect("inspect instruction receipt after restart retry")
    .expect("instruction receipt remains committed after restart retry");
    assert_eq!(after_retry, lost_ack.receipt);
    assert_single_instruction_revision(
        &after_retry,
        &workspace_id,
        &local_principal_id,
        INSTRUCTION_KEY,
    );

    let (restart_status, restart_stdout, restart_stderr) = restarted_child.terminate_and_wait();
    assert!(
        restart_status.success(),
        "restarted Runtime shutdown failed: stdout={restart_stdout} stderr={restart_stderr}"
    );
}

#[test]
fn spawned_daemon_reconciles_context_document_purge_after_restart_and_exact_replay() {
    const WORKSPACE_CREATE_KEY: &str = "spawned-purge-workspace-create-v1";
    const RESOURCE_ID: &str = "spawned-purge-context-document-resource";
    const RESOURCE_REVISION_ID: &str = "spawned-purge-context-document-revision-1";
    const DELETE_KEY: &str = "spawned-context-document-delete-lost-ack-v1";
    const CONTENT: &[u8] = b"Managed local ContextDocument content for daemon restart purge.\n";

    let directory = tempfile::tempdir().expect("private Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict Runtime directory");
    let mut first_child = RuntimeChild::spawn(directory.path());
    let first_readiness = wait_for_operator_readiness(first_child.child(), directory.path());
    assert_operator_readiness(&first_readiness);
    let runtime_id = first_readiness["runtime_id"]
        .as_str()
        .expect("first readiness exposes RuntimeId")
        .to_owned();
    let local_principal_id = read_state(&directory.path().join(STATE_FILE))
        .expect("read first Runtime state")
        .expect("first Runtime state exists")
        .local_principal_id
        .expect("test Runtime has a local principal");

    let workspace_response = send_workspace_create(
        directory.path(),
        "spawned-purge-workspace-create-frame",
        WORKSPACE_CREATE_KEY,
        br#"{"name":"ContextDocument purge restart fixture"}"#,
    );
    assert_eq!(workspace_response.header.status, 201);
    let workspace: Value =
        serde_json::from_slice(workspace_response.body()).expect("Workspace response is JSON");
    let workspace_id = workspace["workspace_id"]
        .as_str()
        .expect("created Workspace ID is returned")
        .to_owned();

    let content_digest = seed_instruction_resource(
        directory.path(),
        &workspace_id,
        RESOURCE_ID,
        RESOURCE_REVISION_ID,
        CONTENT,
    );
    Connection::open(directory.path().join("litecowork.sqlite3"))
        .expect("open Runtime database for ContextDocument fixture")
        .execute(
            "UPDATE resources SET context_document_json=?1, updated_at='2026-10-14T14:00:01Z', version=2 WHERE resource_id=?2 AND workspace_id=?3 AND version=1",
            rusqlite::params![
                serde_json::json!({
                    "kind": "WORKSPACE_NOTES",
                    "owner_ref": {"kind": "WORKSPACE", "workspace_id": workspace_id},
                    "status": "ACTIVE"
                })
                .to_string(),
                RESOURCE_ID,
                workspace_id,
            ],
        )
        .expect("attach active ContextDocument metadata to managed local Resource");

    let blobs = FileBlobStore::new(directory.path().join("blobs"), ReplayTestBlobKeys);
    let content_ref = storage_core::BlobRef {
        digest: content_digest.clone(),
        size_bytes: CONTENT.len() as u64,
        media_type: "text/plain".to_owned(),
    };
    let content_before_deletion = blobs
        .get(&workspace_id, BlobPurpose::Resource, &content_ref)
        .expect("managed local content exists before deletion");
    assert_eq!(content_before_deletion.as_slice(), CONTENT);

    let deletion_path = format!("/v1/resources/{RESOURCE_ID}/context-document/deletion");
    let lost_ack = submit_operator_request_without_reading_response(
        directory.path(),
        "spawned-context-document-delete-before-restart",
        "POST",
        &deletion_path,
        vec![
            ("content-type", "application/json"),
            ("x-workspace-id", workspace_id.as_str()),
            ("if-match", "2"),
            ("idempotency-key", DELETE_KEY),
        ],
        b"{}",
    );
    let committed_before_restart = wait_for_durable_context_document_purge(
        directory.path(),
        DELETE_KEY,
        &local_principal_id,
        RESOURCE_ID,
    )
    .unwrap_or_else(|error| {
        let (status, stdout, stderr) = first_child.terminate_and_wait();
        panic!("{error}; Runtime status={status}; stdout={stdout}; stderr={stderr}");
    });
    assert!(
        committed_before_restart
            .request_digest
            .starts_with("sha256:")
    );
    assert_eq!(
        committed_before_restart.response_digest,
        format!(
            "sha256:{:x}",
            Sha256::digest(committed_before_restart.response_json.as_bytes())
        ),
        "the durable lost-ack response is integrity checked"
    );
    let initial_response: Value = serde_json::from_str(&committed_before_restart.response_json)
        .expect("durable purge response is valid JSON");
    let initial_status = &initial_response["status"];
    assert_eq!(initial_status["resource_id"], RESOURCE_ID);
    assert_eq!(initial_status["status"], "DELETION_PENDING");
    assert_eq!(initial_status["required_replica_count"], 1);
    assert_eq!(initial_status["acknowledged_replica_count"], 0);
    assert_eq!(
        initial_status["manifest_digest"],
        committed_before_restart.manifest_digest
    );
    assert_eq!(committed_before_restart.target_count, 1);
    let targets: Value =
        serde_json::from_str(&committed_before_restart.targets_json).expect("sealed targets JSON");
    assert_eq!(targets.as_array().map(Vec::len), Some(1));
    assert_eq!(targets[0]["replica_ref"], content_digest);
    assert_eq!(targets[0]["replica_kind"], "BLOB");
    assert_eq!(
        targets[0]["target_revision_ids"],
        serde_json::json!([RESOURCE_REVISION_ID])
    );
    assert_eq!(committed_before_restart.receipts.len(), 1);
    let pending_receipt = &committed_before_restart.receipts[0];
    assert_eq!(pending_receipt.replica_ref, content_digest);
    assert_eq!(pending_receipt.replica_kind, "BLOB");
    assert_eq!(pending_receipt.status, "PENDING");
    assert_eq!(pending_receipt.runtime_id, None);
    assert_eq!(pending_receipt.runtime_incarnation_id, None);
    assert_eq!(
        serde_json::from_str::<Value>(&pending_receipt.target_revision_ids_json)
            .expect("pending receipt target revisions are valid JSON"),
        serde_json::json!([RESOURCE_REVISION_ID])
    );
    assert_eq!(pending_receipt.target_upload_chunk_refs_json, "[]");
    assert_eq!(pending_receipt.receipt_digest, None);
    assert_eq!(pending_receipt.acknowledged_at, None);
    assert!(
        registry_row_exists(
            directory.path(),
            "resource_content_blob_registry",
            &workspace_id,
            &content_digest
        ),
        "the local object remains registered until the restarted reconciler removes it"
    );

    let (first_status, first_stdout, first_stderr) = first_child.terminate_and_wait();
    assert!(
        first_status.success(),
        "first Runtime shutdown failed: stdout={first_stdout} stderr={first_stderr}"
    );
    drop(lost_ack);

    let mut restarted_child = RuntimeChild::spawn(directory.path());
    let restarted_readiness =
        wait_for_operator_readiness(restarted_child.child(), directory.path());
    assert_operator_readiness(&restarted_readiness);
    assert_eq!(restarted_readiness["runtime_id"], runtime_id);
    assert_ne!(
        restarted_readiness["local_incarnation_id"], first_readiness["local_incarnation_id"],
        "the purge resumes under a new daemon incarnation"
    );

    let replay = send_operator_request(
        directory.path(),
        "spawned-context-document-delete-after-restart-replay",
        "POST",
        &deletion_path,
        vec![
            ("content-type", "application/json"),
            ("x-workspace-id", workspace_id.as_str()),
            ("if-match", "2"),
            ("idempotency-key", DELETE_KEY),
        ],
        b"{}",
    );
    assert_eq!(
        replay.header.status, 202,
        "exact retry replays the admission"
    );
    assert_eq!(
        replay.header.request_id,
        "spawned-context-document-delete-after-restart-replay"
    );
    let replay_status: Value = serde_json::from_slice(replay.body()).expect("replay is JSON");
    assert_eq!(
        &replay_status, initial_status,
        "restart retry returns the original admission response and manifest identity"
    );

    let status_path = deletion_path;
    let status_deadline = Instant::now() + Duration::from_secs(45);
    let final_status = loop {
        let response = send_operator_request(
            directory.path(),
            "spawned-context-document-delete-status-after-restart",
            "GET",
            &status_path,
            vec![("x-workspace-id", workspace_id.as_str())],
            &[],
        );
        assert_eq!(
            response.header.status, 200,
            "authenticated status is available"
        );
        let status: Value = serde_json::from_slice(response.body()).expect("status is JSON");
        if status["status"] == "DELETED" {
            break status;
        }
        assert_eq!(status["status"], "DELETION_PENDING");
        assert!(
            Instant::now() < status_deadline,
            "daemon reconciler did not finish the durable purge after restart: {status}"
        );
        thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(final_status["resource_id"], RESOURCE_ID);
    assert_eq!(
        final_status["manifest_digest"],
        committed_before_restart.manifest_digest
    );
    assert_eq!(final_status["required_replica_count"], 1);
    assert_eq!(final_status["acknowledged_replica_count"], 1);

    let after_restart = read_durable_context_document_purge(
        directory.path(),
        DELETE_KEY,
        &local_principal_id,
        RESOURCE_ID,
    )
    .expect("inspect final durable purge receipt")
    .expect("original owner request receipt remains durable");
    assert_eq!(
        after_restart.principal_id,
        committed_before_restart.principal_id
    );
    assert_eq!(
        after_restart.request_digest,
        committed_before_restart.request_digest
    );
    assert_eq!(
        after_restart.response_json,
        committed_before_restart.response_json
    );
    assert_eq!(
        after_restart.response_digest,
        committed_before_restart.response_digest
    );
    assert_eq!(
        after_restart.manifest_digest,
        committed_before_restart.manifest_digest
    );
    assert_eq!(
        after_restart.target_count,
        committed_before_restart.target_count
    );
    assert_eq!(
        after_restart.targets_json,
        committed_before_restart.targets_json
    );
    assert_eq!(after_restart.receipts.len(), 1);
    let acknowledged_receipt = &after_restart.receipts[0];
    assert_eq!(
        acknowledged_receipt.replica_ref, content_digest,
        "the acknowledged receipt covers the exact original blob digest"
    );
    assert_eq!(acknowledged_receipt.replica_kind, "BLOB");
    assert_eq!(acknowledged_receipt.status, "ACKNOWLEDGED");
    assert_eq!(acknowledged_receipt.runtime_id, None);
    assert_eq!(acknowledged_receipt.runtime_incarnation_id, None);
    assert_eq!(
        serde_json::from_str::<Value>(&acknowledged_receipt.target_revision_ids_json)
            .expect("acknowledged receipt target revisions are valid JSON"),
        serde_json::json!([RESOURCE_REVISION_ID])
    );
    assert_eq!(acknowledged_receipt.target_upload_chunk_refs_json, "[]");
    assert!(
        acknowledged_receipt
            .receipt_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert!(
        !registry_row_exists(
            directory.path(),
            "resource_content_blob_registry",
            &workspace_id,
            &acknowledged_receipt.replica_ref
        ),
        "physical removal and registry cleanup precede durable acknowledgement"
    );
    assert!(
        matches!(
            blobs.get(&workspace_id, BlobPurpose::Resource, &content_ref),
            Err(StoreError::NotFound)
        ),
        "the exact encrypted local object is physically gone"
    );
    let connection = Connection::open(directory.path().join("litecowork.sqlite3"))
        .expect("open Runtime database for final purge event inspection");
    let acknowledged_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND entity_id=?2 AND type='resource.context_document.purge.acknowledged.v2'",
            rusqlite::params![workspace_id, RESOURCE_ID],
            |row| row.get(0),
        )
        .expect("read exact target acknowledgement event count");
    assert_eq!(acknowledged_events, 1);
    let acknowledged_event_payload: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE workspace_id=?1 AND entity_id=?2 AND type='resource.context_document.purge.acknowledged.v2'",
            rusqlite::params![workspace_id, RESOURCE_ID],
            |row| row.get(0),
        )
        .expect("read exact target acknowledgement payload");
    let acknowledged_event: Value =
        serde_json::from_str(&acknowledged_event_payload).expect("acknowledgement event is JSON");
    assert_eq!(acknowledged_event["resource_id"], RESOURCE_ID);
    assert_eq!(
        acknowledged_event["purge_manifest_digest"],
        after_restart.manifest_digest
    );
    assert_eq!(
        acknowledged_event["replica_ref"],
        acknowledged_receipt.replica_ref
    );
    assert_eq!(
        acknowledged_event["replica_kind"],
        acknowledged_receipt.replica_kind
    );
    assert_eq!(acknowledged_event["runtime_id"], Value::Null);
    assert_eq!(acknowledged_event["runtime_incarnation_id"], Value::Null);
    assert!(
        acknowledged_event["target_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert!(acknowledged_event.get("target_revision_ids").is_none());
    assert!(acknowledged_event.get("target_upload_chunk_refs").is_none());
    assert_eq!(
        acknowledged_event["receipt_digest"],
        acknowledged_receipt.receipt_digest.as_deref().unwrap()
    );
    assert_eq!(
        acknowledged_event["acknowledged_at"],
        acknowledged_receipt.acknowledged_at.as_deref().unwrap()
    );
    let status_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id=?1 AND entity_id=?2 AND type='resource.context_document.status.changed.v1'",
            rusqlite::params![workspace_id, RESOURCE_ID],
            |row| row.get(0),
        )
        .expect("read ContextDocument status transition event count");
    assert_eq!(
        status_events, 2,
        "admission and final deletion are both audited"
    );

    let (restart_status, restart_stdout, restart_stderr) = restarted_child.terminate_and_wait();
    assert!(
        restart_status.success(),
        "restarted Runtime shutdown failed: stdout={restart_stdout} stderr={restart_stderr}"
    );
}

#[test]
fn spawned_daemon_task_stream_resumes_across_restart_and_delivers_live_update() {
    use domain_task::{CreateStandaloneTask, ReviseTaskSpec, TaskService};

    const AGENT_BINDING_ID: &str = "spawned-task-stream-agent-binding";
    const TASK_ID: &str = "spawned-task-stream-task";

    let directory = tempfile::tempdir().expect("private Runtime data directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict Runtime directory");

    let mut first_child = RuntimeChild::spawn(directory.path());
    let first_readiness = wait_for_operator_readiness(first_child.child(), directory.path());
    assert_operator_readiness(&first_readiness);
    let runtime_id = first_readiness["runtime_id"]
        .as_str()
        .expect("first readiness exposes RuntimeId")
        .to_owned();
    let first_incarnation = first_readiness["local_incarnation_id"]
        .as_str()
        .expect("first readiness exposes local incarnation")
        .to_owned();
    let principal_id = read_state(&directory.path().join(STATE_FILE))
        .expect("read first Runtime state")
        .expect("first Runtime state exists")
        .local_principal_id
        .expect("test Runtime has a local principal");

    let workspace_response = send_workspace_create(
        directory.path(),
        "task-stream-workspace-create",
        "spawned-task-stream-workspace-create-v1",
        br#"{"name":"Task stream restart fixture"}"#,
    );
    assert_eq!(workspace_response.header.status, 201);
    let workspace: Value =
        serde_json::from_slice(workspace_response.body()).expect("Workspace response is JSON");
    let workspace_id = workspace["workspace_id"]
        .as_str()
        .expect("created Workspace exposes its ID")
        .to_owned();

    let storage = open_test_storage(
        directory.path(),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("test service opens the daemon's real SQLite database");
    let fixture_connection = Connection::open(directory.path().join("litecowork.sqlite3"))
        .expect("Task agent fixture database connection opens");
    fixture_connection
        .busy_timeout(Duration::from_secs(2))
        .expect("bound fixture database wait");
    fixture_connection
        .execute(
            "INSERT INTO agent_profiles(agent_profile_id,provider_key,display_name,discovered_at) VALUES('spawned-task-stream-profile','fixture','Fixture Agent','2026-10-14T12:00:00.000000000Z')",
            [],
        )
        .expect("Task AgentProfile persists");
    fixture_connection
        .execute(
            "INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version) VALUES(?1,?2,'spawned-task-stream-profile','{}','{}',1,1,'2026-10-14T12:00:00.000000000Z',1)",
            rusqlite::params![AGENT_BINDING_ID, workspace_id],
        )
        .expect("Task lead AgentBinding persists");
    drop(fixture_connection);

    let task_service = TaskService::new(storage.store.clone());
    let task_payload = serde_json::json!({
        "workspace_id": workspace_id,
        "objective": "Task objective at the saved stream cursor",
        "preferred_lead_agent_binding_id": AGENT_BINDING_ID,
        "source_message_refs": [],
        "constraints": [],
        "non_goals": [],
        "input_refs": [],
        "required_outputs": [],
        "acceptance_criteria": [],
        "approvals_required": [],
        "placement_preference": "AUTO"
    });
    task_service
        .create_standalone(CreateStandaloneTask {
            task_id: TASK_ID.to_owned(),
            workspace_id: workspace_id.clone(),
            workspace_instruction_revision: None,
            lead_agent_binding_id: AGENT_BINDING_ID.to_owned(),
            origin_coworker_id: None,
            origin_coworker_revision: None,
            expected_coworker_version: None,
            coworker_default_lead_failover_policy: None,
            principal_id: principal_id.clone(),
            request_id: "spawned-task-stream-create".to_owned(),
            request_payload: task_payload,
            event: spawned_task_stream_event(
                &runtime_id,
                "spawned-task-stream-created",
                "2026-10-14T12:00:01.000000000Z",
            ),
        })
        .expect("Task commits through the real TaskService and SQLite store");

    let initial_stream = open_task_stream(
        directory.path(),
        &workspace_id,
        TASK_ID,
        None,
        "spawned-task-stream-initial-subscribe",
    );
    assert_eq!(initial_stream.ready["type"], "stream.ready");
    assert_eq!(initial_stream.ready["workspace_id"], workspace_id);
    assert_eq!(initial_stream.snapshot["type"], "projection");
    assert_eq!(
        initial_stream.snapshot["projection_type"],
        "task_presentation"
    );
    assert_eq!(
        initial_stream.ready["cursor"],
        initial_stream.snapshot["cursor"]
    );
    assert_eq!(initial_stream.snapshot["payload"]["task_id"], TASK_ID);
    assert_eq!(initial_stream.snapshot["payload"]["origin_sequence"], "2");
    assert_eq!(
        initial_stream.snapshot["payload"]["items"][0]["payload"]["objective"],
        "Task objective at the saved stream cursor"
    );
    let saved_cursor = initial_stream.snapshot["cursor"]
        .as_str()
        .expect("initial Task projection has an opaque cursor")
        .to_owned();
    drop(initial_stream);

    TaskService::new(storage.store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: workspace_id.clone(),
            task_id: TASK_ID.to_owned(),
            principal_id: principal_id.clone(),
            request_id: "spawned-task-stream-revise-before-restart".to_owned(),
            request_payload: serde_json::json!({
                "operation": "task.spec.revise.v1",
                "task_id": TASK_ID,
                "revision": 2
            }),
            expected_task_version: 1,
            parent_revisions: vec![1],
            objective: "Task objective committed while the stream is disconnected".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: spawned_task_stream_event(
                &runtime_id,
                "spawned-task-stream-revised-before-restart",
                "2026-10-14T12:00:02.000000000Z",
            ),
        })
        .expect("Task revision commits while the subscriber is disconnected");

    let (first_status, first_stdout, first_stderr) = first_child.terminate_and_wait();
    assert!(
        first_status.success(),
        "first Runtime shutdown failed: stdout={first_stdout} stderr={first_stderr}"
    );
    drop(storage);

    let mut restarted_child = RuntimeChild::spawn(directory.path());
    let restarted_readiness =
        wait_for_operator_readiness(restarted_child.child(), directory.path());
    assert_operator_readiness(&restarted_readiness);
    assert_eq!(
        restarted_readiness["runtime_id"].as_str(),
        Some(runtime_id.as_str()),
        "Runtime identity remains stable across process restart"
    );
    let restarted_incarnation = restarted_readiness["local_incarnation_id"]
        .as_str()
        .expect("restarted readiness exposes local incarnation");
    assert_ne!(restarted_incarnation, first_incarnation);

    let storage_after_restart = open_test_storage(
        directory.path(),
        SqliteConfig {
            writer_queue_capacity: 8,
            busy_timeout: Duration::from_secs(2),
        },
    )
    .expect("test service reopens the same SQLite database after Runtime restart");
    let mut resumed_stream = open_task_stream(
        directory.path(),
        &workspace_id,
        TASK_ID,
        Some(&saved_cursor),
        "spawned-task-stream-resume-after-restart",
    );
    assert_eq!(resumed_stream.ready["type"], "stream.ready");
    assert_eq!(resumed_stream.snapshot["type"], "projection");
    assert_eq!(
        resumed_stream.snapshot["projection_type"],
        "task_presentation"
    );
    assert_eq!(resumed_stream.snapshot["payload"]["task_id"], TASK_ID);
    assert_eq!(resumed_stream.snapshot["payload"]["origin_sequence"], "3");
    assert_eq!(
        resumed_stream.snapshot["payload"]["items"][0]["payload"]["objective"],
        "Task objective committed while the stream is disconnected",
        "resume catches up to the committed state newer than the saved cursor"
    );
    assert_ne!(resumed_stream.snapshot["cursor"], saved_cursor);
    assert_eq!(
        resumed_stream.ready["cursor"],
        resumed_stream.snapshot["cursor"]
    );

    TaskService::new(storage_after_restart.store.clone())
        .revise_saved_spec(ReviseTaskSpec {
            workspace_id: workspace_id.clone(),
            task_id: TASK_ID.to_owned(),
            principal_id,
            request_id: "spawned-task-stream-revise-after-restart".to_owned(),
            request_payload: serde_json::json!({
                "operation": "task.spec.revise.v1",
                "task_id": TASK_ID,
                "revision": 3
            }),
            expected_task_version: 2,
            parent_revisions: vec![2],
            objective: "Task objective delivered as a live post-restart update".to_owned(),
            constraints: None,
            non_goals: None,
            input_refs: None,
            required_outputs: None,
            acceptance_criteria: None,
            approvals_required: None,
            placement_preference: None,
            preferred_lead_agent_binding_id: None,
            lead_failover_policy: None,
            event: spawned_task_stream_event(
                &runtime_id,
                "spawned-task-stream-revised-after-restart",
                "2026-10-14T12:00:03.000000000Z",
            ),
        })
        .expect("post-restart Task revision commits through TaskService");
    let live_update = resumed_stream.read_next();
    assert_eq!(live_update["type"], "projection");
    assert_eq!(live_update["projection_type"], "task_presentation");
    assert_eq!(live_update["payload"]["task_id"], TASK_ID);
    assert_eq!(live_update["payload"]["origin_sequence"], "4");
    assert_eq!(
        live_update["payload"]["items"][0]["payload"]["objective"],
        "Task objective delivered as a live post-restart update"
    );
    assert_ne!(live_update["cursor"], resumed_stream.snapshot["cursor"]);

    drop(resumed_stream);
    drop(storage_after_restart);
    let (restart_status, restart_stdout, restart_stderr) = restarted_child.terminate_and_wait();
    assert!(
        restart_status.success(),
        "restarted Runtime shutdown failed: stdout={restart_stdout} stderr={restart_stderr}"
    );
}

fn spawned_task_stream_event(
    runtime_id: &str,
    event_id: &str,
    timestamp: &str,
) -> domain_workspace::EventContext {
    domain_workspace::EventContext {
        event_id: event_id.to_owned(),
        origin_runtime_id: runtime_id.to_owned(),
        hlc_timestamp: timestamp.to_owned(),
        correlation_id: format!("correlation-{event_id}"),
        causation_id: None,
        recorded_at: timestamp.to_owned(),
    }
}

struct TaskStreamSession {
    runtime: tokio::runtime::Runtime,
    connection: operator_ipc::unix::AuthenticatedUnixConnection,
    budget: Arc<InFlightBodyBudget>,
    ready: Value,
    snapshot: Value,
}

impl TaskStreamSession {
    fn read_next(&mut self) -> Value {
        let runtime = &self.runtime;
        let connection = &mut self.connection;
        let budget = &self.budget;
        let frame = runtime.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(3),
                connection.read_stream_message(budget),
            )
            .await
            .expect("Task stream frame arrives before its deadline")
            .expect("Task stream frame reads")
            .expect("Task stream remains open")
        });
        serde_json::from_slice(frame.body()).expect("Task stream frame is JSON")
    }
}

fn open_task_stream(
    data_directory: &Path,
    workspace_id: &str,
    task_id: &str,
    resume_cursor: Option<&str>,
    request_id: &str,
) -> TaskStreamSession {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build Task stream client runtime");
    let mut connection = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(2),
            endpoint.connect(|peer| peer.uid == expected_uid),
        )
        .await
        .expect("authenticated Task stream connection arrives before deadline")
        .expect("same-UID daemon authenticates Task stream client")
    });
    let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .expect("Task stream client body budget");
    let body = serde_json::to_vec(&serde_json::json!({
        "workspace_id": workspace_id,
        "task_ids": [task_id],
        "projection_types": ["task_presentation"],
        "resume_cursor": resume_cursor,
    }))
    .expect("Task stream subscription serializes");
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/stream".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: workspace_id.to_owned(),
                },
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body,
        &budget,
    )
    .expect("bounded Task stream subscription frame");
    let (ready, snapshot) = runtime.block_on(async {
        connection
            .write_request(&request)
            .await
            .expect("Task stream subscription writes");
        let handshake = tokio::time::timeout(
            Duration::from_secs(3),
            connection.read_stream_response(&budget),
        )
        .await
        .expect("Task stream handshake arrives before deadline")
        .expect("Task stream handshake reads");
        assert_eq!(handshake.header.status, 200);
        assert_eq!(handshake.body().len(), 0);
        let ready = read_task_stream_value(&mut connection, &budget).await;
        let snapshot = read_task_stream_value(&mut connection, &budget).await;
        (ready, snapshot)
    });
    TaskStreamSession {
        runtime,
        connection,
        budget,
        ready,
        snapshot,
    }
}

async fn read_task_stream_value(
    connection: &mut operator_ipc::unix::AuthenticatedUnixConnection,
    budget: &Arc<InFlightBodyBudget>,
) -> Value {
    let frame = tokio::time::timeout(
        Duration::from_secs(3),
        connection.read_stream_message(budget),
    )
    .await
    .expect("Task stream frame arrives before deadline")
    .expect("Task stream frame reads")
    .expect("Task stream remains open");
    serde_json::from_slice(frame.body()).expect("Task stream frame is JSON")
}

fn submit_create_without_reading_response(
    data_directory: &Path,
    idempotency_key: &str,
    frame_request_id: &str,
    body: &[u8],
    principal_id: &str,
) -> Result<LostAckWorkspaceCreate, String> {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build IPC client runtime");
    let (receipt, connection) = runtime.block_on(async {
        let mut connection = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("authenticate the real daemon Unix peer");
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("client in-flight body budget");
        let request = workspace_create_request(body, idempotency_key, frame_request_id, &budget);
        connection
            .write_request(&request)
            .await
            .expect("send real Workspace-create request");

        let receipt = match wait_for_durable_create(data_directory, idempotency_key, principal_id) {
            Ok(receipt) => receipt,
            Err(error) => {
                let response =
                    tokio::time::timeout(Duration::from_secs(3), connection.read_response(&budget))
                        .await
                        .map_err(|_| "diagnostic response deadline elapsed".to_owned())
                        .and_then(|response| response.map_err(|error| error.to_string()))
                        .map(|response| {
                            format!(
                                "status={}, body={}",
                                response.header.status,
                                String::from_utf8_lossy(response.body())
                            )
                        })
                        .unwrap_or_else(|error| format!("response read failed: {error}"));
                return Err(format!("{error}; first create response: {response}"));
            }
        };
        Ok::<_, String>((receipt, connection))
    })?;
    Ok(LostAckWorkspaceCreate {
        receipt,
        _connection: connection,
        _runtime: runtime,
    })
}

fn wait_for_durable_context_document_purge(
    data_directory: &Path,
    idempotency_key: &str,
    principal_id: &str,
    resource_id: &str,
) -> Result<DurableContextDocumentPurge, String> {
    let deadline = Instant::now() + RECEIPT_DEADLINE;
    loop {
        match read_durable_context_document_purge(
            data_directory,
            idempotency_key,
            principal_id,
            resource_id,
        ) {
            Ok(Some(receipt)) => return Ok(receipt),
            Ok(None) => {}
            Err(error) if Instant::now() < deadline => {
                let _ = error;
            }
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(
                "ContextDocument purge admission receipt was not durable before the deadline"
                    .to_owned(),
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_durable_context_document_purge(
    data_directory: &Path,
    idempotency_key: &str,
    principal_id: &str,
    resource_id: &str,
) -> Result<Option<DurableContextDocumentPurge>, String> {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .map_err(|error| error.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(1))
        .map_err(|error| error.to_string())?;
    let receipt: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT principal_id, request_digest, response_json, response_digest
             FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            rusqlite::params![principal_id, idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((principal_id, request_digest, response_json, response_digest)) = receipt else {
        return Ok(None);
    };
    let plan: Option<(String, i64, String)> = connection
        .query_row(
            "SELECT manifest_digest, target_count, targets_json
             FROM context_document_purge_plans WHERE workspace_id=(
                 SELECT workspace_id FROM resources WHERE resource_id=?1
             ) AND resource_id=?1",
            [resource_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((manifest_digest, target_count, targets_json)) = plan else {
        return Err("owner receipt exists without its immutable purge plan".to_owned());
    };
    let mut statement = connection
        .prepare(
            "SELECT replica_ref, replica_kind, status, runtime_id, runtime_incarnation_id,
                    target_revision_ids_json, target_upload_chunk_refs_json, receipt_digest,
                    acknowledged_at
             FROM context_document_purge_receipts
             WHERE resource_id=?1 ORDER BY replica_ref, replica_kind",
        )
        .map_err(|error| error.to_string())?;
    let receipts = statement
        .query_map([resource_id], |row| {
            Ok(DurableContextDocumentPurgeReceipt {
                replica_ref: row.get(0)?,
                replica_kind: row.get(1)?,
                status: row.get(2)?,
                runtime_id: row.get(3)?,
                runtime_incarnation_id: row.get(4)?,
                target_revision_ids_json: row.get(5)?,
                target_upload_chunk_refs_json: row.get(6)?,
                receipt_digest: row.get(7)?,
                acknowledged_at: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<DurableContextDocumentPurgeReceipt>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(Some(DurableContextDocumentPurge {
        principal_id,
        request_digest,
        response_json,
        response_digest,
        manifest_digest,
        target_count,
        targets_json,
        receipts,
    }))
}

fn registry_row_exists(
    data_directory: &Path,
    registry_table: &str,
    workspace_id: &str,
    digest: &str,
) -> bool {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("open Runtime database for registry inspection");
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM {registry_table} WHERE workspace_id=?1 AND digest=?2)"
    );
    connection
        .query_row(&sql, rusqlite::params![workspace_id, digest], |row| {
            row.get(0)
        })
        .expect("inspect exact registry identity")
}

fn send_workspace_create(
    data_directory: &Path,
    frame_request_id: &str,
    idempotency_key: &str,
    body: &[u8],
) -> ResponseFrame {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build IPC client runtime");
    runtime.block_on(async {
        let mut connection = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("authenticate the real daemon Unix peer");
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("client in-flight body budget");
        let request = workspace_create_request(body, idempotency_key, frame_request_id, &budget);
        connection
            .write_request(&request)
            .await
            .expect("write Workspace-create request");
        connection
            .read_response(&budget)
            .await
            .expect("read correlated Workspace-create response")
    })
}

fn seed_instruction_resource(
    data_directory: &Path,
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    content: &[u8],
) -> String {
    let blobs = FileBlobStore::new(data_directory.join("blobs"), ReplayTestBlobKeys);
    let blob = blobs
        .put(workspace_id, BlobPurpose::Resource, content, "text/plain")
        .expect("store encrypted instruction Resource bytes");
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("open Runtime database for instruction fixture");
    connection
        .busy_timeout(Duration::from_secs(2))
        .expect("configure instruction fixture database timeout");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable instruction fixture foreign keys");
    let transaction = connection
        .unchecked_transaction()
        .expect("start instruction Resource fixture transaction");
    transaction
        .execute(
            "INSERT INTO resources(resource_id,workspace_id,kind,provider_identity_json,display_name,current_revision_id,sensitivity,context_document_json,provenance_json,created_at,updated_at,version)
             VALUES (?1,?2,'FILE','{}','spawned-instruction-resource',?3,'PERSONAL',NULL,'{}','2026-10-14T14:00:00Z','2026-10-14T14:00:00Z',1)",
            rusqlite::params![resource_id, workspace_id, revision_id],
        )
        .expect("insert instruction fixture Resource");
    transaction
        .execute(
            "INSERT INTO resource_revisions(resource_revision_id,resource_id,content_digest,size_bytes,media_type,observed_at,created_by_json)
             VALUES (?1,?2,?3,?4,'text/plain','2026-10-14T14:00:00Z','{}')",
            rusqlite::params![
                revision_id,
                resource_id,
                blob.digest,
                i64::try_from(content.len()).expect("bounded instruction fixture size"),
            ],
        )
        .expect("insert instruction fixture revision");
    transaction
        .execute(
            "INSERT INTO resource_locations(location_id,resource_id,provider_ref,locator_ref_id,availability,writable,observed_revision_id,observed_digest,observed_at,last_checked_at)
             VALUES (?1,?2,'litecowork.encrypted_blob',?3,'AVAILABLE',0,?4,?3,'2026-10-14T14:00:00Z','2026-10-14T14:00:00Z')",
            rusqlite::params![
                format!("location-{resource_id}"),
                resource_id,
                blob.digest,
                revision_id,
            ],
        )
        .expect("insert instruction fixture Resource location");
    transaction
        .commit()
        .expect("instruction fixture Resource commits");
    blob.digest
}

fn workspace_instruction_revision_body(
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    content_digest: &str,
) -> Vec<u8> {
    workspace_instruction_revision_body_with_parents(
        workspace_id,
        resource_id,
        revision_id,
        content_digest,
        &[],
    )
}

fn workspace_instruction_revision_body_with_parents(
    workspace_id: &str,
    resource_id: &str,
    revision_id: &str,
    content_digest: &str,
    parent_revisions: &[u64],
) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "parent_revisions": parent_revisions,
        "content_ref": {
            "workspace_id": workspace_id,
            "resource_id": resource_id,
            "revision_id": revision_id,
        },
        "content_digest": content_digest,
    }))
    .expect("instruction revision request serializes")
}

fn workspace_instruction_revision_request(
    workspace_id: &str,
    frame_request_id: &str,
    idempotency_key: &str,
    if_match: &str,
    body: &[u8],
    budget: &std::sync::Arc<InFlightBodyBudget>,
) -> RequestFrame {
    RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: frame_request_id.to_owned(),
            method: "POST".to_owned(),
            path_and_query: format!("/v1/workspaces/{workspace_id}/instructions/revisions"),
            headers: vec![
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
                LogicalHeader {
                    name: "x-workspace-id".to_owned(),
                    value: workspace_id.to_owned(),
                },
                LogicalHeader {
                    name: "if-match".to_owned(),
                    value: if_match.to_owned(),
                },
                LogicalHeader {
                    name: "idempotency-key".to_owned(),
                    value: idempotency_key.to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body.to_vec(),
        budget,
    )
    .expect("bounded instruction revision frame")
}

fn submit_instruction_revision_without_reading_response(
    data_directory: &Path,
    workspace_id: &str,
    idempotency_key: &str,
    frame_request_id: &str,
    if_match: &str,
    body: &[u8],
    principal_id: &str,
) -> Result<LostAckInstructionRevision, String> {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build instruction IPC client runtime");
    let (receipt, connection) = runtime.block_on(async {
        let mut connection = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("authenticate the real daemon Unix peer");
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("instruction client body budget");
        let request = workspace_instruction_revision_request(
            workspace_id,
            frame_request_id,
            idempotency_key,
            if_match,
            body,
            &budget,
        );
        connection
            .write_request(&request)
            .await
            .expect("send instruction revision request");
        let deadline = Instant::now() + RECEIPT_DEADLINE;
        loop {
            match read_durable_instruction_revision(
                data_directory,
                workspace_id,
                idempotency_key,
                principal_id,
            ) {
                Ok(Some(receipt)) => break Ok((receipt, connection)),
                Ok(None) => {}
                Err(error) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(20));
                    let _ = error;
                    continue;
                }
                Err(error) => break Err(error),
            }
            if Instant::now() >= deadline {
                break Err(
                    "instruction revision receipt was not durable before the deadline".to_owned(),
                );
            }
            thread::sleep(Duration::from_millis(20));
        }
    })?;
    Ok(LostAckInstructionRevision {
        receipt,
        _connection: connection,
        _runtime: runtime,
    })
}

fn send_workspace_instruction_revision(
    data_directory: &Path,
    workspace_id: &str,
    idempotency_key: &str,
    frame_request_id: &str,
    if_match: &str,
    body: &[u8],
) -> ResponseFrame {
    let endpoint = UnixEndpoint::derive(data_directory).expect("safe Runtime endpoint");
    let expected_uid = fs::metadata(data_directory)
        .expect("Runtime data directory metadata")
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build instruction IPC client runtime");
    runtime.block_on(async {
        let mut connection = endpoint
            .connect(|peer| peer.uid == expected_uid)
            .await
            .expect("authenticate the real daemon Unix peer");
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .expect("instruction client body budget");
        let request = workspace_instruction_revision_request(
            workspace_id,
            frame_request_id,
            idempotency_key,
            if_match,
            body,
            &budget,
        );
        connection
            .write_request(&request)
            .await
            .expect("write instruction revision request");
        connection
            .read_response(&budget)
            .await
            .expect("read correlated instruction revision response")
    })
}

fn read_durable_instruction_revision(
    data_directory: &Path,
    workspace_id: &str,
    idempotency_key: &str,
    principal_id: &str,
) -> Result<Option<DurableInstructionRevision>, String> {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .map_err(|error| error.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(1))
        .map_err(|error| error.to_string())?;
    let row: Option<(String, String, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT principal_id, request_digest, response_json, response_digest FROM request_dedup WHERE request_id = ?1",
            [idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((owner_principal_id, request_digest, response_json, response_digest)) = row else {
        return Ok(None);
    };
    let (Some(response_json), Some(response_digest)) = (response_json, response_digest) else {
        return Err("instruction receipt is incomplete".to_owned());
    };
    if owner_principal_id != principal_id {
        return Err("instruction receipt belongs to a different Principal".to_owned());
    }
    let response: Value =
        serde_json::from_str(&response_json).map_err(|error| error.to_string())?;
    let (workspace_version, current_instruction_revision): (i64, Option<i64>) = connection
        .query_row(
            "SELECT version, current_instruction_revision FROM workspaces WHERE workspace_id = ?1",
            [workspace_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let revision_count = connection
        .query_row(
            "SELECT COUNT(*) FROM workspace_instruction_revisions WHERE workspace_id = ?1",
            [workspace_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let event_count = connection
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.instructions.revision.created.v1'",
            [workspace_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let request_receipt_count = connection
        .query_row(
            "SELECT COUNT(*) FROM request_dedup WHERE principal_id = ?1 AND request_id = ?2",
            rusqlite::params![principal_id, idempotency_key],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let event_payload_json: String = connection
        .query_row(
            "SELECT payload_json FROM domain_events WHERE workspace_id = ?1 AND type = 'workspace.instructions.revision.created.v1' ORDER BY entity_revision LIMIT 1",
            [workspace_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(Some(DurableInstructionRevision {
        owner_principal_id,
        request_digest,
        response_digest,
        response,
        workspace_version,
        current_instruction_revision,
        revision_count,
        event_count,
        request_receipt_count,
        event_payload_json,
    }))
}

#[test]
fn durable_instruction_inspection_rejects_incomplete_receipt_rows() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let connection = Connection::open(directory.path().join("litecowork.sqlite3"))
        .expect("temporary SQLite database");
    connection
        .execute_batch(
            "CREATE TABLE request_dedup(
                principal_id TEXT NOT NULL,
                request_id TEXT NOT NULL,
                request_digest TEXT NOT NULL,
                response_json TEXT,
                response_digest TEXT,
                PRIMARY KEY(principal_id, request_id)
            );
            INSERT INTO request_dedup(principal_id, request_id, request_digest)
            VALUES ('principal-local', 'incomplete-receipt-v1', 'sha256:request');",
        )
        .expect("incomplete receipt fixture");
    drop(connection);

    let result = read_durable_instruction_revision(
        directory.path(),
        "workspace-local",
        "incomplete-receipt-v1",
        "principal-local",
    );

    assert_eq!(
        result.expect_err("incomplete durable receipt is invalid"),
        "instruction receipt is incomplete"
    );
}

fn assert_single_instruction_revision(
    receipt: &DurableInstructionRevision,
    workspace_id: &str,
    principal_id: &str,
    idempotency_key: &str,
) {
    assert_eq!(receipt.owner_principal_id, principal_id);
    assert!(receipt.request_digest.starts_with("sha256:"));
    assert!(receipt.response_digest.starts_with("sha256:"));
    assert_eq!(
        receipt.response["instruction_revision"]["workspace_id"],
        workspace_id
    );
    assert_eq!(receipt.response["instruction_revision"]["revision"], 1);
    assert_eq!(
        receipt.response["event"]["type"],
        "workspace.instructions.revision.created.v1"
    );
    assert_eq!(receipt.workspace_version, 2);
    assert_eq!(receipt.current_instruction_revision, Some(1));
    assert_eq!(receipt.revision_count, 1);
    assert_eq!(receipt.event_count, 1);
    assert_eq!(
        receipt.request_receipt_count, 1,
        "receipt for {idempotency_key}"
    );
    assert!(
        !receipt
            .event_payload_json
            .contains("Keep local instructions private"),
        "instruction source text must not enter the domain event payload"
    );
}

fn workspace_create_request(
    body: &[u8],
    idempotency_key: &str,
    frame_request_id: &str,
    budget: &std::sync::Arc<InFlightBodyBudget>,
) -> RequestFrame {
    RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: frame_request_id.to_owned(),
            method: "POST".to_owned(),
            path_and_query: "/v1/workspaces".to_owned(),
            headers: vec![
                LogicalHeader {
                    name: "content-type".to_owned(),
                    value: "application/json".to_owned(),
                },
                LogicalHeader {
                    name: "idempotency-key".to_owned(),
                    value: idempotency_key.to_owned(),
                },
            ],
            body_length: body.len() as u64,
        },
        body.to_vec(),
        budget,
    )
    .expect("bounded Workspace-create frame")
}

fn wait_for_durable_create(
    data_directory: &Path,
    idempotency_key: &str,
    principal_id: &str,
) -> Result<DurableWorkspaceCreate, String> {
    let deadline = Instant::now() + RECEIPT_DEADLINE;
    loop {
        let last_error = match read_durable_create(data_directory, idempotency_key, principal_id) {
            Ok(Some(snapshot)) => return Ok(snapshot),
            Ok(None) => "receipt not committed yet".to_owned(),
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            return Err(format!(
                "Workspace create receipt was not durably visible: {last_error}"
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_durable_create(
    data_directory: &Path,
    idempotency_key: &str,
    principal_id: &str,
) -> Result<Option<DurableWorkspaceCreate>, String> {
    let connection = Connection::open(data_directory.join("litecowork.sqlite3"))
        .map_err(|error| error.to_string())?;
    connection
        .busy_timeout(Duration::from_secs(1))
        .map_err(|error| error.to_string())?;
    let receipt_row: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT principal_id, request_digest, response_json, response_digest FROM request_dedup WHERE request_id = ?1",
            [idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((receipt_principal, request_digest, response_json, response_digest)) = receipt_row
    else {
        return Ok(None);
    };
    if receipt_principal != principal_id {
        return Err("idempotency receipt belongs to a different Principal".to_owned());
    }
    let receipt: Value = serde_json::from_str(&response_json).map_err(|error| error.to_string())?;
    let workspace_id = receipt["workspace"]["workspace_id"]
        .as_str()
        .ok_or_else(|| "committed receipt has no Workspace ID".to_owned())?
        .to_owned();
    let count = |sql: &str, parameter: Option<&str>| -> Result<i64, String> {
        match parameter {
            Some(value) => connection
                .query_row(sql, [value], |row| row.get(0))
                .map_err(|error| error.to_string()),
            None => connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(|error| error.to_string()),
        }
    };
    Ok(Some(DurableWorkspaceCreate {
        owner_principal_id: receipt["workspace"]["owner_principal_id"]
            .as_str()
            .ok_or_else(|| "committed Workspace has no owner Principal".to_owned())?
            .to_owned(),
        request_digest,
        response_digest,
        receipt,
        workspace_count: count("SELECT COUNT(*) FROM workspaces", None)?,
        created_event_count: count(
            "SELECT COUNT(*) FROM domain_events WHERE type = 'workspace.created.v1'",
            None,
        )?,
        workspace_event_count: count(
            "SELECT COUNT(*) FROM domain_events WHERE type = 'workspace.created.v1' AND entity_id = ?1",
            Some(workspace_id.as_str()),
        )?,
        request_receipt_count: count(
            "SELECT COUNT(*) FROM request_dedup WHERE request_id = ?1",
            Some(idempotency_key),
        )?,
    }))
}

fn assert_single_workspace_commit(
    snapshot: &DurableWorkspaceCreate,
    principal_id: &str,
    idempotency_key: &str,
) {
    assert_eq!(snapshot.owner_principal_id, principal_id);
    assert!(snapshot.request_digest.starts_with("sha256:"));
    assert!(snapshot.response_digest.starts_with("sha256:"));
    assert_eq!(snapshot.receipt["event"]["type"], "workspace.created.v1");
    assert_eq!(
        snapshot.receipt["workspace"]["replication_policy"],
        "LOCAL_ONLY"
    );
    assert_eq!(snapshot.workspace_count, 1);
    assert_eq!(snapshot.created_event_count, 1);
    assert_eq!(snapshot.workspace_event_count, 1);
    assert_eq!(
        snapshot.request_receipt_count, 1,
        "receipt for {idempotency_key}"
    );
}

fn wait_for_operator_readiness(child: &mut Child, data_directory: &Path) -> Value {
    let deadline = Instant::now() + CHILD_START_DEADLINE;
    loop {
        if let Some(status) = child.try_wait().expect("check Runtime startup") {
            let (stdout, stderr) = capture_output(child);
            panic!(
                "Runtime exited before the replay test connected ({status}): stdout={stdout} stderr={stderr}"
            );
        }
        let readiness_error = match operator_get_request(
            data_directory,
            "/v1/operator/readiness",
            "workspace-replay-readiness-probe",
        ) {
            Ok(readiness) => return readiness,
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap Runtime after readiness timeout");
            let (stdout, stderr) = capture_output(child);
            panic!(
                "Runtime did not expose authenticated Operator readiness ({readiness_error}); status={status}; stdout={stdout}; stderr={stderr}"
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn assert_operator_readiness(readiness: &Value) {
    assert_eq!(readiness["operator_state"], "SERVING");
    assert!(
        readiness["runtime_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert!(
        readiness["local_incarnation_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert_eq!(readiness["api_contract_version"], 1);
}

fn operator_get_request(
    data_directory: &Path,
    path: &str,
    request_id: &str,
) -> Result<Value, String> {
    let endpoint = UnixEndpoint::derive(data_directory).map_err(|error| format!("{error:?}"))?;
    let expected_uid = fs::metadata(data_directory)
        .map_err(|error| error.to_string())?
        .uid();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async {
        let mut connection = tokio::time::timeout(
            Duration::from_secs(2),
            endpoint.connect(|peer| peer.uid == expected_uid),
        )
        .await
        .map_err(|_| "Runtime lifecycle connection deadline elapsed".to_owned())?
        .map_err(|error| error.to_string())?;
        let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
            .map_err(|error| format!("{error:?}"))?;
        let request = RequestFrame::new(
            RequestHeader {
                protocol_version: PROTOCOL_VERSION,
                request_id: request_id.to_owned(),
                method: "GET".to_owned(),
                path_and_query: path.to_owned(),
                headers: Vec::new(),
                body_length: 0,
            },
            Vec::new(),
            &budget,
        )
        .map_err(|error| format!("{error:?}"))?;
        connection
            .write_request(&request)
            .await
            .map_err(|error| error.to_string())?;
        let response =
            tokio::time::timeout(Duration::from_secs(3), connection.read_response(&budget))
                .await
                .map_err(|_| "Operator response deadline elapsed".to_owned())?
                .map_err(|error| error.to_string())?;
        if response.header.request_id != request_id || response.header.status != 200 {
            return Err("Operator response did not match the request".to_owned());
        }
        serde_json::from_slice(response.body()).map_err(|error| error.to_string())
    })
}

fn wait_for_exit(child: &mut Child, deadline: Duration) -> (ExitStatus, String, String) {
    let until = Instant::now() + deadline;
    loop {
        match child.try_wait().expect("poll Runtime process exit") {
            Some(status) => {
                let (stdout, stderr) = capture_output(child);
                return (status, stdout, stderr);
            }
            None if Instant::now() < until => thread::sleep(Duration::from_millis(20)),
            None => {
                let _ = child.kill();
                let status = child.wait().expect("reap timed-out Runtime process");
                let (stdout, stderr) = capture_output(child);
                panic!(
                    "Runtime missed its shutdown deadline ({status}): stdout={stdout} stderr={stderr}"
                );
            }
        }
    }
}

fn capture_output(child: &mut Child) -> (String, String) {
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut stream) = child.stdout.take() {
        let _ = stream.read_to_string(&mut stdout);
    }
    if let Some(mut stream) = child.stderr.take() {
        let _ = stream.read_to_string(&mut stderr);
    }
    (stdout, stderr)
}
