//! Explicit, short-lived Codex App Server profile probe.
//!
//! This module is not called by installation inventory. Its caller must first
//! authorize the owner-triggered operation and resolve an absolute executable
//! from a Runtime-local admitted endpoint binding. Probe output is a sanitized
//! in-memory projection only: native account/model payloads and local paths are
//! discarded and are never logged or persisted here.

use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;

use super::codex_app_server::{
    AppServerClientInfo, CodexAppServer, NativeEnvironment, NativeEventKind, ProcessObservation,
    RpcId, RpcMethod, TransportError, TransportLimits,
};

const PROBE_TOTAL_BUDGET: Duration = Duration::from_secs(20);
const PROBE_STARTUP_BUDGET: Duration = Duration::from_secs(7);
const PROBE_RPC_BUDGET: Duration = Duration::from_secs(5);
const PROBE_EVENT_POLL: Duration = Duration::from_millis(100);
const PROBE_STOP_BUDGET: Duration = Duration::from_secs(2);
const MODEL_PAGE_LIMIT: u64 = 32;
const MAX_MODEL_OPTIONS: usize = MODEL_PAGE_LIMIT as usize;
const MAX_MODEL_OPTION_TEXT: usize = 128;
const MAX_REASONING_OPTIONS: usize = 16;
const MAX_MODALITIES: usize = 8;

