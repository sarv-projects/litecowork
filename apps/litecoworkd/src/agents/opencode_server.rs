//! Bounded local OpenCode Server transport.
//!
//! This keeps OpenCode's native harness and configuration intact. LiteCowork
//! launches `opencode serve` on IPv4 loopback with a generated Basic-auth
//! password, then speaks only the stable documented session/message/abort/event API.
//! It is transport only: it does not admit a Task/Attempt, authorize tools,
//! mediate Effects, or establish child-process writer quiescence.
//!
//! API reference: https://opencode.ai/docs/server/ . Server routes and request
//! shapes are taken from the OpenCode Server/SDK documentation. This adapter
//! deliberately does not expose arbitrary URLs, headers, methods, or JSON-RPC.

use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json};

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEALTH_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_PROFILE_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_STARTUP_HEALTH_CHECKS: usize = 16;
const MAX_STARTUP_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_STARTUP_LINE_BYTES: usize = 8 * 1024;
const MAX_REQUEST_BYTES: usize = 512 * 1024;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_SSE_LINE_BYTES: usize = 64 * 1024;
const MAX_SSE_EVENT_BYTES: usize = 256 * 1024;
const MAX_SESSION_ID_BYTES: usize = 128;
const SERVER_USERNAME: &str = "litecowork";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenCodeError {
    InvalidConfiguration,
    RandomUnavailable,
    PortUnavailable,
    SpawnFailed,
    ProcessExited,
    ProcessObservationFailed,
    StartupOutputClosed,
    StartupOutputTooLarge,
    StartupTimeout,
    ConnectFailed,
    Io,
    InvalidResponse,
    InvalidHttpFraming,
    ResponseTooLarge,
    RequestTooLarge,
    Unauthorized,
    HttpStatus(u16),
    InvalidSessionId,
    InvalidMessageId,
    Timeout,
    StreamClosed,
}

impl std::fmt::Display for OpenCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenCode Server: {self:?}")
    }
}
impl std::error::Error for OpenCodeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenCodeProcessState {
    Running,
    Exited,
    ObservationFailed,
}

/// Safe transport configuration. The executable and working directory must be
/// selected by an admitted Runtime-local endpoint binding.
pub(crate) struct OpenCodeServerConfig {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    /// Explicit native-user environment. The daemon environment is never inherited.
    pub environment: OpenCodeEnvironment,
    pub startup_timeout: Duration,
    pub request_timeout: Duration,
}

/// Non-secret process settings required to let OpenCode find its native
/// configuration and helper tools. Credential bytes must not be placed here;
/// OpenCode reads its own user-owned credential store.
#[derive(Default)]
pub(crate) struct OpenCodeEnvironment {
    pub home: Option<OsString>,
    pub user_profile: Option<OsString>,
    pub app_data: Option<OsString>,
    pub local_app_data: Option<OsString>,
    pub xdg_config_home: Option<OsString>,
    pub xdg_data_home: Option<OsString>,
    pub xdg_cache_home: Option<OsString>,
    pub xdg_state_home: Option<OsString>,
    pub path: Option<OsString>,
    pub shell: Option<OsString>,
    pub comspec: Option<OsString>,
    pub pathext: Option<OsString>,
    pub windir: Option<OsString>,
    pub system_root: Option<OsString>,
    pub temporary_directory: Option<OsString>,
    pub lang: Option<OsString>,
    pub lc_all: Option<OsString>,
    pub term: Option<OsString>,
}

impl OpenCodeEnvironment {
    fn is_valid(&self) -> bool {
        const MAX_VALUE_BYTES: usize = 32 * 1024;
        [
            &self.home,
            &self.user_profile,
            &self.app_data,
            &self.local_app_data,
            &self.xdg_config_home,
            &self.xdg_data_home,
            &self.xdg_cache_home,
            &self.xdg_state_home,
            &self.path,
            &self.shell,
            &self.comspec,
            &self.pathext,
            &self.windir,
            &self.system_root,
            &self.temporary_directory,
            &self.lang,
            &self.lc_all,
            &self.term,
        ]
        .into_iter()
        .flatten()
        .all(|value| {
            let value = value.to_string_lossy();
            value.len() <= MAX_VALUE_BYTES && !value.contains('\0')
        })
    }

