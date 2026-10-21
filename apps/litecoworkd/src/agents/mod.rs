//! Runtime-local discovery of installed native agent executables.
//!
//! An installed CLI is not yet a negotiated AgentProfile and is never sufficient
//! evidence of authentication or session readiness. Protocol adapters fill those
//! fields only after a scoped, side-effect-bounded handshake.

pub(crate) mod acp_registry;
mod cline;
mod codex;
mod codex_app_server;
mod codex_profile_probe;
pub(crate) mod control_descriptor;
pub(crate) mod module_registry;
mod native_cli;
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
        let native = scope.spawn(native_cli::discover_all);
        let codex = codex.join().unwrap_or_else(|_| codex::unavailable());
        let opencode = opencode.join().unwrap_or_else(|_| opencode::unavailable());
        let cline = cline.join().unwrap_or_else(|_| cline::unavailable());
        let native = native.join().unwrap_or_else(|_| {
            native_cli::INVENTORY_CANDIDATES
                .into_iter()
                .map(native_cli::unavailable)
                .collect()
        });
        [vec![codex, opencode, cline], native].concat()
    });
    *cache = Some((std::time::Instant::now(), entries.clone()));
    entries
}

fn discover_version(binary: &str, expected_prefix: &str) -> (InstallationState, Option<String>) {
    discover_version_with_interpreters(
        binary,
        &["--version"],
        expected_prefix,
        false,
        std::time::Duration::from_secs(2),
        &["node"],
    )
}

fn discover_version_with_args(
    binary: &str,
    version_args: &[&str],
    expected_prefix: &str,
    trailing_period: bool,
    timeout: std::time::Duration,
) -> (InstallationState, Option<String>) {
    discover_version_with_interpreters(
        binary,
        version_args,
        expected_prefix,
        trailing_period,
        timeout,
        &[],
    )
}

