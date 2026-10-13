//! Linux process ownership using systemd transient scopes.
//!
//! This crate is deliberately only a process/cgroup primitive. It does not create an
//! Environment, mediate capabilities, deliver credentials, reconcile Effects, or admit
//! Task Attempts. A caller must not treat successful scope creation as Task readiness.
//!
//! Launch is delegated to `systemd-run --user --scope`. Upstream systemd's implementation
//! registers the invoking `systemd-run` PID in the transient scope, waits for the unit job,
//! then calls `execvpe()` for the requested command. Thus the agent replaces the same PID
//! only after scope placement, and inherited stdin/stdout/stderr survive the exec. We do
//! not use `--pipe` (systemd-run rejects it with `--scope`); Rust's pipes are inherited by
//! systemd-run and remain attached across exec. A trusted gate emits READY, waits for an
//! exact GO, then emits GO_CONSUMED before attempting target exec; that marker proves only
//! that the handshake bytes were consumed, not that target exec succeeded. The runtime
//! qualification suite must pin supported systemd versions and verify this lifecycle on
//! each supported distribution. A dedicated active parent slice remains available while
//! systemd removes the worker scope, allowing explicit recursive `populated 0` observation.
//!
//! This is not a hostile-code sandbox. Without a separate Environment boundary that hides
//! the user systemd control socket, a same-user worker may request another transient unit
//! and move descendants outside this scope. Do not use this crate alone to admit untrusted
//! workers or claim production-safe process containment.

