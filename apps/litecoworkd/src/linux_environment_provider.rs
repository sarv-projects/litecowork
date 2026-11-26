//! Linux Bubblewrap Environment launch-plan adapter.
//!
//! This module maps an already persisted, ready Attempt-private Environment and an
//! already prepared exact-pinned input tree into the process-scope launch vocabulary.
//! Its private capture helper can start that mapped scope and collect transient output,
//! but no production Task Runtime caller invokes it. The module does not provision durable
//! Environment state, produce an attestation, admit an Attempt, or persist a Resource.

use crate::environment_isolation::{
    HeldProviderScopeIdentity, IsolationIdentity, IsolationStoreError, PreparationJournalEntry,
    PreparationJournalPhase, PreparedRootDigests, RuntimeLocalPreparationJournal,
    normalized_policy_digest_from_json,
};
use domain_environment::environment::{
    EnvironmentClass, EnvironmentHealth, EnvironmentLifetime, EnvironmentRecord,
    EnvironmentSharingScope, EnvironmentStatus, FilesystemIsolation, NetworkMode, ProcessIsolation,
};
use linux_process_scope::{
    AgentEnvironment, BubblewrapLaunchSpec, CandidateCaptureSpec, HeldEnvironmentScope, LaunchSpec,
    MAX_SCRATCH_BYTES, ManagedScope, ProcessScopeIdentity, ReadOnlyRuntimeMount, ResourceLimits,
    ScopeError, SystemdCommands,
    output_channel::{CandidateFrame, CandidateRejection, FrameError},
};
use local_environment_staging::{PreparationStatus, PreparedEnvironment, PreparedResourcePin};
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    path::PathBuf,
    process::{ChildStderr, ChildStdin, ChildStdout},
    thread::{self, JoinHandle},
    time::Duration,
};

/// Inputs resolved by trusted Runtime code after it loads the current Environment and
/// Task revisions. Executable/runtime mounts must come from an installed, trusted agent
/// descriptor; task data and credentials must not be included in `args`.
pub struct CandidateLaunchRequest<'a> {
    pub environment: &'a EnvironmentRecord,
    pub prepared: &'a PreparedEnvironment,
    pub task_id: &'a str,
    pub attempt_id: &'a str,
    pub runtime_id: &'a str,
    pub runtime_incarnation_id: &'a str,
    pub step_id: &'a str,
    pub candidate_limit_bytes: usize,
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub runtime_mounts: Vec<ReadOnlyRuntimeMount>,
}

/// A private launch description. Host paths are retained only for the Runtime adapter;
/// this value is not a public IPC/API type and does not assert that launch succeeded.
pub struct CandidateLaunchPlan {
    pub launch: BubblewrapLaunchSpec,
    pub identity: IsolationIdentity,
    pub roots: PreparedRootDigests,
    pub step_id: String,
    pub candidate_limit_bytes: usize,
}

/// Stream and process-scope boundary required to keep the native agent protocol intact.
/// The candidate channel is separate from these agent-owned stdin/stdout streams.
trait CandidateProcessScope {
    type AgentStdin: Write;
    type AgentStdout: Read;
    type CandidateStderr: Read + Send + 'static;

    fn take_agent_stdio(
        &mut self,
    ) -> Result<(Self::AgentStdin, Self::AgentStdout, Self::CandidateStderr), ScopeError>;
    fn wait_for_quiescence(&mut self, timeout: Duration) -> Result<bool, ScopeError>;
    fn kill_and_wait(&mut self, timeout: Duration) -> Result<(), ScopeError>;
}

impl CandidateProcessScope for ManagedScope {
    type AgentStdin = ChildStdin;
    type AgentStdout = ChildStdout;
    type CandidateStderr = ChildStderr;

    fn take_agent_stdio(
        &mut self,
    ) -> Result<(Self::AgentStdin, Self::AgentStdout, Self::CandidateStderr), ScopeError> {
        let child = self.child_mut()?;
        if child.stdin.is_none() || child.stdout.is_none() || child.stderr.is_none() {
            return Err(ScopeError::Control(
                "candidate capture requires all three piped process channels".to_owned(),
            ));
        }
        Ok((
            child.stdin.take().expect("checked piped stdin"),
            child.stdout.take().expect("checked piped stdout"),
            child.stderr.take().expect("checked piped stderr"),
        ))
    }

    fn wait_for_quiescence(&mut self, timeout: Duration) -> Result<bool, ScopeError> {
        ManagedScope::wait_for_quiescence(self, timeout).map(|status| status.success())
    }

    fn kill_and_wait(&mut self, timeout: Duration) -> Result<(), ScopeError> {
        ManagedScope::kill_and_wait(self, timeout).map(|_| ())
    }
}

/// Runtime-private candidate bytes bound to the launch identity and accepted Step.
/// This value is not a Resource, approval, grant, or write authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedCandidate {
    pub bytes: Vec<u8>,
    pub identity: IsolationIdentity,
    pub roots: PreparedRootDigests,
    pub step_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CandidateCaptureOutcome {
    Candidate(CapturedCandidate),
    NoCandidate {
        identity: IsolationIdentity,
        step_id: String,
    },
}

#[derive(Debug)]
enum CandidateCaptureError {
    Scope(ScopeError),
    Frame(FrameError),
    Rejected(CandidateRejection),
    ReaderThreadStart(io::Error),
    ReaderThreadPanicked,
    AgentExchangeNotStarted,
    AgentExchangeAlreadyStarted,
    AgentExchangeFailed,
    AgentFailed,
}

/// The session is retained on every failure so a caller can retry scope observation or
/// perform bounded cancellation without losing ownership of the child/cgroup.
struct CandidateCaptureFailure<S: CandidateProcessScope> {
    pub session: CandidateCaptureSession<S>,
    pub error: CandidateCaptureError,
}

/// One native-agent stdio exchange and a separate bounded candidate reader owned by the
/// same ManagedScope. Candidate bytes are releasable only after the scope proves empty
/// and reaps its direct launcher.
#[must_use = "dropping a candidate session loses retry ownership of its process scope"]
struct CandidateCaptureSession<S: CandidateProcessScope> {
    scope: S,
    identity: IsolationIdentity,
    roots: PreparedRootDigests,
    step_id: String,
    candidate_limit_bytes: usize,
    agent_stdin: Option<S::AgentStdin>,
    agent_stdout: Option<S::AgentStdout>,
    candidate_reader: Option<JoinHandle<Result<CandidateFrame, FrameError>>>,
    exchange_started: bool,
    exchange_succeeded: bool,
    agent_exit_succeeded: Option<bool>,
    quiescent: bool,
}

