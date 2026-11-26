//! Linux filesystem/network namespace launch layered over the cgroup process scope.
//!
//! This is a real OS-enforced worker boundary, but deliberately not a complete
//! Environment provider: it has no egress broker, Runtime-local attestation,
//! Task/Attempt admission, or Trust/Effect/lease reconciliation. Writable output,
//! home, and temporary files share one bounded sandbox-private tmpfs; no host output
//! directory is exposed to the worker.

use crate::{
    AgentEnvironment, LaunchSpec, ManagedScope, ProcessScopeIdentity, ScopeError, SystemdCommands,
    spawn,
};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    process::{ChildStderr, ChildStdin, ChildStdout, ExitStatus},
    time::Duration,
};

const INPUT_TARGET: &str = "/workspace/inputs";
const OUTPUT_TARGET: &str = "/workspace/outputs";
const WORKSPACE_TARGET: &str = "/workspace";
const TMP_TARGET: &str = "/tmp";
const MAX_RUNTIME_MOUNTS: usize = 32;
pub const MAX_SCRATCH_BYTES: u64 = 512 * 1024 * 1024;
const SYSTEM_RUNTIME_ROOTS: [&str; 6] = ["/usr", "/bin", "/sbin", "/lib", "/lib64", "/opt"];

/// Canonical executable, validated arguments, output directory, resource limits, and
/// Attempt ID produced for a Bubblewrap launch.
pub type PreparedBubblewrapCommand = (
    PathBuf,
    Vec<OsString>,
    PathBuf,
    crate::ResourceLimits,
    [u8; 16],
);

/// Trusted host runtime tree exposed read-only at one explicit sandbox path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyRuntimeMount {
    pub source: PathBuf,
    pub target: PathBuf,
}

/// Opt-in handoff for one bounded Attempt proposal. The worker still writes only to its
/// private `/workspace/outputs/proposal`; the trusted scope-gate runner reads it later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateCaptureSpec {
    pub max_bytes: usize,
    /// Keep the trusted PID 1 runner idle until the Runtime releases the scope after its
    /// durable admission receipt has been observed.
    pub start_after_admission: bool,
}

/// Input to the qualified Linux launch path. `process.executable` and
/// `process.environment` are host-side adapter values: the executable must reside under
/// one `runtime_mounts` source, while environment paths must name sandbox paths. The
/// process working directory is ignored and replaced with the fixed output directory.
#[derive(Clone, Debug)]
pub struct BubblewrapLaunchSpec {
    pub bubblewrap: PathBuf,
    pub process: LaunchSpec,
    pub input_root: PathBuf,
    pub output_root: PathBuf,
    /// Aggregate writable scratch budget for the sandbox-private `/workspace`
    /// tmpfs. The host output directory is only the trusted bootstrap cwd; it is
    /// never mounted into the agent's namespace.
    pub scratch_limit_bytes: u64,
    /// `None` preserves the ordinary provider launch. Capture mode requires the
    /// exact packaged scope-gate executable also used by `SystemdCommands`.
    pub candidate_capture: Option<CandidateCaptureSpec>,
    pub runtime_mounts: Vec<ReadOnlyRuntimeMount>,
}

/// An already placed, attested-capable scope whose trusted PID 1 is waiting for one
/// post-receipt signal before it starts the native harness. Dropping this value does not
/// stop the underlying process scope; callers must retain it until quiescence is proven.
#[must_use = "dropping a held Environment loses retry ownership of its process scope"]
pub struct HeldEnvironmentScope {
    scope: ManagedScope,
    admission_signal: AdmissionSignalState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionSignalState {
    Waiting,
    Attempted,
    Sent,
}

fn write_admission_signal(
    state: &mut AdmissionSignalState,
    output: &mut impl Write,
) -> Result<(), ScopeError> {
    if *state != AdmissionSignalState::Waiting {
        return Err(ScopeError::InvalidInput(
            "held Environment launch signal was already attempted",
        ));
    }
    *state = AdmissionSignalState::Attempted;
    output
        .write_all(crate::admission_protocol::START_AFTER_RECEIPT)
        .and_then(|()| output.flush())
        .map_err(ScopeError::Io)?;
    *state = AdmissionSignalState::Sent;
    Ok(())
}

impl HeldEnvironmentScope {
    pub fn unit_name(&self) -> &str {
        self.scope.unit_name()
    }

    /// Capture the exact held process and cgroup configuration for the private Runtime
    /// preparation journal. This is operational identity evidence, not attestation.
    pub fn capture_process_scope_identity(&self) -> Result<ProcessScopeIdentity, ScopeError> {
        self.scope.capture_process_scope_identity()
    }

    /// Recheck process-start identity, cgroup membership and configured limits immediately
    /// before the admission transaction. Any mismatch leaves the held scope unavailable.
    pub fn recheck_process_scope_identity(
        &self,
        expected: &ProcessScopeIdentity,
    ) -> Result<(), ScopeError> {
        self.scope.recheck_process_scope_identity(expected)
    }