fn discover_version_with_interpreters(
    binary: &str,
    version_args: &[&str],
    expected_prefix: &str,
    trailing_period: bool,
    timeout: std::time::Duration,
    allowed_interpreters: &[&str],
) -> (InstallationState, Option<String>) {
    use std::{
        io::Read,
        thread,
        time::{Duration, Instant},
    };

    // Inventory discovery is intentionally not a native-auth operation. Keep
    // the CLI's helper lookup path (needed by launchers such as Codex's
    // `#!/usr/bin/env node` shim), but do not forward credentials, proxy
    // settings, or other arbitrary daemon environment variables.
    #[cfg(unix)]
    let (binary, pinned_path): (String, Option<std::ffi::OsString>) =
        match resolve_version_probe_target(binary, allowed_interpreters) {
            Ok(target) => (target.executable, target.pinned_path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return (InstallationState::Missing, None);
            }
            Err(_) => return (InstallationState::VersionUnavailable, None),
        };
    #[cfg(not(unix))]
    let (binary, pinned_path): (String, Option<std::ffi::OsString>) = (binary.to_owned(), None);

    let mut child = match spawn_version_process(&binary, version_args, pinned_path.as_deref()) {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (InstallationState::Missing, None);
        }
        Err(_) => return (InstallationState::VersionUnavailable, None),
    };

    // On Windows the child remains suspended and inside its Job Object until
    // this transition. Start the execution deadline immediately before resume.
    let started_at = Instant::now();
    if child.resume().is_err() {
        return (InstallationState::VersionUnavailable, None);
    }
    let deadline = started_at + timeout;

    #[cfg(unix)]
    {
        use std::io::ErrorKind;

        let Some(mut stdout) = child.take_stdout().ok().flatten() else {
            let _ = child.kill_tree();
            let _ = child.wait();
            return (InstallationState::VersionUnavailable, None);
        };
        let flags = match rustix::fs::fcntl_getfl(&stdout) {
            Ok(flags) => flags,
            Err(_) => {
                let _ = child.kill_tree();
                let _ = child.wait();
                return (InstallationState::VersionUnavailable, None);
            }
        };
        if rustix::fs::fcntl_setfl(&stdout, flags | rustix::fs::OFlags::NONBLOCK).is_err() {
            let _ = child.kill_tree();
            let _ = child.wait();
            return (InstallationState::VersionUnavailable, None);
        }

        let mut output = Vec::with_capacity(128);
        let mut read_failed = false;
        let status = loop {
            while output.len() < 257 {
                let mut buffer = [0_u8; 257];
                let buffer = &mut buffer[..257 - output.len()];
                match stdout.read(buffer) {
                    Ok(0) => break,
                    Ok(read) => output.extend_from_slice(&buffer[..read]),
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        read_failed = true;
                        break;
                    }
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) | Err(_) => break None,
            }
        };

        // The child can write its version and exit between the last nonblocking
        // read and the status check above, so the pipe can still hold the output
        // at that moment. Projecting the version without draining it would report
        // an installed, working CLI as "version unavailable" purely because of
        // scheduling. The process has already exited here, so its write end is
        // closed and the remaining bytes are available immediately; the bounded
        // read keeps the same output limit as the polling loop.
        if status.is_some() {
            while output.len() < 257 {
                let mut buffer = [0_u8; 257];
                let buffer = &mut buffer[..257 - output.len()];
                match stdout.read(buffer) {
                    Ok(0) => break,
                    Ok(read) => output.extend_from_slice(&buffer[..read]),
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        read_failed = true;
                        break;
                    }
                }
            }
        }

        // A Unix process group is a best-effort cleanup scope. A hostile
        // descendant can escape with setsid, so never block on EOF here; the
        // nonblocking pipe keeps escaped descendants from hanging discovery.
        let _ = child.kill_tree();
        if status.is_none() {
            let _ = child.wait();
        }
        drop(stdout);
        drop(child);
        if read_failed || !status.is_some_and(|status| status.success()) {
            return (InstallationState::VersionUnavailable, None);
        }

        let version = project_version_output(&output, expected_prefix, trailing_period);
        return match version {
            Some(version) => (InstallationState::Installed, Some(version)),
            None => (InstallationState::VersionUnavailable, None),
        };
    }

    #[cfg(not(unix))]
    {
        let Some(mut stdout) = child.take_stdout().ok().flatten() else {
            return (InstallationState::VersionUnavailable, None);
        };

        // Windows Job Objects terminate the whole assigned process tree, so a
        // bounded reader may wait for EOF after cleanup.
        let reader = thread::Builder::new()
            .name("agent-version-output".to_owned())
            .spawn(move || {
                let mut output = Vec::with_capacity(128);
                let _ = stdout.by_ref().take(257).read_to_end(&mut output);
                output
            });
        let Ok(reader) = reader else {
            let _ = child.kill_tree();
            let _ = child.wait();
            return (InstallationState::VersionUnavailable, None);
        };

        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) | Err(_) => break None,
            }
        };

        let _ = child.kill_tree();
        if status.is_none() {
            let _ = child.wait();
        }
        drop(child);

        let Ok(stdout) = reader.join() else {
            return (InstallationState::VersionUnavailable, None);
        };
        if !status.is_some_and(|status| status.success()) {
            return (InstallationState::VersionUnavailable, None);
        }

        // CLI output is untrusted. Accept only a small version-shaped suffix, never
        // return arbitrary stdout/stderr through the authenticated Operator API.
        let version = project_version_output(&stdout, expected_prefix, trailing_period);
        match version {
            Some(version) => (InstallationState::Installed, Some(version)),
            None => (InstallationState::VersionUnavailable, None),
        }
    }
}

