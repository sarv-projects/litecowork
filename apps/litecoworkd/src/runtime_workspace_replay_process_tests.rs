use super::{
    RuntimeState, STATE_FILE, read_state, runtime_process_tests::assert_degraded_lifecycle,
};
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    RequestFrame, RequestHeader, ResponseFrame, unix::UnixEndpoint,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
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