impl<S: CandidateProcessScope> CandidateCaptureSession<S> {
    fn new(
        scope: S,
        identity: IsolationIdentity,
        roots: PreparedRootDigests,
        step_id: String,
        candidate_limit_bytes: usize,
    ) -> Result<Self, CandidateCaptureFailure<S>> {
        let mut session = Self {
            scope,
            identity,
            roots,
            step_id,
            candidate_limit_bytes,
            agent_stdin: None,
            agent_stdout: None,
            candidate_reader: None,
            exchange_started: false,
            exchange_succeeded: false,
            agent_exit_succeeded: None,
            quiescent: false,
        };
        if candidate_limit_bytes == 0
            || candidate_limit_bytes > linux_process_scope::output_channel::MAX_CANDIDATE_BYTES
        {
            return Err(CandidateCaptureFailure {
                session,
                error: CandidateCaptureError::Frame(FrameError::CandidateLimit),
            });
        }
        let (stdin, stdout, stderr) = match session.scope.take_agent_stdio() {
            Ok(channels) => channels,
            Err(error) => {
                return Err(CandidateCaptureFailure {
                    session,
                    error: CandidateCaptureError::Scope(error),
                });
            }
        };
        session.agent_stdin = Some(stdin);
        session.agent_stdout = Some(stdout);
        let reader = match thread::Builder::new()
            .name("litecowork-candidate-channel".to_owned())
            .spawn(move || {
                let mut stderr = stderr;
                let parsed = linux_process_scope::output_channel::read_candidate_channel(
                    &mut stderr,
                    candidate_limit_bytes,
                );
                if parsed.is_err() {
                    // Keep draining after a malformed frame so the trusted runner cannot
                    // block on a full pipe before the process scope settles. The drain
                    // uses a fixed sink and never stores bytes after protocol rejection.
                    let _ = io::copy(&mut stderr, &mut io::sink());
                }
                parsed
            }) {
            Ok(reader) => reader,
            Err(error) => {
                return Err(CandidateCaptureFailure {
                    session,
                    error: CandidateCaptureError::ReaderThreadStart(error),
                });
            }
        };
        session.candidate_reader = Some(reader);
        Ok(session)
    }

    /// Drive the native provider's existing stdin/stdout protocol. The closure owns ACP
    /// or another supported harness protocol; LiteCowork adds no private wire format.
    /// The channels close after the single exchange, even when the adapter returns error.
    fn exchange<T, E>(
        &mut self,
        exchange: impl FnOnce(&mut S::AgentStdout, &mut S::AgentStdin) -> Result<T, E>,
    ) -> Result<Result<T, E>, CandidateCaptureError> {
        if self.exchange_started {
            return Err(CandidateCaptureError::AgentExchangeAlreadyStarted);
        }
        let (Some(stdout), Some(stdin)) = (self.agent_stdout.as_mut(), self.agent_stdin.as_mut())
        else {
            return Err(CandidateCaptureError::Scope(ScopeError::Control(
                "native agent stdio is unavailable".to_owned(),
            )));
        };
        self.exchange_started = true;
        let result = exchange(stdout, stdin);
        self.exchange_succeeded = result.is_ok();
        self.agent_stdout.take();
        self.agent_stdin.take();
        Ok(result)
    }

    fn wait_for_candidate(
        mut self,
        timeout: Duration,
    ) -> Result<CandidateCaptureOutcome, CandidateCaptureFailure<S>> {
        if !self.exchange_started {
            return Err(CandidateCaptureFailure {
                session: self,
                error: CandidateCaptureError::AgentExchangeNotStarted,
            });
        }
        if !self.exchange_succeeded {
            return Err(CandidateCaptureFailure {
                session: self,
                error: CandidateCaptureError::AgentExchangeFailed,
            });
        }
        if self.agent_exit_succeeded == Some(false) {
            return Err(CandidateCaptureFailure {
                session: self,
                error: CandidateCaptureError::AgentFailed,
            });
        }
        self.agent_stdout.take();
        self.agent_stdin.take();
        if !self.quiescent {
            match self.scope.wait_for_quiescence(timeout) {
                Ok(true) => {
                    self.quiescent = true;
                    self.agent_exit_succeeded = Some(true);
                }
                Ok(false) => {
                    self.quiescent = true;
                    self.agent_exit_succeeded = Some(false);
                    if let Some(reader) = self.candidate_reader.take() {
                        if reader.join().is_err() {
                            return Err(CandidateCaptureFailure {
                                session: self,
                                error: CandidateCaptureError::ReaderThreadPanicked,
                            });
                        }
                    }
                    return Err(CandidateCaptureFailure {
                        session: self,
                        error: CandidateCaptureError::AgentFailed,
                    });
                }
                Err(error) => {
                    return Err(CandidateCaptureFailure {
                        session: self,
                        error: CandidateCaptureError::Scope(error),
                    });
                }
            }
        }
        let frame = match self.join_candidate_reader() {
            Ok(frame) => frame,
            Err(error) => {
                return Err(CandidateCaptureFailure {
                    session: self,
                    error,
                });
            }
        };
        match frame {
            CandidateFrame::Candidate(bytes) => {
                Ok(CandidateCaptureOutcome::Candidate(CapturedCandidate {
                    bytes,
                    identity: self.identity,
                    roots: self.roots,
                    step_id: self.step_id,
                }))
            }
            CandidateFrame::NoCandidate => Ok(CandidateCaptureOutcome::NoCandidate {
                identity: self.identity,
                step_id: self.step_id,
            }),
            CandidateFrame::Rejected(rejection) => Err(CandidateCaptureFailure {
                session: self,
                error: CandidateCaptureError::Rejected(rejection),
            }),
        }
    }

    fn cancel_and_wait(mut self, timeout: Duration) -> Result<(), CandidateCaptureFailure<S>> {
        self.agent_stdout.take();
        self.agent_stdin.take();
        if !self.quiescent {
            match self.scope.kill_and_wait(timeout) {
                Ok(()) => self.quiescent = true,
                Err(error) => {
                    return Err(CandidateCaptureFailure {
                        session: self,
                        error: CandidateCaptureError::Scope(error),
                    });
                }
            }
        }
        if let Some(reader) = self.candidate_reader.take() {
            if reader.join().is_err() {
                return Err(CandidateCaptureFailure {
                    session: self,
                    error: CandidateCaptureError::ReaderThreadPanicked,
                });
            }
        }
        Ok(())
    }