    fn apply(self, command: &mut Command) {
        command.env_clear();
        for (key, value) in [
            ("HOME", self.home),
            ("USERPROFILE", self.user_profile),
            ("APPDATA", self.app_data),
            ("LOCALAPPDATA", self.local_app_data),
            ("XDG_CONFIG_HOME", self.xdg_config_home),
            ("XDG_DATA_HOME", self.xdg_data_home),
            ("XDG_CACHE_HOME", self.xdg_cache_home),
            ("XDG_STATE_HOME", self.xdg_state_home),
            ("PATH", self.path),
            ("SHELL", self.shell),
            ("COMSPEC", self.comspec),
            ("PATHEXT", self.pathext),
            ("WINDIR", self.windir),
            ("SYSTEMROOT", self.system_root),
            ("TMPDIR", self.temporary_directory.clone()),
            ("TEMP", self.temporary_directory.clone()),
            ("TMP", self.temporary_directory),
            ("LANG", self.lang),
            ("LC_ALL", self.lc_all),
            ("TERM", self.term),
        ] {
            if let Some(value) = value {
                command.env(key, value);
            }
        }
    }
}

impl OpenCodeServerConfig {
    pub(crate) fn bounded(
        executable: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            executable: executable.into(),
            working_directory: working_directory.into(),
            environment: OpenCodeEnvironment::default(),
            startup_timeout: Duration::from_secs(15),
            request_timeout: Duration::from_secs(30),
        }
    }

    pub(crate) fn with_environment(mut self, environment: OpenCodeEnvironment) -> Self {
        self.environment = environment;
        self
    }
}

/// Owns only the direct `opencode serve` process. A stopped server does not
/// prove its agent/tool descendants stopped writing; a future Task supervisor
/// must fence the Environment independently before replacing an Attempt.
pub(crate) struct OpenCodeServer {
    child: Child,
    address: SocketAddr,
    password: Vec<u8>,
    request_timeout: Duration,
    stopped: bool,
}

impl OpenCodeServer {
    /// Launch OpenCode bound strictly to 127.0.0.1. The native OpenCode
    /// configuration/auth state is read by OpenCode itself and never copied.
    pub(crate) fn spawn(config: OpenCodeServerConfig) -> Result<Self, OpenCodeError> {
        if !config.executable.is_absolute()
            || !config.working_directory.is_absolute()
            || !config.environment.is_valid()
            || config.startup_timeout.is_zero()
            || config.startup_timeout > Duration::from_secs(60)
            || config.request_timeout.is_zero()
            || config.request_timeout > Duration::from_secs(60)
        {
            return Err(OpenCodeError::InvalidConfiguration);
        }

        // OpenCode's CLI does not expose socket activation or an inherited
        // listener. Reserve an OS-selected loopback port, then close it before
        // `serve` binds. Before sending the generated credential, require the
        // owned child to emit its native "server listening" line after its
        // bind succeeds. If another local process wins the port race, OpenCode
        // cannot bind and no credential is sent to that process. Unknown CLI
        // startup formats fail closed; this remains a transport qualification
        // requirement, not Task execution or process-tree containment.
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(|_| OpenCodeError::PortUnavailable)?;
        let port = listener
            .local_addr()
            .map_err(|_| OpenCodeError::PortUnavailable)?
            .port();
        drop(listener);

        // 256 bits of generated entropy. The bytes stay in process memory and
        // are used only to authenticate to this loopback server instance.
        let mut password = vec![0_u8; 32];
        getrandom::fill(&mut password).map_err(|_| OpenCodeError::RandomUnavailable)?;
        let password = hex::encode(password).into_bytes();

        let mut command = Command::new(&config.executable);
        config.environment.apply(&mut command);
        command
            .args(["serve", "--hostname", "127.0.0.1", "--port"])
            .arg(port.to_string())
            .current_dir(&config.working_directory)
            .env("OPENCODE_SERVER_USERNAME", SERVER_USERNAME)
            .env(
                "OPENCODE_SERVER_PASSWORD",
                std::str::from_utf8(&password).expect("hex is UTF-8"),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let child = command.spawn().map_err(|_| OpenCodeError::SpawnFailed)?;
        // Install the RAII owner immediately. Every readiness/HTTP early return
        // below drops this value, which kills and waits for the direct child.
        let mut server = Self {
            child,
            address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)),
            password,
            request_timeout: config.request_timeout,
            stopped: false,
        };

        let stdout = server
            .child
            .stdout
            .take()
            .ok_or(OpenCodeError::SpawnFailed)?;
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("litecowork-opencode-startup".to_owned())
            .spawn(move || drain_startup_output(stdout, startup_tx))
            .map_err(|_| OpenCodeError::SpawnFailed)?;

        let deadline = Instant::now() + config.startup_timeout;
        wait_for_bound_server(&mut server, &startup_rx, deadline)?;

