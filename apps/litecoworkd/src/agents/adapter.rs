//! The native-agent adapter boundary (001-T04, 002-T17).
//!
//! This module defines the provider-neutral [`AgentAdapter`] trait that the
//! Conversation supervisor drives through the B-03 three-phase handshake, the
//! ACP client spine (`initialize` / `session.new` / `session.prompt` /
//! `session.update` / `session.cancel`, plus the client-side
//! `request_permission`, filesystem and terminal surfaces) described in
//! Feature 002 T17, a per-provider launch catalog that selects a provider by
//! exact identity, and the first concrete adapter: an [`OpenCodeAdapter`] that
//! wraps the existing bounded [`OpenCodeServer`] HTTP transport and the
//! [`OpenCodeEventNormalizer`].
//!
//! Boundaries that do not move (AGENTS.md, SPEC 7.4/8/10):
//! - A native subprocess owns reasoning, authentication, model catalogs and
//!   native tools. LiteCowork stays the Client that owns permission decisions,
//!   filesystem access, terminals and evidence. No ACP surface converts a
//!   native tool action into a LiteCowork Effect.
//! - Provider events are untrusted until normalized and correlated. The adapter
//!   never lets an uncorrelated, foreign-session, oversized or malformed frame
//!   mutate state or projection.
//! - The OpenCode v2 HTTP path is one non-ACP adapter and stays that way until
//!   OpenCode's own ACP route qualifies. The ACP client spine here is the
//!   provider-neutral path the other providers take.
#![allow(
    dead_code,
    reason = "The ACP client spine (002-T17) and the typed failure/launch-catalog \
              surfaces are the normative provider-neutral contract defined by this \
              task; the Conversation supervisor consumes the OpenCode adapter now, \
              while the ACP permission/fs/terminal handlers and launch catalog are \
              consumed by later tasks (001-T27, 002-T18). They are exercised by the \
              module tests."
)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::normalizer::{NormalizedAgentEvent, NormalizerCounters, OpenCodeEventNormalizer};
use super::opencode_server::{OpenCodeError, OpenCodeProcessState, OpenCodeServer};
use storage_core::conversation::ConversationContextPackage;

/// Named, finite stream-shaping values that replace the old hard 60 s read cap
/// and the fixed 500 ms poll. The watchdog is line-resetting: the overall
/// deadline is recomputed on every observed event so a long but active turn is
/// never cut off, while a silent stream is bounded by `idle_timeout`
/// (OPEN-DESIGN-CAPABILITY-DEEP-DIVE-2026-11.md, A-52/R-02).
#[derive(Clone, Copy, Debug)]
pub(crate) struct StreamBudget {
    /// Maximum time to wait for the next event with no activity. Reset on every
    /// observed event. This bounds a quiet stream without bounding a busy one.
    pub idle_timeout: Duration,
    /// Per-read socket timeout handed to the SSE reader. A read that times out
    /// is not an error; it lets the idle watchdog re-check the clock.
    pub poll_interval: Duration,
    /// Absolute ceiling on a single turn regardless of activity. A safety stop
    /// far above any expected turn, not the primary bound.
    pub hard_ceiling: Duration,
    /// After an interrupt is acknowledged, the longest the loop waits to observe
    /// native quiescence before reporting [`TurnObservation::StopUnconfirmed`].
    pub quiescence_timeout: Duration,
}

/// Receives each normalized event of a driven turn, on the drive thread, so transient
/// presentation frames and REPORTED activity can be derived without the adapter knowing
/// about the Operator (001-T13, T25). Implementations must never block: the native read
/// loop calls them inline.
pub(crate) trait TurnObserver: Send {
    fn on_event(&mut self, event: &NormalizedAgentEvent);
    /// Called once when the drive loop ends, whatever the outcome.
    fn finish(&mut self);
}

/// Fans each event out to several observers in order.
pub(crate) struct ObserverSet(pub(crate) Vec<Box<dyn TurnObserver>>);

impl TurnObserver for ObserverSet {
    fn on_event(&mut self, event: &NormalizedAgentEvent) {
        for observer in &mut self.0 {
            observer.on_event(event);
        }
    }

    fn finish(&mut self) {
        for observer in &mut self.0 {
            observer.finish();
        }
    }
}

/// Provider-reported token totals accumulated across a turn's usage observations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReportedUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// True once at least one usage observation carried a token count.
    pub observed: bool,
}

/// Shared handle the supervisor reads after the drive loop ends.
#[derive(Clone, Debug, Default)]
pub(crate) struct UsageCollector(Arc<std::sync::Mutex<ReportedUsage>>);

impl UsageCollector {
    pub(crate) fn snapshot(&self) -> ReportedUsage {
        self.0.lock().map(|usage| *usage).unwrap_or_default()
    }
}

impl TurnObserver for UsageCollector {
    fn on_event(&mut self, event: &NormalizedAgentEvent) {
        if let NormalizedAgentEvent::UsageRecorded {
            input_tokens,
            output_tokens,
        } = event
        {
            if input_tokens.is_none() && output_tokens.is_none() {
                return;
            }
            if let Ok(mut usage) = self.0.lock() {
                usage.observed = true;
                usage.input_tokens = usage.input_tokens.saturating_add(input_tokens.unwrap_or(0));
                usage.output_tokens = usage
                    .output_tokens
                    .saturating_add(output_tokens.unwrap_or(0));
            }
        }
    }

    fn finish(&mut self) {}
}

/// Cooperative per-turn Stop signal shared between the Operator-facing supervisor
/// handle and the blocking drive loop (001-T07). Setting it is a request, not a
/// settlement: only observed quiescence settles a cancelled turn.
#[derive(Clone, Debug, Default)]
pub(crate) struct CancelSignal(Arc<AtomicBool>);

impl CancelSignal {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl Default for StreamBudget {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(120),
            poll_interval: Duration::from_millis(500),
            hard_ceiling: Duration::from_secs(1800),
            quiescence_timeout: Duration::from_secs(15),
        }
    }
}

impl StreamBudget {
    fn is_valid(&self) -> bool {
        !self.idle_timeout.is_zero()
            && !self.poll_interval.is_zero()
            && self.poll_interval <= self.idle_timeout
            && self.idle_timeout <= self.hard_ceiling
            && !self.quiescence_timeout.is_zero()
    }
}