#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        ffi::{OsStr, OsString},
        fs,
        io::{self, Read, Write},
        os::unix::ffi::OsStrExt,
        path::{Component, Path, PathBuf},
        process::{Child, ChildStdout, Command, ExitStatus, Output, Stdio},
        thread,
        time::{Duration, Instant},
    };

    const PREFIX: &str = "litecowork-attempt-";
    const GATE_READY: &[u8] = b"LITECOWORK_SCOPE_GATE_V1\n";
    const GATE_GO_CONSUMED: &[u8] = b"LITECOWORK_SCOPE_GO_CONSUMED_V1\n";
    const OUTPUT_LIMIT: usize = 4096;
    const CONTROL_TIMEOUT: Duration = Duration::from_secs(3);
    const POLL_INTERVAL: Duration = Duration::from_millis(20);

    #[derive(Clone, Debug)]
    pub struct SystemdCommands {
        /// Absolute, package-qualified paths; PATH lookup is intentionally not used.
        pub systemd_run: PathBuf,
        pub systemctl: PathBuf,
        /// Trusted LiteCowork bootstrap executable, packaged beside the Runtime.
        pub scope_gate: PathBuf,
        /// Trusted `env` utility used to clear the manager's environment before the agent
        /// starts. Its path must refer to the platform's base OS utility.
        pub env: PathBuf,
    }

    #[derive(Clone, Debug)]
    pub struct AgentEnvironment {
        pub home: PathBuf,
        pub search_path: Vec<PathBuf>,
        pub config_home: Option<PathBuf>,
        pub data_home: Option<PathBuf>,
        pub cache_home: Option<PathBuf>,
        pub temp_dir: Option<PathBuf>,
        pub locale: Option<String>,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct ResourceLimits {
        pub memory_max_bytes: u64,
        pub cpu_quota_percent: u32,
        pub tasks_max: u32,
    }

    #[derive(Clone, Debug)]
    pub struct LaunchSpec {
        /// A stable 128-bit Attempt identifier. The caller must never reuse it, including
        /// after systemd garbage-collects completed units; retries require a new ID.
        pub attempt_id: [u8; 16],
        pub executable: PathBuf,
        /// Must contain non-secret CLI arguments only. Task data and credentials belong on
        /// authenticated protocol/SecretLease channels, never in process arguments.
        pub args: Vec<OsString>,
        pub working_directory: PathBuf,
        pub environment: AgentEnvironment,
        pub limits: ResourceLimits,
    }

    #[derive(Debug)]
    pub enum ScopeError {
        InvalidInput(&'static str),
        Unsupported(String),
        Io(io::Error),
        Control(String),
        CleanupPending {
            cause: Box<ScopeError>,
            cleanup_error: Box<ScopeError>,
            pending: Box<PendingCleanup>,
        },
        QuiescenceUnobservable,
        QuiescenceTimeout,
    }

    impl std::fmt::Display for ScopeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidInput(s) => write!(f, "invalid process scope input: {s}"),
                Self::Unsupported(s) => write!(f, "Linux process scope unavailable: {s}"),
                Self::Io(e) => write!(f, "Linux process scope I/O failed: {e}"),
                Self::Control(s) => write!(f, "systemd process scope control failed: {s}"),
                Self::CleanupPending {
                    cause,
                    cleanup_error,
                    ..
                } => write!(
                    f,
                    "process scope cleanup remains pending after {cause}: {cleanup_error}"
                ),
                Self::QuiescenceUnobservable => {
                    write!(f, "cgroup quiescence could not be observed")
                }
                Self::QuiescenceTimeout => {
                    write!(f, "cgroup did not become empty before the deadline")
                }
            }
        }
    }

    impl std::error::Error for ScopeError {}

    impl From<io::Error> for ScopeError {
        fn from(value: io::Error) -> Self {
            Self::Io(value)
        }
    }

    /// A running worker and its systemd-owned cgroup. Drop does not signal or detach work.
    #[must_use = "dropping ManagedScope does not stop its worker or release its cgroup"]
    #[derive(Debug)]
    pub struct ManagedScope {
        commands: SystemdCommands,
        unit: String,
        slice_unit: String,
        cgroup_events: PathBuf,
        cgroup_observer: CgroupPopulatedObserver,
        child: Child,
        limits: ResourceLimits,
    }

    /// Observes a dedicated active per-Attempt parent slice. systemd may remove a stopped
    /// transient scope immediately, so the parent's stable cgroup.events path is used as
    /// the recursive observer target. Only explicit `populated 0` is proof.
    #[derive(Debug)]
    struct CgroupPopulatedObserver {
        events_path: PathBuf,
    }

    impl CgroupPopulatedObserver {
        fn require_empty_parent(events_path: PathBuf) -> Result<Self, ScopeError> {
            let populated = read_populated(&events_path).map_err(ScopeError::Io)?;
            if populated {
                return Err(ScopeError::QuiescenceUnobservable);
            }
            Ok(Self { events_path })
        }

        fn wait_until_empty(&self, deadline: Instant) -> Result<(), ScopeError> {
            loop {
                match read_populated(&self.events_path) {
                    Ok(false) => return Ok(()),
                    Ok(true) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Err(ScopeError::QuiescenceUnobservable);
                    }
                    Err(error) => return Err(ScopeError::Io(error)),
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(ScopeError::QuiescenceTimeout);
                }
                thread::sleep(POLL_INTERVAL.min(remaining));
            }
        }
    }

    /// Retry handle returned when launch failed and cleanup could not prove it settled.
    /// The pre-worker variant exists only before GO, when the trusted gate has not
    /// launched the agent; it retains the scope/slice units, commands, child process, and
    /// optional cgroup event path. The managed variant retains the stable parent-slice
    /// observer and the resolved worker cgroup path.
    #[must_use = "dropping PendingCleanup loses retry ownership of an unsettled process scope"]
    #[derive(Debug)]
    pub enum PendingCleanup {
        /// The Attempt slice was created but the `systemd-run` process never started.
        /// No worker exists; retry only the exact owned slice stop operation.
        AttemptSlice {
            commands: SystemdCommands,
            slice_unit: String,
            cgroup_events: Option<PathBuf>,
        },
        PreWorker {
            commands: SystemdCommands,
            unit: String,
            slice_unit: String,
            child: Child,
            cgroup_events: Option<PathBuf>,
        },
        ManagedScope {
            scope: ManagedScope,
        },
    }

    impl PendingCleanup {
        pub fn unit_name(&self) -> &str {
            match self {
                Self::AttemptSlice { slice_unit, .. } => slice_unit,
                Self::PreWorker { unit, .. } => unit,
                Self::ManagedScope { scope } => scope.unit_name(),
            }
        }

        pub fn retry_kill_and_wait(&mut self, timeout: Duration) -> Result<ExitStatus, ScopeError> {
            match self {
                Self::AttemptSlice { .. } => Err(ScopeError::InvalidInput(
                    "Attempt slice cleanup has no launcher child; use retry_stop",
                )),
                Self::PreWorker {
                    commands,
                    unit,
                    slice_unit,
                    child,
                    cgroup_events,
                } => {
                    let _ = run_control(
                        &commands.systemctl,
                        [
                            "--user",
                            "--no-pager",
                            "kill",
                            "--kill-whom=all",
                            "--signal=SIGKILL",
                        ],
                        Some(unit),
                        CONTROL_TIMEOUT,
                    );
                    if !kill_direct_until(child, timeout) {
                        return Err(ScopeError::QuiescenceTimeout);
                    }
                    let status = child
                        .try_wait()
                        .map_err(ScopeError::Io)?
                        .ok_or(ScopeError::QuiescenceTimeout)?;
                    prove_preworker_quiescent(commands, unit, cgroup_events.as_deref(), timeout)?;
                    stop_attempt_slice(commands, slice_unit, timeout)?;
                    Ok(status)
                }
                Self::ManagedScope { scope } => scope.kill_and_wait(timeout),
            }
        }

        /// Retries cleanup when the Attempt slice started but launcher exec never did.
        /// Other pending states require their child/cgroup-aware `retry_kill_and_wait` path.
        pub fn retry_stop(&mut self, timeout: Duration) -> Result<(), ScopeError> {
            match self {
                Self::AttemptSlice {
                    commands,
                    slice_unit,
                    cgroup_events,
                } => {
                    let _ = stop_attempt_slice(commands, slice_unit, timeout);
                    prove_preworker_quiescent(
                        commands,
                        slice_unit,
                        cgroup_events.as_deref(),
                        timeout,
                    )
                }
                Self::PreWorker { .. } | Self::ManagedScope { .. } => Err(
                    ScopeError::InvalidInput("pending cleanup owns a launcher child"),
                ),
            }
        }

        pub fn limits_are_enforced(&self) -> Result<bool, ScopeError> {
            match self {
                Self::AttemptSlice { .. } | Self::PreWorker { .. } => {
                    Err(ScopeError::QuiescenceUnobservable)
                }
                Self::ManagedScope { scope } => scope.limits_are_enforced(),
            }
        }
    }

    impl ManagedScope {
        pub fn unit_name(&self) -> &str {
            &self.unit
        }

        /// Pipe handles are the worker's inherited descriptors after systemd-run execs it.
        /// Callers must drain stdout and stderr concurrently or the worker can block when
        /// either pipe fills.
        pub fn child_mut(&mut self) -> Result<&mut Child, ScopeError> {
            Ok(&mut self.child)
        }

        pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, ScopeError> {
            self.child.try_wait().map_err(ScopeError::Io)
        }

        /// Observe explicit recursive parent-slice cgroup-v2 `populated=0` and reap the
        /// direct worker process. A missing or unreadable event file is never proof.
        pub fn wait_for_quiescence(&mut self, timeout: Duration) -> Result<ExitStatus, ScopeError> {
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or(ScopeError::InvalidInput("timeout overflow"))?;
            self.cgroup_observer.wait_until_empty(deadline)?;
            let status = wait_child_until(&mut self.child, deadline)?;
            stop_attempt_slice(&self.commands, &self.slice_unit, CONTROL_TIMEOUT)?;
            Ok(status)
        }

        /// SIGKILL every process in the scope, then require explicit recursive kernel
        /// cgroup-v2 `populated=0` evidence from the stable parent slice before returning.
        /// This does not reconcile external Effects or prove a provider-side operation stopped.
        pub fn kill_and_wait(&mut self, timeout: Duration) -> Result<ExitStatus, ScopeError> {
            self.kill_and_wait_with(timeout, wait_child_until)
        }

        fn kill_and_wait_with(
            &mut self,
            timeout: Duration,
            wait_child: impl FnOnce(&mut Child, Instant) -> Result<ExitStatus, ScopeError>,
        ) -> Result<ExitStatus, ScopeError> {
            let _ = run_control(
                &self.commands.systemctl,
                [
                    "--user",
                    "--no-pager",
                    "kill",
                    "--kill-whom=all",
                    "--signal=SIGKILL",
                ],
                Some(&self.unit),
                CONTROL_TIMEOUT,
            );
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or(ScopeError::InvalidInput("timeout overflow"))?;
            self.cgroup_observer.wait_until_empty(deadline)?;
            let status = wait_child(&mut self.child, deadline)?;
            stop_attempt_slice(&self.commands, &self.slice_unit, CONTROL_TIMEOUT)?;
            Ok(status)
        }

        /// Re-read limit files; a successful systemd request alone is not treated as proof
        /// that the kernel enforced the values.
        pub fn limits_are_enforced(&self) -> Result<bool, ScopeError> {
            let dir = self
                .cgroup_events
                .parent()
                .ok_or(ScopeError::QuiescenceUnobservable)?;
            let memory = read_trimmed(dir.join("memory.max"))?;
            let pids = read_trimmed(dir.join("pids.max"))?;
            let cpu = read_trimmed(dir.join("cpu.max"))?;
            let memory_ok = memory.parse::<u64>().ok() == Some(self.limits.memory_max_bytes);
            let pids_ok = pids.parse::<u32>().ok() == Some(self.limits.tasks_max);
            let mut fields = cpu.split_ascii_whitespace();
            let quota = fields.next().and_then(|x| x.parse::<u64>().ok());
            let period = fields.next().and_then(|x| x.parse::<u64>().ok());
            let cpu_ok = matches!((quota, period), (Some(q), Some(p)) if p > 0 && u128::from(q) * 100 == u128::from(p) * u128::from(self.limits.cpu_quota_percent));
            Ok(memory_ok && pids_ok && cpu_ok)
        }
    }

    /// Launches the agent as a transient user scope. `systemd-run --scope` executes the
    /// command itself after registering its own PID in the scope; no post-spawn cgroup
    /// attachment race exists. Cgroup limits are installed by systemd before that exec.
    pub fn spawn(commands: &SystemdCommands, spec: LaunchSpec) -> Result<ManagedScope, ScopeError> {
        validate_paths(commands, &spec)?;
        validate_limits(spec.limits)?;
        let environment = safe_environment(&spec.environment)?;
        let mount = cgroup2_mount()?;
        spawn_with(
            commands,
            spec,
            environment,
            mount,
            start_attempt_slice,
            |command| command.spawn(),
        )
    }

    fn spawn_with(
        commands: &SystemdCommands,
        spec: LaunchSpec,
        environment: Vec<(&'static str, String)>,
        mount: CgroupMount,
        start_slice: impl FnOnce(
            &SystemdCommands,
            &str,
            &CgroupMount,
        ) -> Result<(PathBuf, CgroupPopulatedObserver), ScopeError>,
        launch_command: impl FnOnce(&mut Command) -> io::Result<Child>,
    ) -> Result<ManagedScope, ScopeError> {
        let unit = unit_name(spec.attempt_id);
        let slice_unit = attempt_slice_name(spec.attempt_id);
        let (slice_cgroup, cgroup_observer) = start_slice(commands, &slice_unit, &mount)?;

        let mut cmd = Command::new(&commands.systemd_run);
        cmd.arg("--user")
            .arg("--scope")
            .arg("--quiet")
            .arg("--expand-environment=no")
            .arg(format!("--unit={unit}"))
            .arg(format!("--slice={slice_unit}"))
            .arg(format!(
                "--property=MemoryMax={}",
                spec.limits.memory_max_bytes
            ))
            .arg(format!(
                "--property=CPUQuota={}%",
                spec.limits.cpu_quota_percent
            ))
            .arg(format!("--property=TasksMax={}", spec.limits.tasks_max))
            .arg("--property=KillMode=control-group")
            .arg("--property=Delegate=no")
            .arg("--property=SendSIGKILL=yes")
            .arg("--")
            .arg(&commands.env)
            .arg("-i");

        for (name, value) in environment {
            cmd.arg(format!("{name}={value}"));
        }
        cmd.arg(&commands.scope_gate)
            .arg(&spec.executable)
            .args(&spec.args)
            .current_dir(&spec.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Keep the manager's user-bus environment for systemd-run itself. `env -i` clears
        // it before the trusted gate/native agent begins; only explicit non-secret
        // path/locale values are placed in the child environment.
        let mut child = match launch_command(&mut cmd) {
            Ok(child) => child,
            Err(error) => {
                return Err(fail_attempt_slice(
                    commands,
                    &slice_unit,
                    cgroup_observer,
                    ScopeError::Io(error),
                ));
            }
        };
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                return Err(fail_before_worker(
                    commands,
                    &unit,
                    &slice_unit,
                    child,
                    None,
                    ScopeError::Control("scope gate stdout was not piped".into()),
                ));
            }
        };
        let stdout = match wait_for_marker(stdout, GATE_READY, CONTROL_TIMEOUT) {
            Ok(stdout) => stdout,
            Err(error) => {
                return Err(fail_before_worker(
                    commands,
                    &unit,
                    &slice_unit,
                    child,
                    None,
                    error,
                ));
            }
        };
        let (cgroup, relative) =
            match resolve_scope_cgroup(commands, &unit, &mount, CONTROL_TIMEOUT) {
                Ok(path) => path,
                Err(error) => {
                    return Err(fail_before_worker(
                        commands,
                        &unit,
                        &slice_unit,
                        child,
                        None,
                        error,
                    ));
                }
            };
        if !cgroup_is_descendant(&slice_cgroup, &cgroup) {
            return Err(fail_before_worker(
                commands,
                &unit,
                &slice_unit,
                child,
                Some(cgroup.join("cgroup.events")),
                ScopeError::Unsupported(
                    "worker scope is outside its observed per-Attempt parent slice".into(),
                ),
            ));
        }
        if let Err(error) = verify_membership(child.id(), &relative, &mount.root) {
            return Err(fail_before_worker(
                commands,
                &unit,
                &slice_unit,
                child,
                Some(cgroup.join("cgroup.events")),
                error,
            ));
        }
        let cgroup_events = cgroup.join("cgroup.events");
        let managed = ManagedScope {
            commands: commands.clone(),
            unit,
            slice_unit,
            cgroup_events,
            cgroup_observer,
            child,
            limits: spec.limits,
        };
        match managed.limits_are_enforced() {
            Ok(true) => {}
            Ok(false) => {
                return Err(fail_scope(
                    managed,
                    ScopeError::Unsupported(
                        "systemd did not apply all requested cgroup-v2 limits".into(),
                    ),
                    CONTROL_TIMEOUT,
                ));
            }
            Err(error) => {
                return Err(fail_scope(managed, error, CONTROL_TIMEOUT));
            }
        }
        let mut managed = managed;
        let mut stdin = match managed.child.stdin.take() {
            Some(stdin) => stdin,
            None => {
                return Err(fail_scope(
                    managed,
                    ScopeError::Control("scope gate stdin was not piped".into()),
                    CONTROL_TIMEOUT,
                ));
            }
        };
        if let Err(error) = stdin.write_all(b"GO\n").and_then(|_| stdin.flush()) {
            return Err(fail_scope(managed, ScopeError::Io(error), CONTROL_TIMEOUT));
        }
        managed.child.stdin = Some(stdin);
        let stdout = match wait_for_marker(stdout, GATE_GO_CONSUMED, CONTROL_TIMEOUT) {
            Ok(stdout) => stdout,
            Err(error) => {
                return Err(fail_scope(managed, error, CONTROL_TIMEOUT));
            }
        };
        managed.child.stdout = Some(stdout);
        Ok(managed)
    }

    fn fail_attempt_slice(
        commands: &SystemdCommands,
        slice_unit: &str,
        cgroup_observer: CgroupPopulatedObserver,
        cause: ScopeError,
    ) -> ScopeError {
        match stop_and_prove_attempt_slice(commands, slice_unit, &cgroup_observer, CONTROL_TIMEOUT)
        {
            Ok(()) => cause,
            Err(cleanup_error) => ScopeError::CleanupPending {
                cause: Box::new(cause),
                cleanup_error: Box::new(cleanup_error),
                pending: Box::new(PendingCleanup::AttemptSlice {
                    commands: commands.clone(),
                    slice_unit: slice_unit.to_owned(),
                    cgroup_events: Some(cgroup_observer.events_path),
                }),
            },
        }
    }

    fn fail_started_attempt_slice(
        commands: &SystemdCommands,
        slice_unit: &str,
        cgroup_events: Option<PathBuf>,
        cause: ScopeError,
    ) -> ScopeError {
        let cleanup = stop_attempt_slice(commands, slice_unit, CONTROL_TIMEOUT).and_then(|()| {
            prove_preworker_quiescent(
                commands,
                slice_unit,
                cgroup_events.as_deref(),
                CONTROL_TIMEOUT,
            )
        });
        match cleanup {
            Ok(()) => cause,
            Err(cleanup_error) => ScopeError::CleanupPending {
                cause: Box::new(cause),
                cleanup_error: Box::new(cleanup_error),
                pending: Box::new(PendingCleanup::AttemptSlice {
                    commands: commands.clone(),
                    slice_unit: slice_unit.to_owned(),
                    cgroup_events,
                }),
            },
        }
    }

    fn stop_and_prove_attempt_slice(
        commands: &SystemdCommands,
        slice_unit: &str,
        cgroup_observer: &CgroupPopulatedObserver,
        timeout: Duration,
    ) -> Result<(), ScopeError> {
        stop_attempt_slice(commands, slice_unit, timeout)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ScopeError::InvalidInput("timeout overflow"))?;
        cgroup_observer.wait_until_empty(deadline)
    }

    fn validate_paths(commands: &SystemdCommands, spec: &LaunchSpec) -> Result<(), ScopeError> {
        for path in [
            &commands.systemd_run,
            &commands.systemctl,
            &commands.env,
            &commands.scope_gate,
            &spec.executable,
            &spec.working_directory,
            &spec.environment.home,
        ] {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, Component::ParentDir))
            {
                return Err(ScopeError::InvalidInput(
                    "executable and environment paths must be absolute and normalized",
                ));
            }
        }
        if spec.executable.as_os_str().is_empty() {
            return Err(ScopeError::InvalidInput("empty executable"));
        }
        Ok(())
    }

    fn validate_limits(limits: ResourceLimits) -> Result<(), ScopeError> {
        if limits.memory_max_bytes == 0
            || limits.cpu_quota_percent == 0
            || limits.cpu_quota_percent > 10_000
            || limits.tasks_max == 0
            || limits.tasks_max > 65_536
        {
            return Err(ScopeError::InvalidInput(
                "resource limits are zero or outside supported bounds",
            ));
        }
        Ok(())
    }

    fn wait_for_marker(
        mut stdout: ChildStdout,
        expected: &'static [u8],
        timeout: Duration,
    ) -> Result<ChildStdout, ScopeError> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut marker = vec![0; expected.len()];
            let result = stdout.read_exact(&mut marker).and_then(|()| {
                if marker == expected {
                    Ok(())
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "scope gate emitted an invalid protocol marker",
                    ))
                }
            });
            let _ = sender.send((stdout, result));
        });
        match receiver.recv_timeout(timeout) {
            Ok((stdout, Ok(()))) => Ok(stdout),
            Ok((_stdout, Err(error))) => Err(ScopeError::Io(error)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(ScopeError::Control(
                "scope gate did not become ready before the deadline".into(),
            )),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(ScopeError::Control(
                "scope gate readiness channel closed".into(),
            )),
        }
    }

    fn abort_scope_with(
        mut scope: ManagedScope,
        timeout: Duration,
        wait_child: impl FnOnce(&mut Child, Instant) -> Result<ExitStatus, ScopeError>,
    ) -> Result<Option<String>, Box<(ManagedScope, ScopeError)>> {
        match scope.kill_and_wait_with(timeout, wait_child) {
            Ok(_) => Ok(take_stderr_diagnostic(&mut scope.child)),
            Err(cleanup_error) => {
                // Try a bounded direct-process reap, but retain the full scope even when
                // that succeeds: it does not prove the recursive cgroup is empty.
                let _ = kill_direct_until(&mut scope.child, timeout);
                Err(Box::new((scope, cleanup_error)))
            }
        }
    }

    fn fail_scope(scope: ManagedScope, cause: ScopeError, timeout: Duration) -> ScopeError {
        fail_scope_with(scope, cause, timeout, wait_child_until)
    }

    fn fail_scope_with(
        scope: ManagedScope,
        cause: ScopeError,
        timeout: Duration,
        wait_child: impl FnOnce(&mut Child, Instant) -> Result<ExitStatus, ScopeError>,
    ) -> ScopeError {
        match abort_scope_with(scope, timeout, wait_child) {
            Ok(diagnostic) => with_gate_diagnostic(cause, diagnostic),
            Err(scope_and_error) => {
                let (scope, cleanup_error) = *scope_and_error;
                ScopeError::CleanupPending {
                    cause: Box::new(cause),
                    cleanup_error: Box::new(cleanup_error),
                    pending: Box::new(PendingCleanup::ManagedScope { scope }),
                }
            }
        }
    }

    fn kill_direct_until(child: &mut Child, timeout: Duration) -> bool {
        let _ = child.kill();
        let deadline = Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) if Instant::now() < deadline => thread::sleep(
                    POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                ),
                Ok(None) | Err(_) => return false,
            }
        }
    }

    fn terminate_scope(
        commands: &SystemdCommands,
        unit: &str,
        slice_unit: &str,
        mut child: Child,
        cgroup_events: Option<PathBuf>,
    ) -> Result<Option<String>, Box<(PendingCleanup, ScopeError)>> {
        let _ = run_control(
            &commands.systemctl,
            [
                "--user",
                "--no-pager",
                "kill",
                "--kill-whom=all",
                "--signal=SIGKILL",
            ],
            Some(unit),
            CONTROL_TIMEOUT,
        );
        if !kill_direct_until(&mut child, CONTROL_TIMEOUT) {
            return Err(Box::new((
                PendingCleanup::PreWorker {
                    commands: commands.clone(),
                    unit: unit.to_owned(),
                    slice_unit: slice_unit.to_owned(),
                    child,
                    cgroup_events,
                },
                ScopeError::QuiescenceTimeout,
            )));
        }
        let status = match child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => {
                return Err(Box::new((
                    PendingCleanup::PreWorker {
                        commands: commands.clone(),
                        unit: unit.to_owned(),
                        slice_unit: slice_unit.to_owned(),
                        child,
                        cgroup_events,
                    },
                    ScopeError::QuiescenceTimeout,
                )));
            }
            Err(error) => {
                return Err(Box::new((
                    PendingCleanup::PreWorker {
                        commands: commands.clone(),
                        unit: unit.to_owned(),
                        slice_unit: slice_unit.to_owned(),
                        child,
                        cgroup_events,
                    },
                    ScopeError::Io(error),
                )));
            }
        };
        if let Err(error) =
            prove_preworker_quiescent(commands, unit, cgroup_events.as_deref(), CONTROL_TIMEOUT)
        {
            return Err(Box::new((
                PendingCleanup::PreWorker {
                    commands: commands.clone(),
                    unit: unit.to_owned(),
                    slice_unit: slice_unit.to_owned(),
                    child,
                    cgroup_events,
                },
                error,
            )));
        }
        if let Err(error) = stop_attempt_slice(commands, slice_unit, CONTROL_TIMEOUT) {
            return Err(Box::new((
                PendingCleanup::PreWorker {
                    commands: commands.clone(),
                    unit: unit.to_owned(),
                    slice_unit: slice_unit.to_owned(),
                    child,
                    cgroup_events,
                },
                error,
            )));
        }
        let _ = status;
        Ok(take_stderr_diagnostic(&mut child))
    }

    fn fail_before_worker(
        commands: &SystemdCommands,
        unit: &str,
        slice_unit: &str,
        child: Child,
        cgroup_events: Option<PathBuf>,
        cause: ScopeError,
    ) -> ScopeError {
        match terminate_scope(commands, unit, slice_unit, child, cgroup_events) {
            Ok(diagnostic) => with_gate_diagnostic(cause, diagnostic),
            Err(pending_and_error) => {
                let (pending, cleanup_error) = *pending_and_error;
                ScopeError::CleanupPending {
                    cause: Box::new(cause),
                    cleanup_error: Box::new(cleanup_error),
                    pending: Box::new(pending),
                }
            }
        }
    }

    fn prove_preworker_quiescent(
        commands: &SystemdCommands,
        unit: &str,
        known_events: Option<&Path>,
        timeout: Duration,
    ) -> Result<(), ScopeError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ScopeError::InvalidInput("timeout overflow"))?;
        let mount = cgroup2_mount().ok();
        let mut saw_observation = false;
        loop {
            let Some(call_timeout) = remaining_control_timeout(deadline) else {
                return Err(if saw_observation {
                    ScopeError::QuiescenceTimeout
                } else {
                    ScopeError::QuiescenceUnobservable
                });
            };
            if let Ok(output) = run_control(
                &commands.systemctl,
                [
                    "--user",
                    "--no-pager",
                    "show",
                    "--value",
                    "--property=ActiveState",
                ],
                Some(unit),
                call_timeout,
            ) {
                saw_observation = true;
                let active_state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if matches!(active_state.as_str(), "inactive" | "dead") {
                    return Ok(());
                }
            }

            let discovered_events = if known_events.is_some() {
                None
            } else if let Some(mount) = mount.as_ref() {
                remaining_control_timeout(deadline).and_then(|call_timeout| {
                    run_control(
                        &commands.systemctl,
                        [
                            "--user",
                            "--no-pager",
                            "show",
                            "--value",
                            "--property=ControlGroup",
                        ],
                        Some(unit),
                        call_timeout,
                    )
                    .ok()
                    .and_then(|output| {
                        let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                        let relative = cgroup_relative_path(&value, &mount.root)?;
                        Some(mount.mountpoint.join(relative).join("cgroup.events"))
                    })
                })
            } else {
                None
            };
            if let Some(events) = known_events.or(discovered_events.as_deref()) {
                match read_populated(events) {
                    Ok(false) => return Ok(()),
                    Ok(true) => saw_observation = true,
                    Err(error) if error.kind() != io::ErrorKind::NotFound => saw_observation = true,
                    Err(_) => {}
                }
            }

            if Instant::now() >= deadline {
                return Err(if saw_observation {
                    ScopeError::QuiescenceTimeout
                } else {
                    ScopeError::QuiescenceUnobservable
                });
            }
            thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
        }
    }

    fn remaining_control_timeout(deadline: Instant) -> Option<Duration> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            None
        } else {
            Some(remaining.min(CONTROL_TIMEOUT))
        }
    }

    fn take_stderr_diagnostic(child: &mut Child) -> Option<String> {
        child.stderr.take().and_then(|mut stderr| {
            let (bytes, over_limit) = read_limited(&mut stderr, OUTPUT_LIMIT).ok()?;
            let message = String::from_utf8_lossy(&bytes).trim().to_owned();
            if message.is_empty() {
                None
            } else if over_limit {
                Some(format!("{} (stderr truncated)", message))
            } else {
                Some(message)
            }
        })
    }

    fn with_gate_diagnostic(error: ScopeError, diagnostic: Option<String>) -> ScopeError {
        match (error, diagnostic) {
            (ScopeError::Io(error), Some(detail)) => {
                ScopeError::Control(format!("scope gate failed: {detail} ({error})"))
            }
            (ScopeError::Control(error), Some(detail)) => {
                ScopeError::Control(format!("{error}: {detail}"))
            }
            (error, _) => error,
        }
    }

    fn verify_membership(pid: u32, expected: &Path, mount_root: &Path) -> Result<(), ScopeError> {
        let membership =
            fs::read_to_string(format!("/proc/{pid}/cgroup")).map_err(ScopeError::Io)?;
        let observed = membership
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .and_then(|path| cgroup_relative_path(path, mount_root))
            .ok_or_else(|| {
                ScopeError::Unsupported(
                    "worker cgroup membership could not be read as unified cgroup-v2".into(),
                )
            })?;
        if observed != expected {
            return Err(ScopeError::Unsupported(
                "scope gate process is not in the expected Attempt cgroup".into(),
            ));
        }
        Ok(())
    }

    fn safe_environment(env: &AgentEnvironment) -> Result<Vec<(&'static str, String)>, ScopeError> {
        let home = safe_path_value(&env.home)?;
        let mut result = vec![("HOME", home)];
        if env.search_path.is_empty() {
            return Err(ScopeError::InvalidInput("agent PATH must not be empty"));
        }
        let mut paths = Vec::with_capacity(env.search_path.len());
        for path in &env.search_path {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, Component::ParentDir))
            {
                return Err(ScopeError::InvalidInput(
                    "PATH entries must be absolute and normalized",
                ));
            }
            let value = safe_path_value(path)?;
            if value.contains(':') {
                return Err(ScopeError::InvalidInput(
                    "PATH entries cannot contain colons",
                ));
            }
            paths.push(value);
        }
        result.push(("PATH", paths.join(":")));
        for (name, value) in [
            ("XDG_CONFIG_HOME", env.config_home.as_ref()),
            ("XDG_DATA_HOME", env.data_home.as_ref()),
            ("XDG_CACHE_HOME", env.cache_home.as_ref()),
            ("TMPDIR", env.temp_dir.as_ref()),
        ] {
            if let Some(value) = value {
                if !value.is_absolute()
                    || value
                        .components()
                        .any(|part| matches!(part, Component::ParentDir))
                {
                    return Err(ScopeError::InvalidInput(
                        "XDG and temporary paths must be absolute and normalized",
                    ));
                }
                result.push((name, safe_path_value(value)?));
            }
        }
        if let Some(locale) = env.locale.as_ref() {
            if locale.is_empty()
                || !locale
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.@".contains(&b))
            {
                return Err(ScopeError::InvalidInput(
                    "locale contains unsupported characters",
                ));
            }
            result.push(("LANG", locale.clone()));
        }
        Ok(result)
    }

    pub(super) fn sandbox_environment_values(
        env: &AgentEnvironment,
    ) -> Result<Vec<(&'static str, String)>, ScopeError> {
        safe_environment(env)
    }

    fn safe_path_value(path: &Path) -> Result<String, ScopeError> {
        let bytes = path.as_os_str().as_bytes();
        let text = std::str::from_utf8(bytes)
            .map_err(|_| ScopeError::InvalidInput("environment paths must be UTF-8"))?;
        if text.contains(['\0', '\n', '\r']) || text.contains('=') {
            return Err(ScopeError::InvalidInput(
                "environment path contains unsupported characters",
            ));
        }
        Ok(text.to_owned())
    }

    fn unit_name(attempt_id: [u8; 16]) -> String {
        let mut out = String::from(PREFIX);
        for byte in attempt_id {
            out.push_str(&format!("{byte:02x}"));
        }
        out.push_str(".scope");
        out
    }

    fn attempt_slice_name(attempt_id: [u8; 16]) -> String {
        let mut out = String::from(PREFIX);
        for byte in attempt_id {
            out.push_str(&format!("{byte:02x}"));
        }
        out.push_str(".slice");
        out
    }

    fn start_attempt_slice(
        commands: &SystemdCommands,
        slice_unit: &str,
        mount: &CgroupMount,
    ) -> Result<(PathBuf, CgroupPopulatedObserver), ScopeError> {
        start_attempt_slice_with_resolver(commands, slice_unit, mount, resolve_scope_cgroup)
    }

    fn start_attempt_slice_with_resolver(
        commands: &SystemdCommands,
        slice_unit: &str,
        mount: &CgroupMount,
        resolve: impl FnOnce(
            &SystemdCommands,
            &str,
            &CgroupMount,
            Duration,
        ) -> Result<(PathBuf, PathBuf), ScopeError>,
    ) -> Result<(PathBuf, CgroupPopulatedObserver), ScopeError> {
        if let Err(error) = run_control(
            &commands.systemctl,
            ["--user", "--no-pager", "start"],
            Some(slice_unit),
            CONTROL_TIMEOUT,
        ) {
            return Err(fail_started_attempt_slice(
                commands, slice_unit, None, error,
            ));
        }
        let (slice_cgroup, _) = match resolve(commands, slice_unit, mount, CONTROL_TIMEOUT) {
            Ok(resolved) => resolved,
            Err(error) => {
                return Err(fail_started_attempt_slice(
                    commands, slice_unit, None, error,
                ));
            }
        };
        let events_path = slice_cgroup.join("cgroup.events");
        match CgroupPopulatedObserver::require_empty_parent(events_path.clone()) {
            Ok(observer) => Ok((slice_cgroup, observer)),
            Err(error) => Err(fail_started_attempt_slice(
                commands,
                slice_unit,
                Some(events_path),
                error,
            )),
        }
    }

    fn stop_attempt_slice(
        commands: &SystemdCommands,
        slice_unit: &str,
        timeout: Duration,
    ) -> Result<(), ScopeError> {
        run_control(
            &commands.systemctl,
            ["--user", "--no-pager", "stop"],
            Some(slice_unit),
            timeout,
        )?;
        Ok(())
    }

    fn resolve_scope_cgroup(
        commands: &SystemdCommands,
        unit: &str,
        mount: &CgroupMount,
        timeout: Duration,
    ) -> Result<(PathBuf, PathBuf), ScopeError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(output) = run_control(
                &commands.systemctl,
                [
                    "--user",
                    "--no-pager",
                    "show",
                    "--value",
                    "--property=ControlGroup",
                ],
                Some(unit),
                timeout,
            ) {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                if let Some(relative) = cgroup_relative_path(&value, &mount.root) {
                    let path = mount.mountpoint.join(&relative);
                    if path.join("cgroup.events").is_file() {
                        return Ok((path, relative));
                    }
                }
            }
            if Instant::now() >= deadline {
                return Err(ScopeError::Unsupported(
                    "transient scope cgroup could not be resolved".into(),
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn cgroup_is_descendant(parent: &Path, child: &Path) -> bool {
        child != parent && child.starts_with(parent)
    }

    fn run_control<const N: usize>(
        program: &Path,
        prefix: [&str; N],
        unit: Option<&str>,
        timeout: Duration,
    ) -> Result<Output, ScopeError> {
        let mut command = Command::new(program);
        command.args(prefix);
        if let Some(unit) = unit {
            command.arg(unit);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(ScopeError::Io)?;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                drop(child.stderr.take());
                let _ = kill_direct_until(&mut child, timeout);
                return Err(ScopeError::Control("control stdout unavailable".into()));
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                drop(stdout);
                let _ = kill_direct_until(&mut child, timeout);
                return Err(ScopeError::Control("control stderr unavailable".into()));
            }
        };
        let out_reader = thread::spawn(move || read_limited(stdout, OUTPUT_LIMIT));
        let err_reader = thread::spawn(move || read_limited(stderr, OUTPUT_LIMIT));
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
                Ok(None) => {
                    let _ = kill_direct_until(&mut child, timeout);
                    return Err(ScopeError::Control("systemd command timed out".into()));
                }
                Err(error) => {
                    let _ = kill_direct_until(&mut child, timeout);
                    return Err(ScopeError::Io(error));
                }
            }
        };
        let stdout = out_reader
            .join()
            .map_err(|_| ScopeError::Control("systemd stdout reader failed".into()))?
            .map_err(ScopeError::Io)?;
        let stderr = err_reader
            .join()
            .map_err(|_| ScopeError::Control("systemd stderr reader failed".into()))?
            .map_err(ScopeError::Io)?;
        if stdout.1 || stderr.1 {
            return Err(ScopeError::Control(
                "systemd response exceeded its output bound".into(),
            ));
        }
        if !status.success() {
            return Err(ScopeError::Control(
                String::from_utf8_lossy(&stderr.0).trim().to_owned(),
            ));
        }
        Ok(Output {
            status,
            stdout: stdout.0,
            stderr: stderr.0,
        })
    }

    fn read_limited(mut input: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
        // Read at most limit+1 bytes, then drop the pipe. A writer that exceeds the
        // contract receives EPIPE/SIGPIPE; it cannot block this bounded control path by
        // filling an undrained pipe.
        let mut bytes = Vec::with_capacity(limit.min(1024));
        input
            .by_ref()
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)?;
        let over = bytes.len() > limit;
        bytes.truncate(limit);
        Ok((bytes, over))
    }

    #[derive(Debug)]
    struct CgroupMount {
        root: PathBuf,
        mountpoint: PathBuf,
    }

    fn cgroup2_mount() -> Result<CgroupMount, ScopeError> {
        let text = fs::read_to_string("/proc/self/mountinfo").map_err(ScopeError::Io)?;
        for line in text.lines() {
            let Some((before, after)) = line.split_once(" - ") else {
                continue;
            };
            let pre = before.split_ascii_whitespace().collect::<Vec<_>>();
            let post = after.split_ascii_whitespace().collect::<Vec<_>>();
            if pre.len() < 5 || post.first() != Some(&"cgroup2") {
                continue;
            }
            let root = decode_mount_path(pre[3])?;
            let mountpoint = decode_mount_path(pre[4])?;
            return Ok(CgroupMount { root, mountpoint });
        }
        Err(ScopeError::Unsupported(
            "unified cgroup-v2 mount is not visible".into(),
        ))
    }

    fn decode_mount_path(text: &str) -> Result<PathBuf, ScopeError> {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' {
                if i + 3 >= bytes.len()
                    || !bytes[i + 1..i + 4]
                        .iter()
                        .all(|b| (b'0'..=b'7').contains(b))
                {
                    return Err(ScopeError::Unsupported(
                        "malformed mountinfo escaping".into(),
                    ));
                }
                let value =
                    (bytes[i + 1] - b'0') * 64 + (bytes[i + 2] - b'0') * 8 + bytes[i + 3] - b'0';
                out.push(value);
                i += 4;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        let path = PathBuf::from(OsStr::from_bytes(&out));
        if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
            return Err(ScopeError::Unsupported("invalid cgroup mount path".into()));
        }
        Ok(path)
    }

    fn cgroup_relative_path(value: &str, mount_root: &Path) -> Option<PathBuf> {
        if value.is_empty() || !value.starts_with('/') || value.contains('\0') {
            return None;
        }
        let path = Path::new(value);
        if path.components().any(|p| matches!(p, Component::ParentDir)) {
            return None;
        }
        let relative = path.strip_prefix(mount_root).ok()?;
        Some(relative.to_path_buf())
    }

    fn read_trimmed(path: PathBuf) -> Result<String, ScopeError> {
        Ok(fs::read_to_string(path)
            .map_err(ScopeError::Io)?
            .trim()
            .to_owned())
    }

    fn read_populated(path: &Path) -> io::Result<bool> {
        let value = fs::read_to_string(path)?;
        for line in value.lines() {
            if let Some(value) = line.strip_prefix("populated ") {
                return match value {
                    "0" => Ok(false),
                    "1" => Ok(true),
                    _ => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid cgroup populated value",
                    )),
                };
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cgroup.events has no populated field",
        ))
    }

    trait ChildWait {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
    }

    impl ChildWait for Child {
        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            Child::try_wait(self)
        }
    }

    fn wait_child_until(
        child: &mut impl ChildWait,
        deadline: Instant,
    ) -> Result<ExitStatus, ScopeError> {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
                Ok(None) => return Err(ScopeError::QuiescenceTimeout),
                Err(error) => return Err(ScopeError::Io(error)),
            }
        }
    }

    pub use {
        AgentEnvironment as PublicAgentEnvironment, LaunchSpec as PublicLaunchSpec,
        ManagedScope as PublicManagedScope, PendingCleanup as PublicPendingCleanup,
        ResourceLimits as PublicResourceLimits, ScopeError as PublicScopeError,
        SystemdCommands as PublicSystemdCommands,
    };

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn test_systemctl(directory: &Path) -> (PathBuf, PathBuf) {
            let path = directory.join("systemctl-test");
            let log = directory.join("systemctl.log");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$3\" >> \"{}\"\n[ \"$3\" != kill ]\n",
                    log.display()
                ),
            )
            .expect("write fake systemctl executable");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("fake systemctl is executable");
            (path, log)
        }

        fn test_systemctl_failing_start(directory: &Path) -> (PathBuf, PathBuf) {
            let path = directory.join("systemctl-failing-start-test");
            let log = directory.join("systemctl-failing-start.log");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$3\" >> \"{}\"\nexit 1\n",
                    log.display()
                ),
            )
            .expect("write failing fake systemctl executable");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("fake systemctl is executable");
            (path, log)
        }

        fn test_systemctl_ambiguous_start(directory: &Path) -> (PathBuf, PathBuf) {
            let path = directory.join("systemctl-ambiguous-start-test");
            let log = directory.join("systemctl-ambiguous-start.log");
            let stop_count = directory.join("systemctl-ambiguous-start-stop-count");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s %s\\n' \"$3\" \"$4\" >> \"{}\"\ncase \"$3\" in start) exit 1 ;; stop) count=$(cat \"{}\" 2>/dev/null || echo 0); count=$((count + 1)); echo \"$count\" > \"{}\"; [ \"$count\" -gt 1 ] || exit 1 ;; show) echo inactive ;; esac\nexit 0\n",
                    log.display(),
                    stop_count.display(),
                    stop_count.display(),
                ),
            )
            .expect("write ambiguous-start fake systemctl");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("fake systemctl is executable");
            (path, log)
        }

        fn test_systemctl_stop_fails_once(directory: &Path) -> (PathBuf, PathBuf) {
            let path = directory.join("systemctl-stop-fails-once-test");
            let log = directory.join("systemctl-stop-fails-once.log");
            let failed_once = directory.join("systemctl-stop-failed-once");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s %s\\n' \"$3\" \"$4\" >> \"{}\"\nif [ \"$3\" = stop ] && [ ! -e \"{}\" ]; then touch \"{}\"; exit 1; fi\nexit 0\n",
                    log.display(),
                    failed_once.display(),
                    failed_once.display(),
                ),
            )
            .expect("write one-shot stop failure fake");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("fake systemctl is executable");
            (path, log)
        }

        fn test_systemctl_stop_fails_twice(directory: &Path) -> (PathBuf, PathBuf) {
            let path = directory.join("systemctl-stop-fails-twice-test");
            let log = directory.join("systemctl-stop-fails-twice.log");
            let stop_count = directory.join("systemctl-stop-count");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s %s\\n' \"$3\" \"$4\" >> \"{}\"\nif [ \"$3\" = stop ]; then count=$(cat \"{}\" 2>/dev/null || echo 0); count=$((count + 1)); echo \"$count\" > \"{}\"; [ \"$count\" -gt 2 ] || exit 1; fi\nexit 0\n",
                    log.display(),
                    stop_count.display(),
                    stop_count.display(),
                ),
            )
            .expect("write two-shot stop failure fake");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("fake systemctl is executable");
            (path, log)
        }

        fn test_systemctl_stop_fails_on_retry(directory: &Path) -> (PathBuf, PathBuf) {
            let path = directory.join("systemctl-stop-fails-on-retry-test");
            let log = directory.join("systemctl-stop-fails-on-retry.log");
            let stop_count = directory.join("systemctl-stop-retry-count");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s %s\\n' \"$3\" \"$4\" >> \"{}\"\nif [ \"$3\" = stop ]; then count=$(cat \"{}\" 2>/dev/null || echo 0); count=$((count + 1)); echo \"$count\" > \"{}\"; [ \"$count\" -ne 2 ] || exit 1; fi\nexit 0\n",
                    log.display(),
                    stop_count.display(),
                    stop_count.display(),
                ),
            )
            .expect("write retry-stop failure fake");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("fake systemctl is executable");
            (path, log)
        }

        fn test_managed_scope(
            systemctl: PathBuf,
            parent_events: PathBuf,
            child: Child,
        ) -> ManagedScope {
            ManagedScope {
                commands: SystemdCommands {
                    systemd_run: PathBuf::from("/usr/bin/systemd-run"),
                    systemctl,
                    scope_gate: PathBuf::from("/usr/bin/litecowork-scope-gate"),
                    env: PathBuf::from("/usr/bin/env"),
                },
                unit: "litecowork-attempt-test.scope".to_owned(),
                slice_unit: "litecowork-attempt-test.slice".to_owned(),
                cgroup_events: parent_events
                    .parent()
                    .expect("parent cgroup directory exists")
                    .join("worker/cgroup.events"),
                cgroup_observer: CgroupPopulatedObserver {
                    events_path: parent_events,
                },
                child,
                limits: ResourceLimits {
                    memory_max_bytes: 1024,
                    cpu_quota_percent: 100,
                    tasks_max: 8,
                },
            }
        }

        fn test_child() -> Child {
            Command::new("/bin/sh")
                .args(["-c", "exec sleep 2"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("test child starts")
        }

        fn valid_spec() -> (SystemdCommands, LaunchSpec) {
            (
                SystemdCommands {
                    systemd_run: PathBuf::from("/usr/bin/systemd-run"),
                    systemctl: PathBuf::from("/usr/bin/systemctl"),
                    scope_gate: PathBuf::from("/usr/bin/litecowork-scope-gate"),
                    env: PathBuf::from("/usr/bin/env"),
                },
                LaunchSpec {
                    attempt_id: [1; 16],
                    executable: PathBuf::from("/usr/bin/codex"),
                    args: vec![OsString::from("app-server")],
                    working_directory: PathBuf::from("/workspace/task"),
                    environment: AgentEnvironment {
                        home: PathBuf::from("/workspace/task/home"),
                        search_path: Vec::new(),
                        config_home: None,
                        data_home: None,
                        cache_home: None,
                        temp_dir: None,
                        locale: None,
                    },
                    limits: ResourceLimits {
                        memory_max_bytes: 1024 * 1024 * 1024,
                        cpu_quota_percent: 200,
                        tasks_max: 128,
                    },
                },
            )
        }

        #[test]
        fn launch_paths_must_be_absolute_and_normalized() {
            let (commands, spec) = valid_spec();
            assert!(validate_paths(&commands, &spec).is_ok());

            let mut unsafe_spec = spec.clone();
            unsafe_spec.working_directory = PathBuf::from("/workspace/../home");
            assert!(matches!(
                validate_paths(&commands, &unsafe_spec),
                Err(ScopeError::InvalidInput(_))
            ));
        }

        #[test]
        fn resource_limits_reject_zero_and_out_of_range_values() {
            let mut limits = valid_spec().1.limits;
            assert!(validate_limits(limits).is_ok());
            limits.cpu_quota_percent = 10_001;
            assert!(validate_limits(limits).is_err());
            limits.cpu_quota_percent = 1;
            limits.tasks_max = 0;
            assert!(validate_limits(limits).is_err());
        }

        #[test]
        fn invalid_environment_is_rejected_before_starting_attempt_slice() {
            let directory = tempfile::tempdir().expect("temporary test directory");
            let (systemctl, log) = test_systemctl_failing_start(directory.path());
            let (mut commands, mut spec) = valid_spec();
            commands.systemctl = systemctl;
            spec.environment.search_path.clear();

            assert!(matches!(
                spawn(&commands, spec),
                Err(ScopeError::InvalidInput("agent PATH must not be empty"))
            ));
            assert!(
                !log.exists(),
                "invalid caller environment must not start or stop a systemd slice"
            );
        }

        #[test]
        fn exec_failure_returns_original_error_when_slice_cleanup_succeeds() {
            let directory = tempfile::tempdir().expect("temporary process-scope fixture");
            let (systemctl, log) = test_systemctl(directory.path());
            let (mut commands, mut spec) = valid_spec();
            commands.systemctl = systemctl;
            spec.environment.search_path = vec![PathBuf::from("/usr/bin")];
            let environment = safe_environment(&spec.environment).expect("valid environment");
            let mount = CgroupMount {
                root: directory.path().to_owned(),
                mountpoint: directory.path().to_owned(),
            };
            let slice_events = directory.path().join("slice/cgroup.events");
            fs::create_dir_all(slice_events.parent().expect("slice directory"))
                .expect("create parent slice fixture");
            fs::write(&slice_events, "populated 0\n").expect("slice is empty");
            let launch_calls = std::cell::Cell::new(0);

            let error = spawn_with(
                &commands,
                spec,
                environment,
                mount,
                |_, _, _| {
                    let observer =
                        CgroupPopulatedObserver::require_empty_parent(slice_events.clone())?;
                    Ok((slice_events.parent().unwrap().to_owned(), observer))
                },
                |_| {
                    launch_calls.set(launch_calls.get() + 1);
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "injected exec failure",
                    ))
                },
            )
            .expect_err("launcher exec failure must be returned");

            assert!(
                matches!(error, ScopeError::Io(ref error) if error.kind() == io::ErrorKind::NotFound)
            );
            assert_eq!(launch_calls.get(), 1, "no worker process was admitted");
            assert_eq!(
                fs::read_to_string(log)
                    .expect("slice stop was logged")
                    .trim(),
                "stop"
            );
        }

        #[test]
        fn exec_failure_retains_slice_cleanup_for_exact_retry() {
            let directory = tempfile::tempdir().expect("temporary process-scope fixture");
            let (systemctl, log) = test_systemctl_stop_fails_twice(directory.path());
            let (mut commands, mut spec) = valid_spec();
            commands.systemctl = systemctl;
            spec.environment.search_path = vec![PathBuf::from("/usr/bin")];
            let environment = safe_environment(&spec.environment).expect("valid environment");
            let mount = CgroupMount {
                root: directory.path().to_owned(),
                mountpoint: directory.path().to_owned(),
            };
            let slice_events = directory.path().join("slice/cgroup.events");
            fs::create_dir_all(slice_events.parent().expect("slice directory"))
                .expect("create parent slice fixture");
            fs::write(&slice_events, "populated 0\n").expect("slice is empty");
            let launch_calls = std::cell::Cell::new(0);

            let error = spawn_with(
                &commands,
                spec,
                environment,
                mount,
                |_, _, _| {
                    let observer =
                        CgroupPopulatedObserver::require_empty_parent(slice_events.clone())?;
                    Ok((slice_events.parent().unwrap().to_owned(), observer))
                },
                |_| {
                    launch_calls.set(launch_calls.get() + 1);
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "injected exec failure",
                    ))
                },
            )
            .expect_err("failed stop must retain cleanup ownership");
            let ScopeError::CleanupPending {
                cause,
                cleanup_error,
                mut pending,
            } = error
            else {
                panic!("failed slice stop must return a retryable cleanup handle");
            };
            assert!(
                matches!(*cause, ScopeError::Io(ref error) if error.kind() == io::ErrorKind::NotFound)
            );
            assert!(matches!(*cleanup_error, ScopeError::Control(_)));
            assert_eq!(
                pending.unit_name(),
                "litecowork-attempt-01010101010101010101010101010101.slice"
            );
            assert!(matches!(*pending, PendingCleanup::AttemptSlice { .. }));
            fs::remove_file(&slice_events).expect("make quiescence proof unavailable");
            assert!(matches!(
                pending.retry_stop(Duration::from_millis(20)),
                Err(ScopeError::QuiescenceTimeout)
            ));
            assert!(matches!(
                pending.retry_stop(Duration::from_millis(20)),
                Err(ScopeError::QuiescenceTimeout)
            ));
            fs::write(&slice_events, "populated 0\n").expect("restore positive empty proof");
            pending
                .retry_stop(Duration::from_secs(1))
                .expect("later retry settles after positive proof returns");
            assert_eq!(launch_calls.get(), 1, "retry never launches a worker");
            assert_eq!(
                fs::read_to_string(log)
                    .expect("both stop attempts were logged")
                    .lines()
                    .collect::<Vec<_>>(),
                [
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "show --value",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "show --value",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "show --value",
                ]
            );
        }

        #[test]
        fn started_slice_retains_cleanup_when_parent_validation_and_stop_fail() {
            let directory = tempfile::tempdir().expect("temporary process-scope fixture");
            let (systemctl, log) = test_systemctl_stop_fails_once(directory.path());
            let commands = SystemdCommands {
                systemd_run: PathBuf::from("/usr/bin/systemd-run"),
                systemctl,
                scope_gate: PathBuf::from("/usr/bin/litecowork-scope-gate"),
                env: PathBuf::from("/usr/bin/env"),
            };
            let slice_unit = "litecowork-attempt-01010101010101010101010101010101.slice";
            let cgroup = directory.path().join("slice");
            let events = cgroup.join("cgroup.events");
            fs::create_dir_all(&cgroup).expect("create slice cgroup fixture");
            fs::write(&events, "populated 1\n")
                .expect("initially populated slice fails validation");
            let mount = CgroupMount {
                root: directory.path().to_owned(),
                mountpoint: directory.path().to_owned(),
            };

            let error =
                start_attempt_slice_with_resolver(&commands, slice_unit, &mount, |_, _, _, _| {
                    Ok((cgroup.clone(), PathBuf::from("slice")))
                })
                .expect_err("failed validation and stop retain cleanup ownership");
            let ScopeError::CleanupPending {
                cause,
                cleanup_error,
                mut pending,
            } = error
            else {
                panic!("failed stop must retain the started Attempt slice");
            };
            assert!(matches!(*cause, ScopeError::QuiescenceUnobservable));
            assert!(matches!(*cleanup_error, ScopeError::Control(_)));
            assert_eq!(pending.unit_name(), slice_unit);
            assert!(matches!(
                &*pending,
                PendingCleanup::AttemptSlice {
                    cgroup_events: Some(path),
                    ..
                } if path == &events
            ));

            fs::write(&events, "populated 0\n").expect("restore positive empty proof");
            pending
                .retry_stop(Duration::from_secs(1))
                .expect("retry stops the exact slice and proves it empty");
            assert_eq!(
                fs::read_to_string(log)
                    .expect("start and stop operations were logged")
                    .lines()
                    .collect::<Vec<_>>(),
                [
                    "start litecowork-attempt-01010101010101010101010101010101.slice",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "show --value",
                ]
            );
        }

        #[test]
        fn ambiguous_slice_start_failure_retains_exact_cleanup_ownership() {
            let directory = tempfile::tempdir().expect("temporary process-scope fixture");
            let (systemctl, log) = test_systemctl_ambiguous_start(directory.path());
            let commands = SystemdCommands {
                systemd_run: PathBuf::from("/usr/bin/systemd-run"),
                systemctl,
                scope_gate: PathBuf::from("/usr/bin/litecowork-scope-gate"),
                env: PathBuf::from("/usr/bin/env"),
            };
            let slice_unit = "litecowork-attempt-01010101010101010101010101010101.slice";
            let mount = CgroupMount {
                root: directory.path().to_owned(),
                mountpoint: directory.path().to_owned(),
            };

            let error =
                start_attempt_slice_with_resolver(&commands, slice_unit, &mount, |_, _, _, _| {
                    panic!("a failed start must not resolve a cgroup")
                })
                .expect_err("ambiguous start failure must preserve cleanup ownership");
            let ScopeError::CleanupPending {
                cause,
                cleanup_error,
                mut pending,
            } = error
            else {
                panic!("ambiguous start and failed stop must retain a retry handle");
            };
            assert!(matches!(*cause, ScopeError::Control(_)));
            assert!(matches!(*cleanup_error, ScopeError::Control(_)));
            assert_eq!(pending.unit_name(), slice_unit);
            pending
                .retry_stop(Duration::from_secs(1))
                .expect("retry stops the slice and observes inactive unit state");
            assert_eq!(
                fs::read_to_string(log)
                    .expect("start, stop, retry, and state query are logged")
                    .lines()
                    .collect::<Vec<_>>(),
                [
                    "start litecowork-attempt-01010101010101010101010101010101.slice",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "show --value",
                ]
            );
        }

        #[test]
        fn exec_failure_retains_slice_when_quiescence_cannot_be_proven() {
            let directory = tempfile::tempdir().expect("temporary process-scope fixture");
            let (systemctl, log) = test_systemctl_stop_fails_on_retry(directory.path());
            let (mut commands, mut spec) = valid_spec();
            commands.systemctl = systemctl;
            spec.environment.search_path = vec![PathBuf::from("/usr/bin")];
            let environment = safe_environment(&spec.environment).expect("valid environment");
            let mount = CgroupMount {
                root: directory.path().to_owned(),
                mountpoint: directory.path().to_owned(),
            };
            let slice_events = directory.path().join("slice/cgroup.events");
            fs::create_dir_all(slice_events.parent().expect("slice directory"))
                .expect("create parent slice fixture");
            fs::write(&slice_events, "populated 0\n").expect("slice is initially empty");

            let error = spawn_with(
                &commands,
                spec,
                environment,
                mount,
                |_, _, _| {
                    let observer =
                        CgroupPopulatedObserver::require_empty_parent(slice_events.clone())?;
                    Ok((slice_events.parent().unwrap().to_owned(), observer))
                },
                |_| {
                    fs::remove_file(&slice_events).expect("remove quiescence evidence");
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "injected exec failure",
                    ))
                },
            )
            .expect_err("missing post-stop quiescence evidence must not settle cleanup");
            let ScopeError::CleanupPending {
                cleanup_error,
                mut pending,
                ..
            } = error
            else {
                panic!("unobservable Attempt slice must retain retry ownership");
            };
            assert!(matches!(*cleanup_error, ScopeError::QuiescenceUnobservable));
            assert_eq!(
                pending.unit_name(),
                "litecowork-attempt-01010101010101010101010101010101.slice"
            );
            fs::write(&slice_events, "populated 0\n").expect("restore positive empty proof");
            pending
                .retry_stop(Duration::from_secs(1))
                .expect("positive empty proof settles cleanup even if repeated stop fails");
            assert_eq!(
                fs::read_to_string(log)
                    .expect("initial stop, retry, and quiescence query are logged")
                    .lines()
                    .collect::<Vec<_>>(),
                [
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "stop litecowork-attempt-01010101010101010101010101010101.slice",
                    "show --value",
                ]
            );
        }

        #[test]
        fn observer_parent_must_be_a_strict_ancestor_of_worker_cgroup() {
            let parent = Path::new("/sys/fs/cgroup/user.slice/litecowork-attempt.slice");
            assert!(cgroup_is_descendant(
                parent,
                &parent.join("litecowork-attempt.scope")
            ));
            assert!(!cgroup_is_descendant(parent, parent));
            assert!(!cgroup_is_descendant(
                parent,
                Path::new("/sys/fs/cgroup/user.slice/other.scope")
            ));
        }

        #[test]
        fn scope_gate_handshake_accepts_only_the_exact_marker() {
            let mut child = Command::new("sh")
                .args([
                    "-c",
                    "printf 'LITECOWORK_SCOPE_GATE_V1\\nLITECOWORK_SCOPE_GO_CONSUMED_V1\\nnative-output'",
                ])
                .stdout(Stdio::piped())
                .spawn()
                .expect("test shell starts");
            let stdout = child.stdout.take().expect("stdout is piped");
            let stdout = wait_for_marker(stdout, GATE_READY, Duration::from_secs(1))
                .expect("exact gate marker is accepted");
            let mut stdout = wait_for_marker(stdout, GATE_GO_CONSUMED, Duration::from_secs(1))
                .expect("exact GO-consumed marker is accepted");
            let mut native_output = Vec::new();
            stdout
                .read_to_end(&mut native_output)
                .expect("remaining worker stdout is readable");
            assert_eq!(native_output, b"native-output");
            assert!(child.wait().expect("shell is reaped").success());

            let mut child = Command::new("sh")
                .args(["-c", "printf 'UNTRUSTED\\n'"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("test shell starts");
            let stdout = child.stdout.take().expect("stdout is piped");
            assert!(matches!(
                wait_for_marker(stdout, GATE_READY, Duration::from_secs(1)),
                Err(ScopeError::Io(_))
            ));
            assert!(child.wait().expect("shell is reaped").success());
        }

        #[test]
        fn gate_readiness_timeout_is_reported_before_worker_admission() {
            let mut child = Command::new("/bin/sh")
                .args(["-c", "sleep 0.05; printf 'LITECOWORK_SCOPE_GATE_V1\\n'"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("delayed gate starts");
            let stdout = child.stdout.take().expect("gate stdout is piped");
            assert!(matches!(
                wait_for_marker(stdout, GATE_READY, Duration::from_millis(1)),
                Err(ScopeError::Control(message))
                    if message == "scope gate did not become ready before the deadline"
            ));
            assert!(child.wait().expect("delayed gate exits").success());
        }

        #[test]
        fn direct_child_reap_failure_is_not_treated_as_quiescence() {
            let directory = tempfile::tempdir().expect("temporary cgroup fixture");
            let parent_events = directory.path().join("slice/cgroup.events");
            fs::create_dir_all(parent_events.parent().expect("slice directory"))
                .expect("create parent cgroup fixture");
            fs::write(&parent_events, "populated 0\n").expect("parent is observably empty");
            let (systemctl, log) = test_systemctl(directory.path());
            let scope = test_managed_scope(systemctl, parent_events, test_child());

            let error = fail_scope_with(
                scope,
                ScopeError::Control("injected pre-worker failure".to_owned()),
                Duration::from_secs(1),
                |_child, _deadline| {
                    Err(ScopeError::Io(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "injected child reap failure",
                    )))
                },
            );
            let ScopeError::CleanupPending {
                cleanup_error,
                pending,
                ..
            } = error
            else {
                panic!("child reap error must retain cleanup ownership");
            };
            assert!(matches!(
                *cleanup_error,
                ScopeError::Io(error) if error.kind() == io::ErrorKind::PermissionDenied
            ));

            let PendingCleanup::ManagedScope { mut scope } = *pending else {
                panic!("child reap error retains the managed scope");
            };
            let status = scope
                .kill_and_wait(Duration::from_secs(1))
                .expect("retained scope can retry child reaping");
            assert!(!status.success(), "the test worker was terminated");
            assert!(
                fs::read_to_string(&log)
                    .expect("systemctl commands are logged")
                    .lines()
                    .any(|line| line == "stop"),
                "slice teardown follows the successful retry"
            );
        }

        #[test]
        fn failed_systemd_kill_retains_retry_handle_until_cgroup_is_empty() {
            let directory = tempfile::tempdir().expect("temporary cgroup fixture");
            let parent_events = directory.path().join("slice/cgroup.events");
            fs::create_dir_all(parent_events.parent().expect("slice directory"))
                .expect("create parent cgroup fixture");
            fs::write(&parent_events, "populated 1\n").expect("parent has live process");
            let (systemctl, log) = test_systemctl(directory.path());
            let scope = test_managed_scope(systemctl, parent_events.clone(), test_child());

            let error = fail_scope(
                scope,
                ScopeError::Control("injected pre-worker failure".to_owned()),
                Duration::from_millis(40),
            );
            let ScopeError::CleanupPending { pending, .. } = error else {
                panic!("unobserved live cgroup must retain cleanup ownership");
            };
            assert_eq!(pending.unit_name(), "litecowork-attempt-test.scope");
            assert!(
                fs::read_to_string(&log)
                    .expect("systemctl kill invocation is logged")
                    .lines()
                    .any(|line| line == "kill"),
                "the fake systemd kill request fails while cgroup evidence remains populated"
            );

            let PendingCleanup::ManagedScope { mut scope } = *pending else {
                panic!("managed failure retains its managed scope");
            };
            assert!(matches!(
                scope.kill_and_wait(Duration::from_millis(20)),
                Err(ScopeError::QuiescenceTimeout)
            ));
            assert_eq!(scope.unit_name(), "litecowork-attempt-test.scope");

            fs::write(&parent_events, "populated 0\n").expect("publish recursive empty proof");
            let status = scope
                .kill_and_wait(Duration::from_secs(1))
                .expect("retry settles only after empty cgroup and child reap");
            assert!(!status.success(), "the test worker was terminated");
            assert!(
                fs::read_to_string(&log)
                    .expect("systemctl stop invocation is logged")
                    .lines()
                    .any(|line| line == "stop"),
                "slice teardown follows positive empty-cgroup evidence"
            );
        }

        #[test]
        fn cgroup_disappearance_is_unobservable_and_pending_retryable() {
            let directory = tempfile::tempdir().expect("temporary cgroup fixture");
            let parent_events = directory.path().join("slice/cgroup.events");
            fs::create_dir_all(parent_events.parent().expect("slice directory"))
                .expect("create parent cgroup fixture");
            let (systemctl, _) = test_systemctl(directory.path());
            let scope = test_managed_scope(systemctl, parent_events.clone(), test_child());

            let error = fail_scope(
                scope,
                ScopeError::Control("injected cgroup disappearance".to_owned()),
                Duration::from_millis(40),
            );
            let ScopeError::CleanupPending {
                cleanup_error,
                pending,
                ..
            } = error
            else {
                panic!("missing cgroup proof must retain cleanup ownership");
            };
            assert!(matches!(*cleanup_error, ScopeError::QuiescenceUnobservable));
            assert_eq!(pending.unit_name(), "litecowork-attempt-test.scope");
            let PendingCleanup::ManagedScope { mut scope } = *pending else {
                panic!("managed failure retains its managed scope");
            };

            fs::write(&parent_events, "populated 0\n").expect("restore an observable empty proof");
            scope
                .kill_and_wait(Duration::from_secs(1))
                .expect("retry settles after evidence becomes observable");
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{
    PublicAgentEnvironment as AgentEnvironment, PublicLaunchSpec as LaunchSpec,
    PublicManagedScope as ManagedScope, PublicPendingCleanup as PendingCleanup,
    PublicResourceLimits as ResourceLimits, PublicScopeError as ScopeError,
    PublicSystemdCommands as SystemdCommands, spawn,
};

#[cfg(target_os = "linux")]
mod bubblewrap;

#[cfg(target_os = "linux")]
pub use bubblewrap::{
    BubblewrapLaunchSpec, PreparedBubblewrapCommand, ReadOnlyRuntimeMount,
    bubblewrap_binary_is_trusted, prepare_bubblewrap_command, spawn_bubblewrapped,
};

#[cfg(not(target_os = "linux"))]
mod unsupported {
    use std::{
        ffi::OsString,
        path::PathBuf,
        process::{Child, ExitStatus},
        time::Duration,
    };

    #[derive(Clone, Debug)]
    pub struct SystemdCommands {
        pub systemd_run: PathBuf,
        pub systemctl: PathBuf,
        pub scope_gate: PathBuf,
        pub env: PathBuf,
    }
    #[derive(Clone, Debug)]
    pub struct AgentEnvironment {
        pub home: PathBuf,
        pub search_path: Vec<PathBuf>,
        pub config_home: Option<PathBuf>,
        pub data_home: Option<PathBuf>,
        pub cache_home: Option<PathBuf>,
        pub temp_dir: Option<PathBuf>,
        pub locale: Option<String>,
    }
    #[derive(Clone, Copy, Debug)]
    pub struct ResourceLimits {
        pub memory_max_bytes: u64,
        pub cpu_quota_percent: u32,
        pub tasks_max: u32,
    }
    #[derive(Clone, Debug)]
    pub struct LaunchSpec {
        pub attempt_id: [u8; 16],
        pub executable: PathBuf,
        pub args: Vec<OsString>,
        pub working_directory: PathBuf,
        pub environment: AgentEnvironment,
        pub limits: ResourceLimits,
    }
    #[derive(Debug)]
    pub enum ScopeError {
        InvalidInput(&'static str),
        Unsupported(String),
        Io(std::io::Error),
        Control(String),
        CleanupPending {
            cause: Box<ScopeError>,
            cleanup_error: Box<ScopeError>,
            pending: Box<PendingCleanup>,
        },
        QuiescenceUnobservable,
        QuiescenceTimeout,
    }
    impl std::fmt::Display for ScopeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidInput(s) => write!(f, "invalid process scope input: {s}"),
                Self::Unsupported(s) => write!(f, "Linux process scope unavailable: {s}"),
                Self::Io(e) => write!(f, "Linux process scope I/O failed: {e}"),
                Self::Control(s) => write!(f, "systemd process scope control failed: {s}"),
                Self::CleanupPending {
                    cause,
                    cleanup_error,
                    ..
                } => write!(
                    f,
                    "process scope cleanup remains pending after {cause}: {cleanup_error}"
                ),
                Self::QuiescenceUnobservable => {
                    f.write_str("cgroup quiescence could not be observed")
                }
                Self::QuiescenceTimeout => {
                    f.write_str("cgroup did not become empty before the deadline")
                }
            }
        }
    }
    impl std::error::Error for ScopeError {}
    #[must_use = "dropping ManagedScope does not stop its worker or release its cgroup"]
    #[derive(Debug)]
    pub struct ManagedScope;
    impl ManagedScope {
        pub fn unit_name(&self) -> &str {
            ""
        }
        pub fn child_mut(&mut self) -> Result<&mut Child, ScopeError> {
            Err(unsupported())
        }
        pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, ScopeError> {
            Err(unsupported())
        }
        pub fn wait_for_quiescence(
            &mut self,
            _timeout: Duration,
        ) -> Result<ExitStatus, ScopeError> {
            Err(unsupported())
        }
        pub fn kill_and_wait(&mut self, _timeout: Duration) -> Result<ExitStatus, ScopeError> {
            Err(unsupported())
        }
        pub fn limits_are_enforced(&self) -> Result<bool, ScopeError> {
            Err(unsupported())
        }
    }
    #[must_use = "dropping PendingCleanup loses retry ownership of an unsettled process scope"]
    #[derive(Debug)]
    // Keep the public cleanup shape aligned with the Linux backend on platforms where
    // spawning is fail-closed; no Windows/macOS process scope is constructed here.
    #[allow(clippy::large_enum_variant)]
    pub enum PendingCleanup {
        AttemptSlice {
            commands: SystemdCommands,
            slice_unit: String,
            cgroup_events: Option<PathBuf>,
        },
        PreWorker {
            commands: SystemdCommands,
            unit: String,
            slice_unit: String,
            child: Child,
            cgroup_events: Option<PathBuf>,
        },
        ManagedScope {
            scope: ManagedScope,
        },
    }
    impl PendingCleanup {
        pub fn unit_name(&self) -> &str {
            match self {
                Self::AttemptSlice { slice_unit, .. } => slice_unit,
                Self::PreWorker { unit, .. } => unit,
                Self::ManagedScope { .. } => "",
            }
        }
        pub fn retry_stop(&mut self, _timeout: Duration) -> Result<(), ScopeError> {
            Err(unsupported())
        }
        pub fn retry_kill_and_wait(
            &mut self,
            _timeout: Duration,
        ) -> Result<ExitStatus, ScopeError> {
            Err(unsupported())
        }
        pub fn limits_are_enforced(&self) -> Result<bool, ScopeError> {
            Err(unsupported())
        }
    }
    fn unsupported() -> ScopeError {
        ScopeError::Unsupported("cgroup-v2 transient scopes require Linux".into())
    }
    pub fn spawn(
        _commands: &SystemdCommands,
        _spec: LaunchSpec,
    ) -> Result<ManagedScope, ScopeError> {
        Err(unsupported())
    }
}

#[cfg(not(target_os = "linux"))]
pub use unsupported::{
    AgentEnvironment, LaunchSpec, ManagedScope, PendingCleanup, ResourceLimits, ScopeError,
    SystemdCommands, spawn,
};
