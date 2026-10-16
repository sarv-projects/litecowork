//! Runtime-local discovery of installed native agent executables.
//!
//! An installed CLI is not yet a negotiated AgentProfile and is never sufficient
//! evidence of authentication or session readiness. Protocol adapters fill those
//! fields only after a scoped, side-effect-bounded handshake.

mod cline;
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
pub(crate) use codex_app_server::NativeEnvironment;
pub(crate) use codex_profile_probe::{
    RuntimeOfferAssessment, codex_runtime_offer_assessment, probe_codex_profile,
};
pub(crate) use opencode_profile_probe::{
    OpenCodeProbeReadiness, opencode_native_environment, probe_opencode_profile,
};
pub(crate) use opencode_server::{
    OpenCodeEnvironment, OpenCodeError, OpenCodeProcessState, OpenCodeServer, OpenCodeServerConfig,
    OpenCodeServerProtocol,
};

use serde::Serialize;
use std::sync::{Mutex, OnceLock};

const INVENTORY_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);
type CachedInventory = Option<(std::time::Instant, Vec<AgentInstallation>)>;
static INVENTORY_CACHE: OnceLock<Mutex<CachedInventory>> = OnceLock::new();

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
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((observed_at, entries)) = cache.as_ref()
        && observed_at.elapsed() < INVENTORY_CACHE_TTL
    {
        return entries.clone();
    }

    let entries = std::thread::scope(|scope| {
        let codex = scope.spawn(codex::discover);
        let opencode = scope.spawn(opencode::discover);
        let cline = scope.spawn(cline::discover);
        let codex = codex.join().unwrap_or_else(|_| codex::unavailable());
        let opencode = opencode.join().unwrap_or_else(|_| opencode::unavailable());
        let cline = cline.join().unwrap_or_else(|_| cline::unavailable());
        vec![codex, opencode, cline]
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

    // Inventory discovery is intentionally not a native-auth operation. Keep
    // the CLI's helper lookup path (needed by launchers such as Codex's
    // `#!/usr/bin/env node` shim), but do not forward credentials, proxy
    // settings, or other arbitrary daemon environment variables.
    let mut command = Command::new(binary);
    command.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }

    let mut child = match command
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
    let core_end = candidate.find(['-', '+']).unwrap_or(candidate.len());
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
pub(crate) use planning::{PlanningDispatchBlocker, prepare_local_task_planning};

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    const CHILD_TEST_MARKER: &str = "LITECOWORK_VERSION_PROBE_TEST_CHILD";
    const SENTINEL_ENV: &str = "LITECOWORK_VERSION_PROBE_SENTINEL";

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "litecowork-agent-version-probe-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temporary test directory");
            Self(path)
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write fake CLI helper");
        let mut permissions = fs::metadata(path)
            .expect("read fake CLI helper metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("make fake CLI helper executable");
    }

    #[test]
    fn version_probe_clears_daemon_secrets_and_keeps_path_for_codex_node_shim() {
        if std::env::var_os(CHILD_TEST_MARKER).is_some() {
            let (state, version) = discover_version("codex", "codex-cli ");
            assert!(matches!(state, InstallationState::Installed));
            assert_eq!(version.as_deref(), Some("7.8.9"));
            return;
        }

        let temporary = TemporaryDirectory::new();
        let bin = temporary.0.join("bin");
        fs::create_dir_all(&bin).expect("create fake CLI directory");
        write_executable(&bin.join("codex"), "#!/usr/bin/env node\n");
        write_executable(
            &bin.join("node"),
            &format!(
                "#!/bin/sh\nif [ \"${{{SENTINEL_ENV}-}}\" = \"test-only-secret-marker\" ]; then exit 71; fi\nprintf '%s\\n' 'codex-cli 7.8.9' 'raw-native-output'\n"
            ),
        );

        let test_binary = std::env::current_exe().expect("current test executable");
        let output = Command::new(test_binary)
            .args([
                "--exact",
                "agents::tests::version_probe_clears_daemon_secrets_and_keeps_path_for_codex_node_shim",
                "--nocapture",
            ])
            .env_clear()
            .env("PATH", &bin)
            .env(CHILD_TEST_MARKER, "1")
            .env(SENTINEL_ENV, "test-only-secret-marker")
            .output()
            .expect("run isolated test child");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "isolated test child failed: stdout={stdout:?}, stderr={stderr:?}"
        );
        assert!(
            stdout.contains("running 1 test"),
            "unexpected child output: {stdout:?}"
        );
        assert!(!stdout.contains("test-only-secret-marker"));
        assert!(!stderr.contains("test-only-secret-marker"));
        assert!(!stdout.contains("raw-native-output"));
        assert!(!stderr.contains("raw-native-output"));
    }
}