    fn join_candidate_reader(&mut self) -> Result<CandidateFrame, CandidateCaptureError> {
        let reader = self
            .candidate_reader
            .take()
            .ok_or(CandidateCaptureError::ReaderThreadPanicked)?;
        reader
            .join()
            .map_err(|_| CandidateCaptureError::ReaderThreadPanicked)?
            .map_err(CandidateCaptureError::Frame)
    }
}

/// Start the private Bubblewrap scope and immediately begin draining its candidate
/// channel. The returned session still needs the native harness exchange and quiescence
/// check; this function creates no attestation, Resource, grant, Effect, or admission.
fn start_candidate_capture(
    commands: &SystemdCommands,
    plan: CandidateLaunchPlan,
) -> Result<CandidateCaptureSession<ManagedScope>, CandidateLaunchError> {
    let CandidateLaunchPlan {
        launch,
        identity,
        roots,
        step_id,
        candidate_limit_bytes,
    } = plan;
    let scope = linux_process_scope::spawn_bubblewrapped(commands, launch)
        .map_err(CandidateLaunchError::Scope)?;
    CandidateCaptureSession::new(scope, identity, roots, step_id, candidate_limit_bytes)
        .map_err(CandidateLaunchError::Capture)
}

enum CandidateLaunchError {
    Scope(ScopeError),
    Capture(CandidateCaptureFailure<ManagedScope>),
}