fn spawn_version_process(
    binary: &str,
    version_args: &[&str],
    pinned_path: Option<&std::ffi::OsStr>,
) -> std::io::Result<AgentChild> {
    use std::process::{Command, Stdio};

    fn command(
        binary: &str,
        version_args: &[&str],
        pinned_path: Option<&std::ffi::OsStr>,
    ) -> Command {
        let mut command = Command::new(binary);
        command.env_clear();
        let path = pinned_path
            .map(std::ffi::OsStr::to_owned)
            .or_else(|| std::env::var_os("PATH"));
        if let Some(path) = path {
            command.env("PATH", path);
        }
        #[cfg(windows)]
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        command
            // Never let an inventory-only version probe inherit a Workspace,
            // project, or arbitrary daemon working directory. Some native
            // CLIs inspect repository configuration even for version queries.
            .current_dir(version_probe_working_directory())
            .args(version_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command
    }

    match AgentChild::spawn(command(binary, version_args, pinned_path)) {
        #[cfg(windows)]
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::path::Path::new(binary).extension().is_none() =>
        {
            // Native npm installations expose `.cmd` shims. Pass the module's
            // fixed version args directly; never synthesize a `cmd /c` string.
            // The contained helper starts the shim host suspended and assigns
            // it to the Job before any command-script code can run.
            AgentChild::spawn(command(&format!("{binary}.cmd"), version_args, pinned_path))
        }
        result => result,
    }
}

#[cfg(unix)]
struct VersionProbeTarget {
    executable: String,
    pinned_path: Option<std::ffi::OsString>,
}

#[cfg(unix)]
fn resolve_version_probe_target(
    binary: &str,
    allowed_interpreters: &[&str],
) -> std::io::Result<VersionProbeTarget> {
    use std::{
        fs::{self, File},
        io::Read,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    fn resolve_executable(path: &Path) -> std::io::Result<PathBuf> {
        let canonical = fs::canonicalize(path)?;
        let metadata = fs::metadata(&canonical)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "version probe candidate is not an executable file",
            ));
        }
        Ok(canonical)
    }

    fn find_on_path(name: &str) -> std::io::Result<PathBuf> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        // A PATH entry that exists but cannot be executed is skipped, exactly like
        // the shell's own lookup order. Otherwise an unrelated non-executable file
        // would shadow the CLI that would actually run and report an unusable
        // version for a perfectly installable agent. The first unusable entry is
        // still remembered so a Workspace with no executable candidate keeps
        // reporting the more precise "installed but not runnable" state.
        let mut unusable = None;
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(name);
            if !candidate.exists() {
                continue;
            }
            match resolve_executable(&candidate) {
                Ok(resolved) => return Ok(resolved),
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    unusable.get_or_insert(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(unusable.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "version probe candidate was not found on PATH",
            )
        }))
    }

    let candidate = Path::new(binary);
    let candidate_path = if candidate.components().count() > 1 {
        resolve_executable(candidate)?
    } else {
        find_on_path(binary)?
    };
    let mut header = [0_u8; 4096];
    let read = File::open(&candidate_path)?.read(&mut header)?;
    let header = &header[..read];
    let Some(first_line) = header.split(|byte| *byte == b'\n').next() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "version probe candidate has no executable header",
        ));
    };
    let mut pinned_path = None;
    if first_line.starts_with(b"#!") {
        let line = std::str::from_utf8(&first_line[2..]).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "version probe launcher has a non-UTF-8 shebang",
            )
        })?;
        if first_line.len() > 255 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "version probe launcher shebang is too long",
            ));
        }
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        let (interpreter_name, interpreter_path) = match fields.as_slice() {
            [absolute] if absolute.starts_with('/') => {
                let path = Path::new(absolute);
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid shebang interpreter",
                        )
                    })?;
                (name, resolve_executable(path)?)
            }
            [env_path, name]
                if matches!(*env_path, "/usr/bin/env" | "/bin/env") && !name.starts_with('-') =>
            {
                let env_interpreter = Path::new(env_path);
                let _validated_env = resolve_executable(env_interpreter)?;
                let path = find_on_path(name)?;
                (*name, path)
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "launcher shebang is not an explicitly supported interpreter form",
                ));
            }
        };
        if !allowed_interpreters.contains(&interpreter_name) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "launcher interpreter is not allowlisted for this AgentModule",
            ));
        }
        if fields.first().is_some_and(|field| field.ends_with("/env")) {
            // Pin env's lookup to the exact interpreter directory validated above,
            // while retaining the original PATH for ordinary helper commands.
            let mut paths = vec![
                interpreter_path
                    .parent()
                    .unwrap_or_else(|| Path::new("/"))
                    .to_path_buf(),
            ];
            if let Some(original) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&original));
            }
            pinned_path = Some(std::env::join_paths(paths).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid PATH")
            })?);
        }
    }
    Ok(VersionProbeTarget {
        executable: candidate_path.to_string_lossy().into_owned(),
        pinned_path,
    })
}