    /// Send the exact one-shot signal after the owning Runtime has resolved and verified
    /// the durable admission receipt. A write error is ambiguous and cannot be retried.
    pub fn start_after_admission(&mut self) -> Result<(), ScopeError> {
        let child = self.scope.child_mut()?;
        let Some(stdin) = child.stdin.as_mut() else {
            return Err(ScopeError::Control(
                "held Environment stdin was not piped".into(),
            ));
        };
        write_admission_signal(&mut self.admission_signal, stdin)
    }

    /// Expose the native protocol channels only after the one-shot launch signal was
    /// successfully written. The candidate frame remains on the separate stderr pipe.
    pub fn take_agent_stdio(
        &mut self,
    ) -> Result<(ChildStdin, ChildStdout, ChildStderr), ScopeError> {
        if self.admission_signal != AdmissionSignalState::Sent {
            return Err(ScopeError::InvalidInput(
                "native agent channels are unavailable before admission release",
            ));
        }
        let child = self.scope.child_mut()?;
        if child.stdin.is_none() || child.stdout.is_none() || child.stderr.is_none() {
            return Err(ScopeError::Control(
                "held Environment requires all three piped process channels".into(),
            ));
        }
        Ok((
            child.stdin.take().expect("checked held stdin"),
            child.stdout.take().expect("checked held stdout"),
            child.stderr.take().expect("checked held stderr"),
        ))
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, ScopeError> {
        self.scope.try_wait()
    }

    pub fn wait_for_quiescence(&mut self, timeout: Duration) -> Result<ExitStatus, ScopeError> {
        self.scope.wait_for_quiescence(timeout)
    }

    pub fn kill_and_wait(&mut self, timeout: Duration) -> Result<ExitStatus, ScopeError> {
        self.scope.kill_and_wait(timeout)
    }

    pub fn limits_are_enforced(&self) -> Result<bool, ScopeError> {
        self.scope.limits_are_enforced()
    }
}

/// Build and launch Bubblewrap under the existing Attempt-scoped cgroup supervisor.
/// No agent starts unless Bubblewrap confirms its required user/PID/network namespaces.
pub fn spawn_bubblewrapped(
    commands: &SystemdCommands,
    spec: BubblewrapLaunchSpec,
) -> Result<ManagedScope, ScopeError> {
    if spec
        .candidate_capture
        .is_some_and(|capture| capture.start_after_admission)
    {
        return Err(ScopeError::InvalidInput(
            "held candidate capture must use spawn_bubblewrapped_held",
        ));
    }
    spawn_bubblewrapped_inner(commands, spec)
}

fn spawn_bubblewrapped_inner(
    commands: &SystemdCommands,
    spec: BubblewrapLaunchSpec,
) -> Result<ManagedScope, ScopeError> {
    let prepared = match spec.candidate_capture {
        Some(_) => prepare_bubblewrap_capture_command(&spec, &commands.scope_gate)?,
        None => prepare_bubblewrap_command(&spec)?,
    };
    let (bubblewrap, arguments, output_root, limits, attempt_id) = prepared;

    let supervisor_spec = LaunchSpec {
        attempt_id,
        executable: bubblewrap,
        args: arguments,
        working_directory: output_root,
        environment: AgentEnvironment {
            // This environment is for the trusted Bubblewrap bootstrap only. It is
            // cleared inside the sandbox before the native agent is exec'd.
            home: PathBuf::from(TMP_TARGET),
            search_path: vec![PathBuf::from("/usr/bin")],
            config_home: None,
            data_home: None,
            cache_home: None,
            temp_dir: None,
            locale: Some("C".to_owned()),
        },
        limits,
    };
    spawn(commands, supervisor_spec)
}

/// Start an idle trusted supervisor inside the exact requested Bubblewrap/cgroup scope.
/// The native harness cannot start until `start_after_admission` is called after the
/// owning Runtime verifies the committed Attempt receipt.
pub fn spawn_bubblewrapped_held(
    commands: &SystemdCommands,
    mut spec: BubblewrapLaunchSpec,
) -> Result<HeldEnvironmentScope, ScopeError> {
    let Some(mut capture) = spec.candidate_capture else {
        return Err(ScopeError::InvalidInput(
            "held Environment requires bounded candidate capture",
        ));
    };
    capture.start_after_admission = true;
    spec.candidate_capture = Some(capture);
    let scope = spawn_bubblewrapped_inner(commands, spec)?;
    let scope = scope.wait_for_held_environment_ready(Duration::from_secs(3))?;
    Ok(HeldEnvironmentScope {
        scope,
        admission_signal: AdmissionSignalState::Waiting,
    })
}

/// Return the canonical Bubblewrap executable and a validated argument vector. This is
/// public for the desktop Runtime's startup qualification and for independent system
/// tests; it does not itself execute or attest a worker.
pub fn prepare_bubblewrap_command(
    spec: &BubblewrapLaunchSpec,
) -> Result<PreparedBubblewrapCommand, ScopeError> {
    prepare_bubblewrap_command_inner(spec, None)
}

/// Build a candidate-capture command using the exact trusted scope-gate executable
/// supplied to the outer process-scope launcher.
pub fn prepare_bubblewrap_capture_command(
    spec: &BubblewrapLaunchSpec,
    scope_gate: &Path,
) -> Result<PreparedBubblewrapCommand, ScopeError> {
    if spec.candidate_capture.is_none() {
        return Err(ScopeError::InvalidInput(
            "candidate capture is not enabled for this Attempt",
        ));
    }
    prepare_bubblewrap_command_inner(spec, Some(scope_gate))
}

fn prepare_bubblewrap_command_inner(
    spec: &BubblewrapLaunchSpec,
    scope_gate: Option<&Path>,
) -> Result<PreparedBubblewrapCommand, ScopeError> {
    match (spec.candidate_capture, scope_gate) {
        (None, None) => {}
        (Some(capture), Some(_))
            if capture.max_bytes > 0
                && capture.max_bytes <= crate::output_channel::MAX_CANDIDATE_BYTES => {}
        (Some(_), Some(_)) => {
            return Err(ScopeError::InvalidInput(
                "candidate byte limit is zero or exceeds the supported bound",
            ));
        }
        (Some(_), None) => {
            return Err(ScopeError::InvalidInput(
                "candidate capture requires the trusted scope-gate executable",
            ));
        }
        (None, Some(_)) => {
            return Err(ScopeError::InvalidInput(
                "scope-gate capture path supplied for a normal Attempt",
            ));
        }
    }

    if spec.scratch_limit_bytes == 0 || spec.scratch_limit_bytes > MAX_SCRATCH_BYTES {
        return Err(ScopeError::InvalidInput(
            "sandbox scratch budget is zero or exceeds the supported bound",
        ));
    }
    let effective_scratch_limit = effective_workspace_scratch_limit(spec.scratch_limit_bytes)?;
    let bubblewrap = trusted_bubblewrap(&spec.bubblewrap)?;
    let input_root = private_directory(&spec.input_root, "input root")?;
    let output_root = private_directory(&spec.output_root, "output root")?;
    if input_root == output_root
        || input_root.starts_with(&output_root)
        || output_root.starts_with(&input_root)
    {
        return Err(ScopeError::InvalidInput(
            "input and output roots must be separate directories",
        ));
    }
    if fs::read_dir(&output_root)
        .map_err(ScopeError::Io)?
        .next()
        .is_some()
    {
        return Err(ScopeError::InvalidInput(
            "Attempt output root must be new and empty",
        ));
    }
    validate_input_tree(&input_root)?;
    if spec.runtime_mounts.is_empty() || spec.runtime_mounts.len() > MAX_RUNTIME_MOUNTS {
        return Err(ScopeError::InvalidInput(
            "runtime mount count is empty or exceeds its bound",
        ));
    }

    let mut mounts = Vec::with_capacity(spec.runtime_mounts.len());
    let mut guest_targets = Vec::with_capacity(spec.runtime_mounts.len());
    for mount in &spec.runtime_mounts {
        let source = trusted_runtime_mount_source(&mount.source)?;
        validate_guest_mount_target(&mount.target)?;
        if source == input_root
            || source == output_root
            || source.starts_with(&input_root)
            || source.starts_with(&output_root)
            || input_root.starts_with(&source)
            || output_root.starts_with(&source)
        {
            return Err(ScopeError::InvalidInput(
                "runtime mount overlaps a Task input/output root",
            ));
        }
        if guest_targets.iter().any(|target: &PathBuf| {
            target == &mount.target
                || target.starts_with(&mount.target)
                || mount.target.starts_with(target)
        }) {
            return Err(ScopeError::InvalidInput(
                "runtime mount targets overlap or duplicate",
            ));
        }
        guest_targets.push(mount.target.clone());
        mounts.push((source, mount.target.clone()));
    }

    let guest_scope_gate = match (spec.candidate_capture, scope_gate) {
        (Some(_), Some(path)) => Some(guest_runner_path(path, &mounts)?),
        _ => None,
    };

    let executable = fs::canonicalize(&spec.process.executable).map_err(ScopeError::Io)?;
    let executable_metadata = fs::metadata(&executable).map_err(ScopeError::Io)?;
    if !executable_metadata.is_file() {
        return Err(ScopeError::InvalidInput("agent executable is not a file"));
    }
    let guest_executable = mounts
        .iter()
        .find_map(|(source, target)| {
            executable
                .strip_prefix(source)
                .ok()
                .map(|relative| target.join(relative))
        })
        .ok_or(ScopeError::InvalidInput(
            "agent executable is outside every read-only runtime mount",
        ))?;

    let mut arguments = vec![
        arg("--die-with-parent"),
        arg("--new-session"),
        arg("--unshare-user"),
        arg("--disable-userns"),
        arg("--assert-userns-disabled"),
        arg("--unshare-pid"),
        arg("--unshare-net"),
        arg("--unshare-ipc"),
        arg("--unshare-uts"),
        arg("--hostname"),
        arg("litecowork-worker"),
        // A user namespace grants namespace-scoped capabilities to its initial
        // process. The harness needs none of them; in particular it must not be
        // able to remount or replace the quota-limited scratch mount.
        arg("--cap-drop"),
        arg("ALL"),
        // Bubblewrap --size applies to the next --tmpfs operation. Keep the
        // argument adjacent to the mount it limits; placing it after `--tmpfs /`
        // would cap the following mount instead of the root tmpfs.
        arg("--size"),
        arg((spec.process.limits.memory_max_bytes / 4)
            .clamp(1, MAX_SCRATCH_BYTES)
            .to_string()),
        arg("--tmpfs"),
        arg("/"),
        arg("--dev"),
        arg("/dev"),
        arg("--proc"),
        arg("/proc"),
        arg("--dir"),
        arg(WORKSPACE_TARGET),
        // The workspace scratch cap is one aggregate quota for outputs, home,
        // and temporary files. `/tmp` below points into the same private tmpfs,
        // so a worker cannot multiply its writable budget by using another path.
        arg("--size"),
        arg(effective_scratch_limit.to_string()),
        arg("--tmpfs"),
        arg(WORKSPACE_TARGET),
        arg("--dir"),
        arg(OUTPUT_TARGET),
        arg("--dir"),
        arg(format!("{OUTPUT_TARGET}/home")),
        arg("--dir"),
        arg("/workspace/tmp"),
        arg("--ro-bind"),
        input_root.as_os_str().to_os_string(),
        arg(INPUT_TARGET),
        // Bubblewrap bind sources are host paths. Use an in-sandbox symlink instead
        // of trying to bind `/workspace/tmp` from the host namespace.
        arg("--symlink"),
        arg("/workspace/tmp"),
        arg(TMP_TARGET),
    ];
    if guest_scope_gate.is_some() {
        // The trusted runner must be PID 1 inside the worker PID namespace so the
        // untrusted harness cannot impersonate its stderr frame channel.
        arguments.push(arg("--as-pid-1"));
    }
    for (source, target) in mounts {
        arguments.push(arg("--ro-bind"));
        arguments.push(source.into_os_string());
        arguments.push(target.into_os_string());
    }
    // Root scaffolding is immutable after the explicit writable mounts are established.
    // Bubblewrap's remount is intentionally non-recursive, leaving the size-capped
    // /workspace tmpfs writable while keeping the rest of the root filesystem read-only.
    arguments.extend([arg("--remount-ro"), arg("/")]);

    arguments.extend([arg("--clearenv")]);
    let sandbox_environment = sandbox_environment(&spec.process.environment, &guest_targets)?;
    for (name, value) in sandbox_environment {
        arguments.extend([arg("--setenv"), OsString::from(name), OsString::from(value)]);
    }
    arguments.extend([arg("--setenv"), arg("PWD"), arg(OUTPUT_TARGET)]);
    arguments.extend([arg("--chdir"), arg(OUTPUT_TARGET), arg("--")]);
    if let (Some(capture), Some(guest_scope_gate)) = (spec.candidate_capture, guest_scope_gate) {
        arguments.extend([
            guest_scope_gate.into_os_string(),
            arg(if capture.start_after_admission {
                "--held-attempt-runner"
            } else {
                "--attempt-runner"
            }),
            arg(capture.max_bytes.to_string()),
            arg(effective_scratch_limit.to_string()),
            arg("--"),
            guest_executable.into_os_string(),
        ]);
    } else {
        arguments.push(guest_executable.into_os_string());
    }
    arguments.extend(spec.process.args.iter().cloned());

    Ok((
        bubblewrap,
        arguments,
        output_root,
        spec.process.limits,
        spec.process.attempt_id,
    ))
}

/// tmpfs accounts capacity in kernel pages. Round an Environment's byte ceiling down so
/// the effective mount can never grant even a partial page more writable space than the
/// configured maximum. The trusted runner later checks this exact effective capacity with
/// `fstatfs` before reporting held readiness.
fn effective_workspace_scratch_limit(requested_bytes: u64) -> Result<u64, ScopeError> {
    let page_size = u64::try_from(rustix::param::page_size())
        .map_err(|_| ScopeError::InvalidInput("kernel page size is not representable"))?;
    if page_size == 0 || !page_size.is_power_of_two() {
        return Err(ScopeError::Unsupported(
            "kernel page size is unavailable or unsupported".into(),
        ));
    }
    let effective = requested_bytes / page_size * page_size;
    if effective == 0 {
        return Err(ScopeError::InvalidInput(
            "sandbox scratch budget is smaller than one kernel page",
        ));
    }
    Ok(effective)
}

fn guest_runner_path(
    scope_gate: &Path,
    mounts: &[(PathBuf, PathBuf)],
) -> Result<PathBuf, ScopeError> {
    let canonical = fs::canonicalize(scope_gate).map_err(ScopeError::Io)?;
    let metadata = fs::metadata(&canonical).map_err(ScopeError::Io)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(ScopeError::Unsupported(
            "candidate runner must be a root-owned, non-group/world-writable executable".into(),
        ));
    }
    mounts
        .iter()
        .find_map(|(source, target)| {
            canonical
                .strip_prefix(source)
                .ok()
                .map(|relative| target.join(relative))
        })
        .ok_or(ScopeError::InvalidInput(
            "trusted scope-gate executable is outside every read-only runtime mount",
        ))
}

fn trusted_bubblewrap(path: &Path) -> Result<PathBuf, ScopeError> {
    let canonical = fs::canonicalize(path).map_err(ScopeError::Io)?;
    let metadata = fs::metadata(&canonical).map_err(ScopeError::Io)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(ScopeError::Unsupported(
            "Bubblewrap must be a root-owned, non-group/world-writable executable".into(),
        ));
    }
    Ok(canonical)
}

