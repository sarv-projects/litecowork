use super::{RuntimeState, STATE_FILE, read_state};
use operator_ipc::{
    DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, PROTOCOL_VERSION,
    RequestFrame, RequestHeader, unix::UnixEndpoint,
};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const CHILD_DATA_DIRECTORY: &str = "LITECOWORK_TEST_RUNTIME_DATA_DIRECTORY";
const CHILD_ENTRY_TEST: &str =
    "runtime::runtime_process_tests::spawned_runtime_lifecycle_process_probe";
const CHILD_START_DEADLINE: Duration = Duration::from_secs(10);
const CHILD_STOP_DEADLINE: Duration = Duration::from_secs(10);

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

#[test]
fn spawned_runtime_lifecycle_process_probe() {
    let Some(directory) = std::env::var_os(CHILD_DATA_DIRECTORY) else {
        return;
    };
    super::run_with_test_identity_providers(Path::new(&directory))
        .expect("spawned Runtime exits after its supervisor signal");
}

#[test]
fn spawned_runtime_lifecycle_restarts_with_current_incarnation() {
    let directory = tempfile::tempdir().expect("private Runtime directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("restrict Runtime directory");

    let mut first_child = RuntimeChild::spawn(directory.path());
    let first_lifecycle = wait_for_lifecycle(first_child.child(), directory.path());
    let runtime_id = first_lifecycle["runtime_id"]
        .as_str()
        .expect("RuntimeId is exposed")
        .to_owned();
    let first_incarnation = first_lifecycle["incarnation"]["runtime_incarnation_id"]
        .as_str()
        .expect("current incarnation is exposed")
        .to_owned();
    assert_degraded_lifecycle(&first_lifecycle, &runtime_id, &first_incarnation);
    assert!(lifecycle_request(directory.path(), "lifecycle-after-client-close").is_ok());
    assert!(
        first_child
            .child()
            .try_wait()
            .expect("check Runtime after client disconnect")
            .is_none(),
        "closing an authenticated IPC client must leave the Runtime running"
    );

    let (first_status, first_stdout, first_stderr) = first_child.terminate_and_wait();
    assert!(
        first_status.success(),
        "first Runtime shutdown failed: stdout={first_stdout} stderr={first_stderr}"
    );
    let first_state = read_state(&directory.path().join(STATE_FILE))
        .expect("read stopped Runtime state")
        .expect("Runtime state persists");
    assert_eq!(first_state.recovery_state, RuntimeState::Stopped);
    assert!(first_state.last_shutdown_clean);
    assert_durable_current_incarnation(
        directory.path(),
        &runtime_id,
        &first_incarnation,
        "STOPPED",
        "OFFLINE",
    );

    let mut restarted_child = RuntimeChild::spawn(directory.path());
    let restarted_lifecycle = wait_for_lifecycle(restarted_child.child(), directory.path());
    let restarted_incarnation = restarted_lifecycle["incarnation"]["runtime_incarnation_id"]
        .as_str()
        .expect("restarted current incarnation is exposed")
        .to_owned();
    assert_degraded_lifecycle(&restarted_lifecycle, &runtime_id, &restarted_incarnation);
    assert_ne!(restarted_incarnation, first_incarnation);
    assert_durable_current_incarnation(
        directory.path(),
        &runtime_id,
        &restarted_incarnation,
        "DEGRADED",
        "DEGRADED",
    );

    let (restarted_status, restarted_stdout, restarted_stderr) =
        restarted_child.terminate_and_wait();
    assert!(
        restarted_status.success(),
        "restarted Runtime shutdown failed: stdout={restarted_stdout} stderr={restarted_stderr}"
    );
    let stopped_state = read_state(&directory.path().join(STATE_FILE))
        .expect("read final Runtime state")
        .expect("Runtime state persists");
    assert_eq!(stopped_state.runtime_id, runtime_id);
    assert_eq!(stopped_state.local_incarnation_id, restarted_incarnation);
    assert_eq!(stopped_state.recovery_state, RuntimeState::Stopped);
    assert!(stopped_state.last_shutdown_clean);
}

fn wait_for_lifecycle(child: &mut Child, data_directory: &Path) -> Value {
    let deadline = Instant::now() + CHILD_START_DEADLINE;
    loop {
        if let Some(status) = child.try_wait().expect("check Runtime startup") {
            let (stdout, stderr) = capture_output(child);
            panic!(
                "Runtime exited before its lifecycle endpoint became ready ({status}): stdout={stdout} stderr={stderr}"
            );
        }
        match lifecycle_request(data_directory, "runtime-lifecycle-startup-probe") {
            Ok(value) => return value,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "Runtime lifecycle endpoint did not become ready: {error}"
                );
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn lifecycle_request(data_directory: &Path, request_id: &str) -> Result<Value, String> {
    let endpoint = UnixEndpoint::derive(data_directory).map_err(|error| format!("{error:?}"))?;
    let uid = fs::metadata(data_directory)
        .map_err(|error| error.to_string())?
        .uid();
    let budget = InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)
        .map_err(|error| format!("{error:?}"))?;
    let request = RequestFrame::new(
        RequestHeader {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.to_owned(),
            method: "GET".to_owned(),
            path_and_query: "/v1/runtime-lifecycle".to_owned(),
            headers: Vec::<LogicalHeader>::new(),
            body_length: 0,
        },
        Vec::new(),
        &budget,
    )
    .map_err(|error| format!("{error:?}"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async {
        let mut connection = tokio::time::timeout(
            Duration::from_secs(2),
            endpoint.connect(|peer| peer.uid == uid),
        )
        .await
        .map_err(|_| "Operator connection deadline expired".to_owned())?
        .map_err(|error| format!("{error:?}"))?;
        connection
            .write_request(&request)
            .await
            .map_err(|error| format!("{error:?}"))?;
        let response =
            tokio::time::timeout(Duration::from_secs(3), connection.read_response(&budget))
                .await
                .map_err(|_| "Operator response deadline expired".to_owned())?
                .map_err(|error| format!("{error:?}"))?;
        if response.header.request_id != request_id {
            return Err("Operator response request ID did not correlate".to_owned());
        }
        if response.header.status != 200 {
            return Err(format!(
                "Operator lifecycle returned HTTP {}: {}",
                response.header.status,
                String::from_utf8_lossy(response.body())
            ));
        }
        let no_store = response.header.headers.iter().any(|header| {
            header.name.eq_ignore_ascii_case("cache-control")
                && header.value.eq_ignore_ascii_case("no-store")
        });
        if !no_store {
            return Err("Operator lifecycle response omitted no-store".to_owned());
        }
        serde_json::from_slice(response.body()).map_err(|error| error.to_string())
    })
}

fn assert_degraded_lifecycle(value: &Value, runtime_id: &str, incarnation_id: &str) {
    assert!(value.get("local_principal_id").is_none());
    assert_eq!(value["runtime_id"], runtime_id);
    assert_eq!(value["availability"], "DEGRADED");
    assert_eq!(value["incarnation"]["runtime_id"], runtime_id);
    assert_eq!(
        value["incarnation"]["runtime_incarnation_id"],
        incarnation_id
    );
    assert_eq!(value["incarnation"]["recovery_state"], "DEGRADED");
    assert!(
        !value["incarnation"]["recovered_from_unclean_shutdown"]
            .as_bool()
            .expect("unclean-shutdown state is boolean")
    );
    let blockers = value["blockers"]
        .as_array()
        .expect("degraded lifecycle has typed blockers");
    assert!(
        blockers.iter().any(|blocker| {
            blocker["safe_message"]
                .as_str()
                .is_some_and(|message| message.contains("Task execution admission"))
        }),
        "Runtime must report the unavailable Task admission dependency"
    );
}

fn assert_durable_current_incarnation(
    data_directory: &Path,
    runtime_id: &str,
    expected_incarnation: &str,
    expected_state: &str,
    expected_availability: &str,
) {
    let connection = rusqlite::Connection::open(data_directory.join("litecowork.sqlite3"))
        .expect("Runtime SQLite database opens");
    let (current_incarnation, state, availability, stopped_at): (String, String, String, Option<String>) =
        connection
            .query_row(
                "SELECT r.current_incarnation_id,i.recovery_state,r.availability,i.stopped_at FROM runtimes r JOIN runtime_incarnations i ON i.runtime_id=r.runtime_id AND i.runtime_incarnation_id=r.current_incarnation_id WHERE r.runtime_id=?1",
                [runtime_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("durable current Runtime incarnation exists");
    assert_eq!(current_incarnation, expected_incarnation);
    assert_eq!(state, expected_state);
    assert_eq!(availability, expected_availability);
    assert_eq!(stopped_at.is_some(), expected_state == "STOPPED");
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