fn version_probe_working_directory() -> std::path::PathBuf {
    #[cfg(unix)]
    {
        std::path::PathBuf::from("/")
    }
    #[cfg(windows)]
    {
        std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .map(|system_root| system_root.join("Temp"))
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows\Temp"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::path::PathBuf::new()
    }
}

pub(crate) enum AgentChild {
    #[cfg(unix)]
    Unix(std::process::Child),
    #[cfg(windows)]
    Windows(windows_local_security::ContainedChild),
}

impl AgentChild {
    pub(crate) fn spawn(command: std::process::Command) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let mut command = command;
            command.process_group(0);
            command.spawn().map(Self::Unix)
        }
        #[cfg(windows)]
        {
            windows_local_security::spawn_kill_on_drop_job(command).map(Self::Windows)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = command;
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "native agent processes are unsupported on this platform",
            ))
        }
    }

    pub(crate) fn id(&self) -> u32 {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => child.id(),
            #[cfg(windows)]
            Self::Windows(child) => child.id(),
        }
    }

    pub(crate) fn resume(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(_) => Ok(()),
            #[cfg(windows)]
            Self::Windows(child) => child.resume(),
        }
    }

    pub(crate) fn take_stdout(&mut self) -> std::io::Result<Option<std::process::ChildStdout>> {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => Ok(child.stdout.take()),
            #[cfg(windows)]
            Self::Windows(child) => child.take_stdout(),
        }
    }

    pub(crate) fn take_stdin(&mut self) -> std::io::Result<Option<std::process::ChildStdin>> {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => Ok(child.stdin.take()),
            #[cfg(windows)]
            Self::Windows(child) => child.take_stdin(),
        }
    }

    pub(crate) fn take_stderr(&mut self) -> std::io::Result<Option<std::process::ChildStderr>> {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => Ok(child.stderr.take()),
            #[cfg(windows)]
            Self::Windows(child) => child.take_stderr(),
        }
    }

    pub(crate) fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => child.try_wait(),
            #[cfg(windows)]
            Self::Windows(child) => child.try_wait(),
        }
    }

    pub(crate) fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => child.wait(),
            #[cfg(windows)]
            Self::Windows(child) => child.wait(),
        }
    }

    pub(crate) fn kill_tree(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(child) => {
                let process_group =
                    rustix::process::Pid::from_raw(child.id() as rustix::process::RawPid)
                        .ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "version probe process ID is invalid",
                            )
                        })?;
                rustix::process::kill_process_group(process_group, rustix::process::Signal::KILL)
                    .map_err(|error| std::io::Error::from_raw_os_error(error.raw_os_error()))
            }
            #[cfg(windows)]
            Self::Windows(child) => child.kill_tree(),
        }
    }
}