fn trusted_runtime_mount_source(path: &Path) -> Result<PathBuf, ScopeError> {
    let canonical = fs::canonicalize(path).map_err(ScopeError::Io)?;
    let metadata = fs::metadata(&canonical).map_err(ScopeError::Io)?;
    let allowed = SYSTEM_RUNTIME_ROOTS.iter().any(|root| {
        fs::canonicalize(root)
            .map(|root| canonical.starts_with(root))
            .unwrap_or(false)
    });
    if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 || !allowed {
        return Err(ScopeError::Unsupported(
            "runtime mounts must be root-owned, non-writable system trees under /usr, /bin, /sbin, /lib, /lib64, or /opt".into(),
        ));
    }
    Ok(canonical)
}

fn private_directory(path: &Path, label: &'static str) -> Result<PathBuf, ScopeError> {
    let metadata = fs::symlink_metadata(path).map_err(ScopeError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
        return Err(ScopeError::InvalidInput(label));
    }
    fs::canonicalize(path).map_err(ScopeError::Io)
}

fn validate_input_tree(root: &Path) -> Result<(), ScopeError> {
    let mut pending = vec![root.to_path_buf()];
    let mut entries = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(ScopeError::Io)? {
            entries = entries
                .checked_add(1)
                .ok_or(ScopeError::InvalidInput("input entry count overflow"))?;
            if entries > 100_000 {
                return Err(ScopeError::InvalidInput(
                    "input root exceeds the entry-count bound",
                ));
            }
            let path = entry.map_err(ScopeError::Io)?.path();
            let metadata = fs::symlink_metadata(&path).map_err(ScopeError::Io)?;
            if metadata.file_type().is_symlink() || metadata.nlink() != 1 {
                return Err(ScopeError::InvalidInput(
                    "input tree contains a symlink or hard link",
                ));
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if !metadata.is_file() {
                return Err(ScopeError::InvalidInput(
                    "input tree contains a non-regular entry",
                ));
            }
        }
    }
    Ok(())
}