        let mut health_checks = 0_usize;
        loop {
            match server.process_state()? {
                OpenCodeProcessState::Running => {}
                OpenCodeProcessState::Exited => return Err(OpenCodeError::ProcessExited),
                OpenCodeProcessState::ObservationFailed => {
                    return Err(OpenCodeError::ProcessObservationFailed);
                }
            }
            // Use the stable Server API surface consistently. `/api/health` is
            // part of OpenCode's experimental V2 API and is not mixed into this
            // legacy `/session` + `/event` adapter.
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(OpenCodeError::StartupTimeout);
            }
            if health_checks >= MAX_STARTUP_HEALTH_CHECKS {
                return Err(OpenCodeError::StartupTimeout);
            }
            health_checks += 1;
            server.request_timeout = config.request_timeout.min(remaining);
            match server.request_with_response_limit(
                "GET",
                "/global/health",
                None,
                "application/json",
                MAX_HEALTH_RESPONSE_BYTES,
            ) {
                Ok(response)
                    if response.status == 200
                        && serde_json::from_slice::<Value>(&response.body)
                            .ok()
                            .and_then(|body| body.get("healthy").and_then(Value::as_bool))
                            == Some(true) =>
                {
                    server.request_timeout = config.request_timeout;
                    return Ok(server);
                }
                Ok(response) if response.status == 401 || response.status == 403 => {
                    return Err(OpenCodeError::Unauthorized);
                }
                Ok(_)
                | Err(OpenCodeError::ConnectFailed | OpenCodeError::Io | OpenCodeError::Timeout) => {
                }
                Err(error) => return Err(error),
            }
            if Instant::now() >= deadline {
                return Err(OpenCodeError::StartupTimeout);
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    pub(crate) fn process_state(&mut self) -> Result<OpenCodeProcessState, OpenCodeError> {
        if self.stopped {
            return Ok(OpenCodeProcessState::Exited);
        }
        match self.child.try_wait() {
            Ok(Some(_)) => Ok(OpenCodeProcessState::Exited),
            Ok(None) => Ok(OpenCodeProcessState::Running),
            Err(_) => Ok(OpenCodeProcessState::ObservationFailed),
        }
    }

    /// Reads the native provider connection summary. This is deliberately
    /// restricted to the documented read-only route; callers must whitelist
    /// fields before exposing any observation outside this module.
    pub(super) fn read_provider_status(&mut self) -> Result<Value, OpenCodeError> {
        let response = self.request_with_response_limit(
            "GET",
            "/provider",
            None,
            "application/json",
            MAX_PROFILE_RESPONSE_BYTES,
        )?;
        response_json(response)
    }

    /// Reads the native configured provider/model catalogue without invoking
    /// a model or changing OpenCode configuration.
    pub(super) fn read_configured_provider_catalog(&mut self) -> Result<Value, OpenCodeError> {
        let response = self.request_with_response_limit(
            "GET",
            "/config/providers",
            None,
            "application/json",
            MAX_PROFILE_RESPONSE_BYTES,
        )?;
        response_json(response)
    }

    /// Creates one native OpenCode session. Session IDs are treated as opaque
    /// values but are validated before they can become URL path segments.
    pub(crate) fn create_session(&mut self, title: Option<&str>) -> Result<String, OpenCodeError> {
        let mut body = serde_json::Map::new();
        if let Some(title) = title {
            if title.len() > 256 || title.chars().any(char::is_control) {
                return Err(OpenCodeError::InvalidConfiguration);
            }
            body.insert("title".to_owned(), Value::String(title.to_owned()));
        }
        let response = self.request(
            "POST",
            "/session",
            Some(Value::Object(body)),
            "application/json",
        )?;
        let value = response_json(response)?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or(OpenCodeError::InvalidResponse)?;
        validate_session_id(id)?;
        Ok(id.to_owned())
    }

    /// Sends a bounded text prompt through the documented session prompt API.
    /// Model selection is an opaque provider/model pair and is omitted unless
    /// an adapter-negotiated caller supplies it.
    pub(crate) fn send_message(
        &mut self,
        session_id: &str,
        text: &str,
        model: Option<(&str, &str)>,
    ) -> Result<Value, OpenCodeError> {
        validate_session_id(session_id)?;
        if text.trim().is_empty() || text.len() > 256 * 1024 || text.chars().any(|c| c == '\0') {
            return Err(OpenCodeError::InvalidConfiguration);
        }
        let mut body = json!({"parts":[{"type":"text","text":text}]});
        if let Some((provider_id, model_id)) = model {
            if !valid_option(provider_id, 128) || !valid_option(model_id, 128) {
                return Err(OpenCodeError::InvalidConfiguration);
            }
            body["model"] = json!({"providerID":provider_id,"modelID":model_id});
        }
        let path = format!("/session/{session_id}/message");
        let response = self.request("POST", &path, Some(body), "application/json")?;
        response_json(response)
    }

    /// Admits one prompt without holding the caller until inference completes.
    /// `message_id` is supplied by the caller for correlation with native events;
    /// this API field is not treated as an idempotency guarantee by LiteCowork.
    pub(crate) fn send_message_async(
        &mut self,
        session_id: &str,
        message_id: &str,
        text: &str,
        model: Option<(&str, &str)>,
    ) -> Result<(), OpenCodeError> {
        validate_session_id(session_id)?;
        if !valid_message_id(message_id) {
            return Err(OpenCodeError::InvalidMessageId);
        }
        if text.trim().is_empty() || text.len() > 256 * 1024 || text.chars().any(|c| c == '\0') {
            return Err(OpenCodeError::InvalidConfiguration);
        }
        let mut body = json!({
            "messageID": message_id,
            "parts": [{"type": "text", "text": text}],
        });
        if let Some((provider_id, model_id)) = model {
            if !valid_option(provider_id, 128) || !valid_option(model_id, 128) {
                return Err(OpenCodeError::InvalidConfiguration);
            }
            body["model"] = json!({"providerID":provider_id,"modelID":model_id});
        }
        let path = format!("/session/{session_id}/prompt_async");
        let response = self.request("POST", &path, Some(body), "application/json")?;
        if response.status == 204 {
            Ok(())
        } else {
            Err(OpenCodeError::InvalidResponse)
        }
    }

    /// Requests cancellation of one native OpenCode session. `true` is only
    /// the Server API acknowledgment; it is not process or writer-quiescence
    /// evidence and must not by itself authorize Attempt replacement.
    pub(crate) fn abort_session(&mut self, session_id: &str) -> Result<bool, OpenCodeError> {
        validate_session_id(session_id)?;
        let path = format!("/session/{session_id}/abort");
        let response = self.request("POST", &path, None, "application/json")?;
        let value = response_json(response)?;
        value.as_bool().ok_or(OpenCodeError::InvalidResponse)
    }

    /// Opens the native server-instance SSE event stream. OpenCode scopes it
    /// to this server's instance; callers must still
    /// correlate session IDs and treat payloads as untrusted observations.
    pub(crate) fn subscribe_events(&mut self) -> Result<OpenCodeEventStream, OpenCodeError> {
        let deadline = Instant::now() + self.request_timeout;
        let stream = connect(self.address, self.request_timeout)?;
        write_request(
            stream,
            self.address,
            "GET",
            "/event",
            &self.authorization(),
            None,
            "text/event-stream",
            true,
            MAX_RESPONSE_BYTES,
            deadline,
        )
        .and_then(|mut response| {
            if response.status == 401 || response.status == 403 {
                return Err(OpenCodeError::Unauthorized);
            }
            if response.status != 200 {
                return Err(OpenCodeError::HttpStatus(response.status));
            }
            let content_type = response
                .headers
                .get("content-type")
                .map(String::as_str)
                .unwrap_or("");
            if !content_type
                .to_ascii_lowercase()
                .starts_with("text/event-stream")
            {
                return Err(OpenCodeError::InvalidResponse);
            }
            Ok(OpenCodeEventStream {
                body: response.body_stream,
                line: Vec::new(),
                event_data: Vec::new(),
                event_type: None,
                event_id: None,
            })
        })
    }

    /// Stops and reaps the owned server process. The returned state covers only
    /// the direct process and does not establish descendant containment.
    pub(crate) fn stop(&mut self) -> OpenCodeProcessState {
        if self.stopped {
            return OpenCodeProcessState::Exited;
        }
        let _ = self.child.kill();
        match self.child.wait() {
            Ok(_) => {
                self.stopped = true;
                OpenCodeProcessState::Exited
            }
            Err(_) => OpenCodeProcessState::ObservationFailed,
        }
    }

    fn request(
        &mut self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
        accept: &str,
    ) -> Result<HttpResponse, OpenCodeError> {
        self.request_with_response_limit(method, path, body, accept, MAX_RESPONSE_BYTES)
    }

    fn request_with_response_limit(
        &mut self,
        method: &'static str,
        path: &str,
        body: Option<Value>,
        accept: &str,
        max_response_bytes: usize,
    ) -> Result<HttpResponse, OpenCodeError> {
        match self.process_state()? {
            OpenCodeProcessState::Running => {}
            OpenCodeProcessState::Exited => return Err(OpenCodeError::ProcessExited),
            OpenCodeProcessState::ObservationFailed => {
                return Err(OpenCodeError::ProcessObservationFailed);
            }
        }
        let bytes = body
            .map(|value| {
                serde_json::to_vec(&value).map_err(|_| OpenCodeError::InvalidConfiguration)
            })
            .transpose()?
            .unwrap_or_default();
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(OpenCodeError::RequestTooLarge);
        }
        let deadline = Instant::now() + self.request_timeout;
        let stream = connect(self.address, self.request_timeout)?;
        let response = write_request(
            stream,
            self.address,
            method,
            path,
            &self.authorization(),
            (!bytes.is_empty()).then_some(bytes.as_slice()),
            accept,
            false,
            max_response_bytes,
            deadline,
        )?;
        if response.status == 401 || response.status == 403 {
            return Err(OpenCodeError::Unauthorized);
        }
        if !(200..300).contains(&response.status) {
            return Err(OpenCodeError::HttpStatus(response.status));
        }
        Ok(response)
    }