impl<S: CandidateProcessScope> std::fmt::Debug for CandidateCaptureFailure<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CandidateCaptureFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Why the Environment cannot be mapped into this Linux launch profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderError {
    InvalidIdentity,
    EnvironmentIdentityMismatch,
    EnvironmentNotReady,
    AttemptPrivateEnvironmentRequired,
    UnsupportedIsolationPolicy,
    UnsupportedNetworkPolicy,
    PreparedInputsUnavailable,
    CandidateLimitUnsupported,
    InvalidResourceLimits,
    UnsupportedCpuGranularity,
    UnsupportedScratchLimit,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidIdentity => "Task launch identity is invalid",
            Self::EnvironmentIdentityMismatch => {
                "Environment does not belong to the current Task Attempt and Runtime incarnation"
            }
            Self::EnvironmentNotReady => "Environment is not ready and healthy",
            Self::AttemptPrivateEnvironmentRequired => {
                "Task execution requires a fresh Attempt-private Environment"
            }
            Self::UnsupportedIsolationPolicy => {
                "Environment isolation policy is not supported by the Linux Bubblewrap provider"
            }
            Self::UnsupportedNetworkPolicy => {
                "Linux Bubblewrap currently supports only default-deny network isolation"
            }
            Self::PreparedInputsUnavailable => {
                "exact-pinned Environment inputs are not in the prepared-only state"
            }
            Self::CandidateLimitUnsupported => {
                "candidate byte limit is outside the supported positive bound"
            }
            Self::InvalidResourceLimits => {
                "Environment resource limits are missing or outside supported bounds"
            }
            Self::UnsupportedCpuGranularity => {
                "CPU limit cannot be represented exactly by the Linux process-scope provider"
            }
            Self::UnsupportedScratchLimit => {
                "Environment scratch limit is outside the supported tmpfs bound"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProviderError {}

/// Narrow process-scope surface needed to bind a live held Environment to the private
/// preparation journal. The abstraction also lets the daemon test the journal bridge
/// without claiming that a fake process scope proves Linux enforcement.
trait HeldScopeIdentitySource {
    fn unit_name(&self) -> &str;
    fn capture_process_scope_identity(&self) -> Result<ProcessScopeIdentity, ScopeError>;
    fn recheck_process_scope_identity(
        &self,
        expected: &ProcessScopeIdentity,
    ) -> Result<(), ScopeError>;
}

impl HeldScopeIdentitySource for HeldEnvironmentScope {
    fn unit_name(&self) -> &str {
        HeldEnvironmentScope::unit_name(self)
    }

    fn capture_process_scope_identity(&self) -> Result<ProcessScopeIdentity, ScopeError> {
        HeldEnvironmentScope::capture_process_scope_identity(self)
    }

    fn recheck_process_scope_identity(
        &self,
        expected: &ProcessScopeIdentity,
    ) -> Result<(), ScopeError> {
        HeldEnvironmentScope::recheck_process_scope_identity(self, expected)
    }
}

#[derive(Debug)]
enum HeldScopeJournalError {
    Journal(IsolationStoreError),
    Scope(ScopeError),
}

impl From<IsolationStoreError> for HeldScopeJournalError {
    fn from(error: IsolationStoreError) -> Self {
        Self::Journal(error)
    }
}

impl From<ScopeError> for HeldScopeJournalError {
    fn from(error: ScopeError) -> Self {
        Self::Scope(error)
    }
}

/// Capture the exact live held-scope process/cgroup identity and durably bind it to the
/// existing Runtime-private journal entry. The borrowed scope remains owned by the
/// caller if capture or journal persistence fails; callers must retain it until cleanup
/// has positive quiescence evidence.
fn record_held_scope_identity<S: HeldScopeIdentitySource>(
    journal: &RuntimeLocalPreparationJournal,
    attempt_id: &str,
    scope: &S,
) -> Result<PreparationJournalEntry, HeldScopeJournalError> {
    let process = scope.capture_process_scope_identity()?;
    let held = HeldProviderScopeIdentity {
        unit_name: scope.unit_name().to_owned(),
        leader_pid: process.leader_pid,
        leader_start_time_ticks: process.leader_start_time_ticks,
        process_scope_digest: process.process_scope_digest,
    };
    let entry = journal.record_scope_held(attempt_id, held.clone())?;
    if entry.phase != PreparationJournalPhase::ScopeHeld || entry.held_scope.as_ref() != Some(&held)
    {
        return Err(HeldScopeJournalError::Journal(
            IsolationStoreError::JournalConflict,
        ));
    }
    Ok(entry)
}

/// Reload the journaled held identity and re-observe the same live scope immediately
/// before the Attempt admission transaction. The caller must not insert a provider or
/// storage side effect between this check and the admission commit.
fn recheck_journaled_held_scope_before_admission<S: HeldScopeIdentitySource>(
    journal: &RuntimeLocalPreparationJournal,
    attempt_id: &str,
    scope: &S,
) -> Result<(), HeldScopeJournalError> {
    let entry = journal.load(attempt_id)?.ok_or_else(|| {
        HeldScopeJournalError::Journal(IsolationStoreError::JournalTransition(
            "held-scope identity is missing before admission",
        ))
    })?;
    if entry.phase != PreparationJournalPhase::ScopeHeld {
        return Err(HeldScopeJournalError::Journal(
            IsolationStoreError::JournalTransition(
                "process identity recheck requires the pre-admission held-scope phase",
            ),
        ));
    }
    let Some(held) = entry.held_scope else {
        return Err(HeldScopeJournalError::Journal(
            IsolationStoreError::JournalConflict,
        ));
    };
    if scope.unit_name() != held.unit_name {
        return Err(HeldScopeJournalError::Journal(
            IsolationStoreError::JournalConflict,
        ));
    }
    scope.recheck_process_scope_identity(&ProcessScopeIdentity {
        leader_pid: held.leader_pid,
        leader_start_time_ticks: held.leader_start_time_ticks,
        process_scope_digest: held.process_scope_digest,
    })?;
    Ok(())
}

/// Prepare a bounded candidate-capture launch plan from canonical Environment state.
///
/// The `EnvironmentRecord` and `PreparedEnvironment` must already have been loaded or
/// produced by their owning services. This function validates their identities and
/// maps only limits that the current cgroup/Bubblewrap interfaces can express exactly.
/// It intentionally performs no filesystem mutation and produces no enforcement facts.
pub fn prepare_candidate_launch(
    request: CandidateLaunchRequest<'_>,
) -> Result<CandidateLaunchPlan, ProviderError> {
    let bubblewrap = linux_process_scope::probe_local_provider()
        .trusted_bubblewrap
        .ok_or(ProviderError::UnsupportedIsolationPolicy)?;
    prepare_candidate_launch_with_bubblewrap(request, bubblewrap)
}

fn prepare_candidate_launch_with_bubblewrap(
    request: CandidateLaunchRequest<'_>,
    bubblewrap: PathBuf,
) -> Result<CandidateLaunchPlan, ProviderError> {
    if request.candidate_limit_bytes == 0
        || request.candidate_limit_bytes > linux_process_scope::output_channel::MAX_CANDIDATE_BYTES
    {
        return Err(ProviderError::CandidateLimitUnsupported);
    }
    if !valid_identity(request.task_id)
        || !valid_identity(request.attempt_id)
        || !valid_identity(request.runtime_id)
        || !valid_identity(request.runtime_incarnation_id)
        || !valid_identity(request.step_id)
    {
        return Err(ProviderError::InvalidIdentity);
    }

    let identity = request.environment.identity();
    if request.environment.status() != EnvironmentStatus::Ready
        || request.environment.health() != EnvironmentHealth::Healthy
    {
        return Err(ProviderError::EnvironmentNotReady);
    }
    if identity.owner_workspace_id.as_str() != request.prepared.workspace_id
        || identity.owner.task_id.as_ref().map(|id| id.as_str()) != Some(request.task_id)
        || identity.owner.attempt_id.as_ref().map(|id| id.as_str()) != Some(request.attempt_id)
        || identity.runtime_id.as_str() != request.runtime_id
        || identity
            .created_by_incarnation_id
            .as_ref()
            .map(|id| id.as_str())
            != Some(request.runtime_incarnation_id)
    {
        return Err(ProviderError::EnvironmentIdentityMismatch);
    }

    let config = request.environment.config();
    if config.class != EnvironmentClass::LocalWorkspace
        || config.lifetime != EnvironmentLifetime::Attempt
        || config.sharing_scope != EnvironmentSharingScope::AttemptPrivate
    {
        return Err(ProviderError::AttemptPrivateEnvironmentRequired);
    }
    if config.isolation.filesystem != FilesystemIsolation::PrivateCopy
        || config.isolation.process != ProcessIsolation::Namespace
        || config.isolation.network != NetworkMode::None
        || config.isolation.write_scope.workspace_id.as_str() != request.prepared.workspace_id
    {
        return Err(ProviderError::UnsupportedIsolationPolicy);
    }
    if config.network_policy.mode != NetworkMode::None
        || !config.network_policy.allowed_domains.is_empty()
    {
        return Err(ProviderError::UnsupportedNetworkPolicy);
    }
    let mut configured_sources = config
        .source_resources
        .iter()
        .map(|source| PreparedResourcePin {
            workspace_id: identity.owner_workspace_id.as_str().to_owned(),
            resource_id: source.resource_id.as_str().to_owned(),
            revision_id: source.revision_id.as_str().to_owned(),
            content_digest: source.content_digest.clone(),
        })
        .collect::<Vec<_>>();
    configured_sources.sort();
    if request.prepared.status != PreparationStatus::PreparedOnly
        || identity.owner_workspace_id.as_str() != request.prepared.workspace_id
        || configured_sources != request.prepared.source_resources
    {
        return Err(ProviderError::PreparedInputsUnavailable);
    }

    let limits = &config.resource_limits;
    if limits.memory_bytes == 0
        || limits.cpu_millis == 0
        || limits.max_processes.is_none_or(|value| value == 0)
        || limits.max_lifetime_seconds == Some(0)
    {
        return Err(ProviderError::InvalidResourceLimits);
    }
    if limits.cpu_millis % 10 != 0 {
        return Err(ProviderError::UnsupportedCpuGranularity);
    }
    if limits.storage_bytes == 0 || limits.storage_bytes > MAX_SCRATCH_BYTES {
        return Err(ProviderError::UnsupportedScratchLimit);
    }
    let cpu_quota_percent = limits.cpu_millis / 10;
    if cpu_quota_percent == 0
        || cpu_quota_percent > 10_000
        || limits.max_processes.is_some_and(|value| value > 65_536)
    {
        return Err(ProviderError::InvalidResourceLimits);
    }

    let scope_digest = crate::environment_isolation::execution_identity_digest(&[
        request.task_id,
        request.step_id,
        request.attempt_id,
        identity.environment_id.as_str(),
        request.runtime_id,
        request.runtime_incarnation_id,
    ]);
    let attempt_scope_id: [u8; 16] = hex::decode(scope_digest.trim_start_matches("sha256:"))
        .map_err(|_| ProviderError::InvalidIdentity)?
        .get(..16)
        .ok_or(ProviderError::InvalidIdentity)?
        .try_into()
        .map_err(|_| ProviderError::InvalidIdentity)?;

    let policy_value = serde_json::json!({
        "isolation": config.isolation,
        "network_policy": config.network_policy,
        "resource_limits": config.resource_limits,
    });
    let normalized_policy_digest = normalized_policy_digest_from_json(&policy_value);
    let prepared_roots = PreparedRootDigests {
        pinned_input_set_digest: request.prepared.input_set_digest.clone(),
        readable_root_set_digest: request.prepared.readable_root_set_digest.clone(),
        writable_root_set_digest: request.prepared.writable_root_set_digest.clone(),
    };
    let isolation_identity = IsolationIdentity {
        environment_id: identity.environment_id.as_str().to_owned(),
        workspace_id: identity.owner_workspace_id.as_str().to_owned(),
        task_id: request.task_id.to_owned(),
        runtime_id: request.runtime_id.to_owned(),
        runtime_incarnation_id: request.runtime_incarnation_id.to_owned(),
        normalized_policy_digest,
    };
    let launch = BubblewrapLaunchSpec {
        bubblewrap,
        process: LaunchSpec {
            attempt_id: attempt_scope_id,
            executable: request.executable,
            args: request.args,
            working_directory: request.prepared.output_root.clone(),
            environment: AgentEnvironment {
                home: PathBuf::from("/workspace/outputs/home"),
                search_path: vec![PathBuf::from("/usr/bin")],
                config_home: None,
                data_home: None,
                cache_home: None,
                temp_dir: None,
                locale: Some("C".to_owned()),
            },
            limits: ResourceLimits {
                memory_max_bytes: limits.memory_bytes,
                cpu_quota_percent,
                tasks_max: limits.max_processes.expect("validated process limit"),
            },
        },
        input_root: request.prepared.input_root.clone(),
        output_root: request.prepared.output_root.clone(),
        scratch_limit_bytes: limits.storage_bytes,
        candidate_capture: Some(CandidateCaptureSpec {
            max_bytes: request.candidate_limit_bytes,
            start_after_admission: false,
        }),
        runtime_mounts: request.runtime_mounts,
    };

    Ok(CandidateLaunchPlan {
        launch,
        identity: isolation_identity,
        roots: prepared_roots,
        step_id: request.step_id.to_owned(),
        candidate_limit_bytes: request.candidate_limit_bytes,
    })
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment_isolation::PreparationJournalIntent;
    use domain_environment::environment::{
        AttemptId, BudgetCeiling, BudgetEnforcement, BudgetEnforcementPolicy,
        EnvironmentBackupPolicy, EnvironmentClass, EnvironmentConfig, EnvironmentHealth,
        EnvironmentId, EnvironmentIdentity, EnvironmentLifetime, EnvironmentOwner,
        EnvironmentRecord, EnvironmentSharingScope, EnvironmentStatus, FilesystemIsolation,
        IsolationSpec, NetworkMode, NetworkPolicy, PinnedSourceResource, ProcessIsolation,
        ResourceId, ResourceLimits as DomainResourceLimits, ResourceRevisionId, ResourceScope,
        RuntimeId, RuntimeIncarnationId, TaskId, WorkspaceId,
    };
    use linux_process_scope::{ProcessScopeIdentity, ReadOnlyRuntimeMount, ScopeError};
    use local_environment_staging::{PinnedInput, StagingLimits, stage_inputs};
    use sha2::{Digest, Sha256};
    use std::{
        io::{Cursor, Read, Write},
        sync::{Arc, Mutex},
        time::Duration,
    };

    fn test_environment(
        lifetime: EnvironmentLifetime,
        sharing_scope: EnvironmentSharingScope,
        cpu_millis: u32,
    ) -> EnvironmentRecord {
        test_environment_with_sources(lifetime, sharing_scope, cpu_millis, Vec::new())
    }

    fn test_environment_with_sources(
        lifetime: EnvironmentLifetime,
        sharing_scope: EnvironmentSharingScope,
        cpu_millis: u32,
        source_resources: Vec<PinnedSourceResource>,
    ) -> EnvironmentRecord {
        EnvironmentRecord::from_persisted_parts(
            EnvironmentIdentity {
                environment_id: EnvironmentId::new("environment-1").unwrap(),
                runtime_id: RuntimeId::new("runtime-1").unwrap(),
                owner_workspace_id: WorkspaceId::new("workspace-1").unwrap(),
                owner: EnvironmentOwner {
                    task_id: Some(TaskId::new("task-1").unwrap()),
                    attempt_id: Some(AttemptId::new("attempt-1").unwrap()),
                    coworker_id: None,
                    principal_id: None,
                },
                created_by_incarnation_id: Some(
                    RuntimeIncarnationId::new("incarnation-1").unwrap(),
                ),
            },
            EnvironmentConfig {
                name: "attempt environment".to_owned(),
                provider_kind: "local-bubblewrap".to_owned(),
                class: EnvironmentClass::LocalWorkspace,
                lifetime,
                sharing_scope,
                source_resources,
                resource_limits: DomainResourceLimits {
                    cpu_millis,
                    memory_bytes: 256 * 1024 * 1024,
                    storage_bytes: 64 * 1024 * 1024,
                    max_processes: Some(32),
                    max_lifetime_seconds: Some(300),
                },
                network_policy: NetworkPolicy {
                    mode: NetworkMode::None,
                    allowed_domains: Vec::new(),
                    max_response_bytes: 0,
                    max_download_bytes: 0,
                    deny_private_networks: true,
                },
                budget_ceiling: BudgetCeiling {
                    max_wall_time_ms: Some(300_000),
                    max_cost_minor_units: None,
                    currency: None,
                    max_tokens: None,
                    max_child_attempts: None,
                    max_concurrency: None,
                },
                budget_enforcement_policy: BudgetEnforcementPolicy::RequireProviderEnforced,
                budget_enforcement: BudgetEnforcement::Unavailable,
                provision_preview_digest: None,
                retention_expires_at: None,
                backup_policy: EnvironmentBackupPolicy::Excluded,
                isolation: IsolationSpec {
                    filesystem: FilesystemIsolation::PrivateCopy,
                    process: ProcessIsolation::Namespace,
                    network: NetworkMode::None,
                    write_scope: ResourceScope {
                        workspace_id: WorkspaceId::new("workspace-1").unwrap(),
                        resource_ids: Vec::new(),
                    },
                },
            },
            EnvironmentStatus::Ready,
            EnvironmentHealth::Healthy,
            "2026-11-18T00:00:00Z".to_owned(),
            "2026-11-18T00:00:00Z".to_owned(),
            1,
        )
        .unwrap()
    }

    fn prepared(parent: &std::path::Path) -> PreparedEnvironment {
        stage_inputs(
            parent,
            "workspace-1",
            &[],
            StagingLimits {
                max_files: 32,
                max_file_bytes: 1024,
                max_total_input_bytes: 4096,
                max_path_bytes: 256,
            },
        )
        .unwrap()
    }

    fn prepared_with_input(parent: &std::path::Path, bytes: &[u8]) -> PreparedEnvironment {
        let content_digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        stage_inputs(
            parent,
            "workspace-1",
            &[PinnedInput {
                workspace_id: "workspace-1".to_owned(),
                resource_id: "resource-1".to_owned(),
                revision_id: "revision-1".to_owned(),
                content_digest,
                relative_path: "source.txt".to_owned(),
                bytes,
            }],
            StagingLimits {
                max_files: 32,
                max_file_bytes: 1024,
                max_total_input_bytes: 4096,
                max_path_bytes: 256,
            },
        )
        .unwrap()
    }

    fn content_digest(bytes: &[u8]) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    fn request<'a>(
        environment: &'a EnvironmentRecord,
        prepared: &'a PreparedEnvironment,
    ) -> CandidateLaunchRequest<'a> {
        CandidateLaunchRequest {
            environment,
            prepared,
            task_id: "task-1",
            attempt_id: "attempt-1",
            runtime_id: "runtime-1",
            runtime_incarnation_id: "incarnation-1",
            step_id: "step-1",
            candidate_limit_bytes: 4096,
            executable: PathBuf::from("/usr/bin/example-agent"),
            args: vec![OsString::from("--stdio")],
            runtime_mounts: vec![ReadOnlyRuntimeMount {
                source: PathBuf::from("/usr"),
                target: PathBuf::from("/usr"),
            }],
        }
    }

