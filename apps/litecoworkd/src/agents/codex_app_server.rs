//! Runtime-local Codex App Server JSONL transport (unqualified implementation).
//!
//! Wire authority: https://developers.openai.com/codex/app-server . Native
//! payloads stay private. No payload is logged, persisted, or auto-approved.
//! This module does not establish descendant-writer quiescence or Trust grants.
use std::{
    collections::HashMap,
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub(crate) struct TransportLimits {
    pub max_line_bytes: usize,
    /// Bounds all retained events: queued plus events held by the caller.
    pub max_queued_events: usize,
    /// Aggregate retained JSONL wire bytes, including events held by the caller.
    /// JSON object overhead is also bounded by the line and event-count limits.
    pub max_retained_wire_bytes: usize,
    pub max_pending_rpcs: usize,
    pub max_server_requests: usize,
    pub startup_timeout: Duration,
    pub rpc_timeout: Duration,
    pub event_timeout: Duration,
}
impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            max_line_bytes: 1024 * 1024,
            max_queued_events: 64,
            max_retained_wire_bytes: 8 * 1024 * 1024,
            max_pending_rpcs: 16,
            max_server_requests: 16,
            startup_timeout: Duration::from_secs(15),
            rpc_timeout: Duration::from_secs(30),
            event_timeout: Duration::from_secs(5),
        }
    }
}

/// Explicit non-secret native discovery/process settings. Nothing is inherited
/// from the daemon environment. HOME/config files can themselves hold provider
/// auth; selecting those directories is an admitted native-auth operation.
/// SecretLease/environment injection is intentionally unsupported here.
#[derive(Default)]
pub(crate) struct NativeEnvironment {
    pub home: Option<OsString>,
    pub user_profile: Option<OsString>,
    pub app_data: Option<OsString>,
    pub local_app_data: Option<OsString>,
    pub codex_home: Option<OsString>,
    pub path: Option<OsString>,
    pub system_root: Option<OsString>,
    pub temporary_directory: Option<OsString>,
}