    fn authorization(&self) -> String {
        let mut credentials = Vec::with_capacity(SERVER_USERNAME.len() + 1 + self.password.len());
        credentials.extend_from_slice(SERVER_USERNAME.as_bytes());
        credentials.push(b':');
        credentials.extend_from_slice(&self.password);
        format!("Basic {}", BASE64.encode(credentials))
    }
}

/// Wait for evidence from the owned process that its native server bind
/// succeeded. The reader continues draining stdout after readiness so a later
/// diagnostic cannot fill the pipe and stall the server process.
fn wait_for_bound_server(
    server: &mut OpenCodeServer,
    startup: &Receiver<Result<SocketAddr, OpenCodeError>>,
    deadline: Instant,
) -> Result<(), OpenCodeError> {
    loop {
        match server.process_state()? {
            OpenCodeProcessState::Running => {}
            OpenCodeProcessState::Exited => return Err(OpenCodeError::ProcessExited),
            OpenCodeProcessState::ObservationFailed => {
                return Err(OpenCodeError::ProcessObservationFailed);
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(OpenCodeError::StartupTimeout);
        }
        match startup.recv_timeout(remaining.min(Duration::from_millis(40))) {
            Ok(Ok(address)) if address == server.address => return Ok(()),
            Ok(Ok(_)) => return Err(OpenCodeError::InvalidResponse),
            Ok(Err(error)) => return Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(OpenCodeError::StartupOutputClosed);
            }
        }
    }
}