fn validate_guest_mount_target(target: &Path) -> Result<(), ScopeError> {
    if !target.is_absolute()
        || target
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || target == Path::new("/")
        || target == Path::new(WORKSPACE_TARGET)
        || target.starts_with(WORKSPACE_TARGET)
        || target == Path::new(TMP_TARGET)
        || target.starts_with(TMP_TARGET)
        || ["/proc", "/dev", "/run", "/sys", "/home", "/root"]
            .iter()
            .any(|reserved| target == Path::new(reserved) || target.starts_with(reserved))
    {
        return Err(ScopeError::InvalidInput(
            "runtime mount target is non-normalized or reserved",
        ));
    }
    Ok(())
}

fn sandbox_environment(
    environment: &AgentEnvironment,
    runtime_targets: &[PathBuf],
) -> Result<Vec<(&'static str, String)>, ScopeError> {
    let safe = crate::linux::sandbox_environment_values(environment)?;
    for (name, value) in &safe {
        if *name == "PATH" {
            for path in value.split(':') {
                validate_guest_environment_path(Path::new(path), runtime_targets)?;
            }
        } else if [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "TMPDIR",
        ]
        .contains(name)
        {
            validate_guest_environment_path(Path::new(value), runtime_targets)?;
        }
    }
    Ok(safe)
}