/// Stable LiteCowork client attribution supplied in the native initialize
/// handshake. These are public app identifiers, never Workspace/user data.
pub(crate) struct AppServerClientInfo<'a> {
    pub name: &'a str,
    pub title: &'a str,
    pub version: &'a str,
}
impl NativeEnvironment {
    fn apply(self, command: &mut Command) {
        command.env_clear();
        for (key, value) in [
            ("HOME", self.home),
            ("USERPROFILE", self.user_profile),
            ("APPDATA", self.app_data),
            ("LOCALAPPDATA", self.local_app_data),
            ("CODEX_HOME", self.codex_home),
            ("PATH", self.path),
            ("SYSTEMROOT", self.system_root),
            ("TMPDIR", self.temporary_directory.clone()),
            ("TEMP", self.temporary_directory.clone()),
            ("TMP", self.temporary_directory),
        ] {
            if let Some(value) = value {
                command.env(key, value);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransportError {
    InvalidLimits,
    SpawnFailed,
    PipeUnavailable,
    WorkerUnavailable,
    Io,
    InvalidMessage,
    InvalidRequestInput,
    MessageTooLarge,
    QueueFull,
    Timeout,
    Closed,
    NotInitialized,
    AlreadyInitialized,
    UnknownRequest,
    UnsupportedReply,
    IdExhausted,
}
impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Codex transport: {self:?}")
    }
}
impl std::error::Error for TransportError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessObservation {
    Running,
    StopRequested,
    Exited,
    ObservationFailed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransportState {
    pub initialized: bool,
    pub fault: Option<TransportError>,
    pub process: ProcessObservation,
    /// Always false; owned-host exit never proves descendant writer exclusion.
    pub writer_quiescence_proven: bool,
}

struct LifecycleState {
    initialized: bool,
    fault: Option<TransportError>,
    process: ProcessObservation,
    stop_requested: bool,
    next_rpc_deadline: Option<Instant>,
}
/// Owner retains this handle to observe faults and direct-child reaping even
/// after dropping the transport. Monitoring is required for lifecycle admission.
#[derive(Clone)]
pub(crate) struct LifecycleObservation(Arc<Mutex<LifecycleState>>);
impl LifecycleObservation {
    pub(crate) fn snapshot(&self) -> TransportState {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        TransportState {
            initialized: state.initialized,
            fault: state.fault,
            process: state.process,
            writer_quiescence_proven: false,
        }
    }
    fn fail(&self, error: TransportError) {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        state.fault.get_or_insert(error);
        state.stop_requested = true;
        if state.process != ProcessObservation::Exited {
            state.process = ProcessObservation::StopRequested;
        }
    }
    fn ensure_open(&self) -> Result<(), TransportError> {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.stop_requested
            || state.fault.is_some()
            || state.process != ProcessObservation::Running
        {
            Err(state.fault.unwrap_or(TransportError::Closed))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RpcId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RpcMethod {
    Initialize,
    AccountRead,
    ModelList,
    ThreadStart,
    ThreadResume,
    TurnStart,
    TurnSteer,
    TurnInterrupt,
}
impl RpcMethod {
    fn wire(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::AccountRead => "account/read",
            Self::ModelList => "model/list",
            Self::ThreadStart => "thread/start",
            Self::ThreadResume => "thread/resume",
            Self::TurnStart => "turn/start",
            Self::TurnSteer => "turn/steer",
            Self::TurnInterrupt => "turn/interrupt",
        }
    }
}
struct PendingRpc {
    method: RpcMethod,
    deadline: Instant,
}
struct ServerRequest {
    method: String,
    available_decisions: Option<Vec<String>>,
}

/// Only one-shot approval is supported; session/policy amendments require a
/// separately qualified authority contract. No arbitrary result-value API.
#[derive(Clone, Copy)]
pub(crate) enum ApprovalDecision {
    AcceptOnce,
    Decline,
    Cancel,
}
pub(crate) enum ServerReply {
    ExecutionApproval(ApprovalDecision),
    FileChangeApproval(ApprovalDecision),
    DenyPermissions,
    DeclineElicitation,
    CancelElicitation,
    Unsupported,
}

struct RetentionBudget {
    wire_bytes: AtomicUsize,
    events: AtomicUsize,
}
struct BytePermit {
    retained: Arc<RetentionBudget>,
    bytes: usize,
}
impl Drop for BytePermit {
    fn drop(&mut self) {
        self.retained
            .wire_bytes
            .fetch_sub(self.bytes, Ordering::AcqRel);
        self.retained.events.fetch_sub(1, Ordering::AcqRel);
    }
}
struct IncomingMessage {
    value: Value,
    permit: BytePermit,
}
/// Native data is deliberately not Debug/Clone. Keeping an event alive retains
/// its byte/count permit, so consumer retention cannot bypass admission.
pub(crate) struct NativeEvent {
    kind: NativeEventKind,
    _permit: BytePermit,
}
impl NativeEvent {
    pub(crate) fn kind(&self) -> &NativeEventKind {
        &self.kind
    }
}
pub(crate) enum NativeEventKind {
    /// A queued native observation drained after transport failure/host exit.
    /// Diagnostic evidence only: never an RPC acknowledgment, initialized
    /// session, authorization, or successful completion admission.
    TerminalObservation {
        fault: TransportError,
        message: Value,
    },
    Notification(Value),
    ServerRequest(Value),
    /// Raw RPC payloads are adapter-private. `account/read` may include
    /// account identity fields; consumers must not log or persist the raw
    /// value and should derive only the minimum sanitized observation needed.
    RpcResponse {
        id: RpcId,
        method: RpcMethod,
        result: Value,
    },
    /// Raw native error is private data; display only a sanitized projection.
    RpcRejected {
        id: RpcId,
        method: RpcMethod,
        error: Value,
    },
}
struct WriteJob {
    bytes: Vec<u8>,
    ack: SyncSender<Result<(), TransportError>>,
}

/// Split request/event dispatcher: begin() sends an RPC without waiting for its
/// response. poll_event() surfaces server requests while RPCs remain pending;
/// the trusted lifecycle owner can consult Trust/UI and reply before continuing.
/// RPC watchdogs stop the host even if the owner stops pumping events.
/// Thread and turn constructors expose only a small bounded subset of the native
/// protocol. They do not admit a LiteCowork Task/Attempt, grant authority, or
/// establish writer fencing; callers must enforce those contracts separately.
pub(crate) struct CodexAppServer {
    outgoing: SyncSender<WriteJob>,
    incoming: Receiver<IncomingMessage>,
    lifecycle: LifecycleObservation,
    limits: TransportLimits,
    initialized: bool,
    next_id: u64,
    pending: HashMap<RpcId, PendingRpc>,
    server_requests: HashMap<String, ServerRequest>,
}

impl CodexAppServer {
    /// Explicit launch only; executable must be an absolute admitted binding.
    /// This operation performs no native config edits or provider model requests.
    pub(crate) fn spawn(
        executable: &Path,
        cwd: &Path,
        environment: NativeEnvironment,
        limits: TransportLimits,
    ) -> Result<Self, TransportError> {
        if !executable.is_absolute() || !cwd.is_absolute() {
            return Err(TransportError::InvalidMessage);
        }
        if limits.max_line_bytes == 0
            || limits.max_line_bytes > 4 * 1024 * 1024
            || limits.max_queued_events == 0
            || limits.max_queued_events > 256
            || limits.max_retained_wire_bytes < limits.max_line_bytes
            || limits.max_retained_wire_bytes > 32 * 1024 * 1024
            || limits.max_pending_rpcs == 0
            || limits.max_pending_rpcs > 64
            || limits.max_server_requests == 0
            || limits.max_server_requests > 64
            || [
                limits.startup_timeout,
                limits.rpc_timeout,
                limits.event_timeout,
            ]
            .iter()
            .any(|timeout| timeout.is_zero() || *timeout > Duration::from_secs(60))
        {
            return Err(TransportError::InvalidLimits);
        }
        let mut command = Command::new(executable);
        environment.apply(&mut command);
        let mut child = command
            .args(["app-server", "--listen", "stdio://"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| TransportError::SpawnFailed)?;
        let (Some(mut stdin), Some(stdout), Some(mut stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            // Owned fallback reaper: child must never be abandoned unreaped.
            let _ = child.kill();
            let _ = child.wait();
            return Err(TransportError::PipeUnavailable);
        };
        let lifecycle = LifecycleObservation(Arc::new(Mutex::new(LifecycleState {
            initialized: false,
            fault: None,
            process: ProcessObservation::Running,
            stop_requested: false,
            next_rpc_deadline: Some(Instant::now() + limits.startup_timeout),
        })));
        // Transfer Child to its sole owner before starting pipe workers. The
        // owner remains alive until try_wait confirms that the child was reaped.
        let owned_child = Arc::new(Mutex::new(Some(child)));
        let reaper_child = Arc::clone(&owned_child);
        let reaper_lifecycle = lifecycle.clone();
        if thread::Builder::new()
            .name("codex-host-reaper".into())
            .spawn(move || {
                if let Some(child) = reaper_child
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .take()
                {
                    monitor_child(child, reaper_lifecycle);
                }
            })
            .is_err()
        {
            if let Some(mut child) = owned_child
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                let _ = child.kill();
                let _ = child.wait();
            }
            return Err(TransportError::WorkerUnavailable);
        }
        let (outgoing, writes) = mpsc::sync_channel::<WriteJob>(1);
        let write_lifecycle = lifecycle.clone();
        if thread::Builder::new()
            .name("codex-stdin".into())
            .spawn(move || {
                while let Ok(job) = writes.recv() {
                    if write_lifecycle.ensure_open().is_err() {
                        break;
                    }
                    let result = stdin
                        .write_all(&job.bytes)
                        .and_then(|_| stdin.flush())
                        .map_err(|_| TransportError::Io);
                    if let Err(error) = result {
                        write_lifecycle.fail(error);
                    }
                    let stopped = result.is_err();
                    let _ = job.ack.try_send(result);
                    if stopped {
                        break;
                    }
                }
            })
            .is_err()
        {
            lifecycle.fail(TransportError::WorkerUnavailable);
            return Err(TransportError::WorkerUnavailable);
        }
        let (messages, incoming) = mpsc::sync_channel(limits.max_queued_events);
        let read_lifecycle = lifecycle.clone();
        let retained = Arc::new(RetentionBudget {
            wire_bytes: AtomicUsize::new(0),
            events: AtomicUsize::new(0),
        });
        if thread::Builder::new()
            .name("codex-stdout".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                loop {
                    if read_lifecycle.ensure_open().is_err() {
                        break;
                    }
                    match read_message(
                        &mut reader,
                        limits.max_line_bytes,
                        &retained,
                        limits.max_retained_wire_bytes,
                        limits.max_queued_events,
                    ) {
                        Ok(message) => {
                            if messages.try_send(message).is_err() {
                                read_lifecycle.fail(TransportError::QueueFull);
                                break;
                            }
                        }
                        Err(error) => {
                            read_lifecycle.fail(error);
                            break;
                        }
                    }
                }
            })
            .is_err()
        {
            lifecycle.fail(TransportError::WorkerUnavailable);
            return Err(TransportError::WorkerUnavailable);
        }
        if thread::Builder::new()
            .name("codex-stderr-drain".into())
            .spawn(move || {
                let mut discarded = [0_u8; 4096];
                loop {
                    match stderr.read(&mut discarded) {
                        Ok(0) => break,
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                    }
                }
            })
            .is_err()
        {
            lifecycle.fail(TransportError::WorkerUnavailable);
            return Err(TransportError::WorkerUnavailable);
        }
        Ok(Self {
            outgoing,
            incoming,
            lifecycle,
            limits,
            initialized: false,
            next_id: 1,
            pending: HashMap::new(),
            server_requests: HashMap::new(),
        })
    }

    pub(crate) fn lifecycle_observation(&self) -> LifecycleObservation {
        self.lifecycle.clone()
    }

    /// Perform the protocol initialize handshake using stable product
    /// attribution. Callers cannot omit or replace protocol fields with an
    /// arbitrary JSON payload.
    pub(crate) fn initialize(
        &mut self,
        client: AppServerClientInfo<'_>,
    ) -> Result<RpcId, TransportError> {
        fn valid(value: &str, maximum: usize) -> bool {
            !value.trim().is_empty()
                && value.len() <= maximum
                && !value.chars().any(char::is_control)
        }
        if !valid(client.name, 64) || !valid(client.title, 96) || !valid(client.version, 48) {
            return Err(TransportError::InvalidMessage);
        }
        self.begin(
            RpcMethod::Initialize,
            json!({
                "clientInfo": {
                    "name": client.name,
                    "title": client.title,
                    "version": client.version
                }
            }),
        )
    }

    /// Read the account summary exposed by Codex App Server without refreshing
    /// tokens. The raw response may contain account
    /// identifiers or an email address; callers must keep it in memory only,
    /// avoid logging it, and must not treat it as proof of model entitlement.
    pub(crate) fn account_read(&mut self) -> Result<RpcId, TransportError> {
        self.begin(RpcMethod::AccountRead, json!({"refreshToken":false}))
    }

    /// Read one bounded page from the server's visible model catalog.
    /// This is an option-discovery hint only, not evidence that the current
    /// account is entitled to use any listed model. A successful inference is
    /// required to establish access. The raw response is not safe to log or
    /// persist as account/config state.
    pub(crate) fn model_list(&mut self) -> Result<RpcId, TransportError> {
        self.begin(
            RpcMethod::ModelList,
            json!({"limit":32,"includeHidden":false}),
        )
    }

    /// Start a native Codex thread using only its working directory and an
    /// optional model selector. Other thread settings remain under Codex's
    /// native configuration and are deliberately not accepted as arbitrary
    /// caller JSON. `cwd` must be an absolute, bounded UTF-8 path.
    pub(crate) fn thread_start(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
    ) -> Result<RpcId, TransportError> {
        let cwd = cwd.to_str().ok_or(TransportError::InvalidRequestInput)?;
        if !Path::new(cwd).is_absolute() || !valid_bounded_identifier(cwd, 4096) {
            return Err(TransportError::InvalidRequestInput);
        }
        let mut params = serde_json::Map::new();
        params.insert("cwd".into(), Value::String(cwd.to_owned()));
        if let Some(model) = model {
            if !valid_bounded_identifier(model, 128) {
                return Err(TransportError::InvalidRequestInput);
            }
            params.insert("model".into(), Value::String(model.to_owned()));
        }
        self.begin(RpcMethod::ThreadStart, Value::Object(params))
    }

    /// Start a Codex thread whose default filesystem sandbox is read-only.
    /// The read-only mode may still read the host filesystem. This does not
    /// establish Task admission, restrict native MCP/app tools, or prove writer
    /// quiescence. Use only inside a separately isolated Environment and after
    /// qualifying every native capability exposed by the effective config.
    pub(crate) fn thread_start_read_only(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
    ) -> Result<RpcId, TransportError> {
        let cwd = validated_absolute_path(cwd)?;
        let mut params = serde_json::Map::new();
        params.insert("cwd".into(), Value::String(cwd));
        params.insert("sandbox".into(), Value::String("read-only".to_owned()));
        if let Some(model) = model {
            if !valid_bounded_identifier(model, 128) {
                return Err(TransportError::InvalidRequestInput);
            }
            params.insert("model".into(), Value::String(model.to_owned()));
        }
        self.begin(RpcMethod::ThreadStart, Value::Object(params))
    }

    /// Resume a native thread addressed by a caller-supplied opaque handle.
    /// The handle is forwarded only for this RPC; this transport does not log
    /// or persist it. It does not imply that the associated Task or workspace
    /// is authorized or safe to resume.
    pub(crate) fn thread_resume(&mut self, thread_handle: &str) -> Result<RpcId, TransportError> {
        if !valid_bounded_identifier(thread_handle, 256) {
            return Err(TransportError::InvalidRequestInput);
        }
        self.begin(RpcMethod::ThreadResume, json!({"threadId":thread_handle}))
    }

    /// Start one text-only native turn on a caller-selected opaque thread
    /// handle. Multimodal/file inputs and per-turn policy overrides are not
    /// accepted by this constructor. The caller owns Task admission, effect
    /// reconciliation, authorization and cancellation policy.
    pub(crate) fn turn_start(
        &mut self,
        thread_handle: &str,
        input_text: &str,
    ) -> Result<RpcId, TransportError> {
        if !valid_bounded_identifier(thread_handle, 256)
            || !valid_bounded_text(input_text, 256 * 1024)
        {
            return Err(TransportError::InvalidRequestInput);
        }
        self.begin(
            RpcMethod::TurnStart,
            json!({
                "threadId":thread_handle,
                "input":[{"type":"text","text":input_text,"textElements":[]}]
            }),
        )
    }

    /// Start a text-only plan turn with Codex's read-only sandbox and shell
    /// network disabled. This does not restrict filesystem reads to a root set:
    /// Codex's default read-only access may include the host filesystem. Only
    /// use this inside an externally isolated, Task-specific Environment after
    /// native MCP/apps and other non-filesystem capabilities are disabled or
    /// mediated. This constructor is not Task admission or an Environment.
    pub(crate) fn turn_start_read_only_plan(
        &mut self,
        thread_handle: &str,
        cwd: &Path,
        input_text: &str,
    ) -> Result<RpcId, TransportError> {
        if !valid_bounded_identifier(thread_handle, 256)
            || !valid_bounded_text(input_text, 256 * 1024)
        {
            return Err(TransportError::InvalidRequestInput);
        }

        let cwd = validated_absolute_path(cwd)?;
        self.begin(
            RpcMethod::TurnStart,
            json!({
                "threadId": thread_handle,
                "input":[{"type":"text","text":input_text,"textElements":[]}],
                "cwd": cwd,
                "sandboxPolicy": {
                    "type":"readOnly",
                    "networkAccess":false
                },
                "outputSchema": initial_plan_output_schema()
            }),
        )
    }

    /// Interrupt one native turn. This is a transport operation only and does
    /// not prove that child processes stopped writing to an Environment.
    pub(crate) fn turn_interrupt(
        &mut self,
        thread_handle: &str,
        turn_handle: &str,
    ) -> Result<RpcId, TransportError> {
        if !valid_bounded_identifier(thread_handle, 256)
            || !valid_bounded_identifier(turn_handle, 256)
        {
            return Err(TransportError::InvalidRequestInput);
        }
        self.begin(
            RpcMethod::TurnInterrupt,
            json!({
                "threadId":thread_handle,
                "turnId":turn_handle
            }),
        )
    }

    fn begin(&mut self, method: RpcMethod, params: Value) -> Result<RpcId, TransportError> {
        self.lifecycle.ensure_open()?;
        if method == RpcMethod::Initialize {
            if self.initialized || !self.pending.is_empty() {
                return Err(TransportError::AlreadyInitialized);
            }
        } else if !self.initialized {
            return Err(TransportError::NotInitialized);
        }
        // These read-only discovery calls have fixed bounded parameters.
        // Keep this guard even though constructors are typed so future crate
        // callers cannot accidentally widen a provider-facing request.
        let fixed_read_only_params = match method {
            RpcMethod::AccountRead => params == json!({"refreshToken":false}),
            RpcMethod::ModelList => params == json!({"limit":32,"includeHidden":false}),
            _ => true,
        };
        if (!params.is_object() || !fixed_read_only_params)
            || (method == RpcMethod::TurnSteer
                && !params.get("expectedTurnId").is_some_and(Value::is_string))
        {
            return Err(TransportError::InvalidMessage);
        }
        if self.pending.len() >= self.limits.max_pending_rpcs {
            return self.poison(TransportError::QueueFull);
        }
        let id = RpcId(self.next_id);
        self.next_id = match self.next_id.checked_add(1) {
            Some(next) => next,
            None => return self.poison(TransportError::IdExhausted),
        };
        let timeout = if method == RpcMethod::Initialize {
            self.limits.startup_timeout
        } else {
            self.limits.rpc_timeout
        };
        let deadline = Instant::now() + timeout;
        self.pending.insert(id, PendingRpc { method, deadline });
        self.refresh_deadline();
        self.write_until(
            json!({"id":id.0,"method":method.wire(),"params":params}),
            deadline,
        )?;
        Ok(id)
    }

    /// Event timeout means no event yet; it does not poison an idle healthy host.
    /// RPC timeout is independently enforced by the owned monitor thread.
    /// Already queued data can drain after a fault as TerminalObservation; new
    /// RPCs and replies remain forbidden by the lifecycle's ensure_open guard.
    pub(crate) fn poll_event(&mut self) -> Result<Option<NativeEvent>, TransportError> {
        let message = match self.incoming.try_recv() {
            Ok(message) => message,
            Err(mpsc::TryRecvError::Disconnected) => return self.poison(TransportError::Closed),
            Err(mpsc::TryRecvError::Empty) => {
                self.lifecycle.ensure_open()?;
                match self.incoming.recv_timeout(self.limits.event_timeout) {
                    Ok(message) => message,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        self.lifecycle.ensure_open()?;
                        return Ok(None);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return self.poison(TransportError::Closed);
                    }
                }
            }
        };
        let IncomingMessage { value, permit } = message;
        if !valid_message_envelope(&value) {
            return self.poison(TransportError::InvalidMessage);
        }
        if let Some(fault) = self.lifecycle.snapshot().fault {
            return Ok(Some(NativeEvent {
                kind: NativeEventKind::TerminalObservation {
                    fault,
                    message: value,
                },
                _permit: permit,
            }));
        }
        let kind = if let Some(method) = value.get("method") {
            let Some(method) = method.as_str() else {
                return self.poison(TransportError::InvalidMessage);
            };
            if method.len() > 256 {
                return self.poison(TransportError::InvalidMessage);
            }
            if value.get("result").is_some() || value.get("error").is_some() {
                return self.poison(TransportError::InvalidMessage);
            }
            if let Some(id) = value.get("id") {
                let key = match request_key(id) {
                    Ok(key) => key,
                    Err(error) => return self.poison(error),
                };
                if self.server_requests.len() >= self.limits.max_server_requests
                    || self.server_requests.contains_key(&key)
                {
                    return self.poison(TransportError::QueueFull);
                }
                let available_decisions =
                    if let Some(available) = value.pointer("/params/availableDecisions") {
                        let Some(values) = available.as_array() else {
                            return self.poison(TransportError::InvalidMessage);
                        };
                        if values.len() > 16 {
                            return self.poison(TransportError::InvalidMessage);
                        }
                        // Object policy-amendment choices remain native payloads;
                        // only the three one-shot string decisions are supported.
                        Some(
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|choice| matches!(*choice, "accept" | "decline" | "cancel"))
                                .map(str::to_owned)
                                .collect(),
                        )
                    } else {
                        None
                    };
                self.server_requests.insert(
                    key,
                    ServerRequest {
                        method: method.to_owned(),
                        available_decisions,
                    },
                );
                NativeEventKind::ServerRequest(value)
            } else {
                if method == "serverRequest/resolved" {
                    if let Some(id) = value.pointer("/params/requestId") {
                        let key = match request_key(id) {
                            Ok(key) => key,
                            Err(error) => return self.poison(error),
                        };
                        self.server_requests.remove(&key);
                    }
                }
                NativeEventKind::Notification(value)
            }
        } else {
            let Some(id) = value.get("id").and_then(Value::as_u64).map(RpcId) else {
                return self.poison(TransportError::InvalidMessage);
            };
            let Some(pending) = self.pending.remove(&id) else {
                return self.poison(TransportError::InvalidMessage);
            };
            if Instant::now() >= pending.deadline {
                self.lifecycle.fail(TransportError::Timeout);
                return Ok(Some(NativeEvent {
                    kind: NativeEventKind::TerminalObservation {
                        fault: TransportError::Timeout,
                        message: value,
                    },
                    _permit: permit,
                }));
            }
            match (value.get("result"), value.get("error")) {
                (Some(result), None) => {
                    if pending.method == RpcMethod::Initialize {
                        if !valid_initialize_result(result) {
                            return self.poison(TransportError::InvalidMessage);
                        }
                        if let Err(fault) = self.write_until(
                            json!({"method":"initialized","params":{}}),
                            pending.deadline,
                        ) {
                            // Host exit may race the dequeue/snapshot above.
                            // Keep the native response as diagnostic evidence;
                            // failed handshake must not activate the session.
                            return Ok(Some(NativeEvent {
                                kind: NativeEventKind::TerminalObservation {
                                    fault,
                                    message: value,
                                },
                                _permit: permit,
                            }));
                        }
                        self.initialized = true;
                        self.lifecycle
                            .0
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .initialized = true;
                    }
                    self.refresh_deadline();
                    NativeEventKind::RpcResponse {
                        id,
                        method: pending.method,
                        result: result.clone(),
                    }
                }
                (None, Some(error)) => {
                    if pending.method == RpcMethod::Initialize {
                        self.lifecycle.fail(TransportError::NotInitialized);
                        return Ok(Some(NativeEvent {
                            kind: NativeEventKind::TerminalObservation {
                                fault: TransportError::NotInitialized,
                                message: value,
                            },
                            _permit: permit,
                        }));
                    }
                    self.refresh_deadline();
                    NativeEventKind::RpcRejected {
                        id,
                        method: pending.method,
                        error: error.clone(),
                    }
                }
                _ => return self.poison(TransportError::InvalidMessage),
            }
        };
        Ok(Some(NativeEvent {
            kind,
            _permit: permit,
        }))
    }

    /// Trusted owner supplies the exact approved decision. Matching reply types
    /// prevent replying to a permission request with an execution approval.
    pub(crate) fn reply(&mut self, id: Value, reply: ServerReply) -> Result<(), TransportError> {
        self.lifecycle.ensure_open()?;
        let key = match request_key(&id) {
            Ok(key) => key,
            Err(error) => return self.poison(error),
        };
        let Some(request) = self.server_requests.get(&key) else {
            return Err(TransportError::UnknownRequest);
        };
        let result = match (&*request.method, reply) {
            ("item/commandExecution/requestApproval", ServerReply::ExecutionApproval(decision))
            | ("item/fileChange/requestApproval", ServerReply::FileChangeApproval(decision)) => {
                let decision = match decision {
                    ApprovalDecision::AcceptOnce => "accept",
                    ApprovalDecision::Decline => "decline",
                    ApprovalDecision::Cancel => "cancel",
                };
                if let Some(available) = &request.available_decisions {
                    if !available.iter().any(|value| value == decision) {
                        return Err(TransportError::UnsupportedReply);
                    }
                }
                Some(json!({"decision":decision}))
            }
            ("item/permissions/requestApproval", ServerReply::DenyPermissions) => {
                Some(json!({"permissions":{},"scope":"turn"}))
            }
            ("mcpServer/elicitation/request", ServerReply::DeclineElicitation) => {
                Some(json!({"action":"decline","content":null}))
            }
            ("mcpServer/elicitation/request", ServerReply::CancelElicitation) => {
                Some(json!({"action":"cancel","content":null}))
            }
            (_, ServerReply::Unsupported) => None,
            _ => return Err(TransportError::UnsupportedReply),
        };
        let response = match result {
            Some(result) => json!({"id":id,"result":result}),
            None => json!({"id":id,"error":{"code":-32601,"message":"Unsupported client request"}}),
        };
        self.write_until(response, Instant::now() + self.limits.rpc_timeout)?;
        self.server_requests.remove(&key);
        Ok(())
    }

    fn refresh_deadline(&self) {
        self.lifecycle
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .next_rpc_deadline = self.pending.values().map(|pending| pending.deadline).min();
    }
    fn poison<T>(&self, error: TransportError) -> Result<T, TransportError> {
        self.lifecycle.fail(error);
        Err(error)
    }
    fn write_until(&self, value: Value, deadline: Instant) -> Result<(), TransportError> {
        self.lifecycle.ensure_open()?;
        // Serialize through a bounded writer; reject before an oversized output
        // buffer is allocated. Input Value allocation belongs to the caller.
        let mut bytes = BoundedBytes {
            bytes: Vec::new(),
            limit: self.limits.max_line_bytes - 1,
        };
        if serde_json::to_writer(&mut bytes, &value).is_err() {
            return self.poison(TransportError::MessageTooLarge);
        }
        bytes.bytes.push(b'\n');
        let (ack, completion) = mpsc::sync_channel(1);
        if self
            .outgoing
            .try_send(WriteJob {
                bytes: bytes.bytes,
                ack,
            })
            .is_err()
        {
            return self.poison(TransportError::QueueFull);
        }
        match completion.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => self.poison(error),
            Err(_) => self.poison(TransportError::Timeout),
        }
    }
    pub(crate) fn state(&self) -> TransportState {
        let mut state = self.lifecycle.snapshot();
        state.initialized = self.initialized;
        state
    }
    /// Requests direct-host termination; owner must observe Exited before reuse.
    /// Even Exited is not evidence that descendants/worktree writers are fenced.
    pub(crate) fn stop_host(&self) -> TransportState {
        self.lifecycle.fail(TransportError::Closed);
        self.state()
    }
}
impl Drop for CodexAppServer {
    fn drop(&mut self) {
        self.lifecycle.fail(TransportError::Closed);
    }
}

fn monitor_child(mut child: Child, lifecycle: LifecycleObservation) {
    loop {
        let stop = {
            let mut state = lifecycle
                .0
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if state
                .next_rpc_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                state.fault.get_or_insert(TransportError::Timeout);
                state.stop_requested = true;
            }
            if state.stop_requested {
                state.process = ProcessObservation::StopRequested;
            }
            state.stop_requested
        };
        if stop {
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                let mut state = lifecycle
                    .0
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.process = ProcessObservation::Exited;
                state.fault.get_or_insert(TransportError::Closed);
                state.stop_requested = true;
                return;
            }
            Ok(None) => {}
            Err(_) => {
                let mut state = lifecycle
                    .0
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.fault.get_or_insert(TransportError::Io);
                state.stop_requested = true;
                state.process = ProcessObservation::ObservationFailed;
                // Keep sole ownership and retry kill/reap; never drop a child
                // merely because process observation failed transiently.
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn request_key(id: &Value) -> Result<String, TransportError> {
    if id.as_str().is_some_and(|value| value.len() <= 256)
        || id.as_i64().is_some()
        || id.as_u64().is_some()
    {
        Ok(id.to_string())
    } else {
        Err(TransportError::InvalidMessage)
    }
}

/// Opaque native handles are validated only for size and unsafe control
/// characters. Their syntax and meaning remain owned by Codex; callers must
/// not derive authorization from their contents.
fn valid_bounded_identifier(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn valid_bounded_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

/// Validate only the portable path shape accepted by the Codex protocol.
/// This is not an Environment identity or symlink check; admission must obtain
/// paths from the Runtime's verified Environment provider, never an Operator
/// request or agent output.
fn validated_absolute_path(path: &Path) -> Result<String, TransportError> {
    use std::path::Component;

    let value = path.to_str().ok_or(TransportError::InvalidRequestInput)?;
    #[cfg(windows)]
    let has_dot_segment = value
        .split(|character| character == '/' || character == '\\')
        .any(|segment| matches!(segment, "." | ".."));
    #[cfg(not(windows))]
    let has_dot_segment = value
        .split('/')
        .any(|segment| matches!(segment, "." | ".."));
    if !path.is_absolute()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || has_dot_segment
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(TransportError::InvalidRequestInput);
    }
    Ok(value.to_owned())
}

/// Bounded structured output contract for the first proposed plan. TaskService
/// and SQLite remain authoritative and validate every field again before any
/// PlanRevision or Step is committed.
fn initial_plan_output_schema() -> Value {
    domain_task::initial_plan_output_schema()
}

/// The native initialize response contains machine-local identity details. Validate
/// the bounded shape required by this protocol version. The raw response remains
/// adapter-private RpcResponse data: consumers must not expose, persist, or log
/// these values; in particular `codexHome` is a local path.
fn valid_initialize_result(value: &Value) -> bool {
    fn bounded_string(value: Option<&Value>, maximum: usize) -> bool {
        value.and_then(Value::as_str).is_some_and(|text| {
            !text.is_empty() && text.len() <= maximum && !text.chars().any(char::is_control)
        })
    }
    value.is_object()
        && bounded_string(value.get("codexHome"), 4096)
        && bounded_string(value.get("platformFamily"), 64)
        && bounded_string(value.get("platformOs"), 64)
        && bounded_string(value.get("userAgent"), 256)
}

fn valid_rpc_error(value: &Value) -> bool {
    value.is_object()
        && value.get("code").and_then(Value::as_i64).is_some()
        && value.get("message").is_some_and(Value::is_string)
}

fn valid_message_envelope(value: &Value) -> bool {
    if let Some(method) = value.get("method") {
        method
            .as_str()
            .is_some_and(|method| !method.is_empty() && method.len() <= 256)
            && value.get("result").is_none()
            && value.get("error").is_none()
            && value.get("id").is_none_or(|id| request_key(id).is_ok())
            && value
                .get("params")
                .is_none_or(|params| params.is_object() || params.is_array())
    } else {
        value.get("id").and_then(Value::as_u64).is_some()
            && match (value.get("result"), value.get("error")) {
                (Some(_), None) => true,
                (None, Some(error)) => valid_rpc_error(error),
                _ => false,
            }
    }
}

fn read_message(
    reader: &mut impl BufRead,
    maximum: usize,
    retained: &Arc<RetentionBudget>,
    budget: usize,
    maximum_events: usize,
) -> Result<IncomingMessage, TransportError> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|_| TransportError::Io)?;
        if available.is_empty() {
            return Err(TransportError::Closed);
        }
        let end = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|position| position + 1);
        let take = end.unwrap_or(available.len());
        if line.len().saturating_add(take) > maximum {
            return Err(TransportError::MessageTooLarge);
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if end.is_some() {
            break;
        }
    }
    let bytes = line.len();
    retained
        .events
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current
                .checked_add(1)
                .filter(|total| *total <= maximum_events)
        })
        .map_err(|_| TransportError::QueueFull)?;
    if retained
        .wire_bytes
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            current.checked_add(bytes).filter(|total| *total <= budget)
        })
        .is_err()
    {
        retained.events.fetch_sub(1, Ordering::AcqRel);
        return Err(TransportError::QueueFull);
    }
    let permit = BytePermit {
        retained: Arc::clone(retained),
        bytes,
    };
    let value: Value = serde_json::from_slice(&line).map_err(|_| TransportError::InvalidMessage)?;
    if !value.is_object() {
        return Err(TransportError::InvalidMessage);
    }
    Ok(IncomingMessage { value, permit })
}
struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.limit {
            return Err(std::io::Error::other("message limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