/// Adapter-reported version evidence from phase 1 of the B-03 handshake. This
/// is CODE evidence of the harness build, never a qualification by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdapterVersionInfo {
    pub adapter_version: String,
    pub harness_version: Option<String>,
    pub capabilities: Vec<AdapterCapability>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterCapability {
    Streaming,
    Interrupt,
}

/// The terminal outcome of a driven turn, as observed by the adapter. The
/// adapter never decides durable settlement: it reports what it observed and
/// the supervisor reconciles it through session state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TurnObservation {
    /// The native session reached idle with accumulated answer text. The text
    /// is the committed answer-so-far; reasoning is excluded by construction.
    Completed { answer_text: String },
    /// The native session reported a typed failure (an error frame or a provider
    /// failure). The USER message is kept; there is no fallback (AC-015).
    Failed { reason_code: TurnFailure },
    /// The adapter observed a stop in response to an interrupt. This is a stop
    /// observation, not a durable settlement (AC-010); the supervisor decides.
    Stopped,
    /// A Stop was requested and the interrupt was sent (or could not be sent), but native
    /// quiescence was not observed within the quiescence timeout. The turn must stay
    /// CANCEL_REQUESTED; CANCELLED is never fabricated.
    StopUnconfirmed,
}

/// Closed set of typed adapter failures. No provider string crosses this
/// boundary; only the classification does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TurnFailure {
    /// The pinned executable was missing or did not resolve.
    MissingExecutable,
    /// The resolved build is not an accepted version.
    UnsupportedVersion,
    /// Authentication with the native server failed.
    AuthFailed,
    /// The native server failed to start or become ready.
    StartupFailed,
    /// The native session reported an error during the turn.
    ProviderError,
    /// The stream ended or went idle without producing any answer text.
    EmptyResponse,
    /// The idle/heartbeat watchdog or the hard ceiling bounded a quiet stream.
    Timeout,
    /// A transport/protocol error was observed while reading the stream.
    Transport,
}

impl TurnFailure {
    /// A stable reason code string for the typed FAILED turn. Safe: it is a
    /// fixed classification, never provider text.
    pub(crate) fn reason_code(self) -> &'static str {
        match self {
            Self::MissingExecutable => "ADAPTER_EXECUTABLE_MISSING",
            Self::UnsupportedVersion => "ADAPTER_VERSION_UNSUPPORTED",
            Self::AuthFailed => "ADAPTER_AUTH_FAILED",
            Self::StartupFailed => "ADAPTER_STARTUP_FAILED",
            Self::ProviderError => "AGENT_PROVIDER_ERROR",
            Self::EmptyResponse => "AGENT_EMPTY_RESPONSE",
            Self::Timeout => "AGENT_TURN_TIMEOUT",
            Self::Transport => "AGENT_TRANSPORT_ERROR",
        }
    }
}

/// The provider-neutral native-agent boundary the supervisor drives. The
/// phases follow the accepted B-03 handshake:
/// 1. [`AgentAdapter::probe_version`] - adapter/harness version evidence.
/// 2. [`AgentAdapter::create_session`] - establish the native session and
///    return the opaque native handle for atomic activation.
/// 3. [`AgentAdapter::send_context`] - dispatch the bounded committed context,
///    only after the caller has committed activation.
/// 4. [`AgentAdapter::drive_turn`] - consume the bounded, correlated event
///    stream to a terminal [`TurnObservation`].
/// 5. [`AgentAdapter::interrupt`] / [`AgentAdapter::shutdown`] - stop and reap.
pub(crate) trait AgentAdapter {
    /// Phase 1: version/capability probe.
    fn probe_version(&mut self) -> Result<AdapterVersionInfo, TurnFailure>;

    /// Phase 2: create the native session at the revalidated Workspace location;
    /// returns the opaque native handle.
    fn create_session(&mut self, workspace_directory: &Path) -> Result<String, TurnFailure>;

    /// Phase 3: dispatch the bounded committed context to the native session.
    /// Must be called only after the activation commit (AC-004/AC-005).
    fn send_context(
        &mut self,
        native_session_id: &str,
        native_message_id: &str,
        context: &ConversationContextPackage,
    ) -> Result<(), TurnFailure>;

    /// Phase 4: consume the correlated, bounded event stream to a terminal
    /// observation under the supplied [`StreamBudget`].
    fn drive_turn(
        &mut self,
        native_session_id: &str,
        budget: StreamBudget,
    ) -> Result<TurnObservation, TurnFailure>;

    /// Phase 5a: request a native interrupt. Returns true only when the native
    /// API acknowledged the request; acknowledgment is not quiescence.
    fn interrupt(&mut self, native_session_id: &str) -> Result<bool, TurnFailure>;

    /// Phase 5b: stop and reap the native process. Idempotent.
    /// Stop and reap the per-turn host process. Returns true only when process
    /// exit was observed; callers must keep host state unknown on false.
    fn shutdown(&mut self) -> bool;
}

/// Exact provider identity for the launch catalog. Selection is by this closed
/// identity, never by a brand substring match (AC-013).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdapterKind {
    /// OpenCode over its bounded v2 HTTP transport. This is the one non-ACP
    /// adapter and stays non-ACP until OpenCode's ACP route qualifies.
    OpenCodeHttpV2_0_32,
    /// A generic ACP-client provider launched from the per-provider catalog.
    /// Reserved for Claude Code, Codex and the other framed providers.
    AcpClient(AcpProvider),
}

/// Closed set of ACP providers the launch catalog knows. Each entry maps to an
/// exact executable identity and launch argument vector; none is a brand match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcpProvider {
    ClaudeCode,
    Codex,
}

/// One entry in the per-provider launch catalog: the exact provider identity
/// plus how to launch its ACP subprocess. The catalog never guesses an
/// executable from a brand string; an entry is selected by exact identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LaunchEntry {
    pub provider: AcpProvider,
    /// The exact executable name/identity the resolver must find.
    pub executable_identity: &'static str,
    /// The fixed launch arguments that put the provider into ACP client mode.
    pub launch_args: &'static [&'static str],
    /// The ACP protocol version this launch entry negotiates.
    pub acp_protocol_version: u32,
}