fn validate_guest_environment_path(
    path: &Path,
    runtime_targets: &[PathBuf],
) -> Result<(), ScopeError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || !(path.starts_with(OUTPUT_TARGET)
            || path.starts_with(TMP_TARGET)
            || runtime_targets.iter().any(|root| path.starts_with(root)))
    {
        return Err(ScopeError::InvalidInput(
            "agent environment path escapes explicit sandbox roots",
        ));
    }
    Ok(())
}

fn arg(value: impl Into<OsString>) -> OsString {
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLimits;
    use std::io::{self, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use tempfile::tempdir;

    struct BrokenWriter;

    impl Write for BrokenWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn admission_signal_is_one_shot_and_exact() {
        let mut state = AdmissionSignalState::Waiting;
        let mut signal = Vec::new();
        write_admission_signal(&mut state, &mut signal).expect("first signal is written");
        assert_eq!(state, AdmissionSignalState::Sent);
        assert_eq!(signal, crate::admission_protocol::START_AFTER_RECEIPT);
        assert!(matches!(
            write_admission_signal(&mut state, &mut signal),
            Err(ScopeError::InvalidInput(_))
        ));
        assert_eq!(signal, crate::admission_protocol::START_AFTER_RECEIPT);
    }

    #[test]
    fn ambiguous_admission_signal_write_is_never_retried() {
        let mut state = AdmissionSignalState::Waiting;
        assert!(matches!(
            write_admission_signal(&mut state, &mut BrokenWriter),
            Err(ScopeError::Io(_))
        ));
        assert_eq!(state, AdmissionSignalState::Attempted);
        assert!(matches!(
            write_admission_signal(&mut state, &mut Vec::new()),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    fn test_spec(root: &Path) -> BubblewrapLaunchSpec {
        let input = root.join("inputs");
        let output = root.join("outputs");
        fs::create_dir(&input).unwrap();
        fs::create_dir(&output).unwrap();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(input.join("pinned.txt"), "exact input\n").unwrap();
        fs::set_permissions(input.join("pinned.txt"), fs::Permissions::from_mode(0o400)).unwrap();
        BubblewrapLaunchSpec {
            bubblewrap: PathBuf::from("/usr/bin/bwrap"),
            process: LaunchSpec {
                attempt_id: [7; 16],
                executable: PathBuf::from("/usr/bin/dash"),
                args: vec![
                    OsString::from("-c"),
                    OsString::from("cat /workspace/inputs/pinned.txt"),
                ],
                working_directory: output.clone(),
                environment: AgentEnvironment {
                    home: PathBuf::from("/workspace/outputs/home"),
                    search_path: vec![PathBuf::from("/usr/bin")],
                    config_home: None,
                    data_home: None,
                    cache_home: None,
                    temp_dir: Some(PathBuf::from("/tmp")),
                    locale: Some("C".into()),
                },
                limits: ResourceLimits {
                    memory_max_bytes: 256 * 1024 * 1024,
                    cpu_quota_percent: 100,
                    tasks_max: 32,
                },
            },
            input_root: input,
            output_root: output,
            scratch_limit_bytes: 64 * 1024 * 1024,
            candidate_capture: None,
            runtime_mounts: vec![
                ReadOnlyRuntimeMount {
                    source: PathBuf::from("/usr"),
                    target: PathBuf::from("/usr"),
                },
                ReadOnlyRuntimeMount {
                    source: PathBuf::from("/lib"),
                    target: PathBuf::from("/lib"),
                },
                ReadOnlyRuntimeMount {
                    source: PathBuf::from("/lib64"),
                    target: PathBuf::from("/lib64"),
                },
            ],
        }
    }

    #[test]
    fn builds_network_and_filesystem_isolation_before_agent_exec() {
        let temp = tempdir().unwrap();
        let spec = test_spec(temp.path());
        let (_, args, output, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert_eq!(output, spec.output_root.canonicalize().unwrap());
        for required in [
            "--unshare-user",
            "--disable-userns",
            "--assert-userns-disabled",
            "--unshare-pid",
            "--unshare-net",
            "--cap-drop",
            "ALL",
            "--ro-bind",
            "--tmpfs",
            "--clearenv",
        ] {
            assert!(args.iter().any(|arg| arg == required), "{required}");
        }
        let cap_drop = args.iter().position(|arg| arg == "--cap-drop").unwrap();
        assert_eq!(args[cap_drop + 1], "ALL");
        let clear = args.iter().position(|arg| arg == "--clearenv").unwrap();
        let first_env = args.iter().position(|arg| arg == "--setenv").unwrap();
        assert!(clear < first_env);
        assert!(args.iter().any(|arg| arg == "/workspace/inputs"));
        assert!(args.iter().any(|arg| arg == "/workspace/outputs"));
        let tmpfs_size = args.iter().position(|arg| arg == "--size").unwrap();
        assert_eq!(
            args[tmpfs_size + 1],
            (spec.process.limits.memory_max_bytes / 4).to_string()
        );
        assert_eq!(args[tmpfs_size + 2], "--tmpfs");
        assert_eq!(args[tmpfs_size + 3], "/");
        let scratch_limit = spec.scratch_limit_bytes.to_string();
        let scratch_size = args
            .windows(4)
            .position(|window| {
                window[0] == "--size"
                    && window[1].as_ref() == scratch_limit
                    && window[2] == "--tmpfs"
                    && window[3] == WORKSPACE_TARGET
            })
            .unwrap();
        assert_eq!(args[scratch_size], "--size");
        assert!(args.iter().any(|arg| arg == "/workspace/outputs"));
        assert!(args.iter().any(|arg| arg == "/workspace/inputs"));
        assert!(args.iter().any(|arg| arg == "/workspace/tmp"));
        assert!(args.iter().any(|arg| arg == TMP_TARGET));
        let host_output_root = spec.output_root.to_string_lossy();
        assert!(
            !args
                .iter()
                .any(|arg| arg.as_ref() == host_output_root.as_ref())
        );
        let tmp_link = args.iter().position(|arg| arg == "--symlink").unwrap();
        assert_eq!(args[tmp_link + 1], "/workspace/tmp");
        assert_eq!(args[tmp_link + 2], TMP_TARGET);
        let readonly_root = args.iter().position(|arg| arg == "--remount-ro").unwrap();
        assert_eq!(args[readonly_root + 1], "/");
    }

    #[test]
    fn workspace_scratch_limit_never_rounds_above_the_requested_budget() {
        let page_size = rustix::param::page_size() as u64;
        assert_eq!(
            effective_workspace_scratch_limit(page_size + 1).unwrap(),
            page_size
        );
        assert_eq!(
            effective_workspace_scratch_limit(page_size).unwrap(),
            page_size
        );
        assert!(effective_workspace_scratch_limit(page_size - 1).is_err());
    }

    #[test]
    fn builder_passes_the_page_bounded_capacity_to_mount_and_runner() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        let page_size = rustix::param::page_size() as u64;
        spec.scratch_limit_bytes = 64 * 1024 * 1024 + page_size - 1;
        spec.candidate_capture = Some(CandidateCaptureSpec {
            max_bytes: 4096,
            start_after_admission: true,
        });
        let (_, args, _, _, _) =
            prepare_bubblewrap_capture_command(&spec, Path::new("/usr/bin/dash")).unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        let mount = args
            .windows(4)
            .position(|window| {
                window[0] == "--size" && window[2] == "--tmpfs" && window[3] == WORKSPACE_TARGET
            })
            .unwrap();
        assert_eq!(args[mount + 1], (64 * 1024 * 1024).to_string());
        let runner = args
            .windows(6)
            .position(|window| {
                window[1] == "--held-attempt-runner"
                    && window[2] == "4096"
                    && window[3] == (64 * 1024 * 1024).to_string()
                    && window[4] == "--"
            })
            .unwrap();
        assert!(runner > mount);
    }

    #[test]
    fn candidate_capture_uses_the_packaged_runner_as_pid_one() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.candidate_capture = Some(CandidateCaptureSpec {
            max_bytes: 4096,
            start_after_admission: false,
        });
        let (_, args, _, _, _) =
            prepare_bubblewrap_capture_command(&spec, Path::new("/usr/bin/dash")).unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();

        assert!(args.iter().any(|arg| arg == "--as-pid-1"));
        let command = args
            .windows(6)
            .position(|window| {
                window[0] == "/usr/bin/dash"
                    && window[1] == "--attempt-runner"
                    && window[2] == "4096"
                    && window[3] == (64 * 1024 * 1024).to_string()
                    && window[4] == "--"
                    && window[5] == "/usr/bin/dash"
            })
            .unwrap();
        assert!(command > args.iter().position(|arg| arg == "--as-pid-1").unwrap());
        assert!(args.iter().any(|arg| arg == "--cap-drop"));
    }

    #[test]
    fn held_candidate_capture_waits_for_post_receipt_admission() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.candidate_capture = Some(CandidateCaptureSpec {
            max_bytes: 4096,
            start_after_admission: true,
        });
        let (_, args, _, _, _) =
            prepare_bubblewrap_capture_command(&spec, Path::new("/usr/bin/dash")).unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();

        assert!(args.iter().any(|arg| arg == "--as-pid-1"));
        assert!(args.iter().any(|arg| arg == "--held-attempt-runner"));
        assert!(!args.iter().any(|arg| arg == "--attempt-runner"));
    }

    #[test]
    fn candidate_capture_requires_a_bounded_runner_inside_a_runtime_mount() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.candidate_capture = Some(CandidateCaptureSpec {
            max_bytes: 0,
            start_after_admission: false,
        });
        assert!(matches!(
            prepare_bubblewrap_capture_command(&spec, Path::new("/usr/bin/dash")),
            Err(ScopeError::InvalidInput(_))
        ));

        spec.candidate_capture = Some(CandidateCaptureSpec {
            max_bytes: crate::output_channel::MAX_CANDIDATE_BYTES + 1,
            start_after_admission: false,
        });
        assert!(matches!(
            prepare_bubblewrap_capture_command(&spec, Path::new("/usr/bin/dash")),
            Err(ScopeError::InvalidInput(_))
        ));

        spec.candidate_capture = Some(CandidateCaptureSpec {
            max_bytes: 16,
            start_after_admission: false,
        });
        assert!(matches!(
            prepare_bubblewrap_capture_command(&spec, Path::new("/tmp")),
            Err(ScopeError::Io(_) | ScopeError::InvalidInput(_) | ScopeError::Unsupported(_))
        ));
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_zero_or_unbounded_scratch_budget() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.scratch_limit_bytes = 0;
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));

        spec.scratch_limit_bytes = MAX_SCRATCH_BYTES + 1;
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_symlinks_hardlinks_and_output_reuse() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        std::os::unix::fs::symlink("/etc/passwd", spec.input_root.join("escape")).unwrap();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        fs::remove_file(spec.input_root.join("escape")).unwrap();
        fs::hard_link(
            spec.input_root.join("pinned.txt"),
            spec.input_root.join("second.txt"),
        )
        .unwrap();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        fs::remove_file(spec.input_root.join("second.txt")).unwrap();
        fs::write(spec.output_root.join("stale"), "not new").unwrap();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        spec.output_root = spec.input_root.clone();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_reserved_mount_targets_and_host_executable_outside_mounts() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.runtime_mounts[0].target = PathBuf::from("/run/user/1000");
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
        spec.runtime_mounts[0].target = PathBuf::from("/usr");
        spec.process.executable = PathBuf::from("/etc/passwd");
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_user_controlled_host_paths_as_runtime_mount_sources() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.runtime_mounts[0].source = temp.path().to_path_buf();
        assert!(matches!(
            prepare_bubblewrap_command(&spec),
            Err(ScopeError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_guest_path_traversal_in_runtime_and_environment_bindings() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.runtime_mounts[0].target = PathBuf::from("/runtime/../../home");
        assert!(prepare_bubblewrap_command(&spec).is_err());
        spec.runtime_mounts[0].target = PathBuf::from("/usr");
        spec.process.environment.home = PathBuf::from("/home/user");
        assert!(prepare_bubblewrap_command(&spec).is_err());
    }

    #[test]
    #[ignore = "real user-namespace Bubblewrap qualification; run on each claimed Linux build"]
    fn live_bubblewrap_exposes_pinned_inputs_read_only_and_only_private_output_writable() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.process.args = vec![
            OsString::from("-c"),
            OsString::from(
                "test ! -e /home && test \"$(cat /workspace/inputs/pinned.txt)\" = 'exact input' && printf 'generated\\n' > /workspace/outputs/result.txt && printf 'blocked\\n' > /workspace/inputs/pinned.txt",
            ),
        ];
        let (program, args, _, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let output = Command::new(program)
            .args(args)
            .output()
            .expect("launch real Bubblewrap namespaces");
        assert!(!output.status.success(), "input mount must be read-only");
        assert_eq!(
            fs::read(spec.input_root.join("pinned.txt")).unwrap(),
            b"exact input\n"
        );
        assert!(!spec.output_root.join("result.txt").exists());
    }

    #[test]
    #[ignore = "requires live user-namespace Bubblewrap; verifies the actual scratch quota"]
    fn live_bubblewrap_enforces_the_aggregate_scratch_limit() {
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.scratch_limit_bytes = 16 * 1024;
        spec.process.args = vec![
            OsString::from("-c"),
            OsString::from(
                "if dd if=/dev/zero of=/workspace/outputs/exhaust bs=8192 count=4 2>&1; then exit 41; else printf 'quota-exhausted\\n'; fi",
            ),
        ];
        let (program, args, _, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let output = Command::new(program)
            .args(args)
            .output()
            .expect("launch real Bubblewrap scratch quota probe");
        assert!(output.status.success(), "{:?}", output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("No space left on device"));
        assert!(!spec.output_root.join("exhaust").exists());
    }

    #[test]
    #[ignore = "real network-namespace Bubblewrap qualification; run on each claimed Linux build"]
    fn live_bubblewrap_network_namespace_cannot_reach_host_loopback() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener
            .set_nonblocking(true)
            .expect("listener accepts nonblocking mode");
        let port = listener.local_addr().unwrap().port();
        let temp = tempdir().unwrap();
        let mut spec = test_spec(temp.path());
        spec.process.executable = PathBuf::from("/usr/bin/python3");
        spec.process.args = vec![
            OsString::from("-c"),
            OsString::from(format!(
                "import socket; s=socket.socket(); s.settimeout(0.5);\ntry: s.connect(('127.0.0.1',{port})); raise SystemExit(7)\nexcept OSError: print('isolated')"
            )),
        ];
        let (program, args, _, _, _) = prepare_bubblewrap_command(&spec).unwrap();
        let output = Command::new(program)
            .args(args)
            .output()
            .expect("launch real Bubblewrap network namespace");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "isolated");
        let mut accepted = false;
        match listener.accept() {
            Ok((mut stream, _)) => {
                accepted = true;
                let _ = stream.write_all(b"unexpected");
                let mut buffer = [0; 8];
                let _ = stream.read(&mut buffer);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("listener check failed: {error}"),
        }
        std::thread::sleep(Duration::from_millis(25));
        assert!(!accepted, "sandbox reached the host loopback listener");
    }
}