    #[test]
    fn candidate_launch_plan_maps_supported_attempt_limits_exactly() {
        let parent = tempfile::tempdir().unwrap();
        let environment = test_environment(
            EnvironmentLifetime::Attempt,
            EnvironmentSharingScope::AttemptPrivate,
            1_000,
        );
        let prepared = prepared(parent.path());
        let plan = prepare_candidate_launch_with_bubblewrap(
            request(&environment, &prepared),
            PathBuf::from("/usr/bin/bwrap"),
        )
        .unwrap();

        assert_eq!(plan.launch.scratch_limit_bytes, 64 * 1024 * 1024);
        assert_eq!(
            plan.launch.candidate_capture,
            Some(CandidateCaptureSpec {
                max_bytes: 4096,
                start_after_admission: false,
            })
        );
        assert_eq!(
            plan.launch.process.limits.memory_max_bytes,
            256 * 1024 * 1024
        );
        assert_eq!(plan.launch.process.limits.cpu_quota_percent, 100);
        assert_eq!(plan.launch.process.limits.tasks_max, 32);
        assert_eq!(plan.identity.environment_id, "environment-1");
        assert_eq!(plan.identity.runtime_incarnation_id, "incarnation-1");
        assert_eq!(
            plan.roots.pinned_input_set_digest,
            prepared.input_set_digest
        );
        assert_eq!(
            plan.roots.readable_root_set_digest,
            prepared.readable_root_set_digest
        );
        assert_eq!(
            plan.roots.writable_root_set_digest,
            prepared.writable_root_set_digest
        );
    }