/// Consume the child's bounded startup output. Only a documented listening
/// marker is sent to the startup waiter; all later output is discarded.
fn drain_startup_output<R: Read>(
    mut reader: R,
    startup: SyncSender<Result<SocketAddr, OpenCodeError>>,
) {
    let mut announced = false;
    let mut total = 0_usize;
    let mut line = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) => {
                if !announced {
                    let _ = startup.send(Err(OpenCodeError::StartupOutputClosed));
                }
                return;
            }
            Ok(count) => count,
            Err(_) => {
                if !announced {
                    let _ = startup.send(Err(OpenCodeError::Io));
                }
                return;
            }
        };
        if announced {
            continue;
        }
        for byte in &buffer[..count] {
            total = total.saturating_add(1);
            if total > MAX_STARTUP_OUTPUT_BYTES || line.len() >= MAX_STARTUP_LINE_BYTES {
                let _ = startup.send(Err(OpenCodeError::StartupOutputTooLarge));
                return;
            }
            if *byte == b'\n' {
                if let Some(address) = parse_server_listening_line(&line) {
                    announced = true;
                    let _ = startup.send(Ok(address));
                    line.clear();
                    break;
                }
                line.clear();
            } else if *byte != b'\r' {
                line.push(*byte);
            }
        }
    }
}

fn parse_server_listening_line(line: &[u8]) -> Option<SocketAddr> {
    let line = std::str::from_utf8(line).ok()?;
    let address = line
        .strip_prefix("opencode server listening on ")
        .or_else(|| line.strip_prefix("server listening on "))?;
    let address = address.strip_prefix("http://")?;
    let socket = address.parse::<SocketAddr>().ok()?;
    if socket.ip().is_loopback() && socket.port() != 0 {
        Some(socket)
    } else {
        None
    }
}

impl Drop for OpenCodeServer {
    fn drop(&mut self) {
        let _ = self.stop();
        self.password.fill(0);
    }
}