/// The per-provider launch catalog (002-T17). Selection is by exact
/// [`AcpProvider`] identity. OpenCode v2 HTTP is deliberately absent: it is a
/// distinct non-ACP adapter, not an ACP launch target.
pub(crate) fn acp_launch_catalog() -> &'static [LaunchEntry] {
    const CATALOG: &[LaunchEntry] = &[
        LaunchEntry {
            provider: AcpProvider::ClaudeCode,
            executable_identity: "claude-code-acp",
            launch_args: &["--acp"],
            acp_protocol_version: ACP_PROTOCOL_VERSION,
        },
        LaunchEntry {
            provider: AcpProvider::Codex,
            executable_identity: "codex",
            launch_args: &["acp"],
            acp_protocol_version: ACP_PROTOCOL_VERSION,
        },
    ];
    CATALOG
}

/// Select a launch entry by exact provider identity. Returns `None` for a
/// provider the catalog does not define; the caller must fail closed rather
/// than brand-match (AC-013).
pub(crate) fn select_launch_entry(provider: AcpProvider) -> Option<&'static LaunchEntry> {
    acp_launch_catalog()
        .iter()
        .find(|entry| entry.provider == provider)
}

/// The ACP protocol version this client speaks. Pinned in
/// specs/002-native-agent-install-auth-binding/RESEARCH.md.
pub(crate) const ACP_PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// ACP client spine (002-T17)
// ---------------------------------------------------------------------------

/// The ACP JSON-RPC method names the client and the client-side surfaces use.
/// Named here once so no call site hand-writes a method string.
pub(crate) mod acp_method {
    /// Agent-bound request methods (client -> agent).
    pub(crate) const INITIALIZE: &str = "initialize";
    pub(crate) const SESSION_NEW: &str = "session/new";
    pub(crate) const SESSION_PROMPT: &str = "session/prompt";
    pub(crate) const SESSION_CANCEL: &str = "session/cancel";
    /// Agent-originated notification/request methods (agent -> client).
    pub(crate) const SESSION_UPDATE: &str = "session/update";
    pub(crate) const REQUEST_PERMISSION: &str = "session/request_permission";
    pub(crate) const FS_READ_TEXT_FILE: &str = "fs/read_text_file";
    pub(crate) const FS_WRITE_TEXT_FILE: &str = "fs/write_text_file";
    pub(crate) const TERMINAL_CREATE: &str = "terminal/create";
    pub(crate) const TERMINAL_OUTPUT: &str = "terminal/output";
    pub(crate) const TERMINAL_KILL: &str = "terminal/kill";
    pub(crate) const TERMINAL_RELEASE: &str = "terminal/release";
}

/// The client-side surfaces LiteCowork owns as the ACP Client. The agent
/// subprocess calls these back over the connection. Each handler is where
/// LiteCowork keeps ownership of permission, files and terminals; none of them
/// converts a native tool action into a LiteCowork Effect receipt.
pub(crate) trait AcpClientHandler {
    /// Answer a native permission request. The returned decision is sent back
    /// through the harness; LiteCowork records a REPORTED native prompt, never
    /// a grant. `Defer` maps to ACP's explicit cancelled outcome; ACP does not
    /// define a harness default or an implicit unanswered-prompt timeout.
    fn request_permission(&mut self, request: &AcpPermissionRequest) -> AcpPermissionDecision;

    /// Read a text file on the Client's behalf, enforcing the Workspace
    /// boundary. Path traversal outside the Workspace root is refused.
    fn fs_read_text_file(&mut self, path: &str) -> Result<String, AcpClientError>;

    /// Write a text file on the Client's behalf, enforcing the Workspace
    /// boundary. The write is a Client-mediated filesystem operation.
    fn fs_write_text_file(&mut self, path: &str, contents: &str) -> Result<(), AcpClientError>;