    #[test]
    fn candidate_launch_refuses_an_inexact_cpu_limit_mapping() {
        let parent = tempfile::tempdir().unwrap();
        let environment = test_environment(
            EnvironmentLifetime::Attempt,
            EnvironmentSharingScope::AttemptPrivate,
            1_001,
        );
        let prepared = prepared(parent.path());
        assert!(matches!(
            prepare_candidate_launch_with_bubblewrap(
                request(&environment, &prepared),
                PathBuf::from("/usr/bin/bwrap")
            ),
            Err(ProviderError::UnsupportedCpuGranularity)
        ));
    }

    #[test]
    fn candidate_launch_refuses_limits_outside_the_process_scope_contract() {
        let parent = tempfile::tempdir().unwrap();
        let environment = test_environment(
            EnvironmentLifetime::Attempt,
            EnvironmentSharingScope::AttemptPrivate,
            100_010,
        );
        let prepared = prepared(parent.path());
        assert!(matches!(
            prepare_candidate_launch_with_bubblewrap(
                request(&environment, &prepared),
                PathBuf::from("/usr/bin/bwrap")
            ),
            Err(ProviderError::InvalidResourceLimits)
        ));
    }

    #[test]
    fn candidate_launch_refuses_shared_or_stale_environment_identity() {
        let parent = tempfile::tempdir().unwrap();
        let environment = test_environment(
            EnvironmentLifetime::TaskRetained,
            EnvironmentSharingScope::TaskShared,
            1_000,
        );
        let prepared = prepared(parent.path());
        assert!(matches!(
            prepare_candidate_launch_with_bubblewrap(
                request(&environment, &prepared),
                PathBuf::from("/usr/bin/bwrap")
            ),
            Err(ProviderError::AttemptPrivateEnvironmentRequired)
        ));

        let environment = test_environment(
            EnvironmentLifetime::Attempt,
            EnvironmentSharingScope::AttemptPrivate,
            1_000,
        );
        let mut request = request(&environment, &prepared);
        request.runtime_incarnation_id = "incarnation-2";
        assert!(matches!(
            prepare_candidate_launch_with_bubblewrap(request, PathBuf::from("/usr/bin/bwrap")),
            Err(ProviderError::EnvironmentIdentityMismatch)
        ));
    }

    #[test]
    fn candidate_launch_binds_the_staged_source_set_to_exact_environment_pins() {
        let parent = tempfile::tempdir().unwrap();
        let bytes = b"pinned source bytes";
        let digest = content_digest(bytes);
        let pin = PinnedSourceResource {
            resource_id: ResourceId::new("resource-1").unwrap(),
            revision_id: ResourceRevisionId::new("revision-1").unwrap(),
            content_digest: digest,
        };
        let environment = test_environment_with_sources(
            EnvironmentLifetime::Attempt,
            EnvironmentSharingScope::AttemptPrivate,
            1_000,
            vec![pin],
        );
        let prepared = prepared_with_input(parent.path(), bytes);
        let plan = prepare_candidate_launch_with_bubblewrap(
            request(&environment, &prepared),
            PathBuf::from("/usr/bin/bwrap"),
        )
        .unwrap();
        assert_eq!(
            plan.roots.pinned_input_set_digest,
            prepared.input_set_digest
        );

        let environment_with_other_digest = test_environment_with_sources(
            EnvironmentLifetime::Attempt,
            EnvironmentSharingScope::AttemptPrivate,
            1_000,
            vec![PinnedSourceResource {
                resource_id: ResourceId::new("resource-1").unwrap(),
                revision_id: ResourceRevisionId::new("revision-1").unwrap(),
                content_digest: content_digest(b"different source bytes"),
            }],
        );
        assert!(matches!(
            prepare_candidate_launch_with_bubblewrap(
                request(&environment_with_other_digest, &prepared),
                PathBuf::from("/usr/bin/bwrap")
            ),
            Err(ProviderError::PreparedInputsUnavailable)
        ));
    }