pub(crate) struct OpenCodeEventStream {
    body: BodyStream,
    line: Vec<u8>,
    event_data: Vec<u8>,
    event_type: Option<String>,
    event_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct OpenCodeEvent {
    pub event_type: Option<String>,
    pub event_id: Option<String>,
    /// Parsed native JSON remains an untrusted, in-memory observation. Do not
    /// log, persist, or treat it as an Effect/Evidence/authorization record.
    pub data: Value,
}

impl OpenCodeEventStream {
    /// Waits up to `timeout` total for one complete SSE event. Heartbeats without
    /// data are skipped. Individual lines and assembled event payloads are
    /// size-limited before JSON parsing.
    pub(crate) fn next_event(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<OpenCodeEvent>, OpenCodeError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(OpenCodeError::InvalidConfiguration);
        }
        let deadline = Instant::now() + timeout;
        self.body.operation_deadline = Some(deadline);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            // Retain a partial line if the socket timeout lands mid-frame;
            // the next call resumes parsing that same SSE line.
            let line_read = match read_body_line(&mut self.body, &mut self.line, MAX_SSE_LINE_BYTES)
            {
                Ok(read) => read,
                Err(OpenCodeError::Timeout) => return Ok(None),
                Err(error) => return Err(error),
            };
            if !line_read {
                return Err(OpenCodeError::StreamClosed);
            }
            if self.line.ends_with(b"\r") {
                self.line.pop();
            }
            if self.line.is_empty() {
                self.line.clear();
                if self.event_data.is_empty() {
                    self.event_type = None;
                    self.event_id = None;
                    continue;
                }
                if self.event_data.last() == Some(&b'\n') {
                    self.event_data.pop();
                }
                let data = serde_json::from_slice(&self.event_data)
                    .map_err(|_| OpenCodeError::InvalidResponse)?;
                self.event_data.clear();
                return Ok(Some(OpenCodeEvent {
                    event_type: self.event_type.take(),
                    event_id: self.event_id.take(),
                    data,
                }));
            }
            if self.line.first() == Some(&b':') {
                self.line.clear();
                continue;
            }
            let Some(colon) = self.line.iter().position(|byte| *byte == b':') else {
                self.line.clear();
                continue;
            };
            let field = &self.line[..colon];
            let value_start = if self.line.get(colon + 1) == Some(&b' ') {
                colon + 2
            } else {
                colon + 1
            };
            let value = &self.line[value_start..];
            match field {
                b"data" => {
                    if self.event_data.len().saturating_add(value.len() + 1) > MAX_SSE_EVENT_BYTES {
                        return Err(OpenCodeError::ResponseTooLarge);
                    }
                    self.event_data.extend_from_slice(value);
                    self.event_data.push(b'\n');
                }
                b"event" => self.event_type = Some(String::from_utf8_lossy(value).into_owned()),
                b"id" if !value.contains(&0) => {
                    self.event_id = Some(String::from_utf8_lossy(value).into_owned())
                }
                _ => {}
            }
            self.line.clear();
        }
    }
}

fn validate_session_id(id: &str) -> Result<(), OpenCodeError> {
    if id.is_empty()
        || id.len() > MAX_SESSION_ID_BYTES
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err(OpenCodeError::InvalidSessionId);
    }
    Ok(())
}

fn valid_message_id(id: &str) -> bool {
    let Some(suffix) = id.strip_prefix("msg_") else {
        return false;
    };
    !suffix.is_empty()
        && id.len() <= MAX_SESSION_ID_BYTES
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_option(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

struct HttpResponse {
    status: u16,
    headers: std::collections::HashMap<String, String>,
    body: Vec<u8>,
    body_stream: BodyStream,
}

struct BodyStream {
    reader: BufReader<TcpStream>,
    chunked: bool,
    content_length: Option<u64>,
    chunk_remaining: usize,
    chunk_terminator_progress: u8,
    chunk_header: Vec<u8>,
    chunk_trailers: bool,
    chunk_trailer_bytes: usize,
    chunk_trailer_count: usize,
    operation_deadline: Option<Instant>,
    finished: bool,
}

impl BodyStream {
    fn read_network(&mut self, output: &mut [u8]) -> Result<usize, OpenCodeError> {
        if let Some(deadline) = self.operation_deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(OpenCodeError::Timeout);
            }
            self.reader
                .get_ref()
                .set_read_timeout(Some(remaining))
                .map_err(|_| OpenCodeError::Io)?;
        }
        self.reader.read(output).map_err(map_read_error)
    }

    fn read_body(&mut self, out: &mut [u8]) -> Result<usize, OpenCodeError> {
        if out.is_empty() || self.finished {
            return Ok(0);
        }
        if self.chunked {
            if self.chunk_remaining == 0 {
                self.consume_chunk_terminator()?;
                if self.read_chunk_header()? == 0 {
                    self.finished = true;
                    return Ok(0);
                }
            }
            let take = out.len().min(self.chunk_remaining);
            let count = self.read_network(&mut out[..take])?;
            if count == 0 {
                return Err(OpenCodeError::InvalidResponse);
            }
            self.chunk_remaining -= count;
            if self.chunk_remaining == 0 {
                self.chunk_terminator_progress = 0;
            }
            return Ok(count);
        }
        if let Some(remaining) = self.content_length {
            if remaining == 0 {
                self.finished = true;
                return Ok(0);
            }
            let take = usize::try_from(remaining.min(out.len() as u64)).unwrap_or(out.len());
            let count = self.read_network(&mut out[..take])?;
            if count == 0 {
                return Err(OpenCodeError::InvalidResponse);
            }
            self.content_length = Some(remaining - count as u64);
            return Ok(count);
        }
        self.read_network(out)
    }

    fn read_chunk_header(&mut self) -> Result<usize, OpenCodeError> {
        loop {
            let mut byte = [0_u8; 1];
            let count = self.read_network(&mut byte)?;
            if count == 0 {
                return Err(OpenCodeError::InvalidResponse);
            }
            if self.chunk_header.len() >= 4096 {
                return Err(OpenCodeError::ResponseTooLarge);
            }
            self.chunk_header.push(byte[0]);
            if byte[0] != b'\n' {
                continue;
            }
            let line = std::mem::take(&mut self.chunk_header);
            if self.chunk_trailers {
                self.chunk_trailer_bytes = self.chunk_trailer_bytes.saturating_add(line.len());
                self.chunk_trailer_count = self.chunk_trailer_count.saturating_add(1);
                if self.chunk_trailer_bytes > 8192 || self.chunk_trailer_count > 32 {
                    return Err(OpenCodeError::ResponseTooLarge);
                }
                if line == b"\r\n" {
                    return Ok(0);
                }
                continue;
            }
            if line.len() > 128 {
                return Err(OpenCodeError::ResponseTooLarge);
            }
            let line = std::str::from_utf8(&line).map_err(|_| OpenCodeError::InvalidResponse)?;
            let digits = line
                .trim()
                .split(';')
                .next()
                .ok_or(OpenCodeError::InvalidResponse)?;
            let size =
                usize::from_str_radix(digits, 16).map_err(|_| OpenCodeError::InvalidResponse)?;
            if size > MAX_RESPONSE_BYTES {
                return Err(OpenCodeError::ResponseTooLarge);
            }
            if size == 0 {
                self.chunk_trailers = true;
                continue;
            }
            self.chunk_remaining = size;
            return Ok(size);
        }
    }

    fn consume_chunk_terminator(&mut self) -> Result<(), OpenCodeError> {
        const TERMINATOR: [u8; 2] = *b"\r\n";
        while self.chunk_terminator_progress < 2 {
            let mut byte = [0_u8; 1];
            let count = self.read_network(&mut byte)?;
            if count == 0 || byte[0] != TERMINATOR[self.chunk_terminator_progress as usize] {
                return Err(OpenCodeError::InvalidResponse);
            }
            self.chunk_terminator_progress += 1;
        }
        self.chunk_terminator_progress = 0;
        Ok(())
    }
}