#[cfg(unix)]
impl Drop for AgentChild {
    fn drop(&mut self) {
        let _ = self.kill_tree();
        let Self::Unix(child) = self;
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

fn project_version_output(
    stdout: &[u8],
    expected_prefix: &str,
    trailing_period: bool,
) -> Option<String> {
    let output = String::from_utf8_lossy(stdout);
    let version = output.lines().next()?.strip_prefix(expected_prefix)?.trim();
    let version = if trailing_period {
        version.strip_suffix('.').unwrap_or(version)
    } else {
        version
    };
    is_version_shaped(version).then(|| version.to_owned())
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
            // The isolated probe cases run concurrently inside one test process.
            // A clock-derived name alone can collide when two threads sample the
            // same instant, which would let one case overwrite the other's
            // fixture. A per-process counter keeps every case disjoint.
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let ordinal = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "litecowork-agent-version-probe-{}-{unique}-{ordinal}",
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

    fn shell_quote(path: &Path) -> String {
        format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
    }

    #[test]
    fn inventory_does_not_execute_a_launcher_without_a_module_interpreter_policy() {
        let temporary = TemporaryDirectory::new();
        let binary = temporary.0.join("unqualified-launcher");
        let marker = temporary.0.join("launched");
        write_executable(
            &binary,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '1.2.3'\n: > {}\n",
                shell_quote(&marker)
            ),
        );

        let (installation, version) = discover_version_with_args(
            &binary.to_string_lossy(),
            &["--version"],
            "",
            false,
            std::time::Duration::from_secs(2),
        );

        assert!(matches!(
            installation,
            InstallationState::VersionUnavailable
        ));
        assert_eq!(version, None);
        assert!(!marker.exists(), "unqualified launcher code was executed");
    }

    #[test]
    fn launcher_interpreter_is_checked_against_module_policy_before_execution() {
        let temporary = TemporaryDirectory::new();
        let allowed_launcher = temporary.0.join("allowed-launcher");
        let allowed_marker = temporary.0.join("allowed-launched");
        write_executable(
            &allowed_launcher,
            &format!(
                "#!/usr/bin/env sh\nprintf '%s\\n' '1.2.3'\n: > {}\n",
                shell_quote(&allowed_marker)
            ),
        );
        let (installation, version) = discover_version_with_interpreters(
            &allowed_launcher.to_string_lossy(),
            &["--version"],
            "",
            false,
            std::time::Duration::from_secs(2),
            &["sh"],
        );
        assert!(matches!(installation, InstallationState::Installed));
        assert_eq!(version.as_deref(), Some("1.2.3"));
        assert!(allowed_marker.exists());

        let denied_launcher = temporary.0.join("denied-launcher");
        let denied_marker = temporary.0.join("denied-launched");
        write_executable(
            &denied_launcher,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' '1.2.3'\n: > {}\n",
                shell_quote(&denied_marker)
            ),
        );
        let (installation, version) = discover_version_with_interpreters(
            &denied_launcher.to_string_lossy(),
            &["--version"],
            "",
            false,
            std::time::Duration::from_secs(2),
            &["node"],
        );
        assert!(matches!(
            installation,
            InstallationState::VersionUnavailable
        ));
        assert_eq!(version, None);
        assert!(!denied_marker.exists(), "disallowed interpreter ran");
    }