/// Outcome of an owner-triggered probe. `COMPLETE` means only that all bounded
/// read-only observations returned; it does not mean a Codex turn was run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ProbeReadiness {
    Complete,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum Observation {
    Supported,
    Rejected,
    Unknown,
    NotProbed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AuthenticationObservation {
    /// The native response explicitly indicated that provider authentication is
    /// required and no account is configured.
    NeedsAuth,
    /// The native response exposed a recognized account configuration. This is
    /// not proof that a future inference request will succeed.
    Configured,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ModelCatalogObservation {
    Observed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ProbeFailure {
    InvalidAdmission,
    HostUnavailable,
    ProtocolTimeout,
    ProtocolRejected,
    UnsafeServerRequest,
    HostStopUnobserved,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CodexProfileProbe {
    pub agent_id: &'static str,
    pub protocol: &'static str,
    pub readiness: ProbeReadiness,
    pub failure: Option<ProbeFailure>,
    pub protocol_initialize: Observation,
    pub account_read: Observation,
    pub model_list: Observation,
    /// Session execution is deliberately not tested by a profile probe.
    pub session_start: Observation,
    pub session_resume: Observation,
    pub turn_control: Observation,
    pub authentication: AuthenticationObservation,
    pub model_catalog: ModelCatalogObservation,
    /// Always false for this probe: a listed option is not successful inference.
    pub inference_access_verified: bool,
    /// Catalog entries are bounded option metadata, never an entitlement claim.
    pub listed_model_options: Vec<ListedModelOption>,
    pub model_catalog_truncated: bool,
    /// This reports direct child-process reaping only. Writer quiescence stays
    /// unproven because no session/work Environment was launched.
    pub host_process_stopped: bool,
    pub writer_quiescence_proven: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ListedModelOption {
    pub id: String,
    pub display_name: Option<String>,
    pub default_reasoning_effort: Option<String>,
    pub reasoning_efforts: Vec<String>,
    pub input_modalities: Vec<String>,
    pub is_default: Option<bool>,
}

impl CodexProfileProbe {
    fn initial() -> Self {
        Self {
            agent_id: "codex-cli",
            protocol: "CODEX_APP_SERVER",
            readiness: ProbeReadiness::Failed,
            failure: None,
            protocol_initialize: Observation::NotProbed,
            account_read: Observation::NotProbed,
            model_list: Observation::NotProbed,
            session_start: Observation::NotProbed,
            session_resume: Observation::NotProbed,
            turn_control: Observation::NotProbed,
            authentication: AuthenticationObservation::Unknown,
            model_catalog: ModelCatalogObservation::Unknown,
            inference_access_verified: false,
            listed_model_options: Vec::new(),
            model_catalog_truncated: false,
            host_process_stopped: false,
            writer_quiescence_proven: false,
        }
    }
}

/// Starts a fresh app-server process, performs only initialize/account-read/
/// bounded model-list, and requests immediate process shutdown. The caller is
/// responsible for authentication/owner authorization and endpoint admission.
/// No Task, AgentSession, Environment, grant, or provider inference is created.
pub(crate) fn probe_codex_profile(
    executable: &Path,
    cwd: &Path,
    environment: NativeEnvironment,
) -> CodexProfileProbe {
    let mut result = CodexProfileProbe::initial();
    if !executable.is_absolute() || !cwd.is_absolute() {
        result.failure = Some(ProbeFailure::InvalidAdmission);
        return result;
    }

    let limits = TransportLimits {
        max_line_bytes: 512 * 1024,
        max_queued_events: 16,
        max_retained_wire_bytes: 1024 * 1024,
        max_pending_rpcs: 2,
        max_server_requests: 4,
        startup_timeout: PROBE_STARTUP_BUDGET,
        rpc_timeout: PROBE_RPC_BUDGET,
        event_timeout: PROBE_EVENT_POLL,
    };
    let mut server = match CodexAppServer::spawn(executable, cwd, environment, limits) {
        Ok(server) => server,
        Err(TransportError::InvalidMessage) => {
            result.failure = Some(ProbeFailure::InvalidAdmission);
            return result;
        }
        Err(_) => {
            result.failure = Some(ProbeFailure::HostUnavailable);
            return result;
        }
    };
    let lifecycle = server.lifecycle_observation();
    let total_deadline = Instant::now() + PROBE_TOTAL_BUDGET;
    let client = AppServerClientInfo {
        name: "litecowork",
        title: "LiteCowork",
        version: env!("CARGO_PKG_VERSION"),
    };

    let mut fatal = None;
    match server.initialize(client) {
        Ok(id) => match receive_response(&mut server, id, total_deadline) {
            Ok(Response::Success(RpcMethod::Initialize, _native)) => {
                // Drop the private initialize payload (it contains codexHome).
                result.protocol_initialize = Observation::Supported;
            }
            Ok(Response::Rejected(RpcMethod::Initialize)) => {
                result.protocol_initialize = Observation::Rejected;
                fatal = Some(ProbeFailure::ProtocolRejected);
            }
            Ok(_) => fatal = Some(ProbeFailure::ProtocolRejected),
            Err(failure) => fatal = Some(failure),
        },
        Err(_) => fatal = Some(ProbeFailure::ProtocolRejected),
    }

    if fatal.is_none() && Instant::now() < total_deadline {
        match server.account_read() {
            Ok(id) => match receive_response(&mut server, id, total_deadline) {
                Ok(Response::Success(RpcMethod::AccountRead, native)) => {
                    result.account_read = Observation::Supported;
                    result.authentication = authentication_observation(&native);
                    // `native` is dropped here after extracting only an enum.
                }
                Ok(Response::Rejected(RpcMethod::AccountRead)) => {
                    result.account_read = Observation::Rejected;
                }
                Ok(_) => result.account_read = Observation::Unknown,
                Err(failure) => fatal = Some(failure),
            },
            Err(_) => result.account_read = Observation::Unknown,
        }
    }

    // Model discovery is read-only and page bounded. It is not an inference
    // request and its output never proves a durable provider entitlement.
    if fatal.is_none() && Instant::now() < total_deadline {
        match server.model_list() {
            Ok(id) => match receive_response(&mut server, id, total_deadline) {
                Ok(Response::Success(RpcMethod::ModelList, native)) => {
                    result.model_list = Observation::Supported;
                    if let Some((options, truncated)) = model_options(&native) {
                        result.model_catalog = ModelCatalogObservation::Observed;
                        result.listed_model_options = options;
                        result.model_catalog_truncated = truncated;
                    }
                }
                Ok(Response::Rejected(RpcMethod::ModelList)) => {
                    result.model_list = Observation::Rejected;
                }
                Ok(_) => result.model_list = Observation::Unknown,
                Err(failure) => fatal = Some(failure),
            },
            Err(_) => result.model_list = Observation::Unknown,
        }
    }

    if let Some(failure) = fatal {
        result.failure = Some(failure);
    }
    let complete = result.protocol_initialize == Observation::Supported
        && result.account_read == Observation::Supported
        && result.model_list == Observation::Supported
        && result.model_catalog == ModelCatalogObservation::Observed;

    // The profile probe launches no thread/turn, but always closes its host.
    // A direct-child exit is not a descendant-writer/quiescence guarantee.
    let _ = server.stop_host();
    result.host_process_stopped =
        wait_for_direct_exit(&lifecycle, Instant::now() + PROBE_STOP_BUDGET);
    if !result.host_process_stopped {
        result.failure = Some(ProbeFailure::HostStopUnobserved);
    }
    result.readiness = if result.failure.is_some() {
        if result.protocol_initialize == Observation::Supported {
            ProbeReadiness::Partial
        } else {
            ProbeReadiness::Failed
        }
    } else if complete && result.host_process_stopped {
        ProbeReadiness::Complete
    } else {
        ProbeReadiness::Partial
    };
    result
}

enum Response {
    Success(RpcMethod, Value),
    Rejected(RpcMethod),
}

fn receive_response(
    server: &mut CodexAppServer,
    expected: RpcId,
    total_deadline: Instant,
) -> Result<Response, ProbeFailure> {
    let deadline = (Instant::now() + PROBE_RPC_BUDGET).min(total_deadline);
    while Instant::now() < deadline {
        let event = server.poll_event().map_err(map_transport_failure)?;
        let Some(event) = event else { continue };
        match event.kind() {
            NativeEventKind::RpcResponse { id, method, result } if *id == expected => {
                return Ok(Response::Success(*method, result.clone()));
            }
            NativeEventKind::RpcRejected { id, method, .. } if *id == expected => {
                return Ok(Response::Rejected(*method));
            }
            NativeEventKind::ServerRequest(_) => {
                // A profile probe has no grant/approval authority. Never answer
                // an unsolicited request; stop the disposable host instead.
                return Err(ProbeFailure::UnsafeServerRequest);
            }
            NativeEventKind::TerminalObservation { .. } => {
                return Err(ProbeFailure::ProtocolRejected);
            }
            NativeEventKind::Notification(_) => {}
            NativeEventKind::RpcResponse { .. } | NativeEventKind::RpcRejected { .. } => {
                return Err(ProbeFailure::ProtocolRejected);
            }
        }
    }
    Err(ProbeFailure::ProtocolTimeout)
}

fn map_transport_failure(error: TransportError) -> ProbeFailure {
    match error {
        TransportError::Timeout => ProbeFailure::ProtocolTimeout,
        TransportError::SpawnFailed
        | TransportError::PipeUnavailable
        | TransportError::WorkerUnavailable
        | TransportError::Closed
        | TransportError::Io => ProbeFailure::HostUnavailable,
        _ => ProbeFailure::ProtocolRejected,
    }
}

fn wait_for_direct_exit(
    lifecycle: &super::codex_app_server::LifecycleObservation,
    deadline: Instant,
) -> bool {
    while Instant::now() < deadline {
        if lifecycle.snapshot().process == ProcessObservation::Exited {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    lifecycle.snapshot().process == ProcessObservation::Exited
}

fn authentication_observation(result: &Value) -> AuthenticationObservation {
    let Some(object) = result.as_object() else {
        return AuthenticationObservation::Unknown;
    };
    if object.get("account").is_some_and(Value::is_null)
        && object.get("requiresOpenaiAuth") == Some(&Value::Bool(true))
    {
        return AuthenticationObservation::NeedsAuth;
    }
    let Some(account) = object.get("account").and_then(Value::as_object) else {
        return AuthenticationObservation::Unknown;
    };
    let Some(account_type) = account.get("type").and_then(Value::as_str) else {
        return AuthenticationObservation::Unknown;
    };
    // These values are explicitly documented account/auth modes. No identity,
    // plan, email, or credential source is exposed. Bedrock is excluded because
    // account/read does not validate the external AWS credential chain.
    match account_type {
        "apiKey" | "chatgpt" | "chatgptAuthTokens" | "agentIdentity" | "personalAccessToken" => {
            AuthenticationObservation::Configured
        }
        _ => AuthenticationObservation::Unknown,
    }
}

fn model_options(result: &Value) -> Option<(Vec<ListedModelOption>, bool)> {
    let data = result.get("data")?.as_array()?;
    if data.len() > MAX_MODEL_OPTIONS {
        return None;
    }
    let mut options = Vec::with_capacity(data.len());
    for entry in data {
        let id = entry
            .get("id")
            .and_then(safe_identifier)
            .or_else(|| entry.get("model").and_then(safe_identifier))?;
        let display_name = entry.get("displayName").and_then(safe_text);
        let default_reasoning_effort = entry
            .get("defaultReasoningEffort")
            .and_then(safe_identifier);
        let reasoning_efforts = entry
            .get("supportedReasoningEfforts")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .take(MAX_REASONING_OPTIONS)
                    .filter_map(|value| value.get("reasoningEffort").and_then(safe_identifier))
                    .collect()
            })
            .unwrap_or_default();
        let input_modalities = entry
            .get("inputModalities")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .take(MAX_MODALITIES)
                    .filter_map(safe_identifier)
                    .collect()
            })
            .unwrap_or_default();
        let is_default = entry.get("isDefault").and_then(Value::as_bool);
        options.push(ListedModelOption {
            id,
            display_name,
            default_reasoning_effort,
            reasoning_efforts,
            input_modalities,
            is_default,
        });
    }
    let truncated = result
        .get("nextCursor")
        .is_some_and(|cursor| !cursor.is_null());
    Some((options, truncated))
}

fn safe_identifier(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    if value.is_empty()
        || value.len() > MAX_MODEL_OPTION_TEXT
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
    {
        return None;
    }
    Some(value.to_owned())
}

fn safe_text(value: &Value) -> Option<String> {
    let value = value.as_str()?;
    if value.is_empty()
        || value.len() > MAX_MODEL_OPTION_TEXT
        || value.chars().any(char::is_control)
    {
        return None;
    }
    Some(value.to_owned())
}