fn write_request(
    mut stream: TcpStream,
    address: SocketAddr,
    method: &str,
    path: &str,
    authorization: &str,
    body: Option<&[u8]>,
    accept: &str,
    stream_body: bool,
    max_response_bytes: usize,
    deadline: Instant,
) -> Result<HttpResponse, OpenCodeError> {
    if !path.starts_with('/')
        || path.chars().any(|c| matches!(c, '\r' | '\n' | ' '))
        || !valid_option(authorization, 256)
        || max_response_bytes == 0
        || max_response_bytes > MAX_RESPONSE_BYTES
    {
        return Err(OpenCodeError::InvalidConfiguration);
    }
    let body = body.unwrap_or_default();
    if body.len() > MAX_REQUEST_BYTES {
        return Err(OpenCodeError::RequestTooLarge);
    }
    let header = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nAuthorization: {authorization}\r\nAccept: {accept}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|_| OpenCodeError::Io)?;
    if !body.is_empty() {
        stream.write_all(body).map_err(|_| OpenCodeError::Io)?;
    }
    stream.flush().map_err(|_| OpenCodeError::Io)?;

    let mut reader = BufReader::new(stream);
    let mut status_line = Vec::new();
    if !read_bounded_line(&mut reader, &mut status_line, 4096, deadline)? {
        return Err(OpenCodeError::InvalidResponse);
    }
    let status_text =
        std::str::from_utf8(&status_line).map_err(|_| OpenCodeError::InvalidResponse)?;
    let mut parts = status_text.split_whitespace();
    if !parts
        .next()
        .is_some_and(|version| version.starts_with("HTTP/1."))
    {
        return Err(OpenCodeError::InvalidResponse);
    }
    let status = parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(OpenCodeError::InvalidResponse)?;

    let mut headers = std::collections::HashMap::new();
    let mut header_bytes = status_line.len();
    loop {
        let mut line = Vec::new();
        if !read_bounded_line(&mut reader, &mut line, 8192, deadline)? {
            return Err(OpenCodeError::InvalidResponse);
        }
        header_bytes = header_bytes.saturating_add(line.len());
        if header_bytes > MAX_HEADER_BYTES {
            return Err(OpenCodeError::ResponseTooLarge);
        }
        if line == b"\r\n" {
            break;
        }
        let line = std::str::from_utf8(&line).map_err(|_| OpenCodeError::InvalidResponse)?;
        let Some((name, value)) = line.trim_end_matches(['\r', '\n']).split_once(':') else {
            return Err(OpenCodeError::InvalidResponse);
        };
        let key = name.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if key.is_empty() || headers.insert(key, value).is_some() {
            return Err(OpenCodeError::InvalidResponse);
        }
    }
    let transfer_encoding = headers.get("transfer-encoding");
    let content_length = headers
        .get("content-length")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| OpenCodeError::InvalidResponse)
        })
        .transpose()?;
    if transfer_encoding.is_some() && content_length.is_some() {
        return Err(OpenCodeError::InvalidHttpFraming);
    }
    let chunked = match transfer_encoding {
        None => false,
        Some(value) if value.trim().eq_ignore_ascii_case("chunked") => true,
        // This transport implements only a single chunked coding. Reject lists
        // and all other codings instead of treating a trailing `chunked` token
        // as if earlier transfer codings had already been decoded.
        Some(_) => return Err(OpenCodeError::InvalidHttpFraming),
    };
    if content_length.is_some_and(|length| length > max_response_bytes as u64) {
        return Err(OpenCodeError::ResponseTooLarge);
    }
    let mut body_stream = BodyStream {
        reader,
        chunked,
        content_length,
        chunk_remaining: 0,
        chunk_terminator_progress: 2,
        chunk_header: Vec::new(),
        chunk_trailers: false,
        chunk_trailer_bytes: 0,
        chunk_trailer_count: 0,
        operation_deadline: (!stream_body).then_some(deadline),
        finished: false,
    };
    let mut response_body = Vec::new();
    while !stream_body && response_body.len() <= max_response_bytes {
        let remaining = max_response_bytes + 1 - response_body.len();
        let mut buffer = vec![0_u8; remaining.min(16 * 1024)];
        let count = body_stream.read_body(&mut buffer)?;
        if count == 0 {
            break;
        }
        response_body.extend_from_slice(&buffer[..count]);
    }
    if response_body.len() > max_response_bytes {
        return Err(OpenCodeError::ResponseTooLarge);
    }
    Ok(HttpResponse {
        status,
        headers,
        body: response_body,
        body_stream,
    })
}