    #[test]
    fn env_split_and_malformed_shebangs_fail_closed() {
        let temporary = TemporaryDirectory::new();
        for (name, shebang) in [
            ("split", "#!/usr/bin/env -S sh -c"),
            ("relative", "#!sh"),
            ("nonstandard-env", "#!/usr/bin/../bin/env sh"),
        ] {
            let binary = temporary.0.join(name);
            let marker = temporary.0.join(format!("{name}-launched"));
            write_executable(
                &binary,
                &format!(
                    "{shebang}\nprintf '%s\\n' '1.2.3'\n: > {}\n",
                    shell_quote(&marker)
                ),
            );
            let (installation, version) = discover_version_with_interpreters(
                &binary.to_string_lossy(),
                &["--version"],
                "",
                false,
                std::time::Duration::from_secs(2),
                &["sh"],
            );
            assert!(matches!(
                installation,
                InstallationState::VersionUnavailable
            ));
            assert_eq!(version, None);
            assert!(!marker.exists(), "malformed launcher {name} executed");
        }

        let native = resolve_version_probe_target(
            &std::env::current_exe()
                .expect("test binary path")
                .to_string_lossy(),
            &[],
        )
        .expect("a native executable needs no script interpreter policy");
        assert!(native.pinned_path.is_none());
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

    #[test]
    fn version_probe_kills_background_launcher_children_that_hold_stdout() {
        let temporary = TemporaryDirectory::new();
        let binary = temporary.0.join("launcher-with-background-child");
        write_executable(&binary, "#!/bin/sh\n(sleep 5) &\nprintf '%s\\n' '1.2.3'\n");

        let started_at = std::time::Instant::now();
        let (installation, version) = discover_version_with_interpreters(
            &binary.to_string_lossy(),
            &["--version"],
            "",
            false,
            std::time::Duration::from_secs(2),
            &["sh"],
        );

        assert!(matches!(installation, InstallationState::Installed));
        assert_eq!(version.as_deref(), Some("1.2.3"));
        assert!(
            started_at.elapsed() < std::time::Duration::from_secs(2),
            "a launcher descendant holding stdout must not extend the probe timeout"
        );
    }

    #[test]
    fn version_probe_keeps_the_output_of_a_cli_that_exits_immediately() {
        // A CLI that writes its version and exits can finish between the probe's
        // nonblocking read and its exit check. Without draining the pipe the
        // probe would report an installed, working CLI as "version unavailable"
        // purely because of scheduling. Repeating the probe makes that race
        // observable on a loaded host.
        let temporary = TemporaryDirectory::new();
        let binary = temporary.0.join("fast-exiting-version");
        write_executable(&binary, "#!/bin/sh\nprintf '%s\\n' '1.2.3'\nexit 0\n");

        for _ in 0..32 {
            let (installation, version) = discover_version_with_interpreters(
                &binary.to_string_lossy(),
                &["--version"],
                "",
                false,
                std::time::Duration::from_secs(5),
                &["sh"],
            );
            assert!(
                matches!(installation, InstallationState::Installed),
                "a fast-exiting CLI must not be reported as version unavailable"
            );
            assert_eq!(version.as_deref(), Some("1.2.3"));
        }
    }

    #[test]
    fn version_probe_uses_a_non_workspace_working_directory() {        let temporary = TemporaryDirectory::new();
        let binary = temporary.0.join("cwd-sensitive-version");
        let expected_cwd = version_probe_working_directory();
        let expected_cwd = expected_cwd.to_string_lossy().replace('\'', "'\\''");
        write_executable(
            &binary,
            &format!(
                "#!/bin/sh\nif [ \"$PWD\" = '{expected_cwd}' ]; then printf '%s\\n' '1.2.3'; else exit 7; fi\n"
            ),
        );

        let (installation, version) = discover_version_with_interpreters(
            &binary.to_string_lossy(),
            &["--version"],
            "",
            false,
            std::time::Duration::from_secs(2),
            &["sh"],
        );

        assert!(matches!(installation, InstallationState::Installed));
        assert_eq!(version.as_deref(), Some("1.2.3"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn version_probe_timeout_does_not_wait_for_a_descendant_that_escaped_its_process_group() {
        let temporary = TemporaryDirectory::new();
        let binary = temporary.0.join("launcher-with-session-escape");
        let escaped_pid_path = temporary.0.join("escaped-pid");
        write_executable(
            &binary,
            &format!(
                "#!/bin/sh\nsetsid sh -c 'echo $$ > \"$1\"; sleep 5' sh '{}' &\nprintf '%s\\n' '1.2.3'\n",
                escaped_pid_path.display()
            ),
        );

        let started_at = std::time::Instant::now();
        let (installation, version) = discover_version_with_interpreters(
            &binary.to_string_lossy(),
            &["--version"],
            "",
            false,
            std::time::Duration::from_secs(2),
            &["sh"],
        );
        let elapsed = started_at.elapsed();

        let cleanup_deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
        while !escaped_pid_path.exists() && std::time::Instant::now() < cleanup_deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if let Ok(pid) = fs::read_to_string(&escaped_pid_path)
            && let Ok(pid) = pid.trim().parse::<u32>()
            && let Some(group) = rustix::process::Pid::from_raw(pid as rustix::process::RawPid)
        {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
        assert!(matches!(installation, InstallationState::Installed));
        assert_eq!(version.as_deref(), Some("1.2.3"));
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "a descendant that escaped the launcher's process group must not block the probe"
        );
    }

    #[test]
    fn local_inventory_reports_native_cli_candidates_without_claiming_auth_or_session_readiness() {
        let inventory = discover_local_installations();
        for (agent_id, display_name) in [
            ("claude-code", "Claude Code"),
            ("gemini-cli", "Gemini CLI"),
            ("qwen-code", "Qwen Code"),
            ("cursor-cli", "Cursor CLI"),
            ("kiro-cli", "Kiro CLI"),
            ("goose", "Goose"),
            ("mistral-vibe", "Mistral Vibe"),
            ("jetbrains-junie-cli", "JetBrains Junie CLI"),
            ("github-copilot-cli", "GitHub Copilot CLI"),
            ("kimi-code-cli", "Kimi Code CLI"),
            ("factory-droid", "Factory Droid"),
            ("auggie-cli", "Auggie CLI"),
        ] {
            let row = inventory
                .iter()
                .find(|entry| entry.agent_id == agent_id)
                .unwrap_or_else(|| panic!("inventory omitted {agent_id}"));
            assert_eq!(row.display_name, display_name);
            assert_eq!(row.protocol_candidate, "CLI");
            assert!(matches!(
                row.authentication,
                AuthenticationReadiness::Unknown
            ));
            assert!(matches!(row.session_readiness, SessionReadiness::NotProbed));
            assert!(matches!(
                row.installation,
                InstallationState::Missing
                    | InstallationState::Installed
                    | InstallationState::VersionUnavailable
            ));
        }
    }

    #[test]
    fn native_version_projection_accepts_vendor_prefix_without_returning_other_output() {
        let output = "GitHub Copilot CLI 1.0.78.\nRun 'copilot update' to check for updates.\n";
        let version = project_version_output(output.as_bytes(), "GitHub Copilot CLI ", true);

        assert_eq!(version.as_deref(), Some("1.0.78"));
    }
}

#[cfg(all(test, windows))]
mod windows_cli_tests {
    use super::{InstallationState, discover_version_with_args};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "litecowork-windows-agent-version-probe-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temporary Windows probe directory");
            Self(path)
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn version_probe_uses_fixed_arguments_and_the_os_owned_cwd_for_cmd_shims() {
        let temporary = TemporaryDirectory::new();
        let binary_without_extension = temporary.0.join("codex");
        let shim = temporary.0.join("codex.cmd");
        fs::write(
            &shim,
            "@echo off\r\nif not \"%~1\"==\"--version\" exit /b 91\r\nif /I not \"%CD%\"==\"%SystemRoot%\\Temp\" exit /b 92\r\necho codex-cli 7.8.9\r\n",
        )
        .expect("write fixed version-only cmd shim");

        let (installation, version) = discover_version_with_args(
            &binary_without_extension.to_string_lossy(),
            &["--version"],
            "codex-cli ",
            false,
            std::time::Duration::from_secs(2),
        );

        assert!(matches!(installation, InstallationState::Installed));
        assert_eq!(version.as_deref(), Some("7.8.9"));
    }
}