    /// Create a terminal the agent can observe. The Client owns the terminal
    /// lifecycle (`create`/`output`/`kill`/`release`).
    fn terminal_create(&mut self, command: &str, args: &[String])
    -> Result<String, AcpClientError>;
    fn terminal_output(&mut self, terminal_id: &str) -> Result<String, AcpClientError>;
    fn terminal_kill(&mut self, terminal_id: &str) -> Result<(), AcpClientError>;
    fn terminal_release(&mut self, terminal_id: &str) -> Result<(), AcpClientError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcpPermissionRequest {
    pub session_id: String,
    pub permission_id: String,
    /// The harness-provided human-readable title. Treated as untrusted display
    /// text, never policy.
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcpPermissionDecision {
    Allow,
    Deny,
    /// No owner answer is available. The ACP adapter must respond with the
    /// protocol's cancelled outcome; it must never imply a default approval.
    Defer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcpClientError {
    /// The path escaped the Workspace boundary.
    OutsideWorkspace,
    /// The operation was refused by the Client.
    Refused,
    /// The underlying operation failed.
    Io,
}

/// A parsed, correlated `session/update` notification from the agent. The raw
/// notification is untrusted; this is the normalized projection the Client acts
/// on. Correlation is on the envelope `sessionId`, never a top-level guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AcpSessionUpdate {
    AgentMessageChunk { text: String },
    AgentThoughtChunk,
    ToolCall { tool: String },
    ToolCallUpdate { status: String },
    Plan { entry_count: usize },
    EndTurn,
    Unknown,
}

/// Correlate and parse a raw ACP `session/update` params object for an expected
/// session id. Returns `None` when the update is for another session (it cannot
/// mutate this session's state, AC-008). The `sessionUpdate` discriminator
/// selects the variant; an unknown discriminator maps to
/// [`AcpSessionUpdate::Unknown`] (counted and dropped by the caller).
pub(crate) fn parse_acp_session_update(
    expected_session_id: &str,
    params: &Value,
) -> Option<AcpSessionUpdate> {
    let session_id = params.get("sessionId").and_then(Value::as_str)?;
    if session_id != expected_session_id {
        return None;
    }
    let update = params.get("update").unwrap_or(params);
    let kind = update.get("sessionUpdate").and_then(Value::as_str)?;
    Some(match kind {
        "agent_message_chunk" => {
            let text = update
                .get("content")
                .and_then(|content| content.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            AcpSessionUpdate::AgentMessageChunk { text }
        }
        "agent_thought_chunk" => AcpSessionUpdate::AgentThoughtChunk,
        "tool_call" => {
            let tool = update
                .get("title")
                .or_else(|| update.get("kind"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            AcpSessionUpdate::ToolCall { tool }
        }
        "tool_call_update" => {
            let status = update
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            AcpSessionUpdate::ToolCallUpdate { status }
        }
        "plan" => {
            let entry_count = update
                .get("entries")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            AcpSessionUpdate::Plan { entry_count }
        }
        _ => AcpSessionUpdate::Unknown,
    })
}

// ---------------------------------------------------------------------------
// OpenCode v2 HTTP adapter (the first, non-ACP adapter)
// ---------------------------------------------------------------------------

/// Adapter over the bounded [`OpenCodeServer`] HTTP transport and the
/// [`OpenCodeEventNormalizer`]. This is the one non-ACP adapter (SPEC 7.4,
/// register B-02 rider). It owns the server for the life of a turn and reaps it
/// on `shutdown`.
pub(crate) struct OpenCodeAdapter {
    server: OpenCodeServer,
    /// Captured once per turn so the normalizer's safe drop counters can be
    /// reported as observability facts after the stream ends.
    last_counters: NormalizerCounters,
    cancel: Option<CancelSignal>,
    observer: Option<Box<dyn TurnObserver>>,
    control_service: Option<Box<dyn FnMut(&mut OpenCodeServer) -> Result<(), TurnFailure> + Send>>,
}

impl OpenCodeAdapter {
    pub(crate) fn new(server: OpenCodeServer) -> Self {
        Self {
            server,
            last_counters: NormalizerCounters::default(),
            cancel: None,
            observer: None,
            control_service: None,
        }
    }

    /// Attach the per-turn Stop signal the drive loop observes.
    pub(crate) fn with_cancel(mut self, cancel: CancelSignal) -> Self {
        self.cancel = Some(cancel);
        self
    }

    /// Attach the per-turn observer that sees every normalized event.
    pub(crate) fn with_observer(mut self, observer: Box<dyn TurnObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Attach the Runtime-owned command service. It runs only on this active
    /// adapter owner, before each native event poll, so HTTP handlers never call
    /// the provider server directly.
    pub(crate) fn with_control_service(
        mut self,
        service: Box<dyn FnMut(&mut OpenCodeServer) -> Result<(), TurnFailure> + Send>,
    ) -> Self {
        self.control_service = Some(service);
        self
    }

    /// The safe normalizer drop counters observed during the last driven turn.
    pub(crate) fn last_counters(&self) -> NormalizerCounters {
        self.last_counters
    }

    fn map_transport_error(error: OpenCodeError) -> TurnFailure {
        match error {
            OpenCodeError::Unauthorized => TurnFailure::AuthFailed,
            OpenCodeError::SpawnFailed
            | OpenCodeError::ProcessExited
            | OpenCodeError::StartupTimeout
            | OpenCodeError::StartupOutputClosed
            | OpenCodeError::StartupOutputTooLarge
            | OpenCodeError::ConnectFailed
            | OpenCodeError::ProcessObservationFailed => TurnFailure::StartupFailed,
            OpenCodeError::Timeout => TurnFailure::Timeout,
            OpenCodeError::StreamClosed => TurnFailure::Transport,
            _ => TurnFailure::Transport,
        }
    }
}

impl AgentAdapter for OpenCodeAdapter {
    fn probe_version(&mut self) -> Result<AdapterVersionInfo, TurnFailure> {
        // The server was spawned for the exact pinned protocol; readiness was
        // already proven during spawn. Report the pinned v2.0.32 route.
        match self.server.process_state() {
            Ok(OpenCodeProcessState::Running) => Ok(AdapterVersionInfo {
                adapter_version: "1.0.0".to_owned(),
                harness_version: Some("2.0.32".to_owned()),
                capabilities: vec![AdapterCapability::Streaming, AdapterCapability::Interrupt],
            }),
            Ok(_) => Err(TurnFailure::StartupFailed),
            Err(error) => Err(Self::map_transport_error(error)),
        }
    }

    fn create_session(&mut self, workspace_directory: &Path) -> Result<String, TurnFailure> {
        // The V2 session API has no title; the transport refuses a title rather
        // than dropping it, so none is supplied.
        self.server
            .create_session(None, workspace_directory)
            .map_err(Self::map_transport_error)
    }

    fn send_context(
        &mut self,
        native_session_id: &str,
        native_message_id: &str,
        context: &ConversationContextPackage,
    ) -> Result<(), TurnFailure> {
        self.server
            .send_conversation_context_async(native_session_id, native_message_id, context)
            .map_err(Self::map_transport_error)
    }

    fn drive_turn(
        &mut self,
        native_session_id: &str,
        budget: StreamBudget,
    ) -> Result<TurnObservation, TurnFailure> {
        if !budget.is_valid() {
            return Err(TurnFailure::Transport);
        }
        let mut stream = self
            .server
            .subscribe_events()
            .map_err(Self::map_transport_error)?;
        let mut normalizer = OpenCodeEventNormalizer::new(native_session_id);
        let cancel = self.cancel.clone();
        let server = std::cell::RefCell::new(&mut self.server);
        let control_service = &mut self.control_service;
        let observer = &mut self.observer;
        let mut service_control = || match control_service.as_mut() {
            Some(service) => service(&mut server.borrow_mut()),
            None => Ok(()),
        };
        let mut interrupt = || {
            server
                .borrow_mut()
                .abort_session(native_session_id)
                .unwrap_or(false)
        };
        let observation = drive_opencode_stream_observed_with_control(
            &mut stream,
            &mut normalizer,
            budget,
            Instant::now(),
            cancel.as_ref(),
            &mut interrupt,
            observer.as_deref_mut().map(|o| o as &mut dyn TurnObserver),
            &mut service_control,
        );
        if let Some(observer) = observer.as_deref_mut() {
            observer.finish();
        }
        self.last_counters = normalizer.counters();
        observation
    }

    fn interrupt(&mut self, native_session_id: &str) -> Result<bool, TurnFailure> {
        self.server
            .abort_session(native_session_id)
            .map_err(Self::map_transport_error)
    }

    fn shutdown(&mut self) -> bool {
        self.server.stop() == OpenCodeProcessState::Exited
    }
}

/// A bounded source of raw SSE events the drive loop pulls from. The live
/// transport ([`OpenCodeEventStream`]) implements it; a fixture replay
/// implements it too, so the watchdog and normalization logic are exercised
/// deterministically offline without a live server.
pub(crate) trait RawEventSource {
    /// Pull the next raw event, waiting at most `timeout`. `Ok(None)` means the
    /// wait elapsed with no complete event (a heartbeat-like quiet), not an end
    /// of stream; the loop's idle watchdog decides when quiet is too long.
    fn next_raw(&mut self, timeout: Duration) -> Result<Option<RawEvent>, OpenCodeError>;
}

/// The minimal raw event shape the drive loop consumes.
#[derive(Clone, Debug)]
pub(crate) struct RawEvent {
    pub event_type: Option<String>,
    pub data: Value,
}

impl RawEventSource for super::opencode_server::OpenCodeEventStream {
    fn next_raw(&mut self, timeout: Duration) -> Result<Option<RawEvent>, OpenCodeError> {
        match self.next_event(timeout)? {
            Some(event) => Ok(Some(RawEvent {
                event_type: event.event_type,
                data: event.data,
            })),
            None => Ok(None),
        }
    }
}

/// The line-resetting idle/heartbeat watchdog loop. Reads normalized events
/// until a terminal observation is reached or the stream goes quiet past the
/// idle timeout (or the hard ceiling trips). Every observed event resets the
/// idle deadline, so an active turn is never cut off; a silent stream is
/// bounded by `idle_timeout` rather than a fixed 60 s cap.
///
/// Accumulation rule: a `TextDelta` appends to the answer; a `TextEnded`
/// replaces the answer with the terminal part text (OpenCode re-sends the full
/// text on the terminal part). Reasoning is never accumulated into the answer.
fn drive_opencode_stream<S: RawEventSource>(
    stream: &mut S,
    normalizer: &mut OpenCodeEventNormalizer,
    budget: StreamBudget,
    started_at: Instant,
) -> Result<TurnObservation, TurnFailure> {
    drive_opencode_stream_cancellable(stream, normalizer, budget, started_at, None, &mut || false)
}

/// The drive loop with a cooperative Stop. When `cancel` is set the loop calls
/// `interrupt` exactly once; an unacknowledged interrupt, or no observed
/// quiescence within `budget.quiescence_timeout`, ends in `StopUnconfirmed`. After an
/// acknowledged interrupt, a final non-empty text part followed by idle preserves
/// completion, a correlated provider error preserves failure, and partial text or a
/// closed stream followed by quiescence is treated as stopped. Live provider ordering
/// remains unqualified until the pinned event sequence is captured.
fn drive_opencode_stream_cancellable<S: RawEventSource>(
    stream: &mut S,
    normalizer: &mut OpenCodeEventNormalizer,
    budget: StreamBudget,
    started_at: Instant,
    cancel: Option<&CancelSignal>,
    interrupt: &mut dyn FnMut() -> bool,
) -> Result<TurnObservation, TurnFailure> {
    drive_opencode_stream_observed(
        stream, normalizer, budget, started_at, cancel, interrupt, None,
    )
}

/// The cancellable drive loop that also feeds every normalized event to `observer`.
fn drive_opencode_stream_observed<S: RawEventSource>(
    stream: &mut S,
    normalizer: &mut OpenCodeEventNormalizer,
    budget: StreamBudget,
    started_at: Instant,
    cancel: Option<&CancelSignal>,
    interrupt: &mut dyn FnMut() -> bool,
    observer: Option<&mut dyn TurnObserver>,
) -> Result<TurnObservation, TurnFailure> {
    drive_opencode_stream_observed_with_control(
        stream,
        normalizer,
        budget,
        started_at,
        cancel,
        interrupt,
        observer,
        &mut || Ok(()),
    )
}

/// Drives the provider stream while giving the Runtime owner one bounded chance to
/// service attempt-scoped controls before each provider poll. The callback must not
/// wait for new commands; it may only drain already-queued work and return its result.
fn drive_opencode_stream_observed_with_control<S: RawEventSource>(
    stream: &mut S,
    normalizer: &mut OpenCodeEventNormalizer,
    budget: StreamBudget,
    started_at: Instant,
    cancel: Option<&CancelSignal>,
    interrupt: &mut dyn FnMut() -> bool,
    mut observer: Option<&mut dyn TurnObserver>,
    service_control: &mut dyn FnMut() -> Result<(), TurnFailure>,
) -> Result<TurnObservation, TurnFailure> {
    let hard_deadline = started_at + budget.hard_ceiling;
    let mut idle_deadline = started_at + budget.idle_timeout;
    let mut quiescence_deadline: Option<Instant> = None;
    let mut answer_text = String::new();
    let mut saw_text = false;
    let mut saw_ended_text = false;

    loop {
        let now = Instant::now();
        if quiescence_deadline.is_none() && cancel.is_some_and(CancelSignal::is_requested) {
            if !interrupt() {
                return Ok(TurnObservation::StopUnconfirmed);
            }
            quiescence_deadline = Some(Instant::now() + budget.quiescence_timeout);
        }
        if let Some(deadline) = quiescence_deadline {
            if now >= deadline {
                return Ok(TurnObservation::StopUnconfirmed);
            }
        } else if now >= hard_deadline || now >= idle_deadline {
            // A quiet stream is bounded here, not by a fixed cap.
            if saw_text && !answer_text.trim().is_empty() {
                // Text was produced but the stream never idled; treat the
                // accumulated answer as complete rather than losing it.
                return Ok(TurnObservation::Completed {
                    answer_text: answer_text.trim().to_owned(),
                });
            }
            return Err(TurnFailure::Timeout);
        }
        let stopping = quiescence_deadline.is_some();

        service_control()?;

        match stream.next_raw(budget.poll_interval) {
            Ok(Some(event)) => {
                // Any observed frame (even a dropped one) is activity: reset the
                // idle watchdog so a busy-but-noisy stream is not cut off.
                idle_deadline = Instant::now() + budget.idle_timeout;
                let event_type = event.event_type.as_deref();
                let normalized = normalizer.normalize(event_type, &event.data);
                if let (Some(observer), Some(normalized)) = (observer.as_deref_mut(), &normalized) {
                    observer.on_event(normalized);
                }
                match normalized {
                    Some(NormalizedAgentEvent::TextDelta { text }) => {
                        saw_text = true;
                        answer_text.push_str(&text);
                    }
                    Some(NormalizedAgentEvent::TextEnded { text }) => {
                        saw_text = true;
                        saw_ended_text = true;
                        answer_text = text;
                    }
                    Some(NormalizedAgentEvent::Idle) => {
                        if stopping {
                            // The current normalizer treats a final text part followed
                            // by the idle boundary as completion. Partial text followed
                            // by idle remains stopped; live provider semantics still
                            // require separate qualification.
                            if saw_ended_text && !answer_text.trim().is_empty() {
                                return Ok(TurnObservation::Completed {
                                    answer_text: answer_text.trim().to_owned(),
                                });
                            }
                            return Ok(TurnObservation::Stopped);
                        }
                        if saw_text && !answer_text.trim().is_empty() {
                            return Ok(TurnObservation::Completed {
                                answer_text: answer_text.trim().to_owned(),
                            });
                        }
                        return Err(TurnFailure::EmptyResponse);
                    }
                    Some(NormalizedAgentEvent::ExecutionSettled {
                        outcome: super::normalizer::ToolOutcome::Failed,
                    }) => {
                        return Err(TurnFailure::ProviderError);
                    }
                    // Reasoning, tool, file, todo, step, usage and other
                    // observations are not accumulated into the answer here;
                    // they are activity observations surfaced by later tasks.
                    _ => {}
                }
            }
            // A poll that yields nothing is not an error; the idle watchdog
            // above decides whether the quiet has lasted too long.
            Ok(None) => {}
            Err(OpenCodeError::StreamClosed) => {
                if stopping {
                    return Ok(TurnObservation::Stopped);
                }
                if saw_text && !answer_text.trim().is_empty() {
                    return Ok(TurnObservation::Completed {
                        answer_text: answer_text.trim().to_owned(),
                    });
                }
                return Err(TurnFailure::EmptyResponse);
            }
            Err(error) => return Err(OpenCodeAdapter::map_transport_error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn launch_catalog_selects_by_exact_identity_never_brand_match() {
        let claude = select_launch_entry(AcpProvider::ClaudeCode).expect("claude entry");
        assert_eq!(claude.executable_identity, "claude-code-acp");
        assert_eq!(claude.acp_protocol_version, ACP_PROTOCOL_VERSION);

        let codex = select_launch_entry(AcpProvider::Codex).expect("codex entry");
        assert_eq!(codex.executable_identity, "codex");
        // OpenCode v2 HTTP is deliberately not an ACP launch target.
        assert!(
            acp_launch_catalog()
                .iter()
                .all(|entry| entry.executable_identity != "opencode")
        );
    }

    #[test]
    fn acp_session_update_correlates_on_envelope_session_id() {
        let params = json!({
            "sessionId": "sess_a",
            "update": {"sessionUpdate": "agent_message_chunk", "content": {"text": "hi"}}
        });
        assert_eq!(
            parse_acp_session_update("sess_a", &params),
            Some(AcpSessionUpdate::AgentMessageChunk {
                text: "hi".to_owned()
            })
        );
        // A foreign session update cannot mutate another session's state.
        assert_eq!(parse_acp_session_update("sess_b", &params), None);
    }

    #[test]
    fn acp_unknown_update_is_distinguished_from_foreign() {
        let params = json!({
            "sessionId": "sess_a",
            "update": {"sessionUpdate": "invented_update"}
        });
        assert_eq!(
            parse_acp_session_update("sess_a", &params),
            Some(AcpSessionUpdate::Unknown)
        );
    }

    #[test]
    fn stream_budget_validity_is_bounded_and_ordered() {
        assert!(StreamBudget::default().is_valid());
        let bad = StreamBudget {
            idle_timeout: Duration::from_secs(1),
            poll_interval: Duration::from_secs(2),
            hard_ceiling: Duration::from_secs(10),
            ..StreamBudget::default()
        };
        assert!(!bad.is_valid());
    }

    #[test]
    fn turn_failures_have_stable_safe_reason_codes() {
        assert_eq!(
            TurnFailure::MissingExecutable.reason_code(),
            "ADAPTER_EXECUTABLE_MISSING"
        );
        assert_eq!(TurnFailure::Timeout.reason_code(), "AGENT_TURN_TIMEOUT");
        assert_eq!(
            TurnFailure::ProviderError.reason_code(),
            "AGENT_PROVIDER_ERROR"
        );
    }
}

/// Offline fixture-replay tests for the adapter drive loop and normalization.
///
/// These exercise the exact stream-driving path `drive_turn` uses, replacing
/// only the live socket with the committed redacted v2.0.32 turn fixture. The
/// module name carries the `selected_adapter_conversation` and
/// `opencode_turn_fixture` filter substrings so the task's `cargo test` filters
/// select them.
#[cfg(test)]
mod selected_adapter_conversation_opencode_turn_fixture_tests {
    use super::*;
    use serde_json::{Value, json};
    use std::collections::VecDeque;

    const FIXTURE: &str = include_str!("fixtures/opencode_v2_0_32_turn.jsonl");
    const FIXTURE_SESSION: &str = "ses_fixture_REDACTED";

    #[test]
    fn runtime_permission_commands_are_serviced_before_each_provider_poll() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        struct ControlObservedSource(Arc<AtomicUsize>);
        impl RawEventSource for ControlObservedSource {
            fn next_raw(&mut self, _timeout: Duration) -> Result<Option<RawEvent>, OpenCodeError> {
                assert!(
                    self.0.load(Ordering::SeqCst) > 0,
                    "Runtime control must be drained before waiting on provider events"
                );
                Err(OpenCodeError::StreamClosed)
            }
        }

        let serviced = Arc::new(AtomicUsize::new(0));
        let mut source = ControlObservedSource(Arc::clone(&serviced));
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let control = Arc::clone(&serviced);
        let mut service_control = move || {
            control.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };

        let result = drive_opencode_stream_observed_with_control(
            &mut source,
            &mut normalizer,
            StreamBudget::default(),
            Instant::now(),
            None,
            &mut || false,
            None,
            &mut service_control,
        );

        assert_eq!(result, Err(TurnFailure::EmptyResponse));
        assert_eq!(serviced.load(Ordering::SeqCst), 1);
    }

    /// Replays committed fixture lines as raw events. A `None`-yielding variant
    /// can be interleaved to simulate heartbeat quiet for the watchdog test.
    struct FixtureSource {
        frames: VecDeque<Option<RawEvent>>,
    }

    impl FixtureSource {
        fn from_fixture() -> Self {
            let frames = FIXTURE
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    let data: Value = serde_json::from_str(line).expect("fixture line is JSON");
                    Some(RawEvent {
                        event_type: None,
                        data,
                    })
                })
                .collect();
            Self { frames }
        }
    }

    impl RawEventSource for FixtureSource {
        fn next_raw(&mut self, _timeout: Duration) -> Result<Option<RawEvent>, OpenCodeError> {
            match self.frames.pop_front() {
                Some(frame) => Ok(frame),
                // Exhausted: behave like a closed stream.
                None => Err(OpenCodeError::StreamClosed),
            }
        }
    }

    #[test]
    fn conversation_turn_delta_fixture_stream_publishes_the_answer_without_reasoning() {
        use crate::transient_frames::{TransientFrameBus, TurnFramePublisher};
        // A private bus keeps this test independent of the process-wide one.
        let bus = TransientFrameBus::for_tests();
        let mut rx = bus.subscribe();
        let mut publisher = TurnFramePublisher::new(bus, "ws", "conv", "turn", 0);
        let mut source = FixtureSource::from_fixture();
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let observation = drive_opencode_stream_observed(
            &mut source,
            &mut normalizer,
            StreamBudget::default(),
            Instant::now(),
            None,
            &mut || false,
            Some(&mut publisher),
        )
        .expect("fixture drives to completion");
        publisher.finish();
        let TurnObservation::Completed { answer_text } = observation else {
            panic!("expected Completed");
        };
        let mut streamed = String::new();
        while let Ok(frame) = rx.try_recv() {
            let value: Value = serde_json::from_slice(&frame.bytes).expect("frame json");
            streamed.push_str(value["text_delta"].as_str().expect("delta text"));
        }
        assert_eq!(
            streamed, answer_text,
            "transient text equals the committed answer"
        );
        assert!(!streamed.contains("REASONING"));
    }

    #[test]
    fn opencode_turn_fixture_replays_deterministically_to_completed() {
        let mut source = FixtureSource::from_fixture();
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let observation = drive_opencode_stream(
            &mut source,
            &mut normalizer,
            StreamBudget::default(),
            Instant::now(),
        )
        .expect("fixture drives to a terminal observation");

        // The idle frame arrives after the terminal text part, so the turn
        // completes with the accumulated answer and no reasoning text.
        match observation {
            TurnObservation::Completed { answer_text } => {
                assert_eq!(answer_text, "I will update the file. Done.");
                assert!(!answer_text.contains("REASONING"));
                assert!(!answer_text.contains("REDACTED"));
            }
            other => panic!("expected Completed, got {other:?}"),
        }

        // The foreign-session text frame and the invented event are dropped and
        // counted; neither mutated the answer.
        let counters = normalizer.counters();
        assert_eq!(counters.foreign_session, 1, "foreign text frame dropped");
        assert_eq!(counters.unknown, 1, "invented event dropped");
    }

    #[test]
    fn opencode_turn_fixture_contains_no_credential_handle_or_prompt_body() {
        // The committed fixture must not carry secrets, handles or prompt text.
        // Exclude the usage-metadata `tokens` field (token counts, not a
        // credential) before scanning for secret-bearing substrings.
        let lowered = FIXTURE
            .to_ascii_lowercase()
            .replace("\"tokens\"", "\"usage\"");
        for forbidden in [
            "authorization",
            "password",
            "bearer",
            "api_key",
            "apikey",
            "access_token",
            "secret",
            "cookie",
        ] {
            assert!(!lowered.contains(forbidden), "fixture leaks `{forbidden}`");
        }
        // Reasoning and error text are redacted placeholders, never content.
        assert!(FIXTURE.contains("[REASONING REDACTED]"));
        assert!(FIXTURE.contains("REDACTED"));
    }

    #[test]
    fn selected_adapter_conversation_bounds_a_quiet_stream_by_idle_timeout() {
        // A source that only ever yields quiet (no complete event) must be
        // bounded by the idle watchdog, not run forever, and must not fabricate
        // an answer.
        struct QuietSource;
        impl RawEventSource for QuietSource {
            fn next_raw(&mut self, _timeout: Duration) -> Result<Option<RawEvent>, OpenCodeError> {
                Ok(None)
            }
        }
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let budget = StreamBudget {
            idle_timeout: Duration::from_millis(30),
            poll_interval: Duration::from_millis(5),
            hard_ceiling: Duration::from_secs(1),
            ..StreamBudget::default()
        };
        let observation =
            drive_opencode_stream(&mut QuietSource, &mut normalizer, budget, Instant::now());
        assert_eq!(observation, Err(TurnFailure::Timeout));
    }

    #[test]
    fn selected_adapter_conversation_text_only_turn_completes_without_idle() {
        // If the stream closes after a terminal text part but before an idle
        // frame, the accumulated answer is still committed, never lost.
        let lines: Vec<&str> = FIXTURE
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take_while(|line| !line.contains("session.idle"))
            .collect();
        let frames = lines
            .into_iter()
            .map(|line| {
                let data: Value = serde_json::from_str(line).expect("json");
                Some(RawEvent {
                    event_type: None,
                    data,
                })
            })
            .collect();
        let mut source = FixtureSource { frames };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let observation = drive_opencode_stream(
            &mut source,
            &mut normalizer,
            StreamBudget::default(),
            Instant::now(),
        )
        .expect("terminal text part yields a completed answer on stream close");
        assert!(matches!(observation, TurnObservation::Completed { .. }));
    }

    #[test]
    fn opencode_event_normalizer_drops_foreign_and_unknown_from_the_fixture() {
        // Drive the fixture to idle and confirm every drop class is counted, so
        // an uncorrelated or foreign-session frame can never mutate state
        // (AC-008). The foreign text frame and the invented event both precede
        // the terminal idle, so the drive observes and drops both.
        let mut source = FixtureSource::from_fixture();
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let _ = drive_opencode_stream(
            &mut source,
            &mut normalizer,
            StreamBudget::default(),
            Instant::now(),
        );
        let counters = normalizer.counters();
        assert_eq!(counters.foreign_session, 1);
        assert_eq!(counters.unknown, 1);
        assert_eq!(counters.uncorrelated, 0);
    }

    /// A source that yields scripted frames, then goes quiet (or closes).
    struct ScriptedSource {
        frames: std::collections::VecDeque<Result<Option<RawEvent>, OpenCodeError>>,
        then_quiet: bool,
    }

    impl RawEventSource for ScriptedSource {
        fn next_raw(&mut self, timeout: Duration) -> Result<Option<RawEvent>, OpenCodeError> {
            match self.frames.pop_front() {
                Some(frame) => frame,
                None if self.then_quiet => {
                    std::thread::sleep(timeout.min(Duration::from_millis(2)));
                    Ok(None)
                }
                None => Err(OpenCodeError::StreamClosed),
            }
        }
    }

    fn idle_frame() -> RawEvent {
        RawEvent {
            event_type: Some("session.idle".to_owned()),
            data: json!({"type": "session.idle", "properties": {"sessionID": FIXTURE_SESSION}}),
        }
    }

    fn stop_budget() -> StreamBudget {
        StreamBudget {
            idle_timeout: Duration::from_millis(500),
            poll_interval: Duration::from_millis(5),
            hard_ceiling: Duration::from_secs(5),
            quiescence_timeout: Duration::from_millis(40),
        }
    }

    #[test]
    fn selected_adapter_conversation_stop_interrupts_once_and_reports_observed_quiescence() {
        let cancel = CancelSignal::new();
        cancel.request();
        let mut source = ScriptedSource {
            frames: [Ok(Some(idle_frame()))].into_iter().collect(),
            then_quiet: false,
        };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let mut interrupts = 0;
        let observation = drive_opencode_stream_cancellable(
            &mut source,
            &mut normalizer,
            stop_budget(),
            Instant::now(),
            Some(&cancel),
            &mut || {
                interrupts += 1;
                true
            },
        );
        assert_eq!(observation, Ok(TurnObservation::Stopped));
        assert_eq!(interrupts, 1, "the interrupt is sent exactly once");
    }

    #[test]
    fn selected_adapter_conversation_stop_without_quiescence_is_unconfirmed_not_stopped() {
        let cancel = CancelSignal::new();
        cancel.request();
        let mut source = ScriptedSource {
            frames: std::collections::VecDeque::new(),
            then_quiet: true,
        };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let observation = drive_opencode_stream_cancellable(
            &mut source,
            &mut normalizer,
            stop_budget(),
            Instant::now(),
            Some(&cancel),
            &mut || true,
        );
        assert_eq!(observation, Ok(TurnObservation::StopUnconfirmed));
    }

    #[test]
    fn selected_adapter_conversation_unacknowledged_interrupt_is_unconfirmed() {
        let cancel = CancelSignal::new();
        cancel.request();
        let mut source = ScriptedSource {
            frames: std::collections::VecDeque::new(),
            then_quiet: true,
        };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let observation = drive_opencode_stream_cancellable(
            &mut source,
            &mut normalizer,
            stop_budget(),
            Instant::now(),
            Some(&cancel),
            &mut || false,
        );
        assert_eq!(observation, Ok(TurnObservation::StopUnconfirmed));
    }

    #[test]
    fn selected_adapter_conversation_completion_before_the_stop_request_stays_completed() {
        // The terminal text and idle arrive before the Stop is requested: the race is
        // won by the completion and the interrupt is never sent.
        let cancel = CancelSignal::new();
        let mut frames = std::collections::VecDeque::new();
        frames.push_back(Ok(Some(RawEvent {
            event_type: Some("message.part.updated".to_owned()),
            data: json!({"type": "message.part.updated", "properties": {"part": {
                "type": "text", "sessionID": FIXTURE_SESSION, "messageID": "m", "id": "p",
                "text": "done", "time": {"start": 1, "end": 2}}}}),
        })));
        frames.push_back(Ok(Some(idle_frame())));
        let mut source = ScriptedSource {
            frames,
            then_quiet: false,
        };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let mut interrupts = 0;
        let observation = drive_opencode_stream_cancellable(
            &mut source,
            &mut normalizer,
            stop_budget(),
            Instant::now(),
            Some(&cancel),
            &mut || {
                interrupts += 1;
                true
            },
        );
        assert_eq!(
            observation,
            Ok(TurnObservation::Completed {
                answer_text: "done".to_owned()
            })
        );
        assert_eq!(interrupts, 0);
    }

    #[test]
    fn selected_adapter_conversation_completion_observed_during_stop_wins_the_race() {
        // The stop request has already been sent, but the native stream then reports
        // a completed text part followed by session.idle. The observed answer is the
        // winning outcome; quiescence alone must not relabel it as Stopped.
        let cancel = CancelSignal::new();
        cancel.request();
        let mut frames = std::collections::VecDeque::new();
        frames.push_back(Ok(Some(RawEvent {
            event_type: Some("message.part.updated".to_owned()),
            data: json!({"type": "message.part.updated", "properties": {"part": {
                "type": "text", "sessionID": FIXTURE_SESSION, "messageID": "m", "id": "p",
                "text": "completed during stop", "time": {"start": 1, "end": 2}}}}),
        })));
        frames.push_back(Ok(Some(idle_frame())));
        let mut source = ScriptedSource {
            frames,
            then_quiet: false,
        };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let mut interrupts = 0;
        let observation = drive_opencode_stream_cancellable(
            &mut source,
            &mut normalizer,
            stop_budget(),
            Instant::now(),
            Some(&cancel),
            &mut || {
                interrupts += 1;
                true
            },
        );
        assert_eq!(
            observation,
            Ok(TurnObservation::Completed {
                answer_text: "completed during stop".to_owned()
            })
        );
        assert_eq!(interrupts, 1, "the interrupt was sent exactly once");
    }

    #[test]
    fn selected_adapter_conversation_provider_failure_observed_during_stop_wins_the_race() {
        let cancel = CancelSignal::new();
        cancel.request();
        let mut source = ScriptedSource {
            frames: [Ok(Some(RawEvent {
                event_type: Some("session.error".to_owned()),
                data: json!({"type": "session.error", "properties": {
                    "sessionID": FIXTURE_SESSION, "error": {"name": "ProviderError"}
                }}),
            }))]
            .into_iter()
            .collect(),
            then_quiet: false,
        };
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        let observation = drive_opencode_stream_cancellable(
            &mut source,
            &mut normalizer,
            stop_budget(),
            Instant::now(),
            Some(&cancel),
            &mut || true,
        );
        assert_eq!(observation, Err(TurnFailure::ProviderError));
    }

    #[test]
    fn opencode_turn_fixture_error_frame_maps_to_execution_failed() {
        // The committed fixture carries an error event (001-T23). Normalize that
        // exact line directly to confirm it classifies as a typed failure.
        let error_line = FIXTURE
            .lines()
            .find(|line| line.contains("session.error"))
            .expect("fixture carries a session.error frame");
        let data: Value = serde_json::from_str(error_line).expect("json");
        let mut normalizer = OpenCodeEventNormalizer::new(FIXTURE_SESSION);
        assert_eq!(
            normalizer.normalize(None, &data),
            Some(NormalizedAgentEvent::ExecutionSettled {
                outcome: crate::agents::normalizer::ToolOutcome::Failed
            })
        );
    }
}
