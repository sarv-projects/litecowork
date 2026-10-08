//! Runtime-local discovery of installed native agent executables.
//!
//! An installed CLI is not yet a negotiated AgentProfile and is never sufficient
//! evidence of authentication or session readiness. Protocol adapters fill those
//! fields only after a scoped, side-effect-bounded handshake.

mod codex;
mod codex_app_server;
mod codex_profile_probe;
mod opencode;
mod opencode_profile_probe;
mod opencode_server;
mod planning;

// Explicit profile probing is intentionally kept separate from the bounded
// executable inventory above. Only an authenticated owner-triggered caller may
// invoke this operation with a Runtime-admitted endpoint binding.
pub(crate) use codex_profile_probe::{
    AuthenticationObservation, CodexProfileProbe, ListedModelOption,
    ModelCatalogObservation, Observation, ProbeFailure, ProbeReadiness,
    probe_codex_profile,
};
pub(crate) use codex_app_server::NativeEnvironment;
pub(crate) use opencode_server::{
    OpenCodeError, OpenCodeEvent, OpenCodeEventStream, OpenCodeProcessState,
    OpenCodeEnvironment, OpenCodeServer, OpenCodeServerConfig,
};
pub(crate) use opencode_profile_probe::{
    OpenCodeProbeReadiness, opencode_native_environment, probe_opencode_profile,
};

use serde::Serialize;
use std::sync::{Mutex, OnceLock};

const INVENTORY_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);
static INVENTORY_CACHE: OnceLock<Mutex<Option<(std::time::Instant, Vec<AgentInstallation>)>>> =
    OnceLock::new();

#[derive(Clone, Debug, Serialize)]
pub struct AgentInstallation {
    pub agent_id: &'static str,
    pub display_name: &'static str,
    pub protocol_candidate: &'static str,
    pub installation: InstallationState,
    pub version: Option<String>,
    pub authentication: AuthenticationReadiness,
    pub session_readiness: SessionReadiness,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstallationState {
    Missing,
    Installed,
    VersionUnavailable,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthenticationReadiness {
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionReadiness {
    NotProbed,
}

pub fn discover_local_installations() -> Vec<AgentInstallation> {
    let cache = INVENTORY_CACHE.get_or_init(|| Mutex::new(None));
    let mut cache = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((observed_at, entries)) = cache.as_ref()
        && observed_at.elapsed() < INVENTORY_CACHE_TTL
    {
        return entries.clone();
    }

    let entries = std::thread::scope(|scope| {
        let codex = scope.spawn(codex::discover);
        let opencode = scope.spawn(opencode::discover);
        let codex = codex.join().unwrap_or_else(|_| codex::unavailable());
        let opencode = opencode.join().unwrap_or_else(|_| opencode::unavailable());
        vec![codex, opencode]
    });
    *cache = Some((std::time::Instant::now(), entries.clone()));
    entries
}

fn discover_version(binary: &str, expected_prefix: &str) -> (InstallationState, Option<String>) {
    use std::{
        io::Read,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    let mut child = match Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (InstallationState::Missing, None);
        }
        Err(_) => return (InstallationState::VersionUnavailable, None),
    };
    let Some(mut stdout) = child.stdout.take() else {
        return (InstallationState::VersionUnavailable, None);
    };
    let (output_sender, output_receiver) = mpsc::sync_channel(1);
    let _reader = thread::spawn(move || {
        let mut output = Vec::with_capacity(128);
        let _ = stdout.by_ref().take(257).read_to_end(&mut output);
        let _ = output_sender.send(output);
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return (InstallationState::VersionUnavailable, None);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return (InstallationState::VersionUnavailable, None);
            }
        }
    };
    let Ok(stdout) = output_receiver.recv_timeout(Duration::from_millis(100)) else {
        return (InstallationState::VersionUnavailable, None);
    };
    if !status.is_some_and(|status| status.success()) {
        return (InstallationState::VersionUnavailable, None);
    }

    // CLI output is untrusted. Accept only a small version-shaped suffix, never
    // return arbitrary stdout/stderr through the authenticated Operator API.
    let stdout = String::from_utf8_lossy(&stdout);
    let version = stdout
        .lines()
        .next()
        .and_then(|line| line.strip_prefix(expected_prefix))
        .map(str::trim)
        .filter(|candidate| is_version_shaped(candidate))
        .map(str::to_owned);
    match version {
        Some(version) => (InstallationState::Installed, Some(version)),
        None => (InstallationState::VersionUnavailable, None),
    }
}

fn is_version_shaped(candidate: &str) -> bool {
    let candidate = candidate.strip_prefix('v').unwrap_or(candidate);
    if candidate.is_empty() || candidate.len() > 48 {
        return false;
    }
    let core_end = candidate
        .find(|character| character == '-' || character == '+')
        .unwrap_or(candidate.len());
    let core = &candidate[..core_end];
    let segments: Vec<_> = core.split('.').collect();
    if segments.len() != 3
        || segments.iter().any(|segment| {
            segment.is_empty()
                || !segment.bytes().all(|byte| byte.is_ascii_digit())
                || (segment.len() > 1 && segment.starts_with('0'))
        })
    {
        return false;
    }
    if core_end == candidate.len() {
        return true;
    }
    let suffix = &candidate[core_end + 1..];
    !suffix.is_empty()
        && suffix.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

// Pure storage-backed preflight; never starts an unrestricted native provider.
pub(crate) use planning::{
    LocalPlanningPreflight, LocalPlanningPreflightView, PlanningDispatchBlocker,
    prepare_local_task_planning,
};