fn connect(address: SocketAddr, timeout: Duration) -> Result<TcpStream, OpenCodeError> {
    TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => OpenCodeError::Timeout,
            _ => OpenCodeError::ConnectFailed,
        })
        .and_then(|stream| {
            stream
                .set_read_timeout(Some(timeout))
                .map_err(|_| OpenCodeError::Io)?;
            stream
                .set_write_timeout(Some(timeout))
                .map_err(|_| OpenCodeError::Io)?;
            Ok(stream)
        })
}

fn response_json(response: HttpResponse) -> Result<Value, OpenCodeError> {
    serde_json::from_slice(&response.body).map_err(|_| OpenCodeError::InvalidResponse)
}

fn read_body_line(
    body: &mut BodyStream,
    output: &mut Vec<u8>,
    limit: usize,
) -> Result<bool, OpenCodeError> {
    loop {
        let mut byte = [0_u8; 1];
        let count = body.read_body(&mut byte)?;
        if count == 0 {
            return Ok(!output.is_empty());
        }
        if output.len() >= limit {
            return Err(OpenCodeError::ResponseTooLarge);
        }
        if byte[0] == b'\n' {
            return Ok(true);
        }
        output.push(byte[0]);
    }
}

fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    output: &mut Vec<u8>,
    limit: usize,
    deadline: Instant,
) -> Result<bool, OpenCodeError> {
    output.clear();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(OpenCodeError::Timeout);
        }
        reader
            .get_ref()
            .set_read_timeout(Some(remaining))
            .map_err(|_| OpenCodeError::Io)?;
        let available = reader.fill_buf().map_err(map_read_error)?;
        if available.is_empty() {
            return Ok(!output.is_empty());
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if output.len().saturating_add(take) > limit {
            return Err(OpenCodeError::ResponseTooLarge);
        }
        let has_newline = available.get(take.saturating_sub(1)) == Some(&b'\n');
        output.extend_from_slice(&available[..take]);
        reader.consume(take);
        if has_newline {
            return Ok(true);
        }
    }
}

fn map_read_error(error: std::io::Error) -> OpenCodeError {
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => OpenCodeError::Timeout,
        _ => OpenCodeError::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_documented_native_server_listening_markers() {
        assert_eq!(
            parse_server_listening_line(b"opencode server listening on http://127.0.0.1:4096"),
            Some(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 4096))),
        );
        assert_eq!(
            parse_server_listening_line(b"server listening on http://127.0.0.1:4096"),
            Some(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 4096))),
        );
    }

    #[test]
    fn rejects_unexpected_or_non_loopback_startup_addresses() {
        assert_eq!(
            parse_server_listening_line(b"server listening on http://0.0.0.0:4096"),
            None,
        );
        assert_eq!(
            parse_server_listening_line(b"server listening on https://127.0.0.1:4096"),
            None,
        );
        assert_eq!(parse_server_listening_line(b"ready"), None);
    }
}