    #[derive(Default)]
    struct FakeCandidateScopeState {
        quiescence_failures: usize,
        quiescence_checks: usize,
        killed: bool,
    }

    struct FakeCandidateScope {
        state: Arc<Mutex<FakeCandidateScopeState>>,
        stdin: Option<Vec<u8>>,
        stdout: Option<Cursor<Vec<u8>>>,
        stderr: Option<Cursor<Vec<u8>>>,
        exit_success: bool,
    }

    impl CandidateProcessScope for FakeCandidateScope {
        type AgentStdin = Vec<u8>;
        type AgentStdout = Cursor<Vec<u8>>;
        type CandidateStderr = Cursor<Vec<u8>>;

        fn take_agent_stdio(
            &mut self,
        ) -> Result<(Self::AgentStdin, Self::AgentStdout, Self::CandidateStderr), ScopeError>
        {
            match (self.stdin.take(), self.stdout.take(), self.stderr.take()) {
                (Some(stdin), Some(stdout), Some(stderr)) => Ok((stdin, stdout, stderr)),
                _ => Err(ScopeError::Unsupported("fake stdio unavailable".to_owned())),
            }
        }

        fn wait_for_quiescence(&mut self, _timeout: Duration) -> Result<bool, ScopeError> {
            let mut state = self.state.lock().expect("fake scope state");
            state.quiescence_checks += 1;
            if state.quiescence_failures > 0 {
                state.quiescence_failures -= 1;
                return Err(ScopeError::QuiescenceTimeout);
            }
            Ok(self.exit_success)
        }

        fn kill_and_wait(&mut self, _timeout: Duration) -> Result<(), ScopeError> {
            self.state.lock().expect("fake scope state").killed = true;
            Ok(())
        }
    }

    fn fake_candidate_scope(
        encoded_frame: Vec<u8>,
        quiescence_failures: usize,
        exit_success: bool,
    ) -> (FakeCandidateScope, Arc<Mutex<FakeCandidateScopeState>>) {
        let state = Arc::new(Mutex::new(FakeCandidateScopeState {
            quiescence_failures,
            ..FakeCandidateScopeState::default()
        }));
        (
            FakeCandidateScope {
                state: Arc::clone(&state),
                stdin: Some(Vec::new()),
                stdout: Some(Cursor::new(b"native ACP stdout".to_vec())),
                stderr: Some(Cursor::new(encoded_frame)),
                exit_success,
            },
            state,
        )
    }

    fn fake_identity() -> IsolationIdentity {
        IsolationIdentity {
            environment_id: "environment-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            normalized_policy_digest: "sha256:policy".to_owned(),
        }
    }

    fn fake_roots() -> PreparedRootDigests {
        PreparedRootDigests {
            pinned_input_set_digest: "sha256:inputs".to_owned(),
            readable_root_set_digest: "sha256:readable".to_owned(),
            writable_root_set_digest: "sha256:writable".to_owned(),
        }
    }

    fn candidate_frame(bytes: &[u8]) -> Vec<u8> {
        let mut frame = Vec::new();
        linux_process_scope::output_channel::write_candidate_frame(&mut frame, bytes).unwrap();
        frame
    }

    #[test]
    fn candidate_capture_withholds_bytes_until_quiescence_and_retries_the_same_scope() {
        let (scope, state) = fake_candidate_scope(candidate_frame(b"approved proposal"), 1, true);
        let mut session = CandidateCaptureSession::new(
            scope,
            fake_identity(),
            fake_roots(),
            "step-1".to_owned(),
            4096,
        )
        .unwrap();

        let exchange = session.exchange(|stdout, stdin| {
            let mut native_output = Vec::new();
            stdout.read_to_end(&mut native_output)?;
            stdin.write_all(b"native ACP request")?;
            Ok::<_, std::io::Error>(native_output)
        });
        assert_eq!(exchange.unwrap().unwrap(), b"native ACP stdout");

        let failure = match session.wait_for_candidate(Duration::from_secs(1)) {
            Ok(_) => panic!("candidate must not escape before quiescence is observed"),
            Err(failure) => failure,
        };
        assert!(matches!(failure.error, CandidateCaptureError::Scope(_)));
        assert_eq!(state.lock().unwrap().quiescence_checks, 1);

        let outcome = failure
            .session
            .wait_for_candidate(Duration::from_secs(1))
            .unwrap();
        let CandidateCaptureOutcome::Candidate(candidate) = outcome else {
            panic!("expected candidate frame");
        };
        assert_eq!(candidate.bytes, b"approved proposal");
        assert_eq!(candidate.identity.runtime_incarnation_id, "incarnation-1");
        assert_eq!(candidate.roots.pinned_input_set_digest, "sha256:inputs");
        assert_eq!(candidate.step_id, "step-1");
        assert_eq!(state.lock().unwrap().quiescence_checks, 2);
    }

    #[test]
    fn candidate_capture_discards_frames_after_failed_native_exchange_or_process_exit() {
        let (scope, _) = fake_candidate_scope(candidate_frame(b"must be discarded"), 0, true);
        let mut session = CandidateCaptureSession::new(
            scope,
            fake_identity(),
            fake_roots(),
            "step-1".to_owned(),
            4096,
        )
        .unwrap();
        assert!(matches!(
            session.exchange(|_, _| Err::<(), _>("native exchange failed")),
            Ok(Err("native exchange failed"))
        ));
        let failure = session
            .wait_for_candidate(Duration::from_secs(1))
            .expect_err("failed exchange cannot release candidate bytes");
        assert!(matches!(
            failure.error,
            CandidateCaptureError::AgentExchangeFailed
        ));
        failure
            .session
            .cancel_and_wait(Duration::from_secs(1))
            .expect("cancel settles fake scope");

        let (scope, _) = fake_candidate_scope(candidate_frame(b"nonzero exit"), 0, false);
        let mut session = CandidateCaptureSession::new(
            scope,
            fake_identity(),
            fake_roots(),
            "step-1".to_owned(),
            4096,
        )
        .unwrap();
        session
            .exchange(|_, _| Ok::<_, std::io::Error>(()))
            .unwrap()
            .unwrap();
        let failure = session
            .wait_for_candidate(Duration::from_secs(1))
            .expect_err("nonzero child exit cannot release candidate bytes");
        assert!(matches!(failure.error, CandidateCaptureError::AgentFailed));
        let retry = failure
            .session
            .wait_for_candidate(Duration::from_secs(1))
            .expect_err("retry after a nonzero child exit cannot release its candidate frame");
        assert!(matches!(retry.error, CandidateCaptureError::AgentFailed));
    }

    #[test]
    fn candidate_capture_cancel_closes_native_channels_and_discards_candidate() {
        let (scope, state) = fake_candidate_scope(candidate_frame(b"discard"), 0, true);
        let mut session = CandidateCaptureSession::new(
            scope,
            fake_identity(),
            fake_roots(),
            "step-1".to_owned(),
            4096,
        )
        .unwrap();
        session
            .exchange(|_, _| Ok::<_, std::io::Error>(()))
            .unwrap()
            .unwrap();
        session
            .cancel_and_wait(Duration::from_secs(1))
            .expect("cancel waits for scope cleanup");
        assert!(state.lock().unwrap().killed);
    }

    fn preparation_journal_test_intent() -> PreparationJournalIntent {
        let digest = |value: &[u8]| format!("sha256:{}", hex::encode(Sha256::digest(value)));
        PreparationJournalIntent {
            attempt_id: "attempt-journal-1".to_owned(),
            environment_id: "environment-journal-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            step_id: "step-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            provider_scope_id: "0123456789abcdef0123456789abcdef".to_owned(),
            admission_idempotency_key: "admit-journal-1".to_owned(),
            request_digest: digest(b"request"),
            normalized_policy_digest: digest(b"policy"),
            pinned_input_set_digest: digest(b"inputs"),
            readable_root_set_digest: digest(b"readable"),
            writable_root_set_digest: digest(b"writable"),
        }
    }

    fn preparation_process_identity() -> ProcessScopeIdentity {
        ProcessScopeIdentity {
            leader_pid: 1234,
            leader_start_time_ticks: 5678,
            process_scope_digest: content_digest(b"held process scope"),
        }
    }

    struct FakeHeldScopeIdentitySource {
        unit_name: String,
        identity: Mutex<ProcessScopeIdentity>,
        last_rechecked: Mutex<Option<ProcessScopeIdentity>>,
    }

    impl FakeHeldScopeIdentitySource {
        fn new(unit_name: String, identity: ProcessScopeIdentity) -> Self {
            Self {
                unit_name,
                identity: Mutex::new(identity),
                last_rechecked: Mutex::new(None),
            }
        }
    }

    impl HeldScopeIdentitySource for FakeHeldScopeIdentitySource {
        fn unit_name(&self) -> &str {
            &self.unit_name
        }

        fn capture_process_scope_identity(&self) -> Result<ProcessScopeIdentity, ScopeError> {
            Ok(self.identity.lock().unwrap().clone())
        }

        fn recheck_process_scope_identity(
            &self,
            expected: &ProcessScopeIdentity,
        ) -> Result<(), ScopeError> {
            *self.last_rechecked.lock().unwrap() = Some(expected.clone());
            if &*self.identity.lock().unwrap() == expected {
                Ok(())
            } else {
                Err(ScopeError::Unsupported(
                    "fake held process identity changed".to_owned(),
                ))
            }
        }
    }

    #[test]
    fn journaled_held_scope_identity_is_persisted_and_rechecked_before_admission() {
        let temp = tempfile::tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1").unwrap();
        let intent = preparation_journal_test_intent();
        journal.begin(intent.clone()).unwrap();
        let scope_identity = preparation_process_identity();
        let scope = FakeHeldScopeIdentitySource::new(
            format!("litecowork-attempt-{}.scope", intent.provider_scope_id),
            scope_identity.clone(),
        );

        let entry = record_held_scope_identity(&journal, &intent.attempt_id, &scope).unwrap();

        assert_eq!(entry.phase, PreparationJournalPhase::ScopeHeld);
        assert_eq!(
            entry.held_scope,
            Some(HeldProviderScopeIdentity {
                unit_name: scope.unit_name.clone(),
                leader_pid: scope_identity.leader_pid,
                leader_start_time_ticks: scope_identity.leader_start_time_ticks,
                process_scope_digest: scope_identity.process_scope_digest.clone(),
            })
        );
        recheck_journaled_held_scope_before_admission(&journal, &intent.attempt_id, &scope)
            .unwrap();
        assert_eq!(*scope.last_rechecked.lock().unwrap(), Some(scope_identity));
    }

    #[test]
    fn journaled_held_scope_recheck_refuses_changed_live_identity() {
        let temp = tempfile::tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1").unwrap();
        let intent = preparation_journal_test_intent();
        journal.begin(intent.clone()).unwrap();
        let original_identity = preparation_process_identity();
        let scope = FakeHeldScopeIdentitySource::new(
            format!("litecowork-attempt-{}.scope", intent.provider_scope_id),
            original_identity,
        );
        record_held_scope_identity(&journal, &intent.attempt_id, &scope).unwrap();
        scope.identity.lock().unwrap().leader_start_time_ticks += 1;

        let result =
            recheck_journaled_held_scope_before_admission(&journal, &intent.attempt_id, &scope);

        assert!(matches!(
            result,
            Err(HeldScopeJournalError::Scope(ScopeError::Unsupported(_)))
        ));
    }

    #[test]
    fn journaled_held_scope_recheck_requires_scope_held_phase() {
        let temp = tempfile::tempdir().unwrap();
        let journal = RuntimeLocalPreparationJournal::open(temp.path(), "runtime-1").unwrap();
        let intent = preparation_journal_test_intent();
        journal.begin(intent.clone()).unwrap();
        let scope = FakeHeldScopeIdentitySource::new(
            format!("litecowork-attempt-{}.scope", intent.provider_scope_id),
            preparation_process_identity(),
        );

        let result =
            recheck_journaled_held_scope_before_admission(&journal, &intent.attempt_id, &scope);

        assert!(matches!(
            result,
            Err(HeldScopeJournalError::Journal(
                IsolationStoreError::JournalTransition(_)
            ))
        ));
        assert_eq!(*scope.last_rechecked.lock().unwrap(), None);
    }
}
