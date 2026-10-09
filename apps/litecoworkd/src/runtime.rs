use crate::{
    local_filesystem::{LocalDirectoryError, reopen_saved_directory},
    operator::OperatorServer,
};
use domain_task::StepAttemptCoordinator;
use domain_workspace::{EventContext, ResumeWorkspaceRoot, WorkspaceRootService};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc,
};
use storage_core::{
    CommittedWorkspaceRootRevalidation, EventDraft, ExecutionEventContext, ExpireExecutionLease,
    ExpiredAttemptEvents, LocalFileIdentityBindingRecord, LocalResourceLocationBindingRecord,
    LocalRuntimeWorkspaceBindingLookup, RuntimeIncarnationLocalObservationRecord,
    RuntimeIncarnationRecord, RuntimeIncarnationStateUpdate, RuntimeLifecycleStore, RuntimeRecord,
    RuntimeWorkspaceBindingStore, StateStore, StepAttemptStore, WorkspaceCreateRequest,
    WorkspaceRootRevalidationBindings, WorkspaceRootRevalidationCandidate,
    WorkspaceRootRevalidationCommit, WorkspaceRootRevalidationFailure, WorkspaceRootStore,
};
use storage_sqlite::{
    LocalWorkspaceStorage, OsRuntimeDeviceIdentityProvider, OsRuntimePrincipalBindingProvider,
    RuntimeOsPrincipalIdentity, SqliteConfig, SqliteStepAttemptStore, SqliteWorkspaceStore,
};
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const STATE_FILE: &str = "runtime-state.json";
const LOCK_FILE: &str = "runtime.lock";
const STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RuntimeState {
    Starting,
    Recovering,
    Ready,
    Degraded,
    Draining,
    Stopping,
    Stopped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum StartupPolicy {
    Manual,
    LoginBackground,
    AlwaysOnService,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeLocalState {
    schema_version: u32,
    runtime_id: String,
    #[serde(default)]
    local_principal_id: Option<String>,
    #[serde(default)]
    os_principal_binding_established: bool,
    local_incarnation_id: String,
    startup_policy: StartupPolicy,
    recovery_state: RuntimeState,
    recovered_from_unclean_shutdown: bool,
    last_shutdown_clean: bool,
    blockers: Vec<String>,
    version: u64,
}

impl RuntimeLocalState {
    fn begin(data_directory: &Path) -> Result<Self, String> {
        let path = data_directory.join(STATE_FILE);
        let previous = read_state(&path)?;
        let recovered_from_unclean_shutdown = previous
            .as_ref()
            .is_some_and(|state| !state.last_shutdown_clean);
        let runtime_id = match previous.as_ref() {
            Some(state) => state.runtime_id.clone(),
            None => random_id("rt")?,
        };
        let local_principal_id = match previous.as_ref() {
            Some(state) => state.local_principal_id.clone().ok_or_else(|| {
                "existing Runtime state is missing its local Principal identity; explicit local recovery is required".to_owned()
            })?,
            None => random_id("principal")?,
        };
        let state = Self {
            schema_version: STATE_SCHEMA_VERSION,
            runtime_id,
            local_principal_id: Some(local_principal_id),
            os_principal_binding_established: previous
                .as_ref()
                .is_some_and(|state| state.os_principal_binding_established),
            local_incarnation_id: random_id("rli")?,
            startup_policy: previous
                .as_ref()
                .map(|state| state.startup_policy)
                .unwrap_or(StartupPolicy::Manual),
            recovery_state: RuntimeState::Starting,
            recovered_from_unclean_shutdown,
            last_shutdown_clean: false,
            blockers: Vec::new(),
            version: previous.map_or(1, |state| state.version.saturating_add(1)),
        };
        Ok(state)
    }

    fn transition(&mut self, state: RuntimeState, blockers: Vec<String>) {
        self.recovery_state = state;
        self.blockers = blockers;
        self.last_shutdown_clean = state == RuntimeState::Stopped;
        self.version = self.version.saturating_add(1);
    }
}

pub fn run(data_directory: &Path) -> Result<(), String> {
    ensure_private_state_directory(data_directory)?;
    let _instance_lock = InstanceLock::acquire(data_directory)?;
    let shutdown_receiver = install_shutdown_handler()?;
    let first_install = !data_directory.join("litecowork.sqlite3").exists();
    let mut state = RuntimeLocalState::begin(data_directory)?;

    // Persist the generated Runtime ID before writing its credential-store binding.
    // If the process crashes between those operations, the next startup retries the
    // incomplete first-install binding against the same ID instead of orphaning it.
    persist_state(data_directory, &state)?;

    // This must precede durable bootstrap state creation: only an installation with
    // neither a Runtime record nor a database may establish its OS principal for the
    // first time. Losing the keyring entry for an existing install fails closed.
    let os_principal = match OsRuntimePrincipalBindingProvider.load_or_create(
        data_directory,
        &state.runtime_id,
        first_install && !state.os_principal_binding_established,
    ) {
        Ok(identity) => identity,
        Err(_) => {
            return fail_startup(
                data_directory,
                &mut state,
                "RUNTIME_OS_PRINCIPAL_UNAVAILABLE",
                "Runtime OS principal is unavailable or requires explicit local recovery",
            );
        }
    };
    state.os_principal_binding_established = true;
    persist_state(data_directory, &state)?;

    // The local store is opened before the daemon announces any status. This applies
    // migrations and checks the storage adapter's startup gates. No workers are started.
    let storage = match LocalWorkspaceStorage::open(data_directory, SqliteConfig::default()) {
        Ok(storage) => storage,
        Err(_) => {
            return fail_startup(
                data_directory,
                &mut state,
                "DURABLE_STORAGE_OPEN_FAILED",
                "local durable storage could not be opened",
            );
        }
    };

    let started_at = match timestamp_now() {
        Ok(timestamp) => timestamp,
        Err(error) => {
            return fail_startup(
                data_directory,
                &mut state,
                "RUNTIME_CLOCK_UNAVAILABLE",
                &error,
            );
        }
    };
    let identity = match OsRuntimeDeviceIdentityProvider.load_or_create(
        data_directory,
        &state.runtime_id,
        &started_at,
    ) {
        Ok(identity) => identity,
        Err(_) => {
            state.transition(
                RuntimeState::Degraded,
                vec!["RUNTIME_DEVICE_IDENTITY_UNAVAILABLE".to_owned()],
            );
            persist_state(data_directory, &state)?;
            print_state(&state)?;
            return Err("Runtime device identity is unavailable; startup failed closed".to_owned());
        }
    };
    let runtime_record = RuntimeRecord {
        runtime_id: state.runtime_id.clone(),
        device_identity: identity,
        runtime_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        roles: vec!["OPERATOR_ENDPOINT".to_owned()],
        trust_zone: "PERSONAL_DEVICE".to_owned(),
        availability: "RECOVERING".to_owned(),
        startup_policy: startup_policy_name(state.startup_policy).to_owned(),
        current_incarnation_id: state.local_incarnation_id.clone(),
        resource_capacity: json!({"sampled_at": started_at}),
        last_seen: started_at.clone(),
    };
    let incarnation = RuntimeIncarnationRecord {
        runtime_incarnation_id: state.local_incarnation_id.clone(),
        runtime_id: state.runtime_id.clone(),
        process_started_at: started_at.clone(),
        litecowork_version: env!("CARGO_PKG_VERSION").to_owned(),
        recovered_from_unclean_shutdown: state.recovered_from_unclean_shutdown,
        recovery_state: "RECOVERING".to_owned(),
        ready_at: None,
        stopped_at: None,
        version: 1,
    };
    let observation = RuntimeIncarnationLocalObservationRecord {
        runtime_incarnation_id: state.local_incarnation_id.clone(),
        os_boot_id: None,
        observed_at: started_at,
    };
    let mut incarnation_version =
        match storage
            .store
            .register_local_incarnation(runtime_record, incarnation, observation)
        {
            Ok(incarnation) => incarnation.version,
            Err(_) => {
                state.transition(
                    RuntimeState::Degraded,
                    vec!["RUNTIME_REGISTRATION_FAILED".to_owned()],
                );
                persist_state(data_directory, &state)?;
                print_state(&state)?;
                return Err("durable Runtime registration failed; startup failed closed".to_owned());
            }
        };
    state.transition(RuntimeState::Recovering, Vec::new());
    persist_state(data_directory, &state)?;

    if let Err(error) = revalidate_workspace_roots(
        &storage.store,
        &state.runtime_id,
        &state.local_incarnation_id,
        &os_principal,
    ) {
        return fail_registered_startup(
            data_directory,
            &storage.store,
            &mut state,
            &mut incarnation_version,
            "WORKSPACE_ROOT_REVALIDATION_COMMIT_FAILED",
            &error,
        );
    }

    // Expiry recovery is a real durable operation, but it only abandons an
    // expired Attempt and blocks its Task pending Effect reconciliation. It does
    // not resume Tasks or admit new provider work, so the Runtime remains degraded.
    let mut blockers = vec![
        "TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED".to_owned(),
        "TRUST_EFFECT_RECONCILIATION_NOT_INTEGRATED".to_owned(),
    ];
    state.transition(RuntimeState::Degraded, blockers.clone());
    persist_incarnation_transition(
        &storage.store,
        &state,
        &mut incarnation_version,
        RuntimeState::Degraded,
    )?;
    persist_state(data_directory, &state)?;

    match recover_expired_execution_leases(
        &storage.store,
        state
            .local_principal_id
            .as_deref()
            .ok_or_else(|| "local Principal identity is unavailable".to_owned())?,
        &state.runtime_id,
        &state.local_incarnation_id,
    ) {
        Ok(_) => {}
        Err(_) => blockers.push("EXPIRED_EXECUTION_LEASE_RECOVERY_FAILED".to_owned()),
    }

    // Local Workspace reads use an OS-peer-authenticated IPC endpoint. Start
    // accepting commands only after the bounded startup recovery pass settles.
    let operator = OperatorServer::start(
        data_directory,
        storage.store.clone(),
        state
            .local_principal_id
            .clone()
            .ok_or_else(|| "local Principal identity is unavailable".to_owned())?,
        state.runtime_id.clone(),
        state.local_incarnation_id.clone(),
        os_principal.principal.uid(),
        os_principal,
    );
    let mut operator = match operator {
        Ok(server) => OperatorLifecycle::new(Some(server)),
        Err(_) => {
            // Do not leave a live, lock-owning daemon waiting for a supervisor signal
            // when its only Operator endpoint failed to bind or initialize. Persist the
            // specific local blocker and exit nonzero so the service manager can apply its
            // bounded restart policy; a later start can then retry endpoint creation.
            // The durable catalog was already transitioned RECOVERING -> DEGRADED
            // before this bind. Repeating the same durable transition is forbidden, so
            // persist the precise blocker in the private local status and leave the
            // catalog's already-durable availability at DEGRADED.
            return fail_startup(
                data_directory,
                &mut state,
                "OPERATOR_API_START_FAILED",
                "authenticated local Operator endpoint failed to initialize",
            );
        }
    };
    state.transition(RuntimeState::Degraded, blockers);
    persist_state(data_directory, &state)?;
    print_state(&state)?;

    // This is currently reachable only from the process-supervisor signal handler.
    // Do not wire an Operator route to this receiver until RuntimeLifecycleService
    // can prove Task/Attempt, Effect, lease, and local service-reference settlement.
    shutdown_receiver
        .recv()
        .map_err(|_| "daemon shutdown signal channel closed unexpectedly".to_owned())?;

    let shutdown_blockers = state.blockers.clone();
    // Stop accepting new Operator commands before publishing DRAINING. Existing
    // admitted handlers may finish under the bounded server drain below.
    operator.stop_admission()?;
    state.transition(RuntimeState::Draining, shutdown_blockers.clone());
    persist_incarnation_transition(
        &storage.store,
        &state,
        &mut incarnation_version,
        RuntimeState::Draining,
    )?;
    persist_state(data_directory, &state)?;
    // Keep the durable state at DRAINING while already-admitted Operator handlers
    // settle. OperatorServer bounds that drain and closes its listener before this
    // call returns. Only then may the incarnation report STOPPING.
    operator.stop();
    state.transition(RuntimeState::Stopping, shutdown_blockers.clone());
    persist_incarnation_transition(
        &storage.store,
        &state,
        &mut incarnation_version,
        RuntimeState::Stopping,
    )?;
    persist_state(data_directory, &state)?;
    state.transition(RuntimeState::Stopped, shutdown_blockers);
    persist_incarnation_transition(
        &storage.store,
        &state,
        &mut incarnation_version,
        RuntimeState::Stopped,
    )?;
    persist_state(data_directory, &state)?;
    drop(storage);
    print_state(&state)
}

fn install_shutdown_handler() -> Result<mpsc::Receiver<()>, String> {
    // A one-slot channel coalesces repeated signals. Signal handlers only notify the
    // lifecycle thread; all storage, endpoint, and shutdown work stays out of the
    // signal callback.
    let (sender, receiver) = mpsc::sync_channel(1);
    ctrlc::set_handler(move || {
        let _ = sender.try_send(());
    })
    .map_err(|_| "could not register the daemon shutdown handler".to_owned())?;
    Ok(receiver)
}

fn recover_expired_execution_leases(
    store: &SqliteWorkspaceStore,
    owner_principal_id: &str,
    runtime_id: &str,
    runtime_incarnation_id: &str,
) -> Result<usize, String> {
    const PAGE_SIZE: usize = 32;
    const MAX_RECOVERIES_PER_START: usize = 128;

    let attempt_store = SqliteStepAttemptStore::new(store.clone());
    let coordinator = StepAttemptCoordinator::new(attempt_store.clone());
    let workspaces = store
        .list_workspaces()
        .map_err(|_| "local recovery Workspace listing failed".to_owned())?;
    let mut recovered = 0usize;

    for workspace in workspaces {
        if workspace.owner_principal_id != owner_principal_id || workspace.status != "ACTIVE" {
            continue;
        }
        let lookup = LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: owner_principal_id.to_owned(),
            workspace_id: workspace.workspace_id.clone(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
        };
        let Some(binding) = store.get_current_local_binding(lookup).map_err(|_| {
            "current Runtime Workspace authorization could not be checked".to_owned()
        })?
        else {
            continue;
        };
        if !binding.roles.iter().any(|role| role == "EXECUTOR") {
            continue;
        }

        loop {
            let candidates = attempt_store
                .list_expired_execution_leases(
                    owner_principal_id,
                    &workspace.workspace_id,
                    runtime_id,
                    runtime_incarnation_id,
                    PAGE_SIZE,
                )
                .map_err(|_| "expired execution lease inventory failed closed".to_owned())?;
            if candidates.is_empty() {
                break;
            }
            for candidate in candidates {
                if recovered >= MAX_RECOVERIES_PER_START {
                    return Err("startup recovery work limit reached".to_owned());
                }
                let now = timestamp_now()?;
                let correlation_id = random_id("cor")?;
                let event = |event_id: String| ExecutionEventContext {
                    event_id,
                    origin_runtime_id: runtime_id.to_owned(),
                    hlc_timestamp: now.clone(),
                    correlation_id: correlation_id.clone(),
                    causation_id: None,
                    recorded_at: now.clone(),
                };
                let command = ExpireExecutionLease {
                    workspace_id: candidate.workspace_id,
                    owner_principal_id: owner_principal_id.to_owned(),
                    request_id: random_id("recover")?,
                    task_id: candidate.task_id,
                    step_id: candidate.step_id,
                    attempt_id: candidate.attempt_id,
                    lease_id: candidate.lease_id,
                    expected_task_version: candidate.task_version,
                    expected_step_version: candidate.step_version,
                    expected_attempt_version: candidate.attempt_version,
                    expected_lease_version: candidate.lease_version,
                    recovery_runtime_id: runtime_id.to_owned(),
                    recovery_runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                    events: ExpiredAttemptEvents {
                        lease: event(random_id("evt")?),
                        attempt: event(random_id("evt")?),
                        step: event(random_id("evt")?),
                        task: event(random_id("evt")?),
                    },
                };
                coordinator
                    .expire(command)
                    .map_err(|_| "expired execution lease transition did not commit".to_owned())?;
                recovered += 1;
            }
        }
    }
    Ok(recovered)
}

fn revalidate_workspace_roots(
    store: &impl WorkspaceRootStore,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    runtime_identity: &RuntimeOsPrincipalIdentity,
) -> Result<(), String> {
    let mut cursor: Option<(String, String)> = None;
    loop {
        let candidates = store
            .list_workspace_root_revalidation_candidates(
                runtime_id,
                runtime_incarnation_id,
                cursor.as_ref().map(|value| value.0.as_str()),
                cursor.as_ref().map(|value| value.1.as_str()),
                100,
            )
            .map_err(|_| "local WorkspaceRoot recovery query failed".to_owned())?;
        if candidates.is_empty() {
            return Ok(());
        }
        for candidate in candidates {
            let next_cursor = (
                candidate.root.created_at.clone(),
                candidate.root.workspace_root_id.clone(),
            );
            commit_root_revalidation(
                store,
                runtime_id,
                runtime_incarnation_id,
                runtime_identity,
                candidate,
            )?;
            cursor = Some(next_cursor);
        }
    }
}

pub(crate) enum WorkspaceRootResumeError {
    Stale,
    Unavailable,
    NotFound,
    Internal,
}

/// Reopens the saved folder without following symlinks and commits its refreshed private
/// bindings, AVAILABLE location, PAUSED -> ACTIVE status, events, aggregate snapshots,
/// and user idempotency receipt in one transaction. The verified open handle remains
/// alive through the commit. Later filesystem consumers must still use a qualified
/// handle-based provider or perform their own immediate identity check.
pub(crate) fn resume_workspace_root_live(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    runtime_identity: &RuntimeOsPrincipalIdentity,
    workspace_id: &str,
    workspace_root_id: &str,
    principal_id: &str,
    request_id: &str,
    expected_root_version: u64,
    event: EventContext,
) -> Result<storage_core::CommittedWorkspaceRootStatus, WorkspaceRootResumeError> {
    let service = WorkspaceRootService::new(store.clone());
    if let Some(receipt) = service
        .get_root_status_receipt(
            workspace_id,
            workspace_root_id,
            principal_id,
            request_id,
            expected_root_version,
            storage_core::WorkspaceRootStatusAction::Resume,
        )
        .map_err(|_| WorkspaceRootResumeError::Internal)?
    {
        return Ok(receipt);
    }
    let current = store
        .get_workspace_root(workspace_id, workspace_root_id)
        .map_err(|_| WorkspaceRootResumeError::Internal)?
        .ok_or(WorkspaceRootResumeError::NotFound)?;
    if current.version != expected_root_version || current.status != "PAUSED" {
        return Err(WorkspaceRootResumeError::Stale);
    }
    let mut cursor: Option<(String, String)> = None;
    let candidate = 'candidate: loop {
        let candidates = store
            .list_workspace_root_revalidation_candidates(
                runtime_id,
                runtime_incarnation_id,
                cursor.as_ref().map(|value| value.0.as_str()),
                cursor.as_ref().map(|value| value.1.as_str()),
                100,
            )
            .map_err(|_| WorkspaceRootResumeError::Internal)?;
        if candidates.is_empty() {
            return Err(WorkspaceRootResumeError::NotFound);
        }
        for candidate in candidates {
            let next_cursor = (
                candidate.root.created_at.clone(),
                candidate.root.workspace_root_id.clone(),
            );
            if candidate.root.workspace_root_id == workspace_root_id {
                if candidate.root.workspace_id != workspace_id {
                    return Err(WorkspaceRootResumeError::NotFound);
                }
                if candidate.root.version != expected_root_version
                    || candidate.root.status != "PAUSED"
                {
                    return Err(WorkspaceRootResumeError::Stale);
                }
                break 'candidate candidate;
            }
            cursor = Some(next_cursor);
        }
    };
    let (previous_locator_binding, previous_file_identity_binding) = match &candidate.bindings {
        WorkspaceRootRevalidationBindings::Previous {
            locator,
            file_identity,
        }
        | WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        } => (locator, file_identity),
        WorkspaceRootRevalidationBindings::Unavailable(_) => {
            if candidate.location.availability == "AVAILABLE" {
                commit_root_revalidation(
                    store,
                    runtime_id,
                    runtime_incarnation_id,
                    runtime_identity,
                    candidate,
                )
                .map_err(|_| WorkspaceRootResumeError::Internal)?;
            }
            return Err(WorkspaceRootResumeError::Unavailable);
        }
    };
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        if candidate.location.availability == "AVAILABLE" {
            commit_root_revalidation(
                store,
                runtime_id,
                runtime_incarnation_id,
                runtime_identity,
                candidate,
            )
            .map_err(|_| WorkspaceRootResumeError::Internal)?;
        }
        return Err(WorkspaceRootResumeError::Unavailable);
    }
    let opened = match reopen_saved_directory(
        &previous_locator_binding.private_locator,
        &candidate.root.display_name,
    ) {
        Ok(opened) => opened,
        Err(_) => {
            if candidate.location.availability == "AVAILABLE" {
                commit_root_revalidation(
                    store,
                    runtime_id,
                    runtime_incarnation_id,
                    runtime_identity,
                    candidate,
                )
                .map_err(|_| WorkspaceRootResumeError::Internal)?;
            }
            return Err(WorkspaceRootResumeError::Unavailable);
        }
    };
    let live = opened.identity();
    let (identity_digest, keyed_identity) = live.keyed_projection(runtime_identity);
    let identity_matches = live.matches_binding(&previous_file_identity_binding)
        && candidate.resource.identity_digest.as_deref() == Some(identity_digest.as_str())
        && candidate.resource.provider_identity.get("file_identity") == Some(&keyed_identity)
        && candidate
            .resource
            .provider_identity
            .get("provider_instance_id")
            .and_then(serde_json::Value::as_str)
            == Some("litecowork.local_filesystem")
        && opened.revalidate().is_ok();
    if !identity_matches {
        if candidate.location.availability == "AVAILABLE" {
            commit_root_revalidation(
                store,
                runtime_id,
                runtime_incarnation_id,
                runtime_identity,
                candidate,
            )
            .map_err(|_| WorkspaceRootResumeError::Internal)?;
        }
        return Err(WorkspaceRootResumeError::Unavailable);
    }
    let now = timestamp_now().map_err(|_| WorkspaceRootResumeError::Internal)?;
    let location_id = candidate.location.location_id.clone();
    let WorkspaceRootRevalidationCandidate {
        resource,
        location,
        bindings,
        ..
    } = candidate;
    let (previous_locator_binding, previous_file_identity_binding) = match bindings {
        WorkspaceRootRevalidationBindings::Previous {
            locator,
            file_identity,
        }
        | WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        } => (locator, file_identity),
        WorkspaceRootRevalidationBindings::Unavailable(_) => {
            return Err(WorkspaceRootResumeError::Unavailable);
        }
    };
    let new_locator_binding = LocalResourceLocationBindingRecord {
        location_id: location_id.clone(),
        locator_ref_id: location.locator_ref_id.clone(),
        runtime_id: runtime_id.to_owned(),
        runtime_incarnation_id: runtime_incarnation_id.to_owned(),
        private_locator: previous_locator_binding.private_locator.clone(),
        observed_at: now.clone(),
    };
    let new_file_identity_binding = live.binding_record(
        location_id,
        runtime_id.to_owned(),
        runtime_incarnation_id.to_owned(),
        now,
    );
    let result = service
        .resume_root(ResumeWorkspaceRoot {
            workspace_id: workspace_id.to_owned(),
            workspace_root_id: workspace_root_id.to_owned(),
            principal_id: principal_id.to_owned(),
            request_id: request_id.to_owned(),
            expected_version: expected_root_version,
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
            previous_locator_binding,
            previous_file_identity_binding,
            resource,
            location,
            locator_binding: new_locator_binding,
            file_identity_binding: new_file_identity_binding,
            event,
        })
        .map_err(|error| match error {
            storage_core::StoreError::Conflict {
                expected: Some(expected),
                actual: Some(actual),
            } if expected != actual => WorkspaceRootResumeError::Stale,
            storage_core::StoreError::NotFound => WorkspaceRootResumeError::NotFound,
            _ => WorkspaceRootResumeError::Internal,
        });
    // Keep the verified directory handle alive until the atomic owner/status commit has
    // returned. This is a point-in-time proof; no watcher or content read occurs here.
    drop(opened);
    result
}

fn commit_root_revalidation(
    store: &impl WorkspaceRootStore,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    runtime_identity: &RuntimeOsPrincipalIdentity,
    candidate: WorkspaceRootRevalidationCandidate,
) -> Result<CommittedWorkspaceRootRevalidation, String> {
    if candidate.root.status == "REVOKED" {
        return Err("revoked WorkspaceRoot was returned by recovery query".to_owned());
    }
    let mut failure = None;
    let mut verified_pair: Option<(
        LocalResourceLocationBindingRecord,
        LocalFileIdentityBindingRecord,
    )> = None;
    let (previous_locator, previous_identity) = match candidate.bindings {
        WorkspaceRootRevalidationBindings::Previous {
            locator,
            file_identity,
        }
        | WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        } => (Some(locator), Some(file_identity)),
        WorkspaceRootRevalidationBindings::Unavailable(reason) => {
            failure = Some(reason);
            (None, None)
        }
    };
    if let (Some(locator), Some(previous_identity)) = (previous_locator, previous_identity) {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            match reopen_saved_directory(&locator.private_locator, &candidate.root.display_name) {
                Ok(opened) => {
                    let live = opened.identity();
                    if !live.matches_binding(&previous_identity) {
                        failure = Some(WorkspaceRootRevalidationFailure::IdentityChanged);
                    } else {
                        let (identity_digest, keyed_identity) =
                            live.keyed_projection(runtime_identity);
                        let stored_identity =
                            candidate.resource.provider_identity.get("file_identity");
                        if candidate.resource.identity_digest.as_deref()
                            != Some(identity_digest.as_str())
                            || stored_identity != Some(&keyed_identity)
                            || candidate
                                .resource
                                .provider_identity
                                .get("provider_instance_id")
                                .and_then(serde_json::Value::as_str)
                                != Some("litecowork.local_filesystem")
                        {
                            failure = Some(WorkspaceRootRevalidationFailure::IdentityChanged);
                        } else if opened.revalidate().is_err() {
                            failure = Some(WorkspaceRootRevalidationFailure::IdentityChanged);
                        } else {
                            let observed_at = timestamp_now()?;
                            let locator_binding = LocalResourceLocationBindingRecord {
                                location_id: candidate.location.location_id.clone(),
                                locator_ref_id: candidate.location.locator_ref_id.clone(),
                                runtime_id: runtime_id.to_owned(),
                                runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                                private_locator: locator.private_locator,
                                observed_at: observed_at.clone(),
                            };
                            let file_identity_binding = live.binding_record(
                                candidate.location.location_id.clone(),
                                runtime_id.to_owned(),
                                runtime_incarnation_id.to_owned(),
                                observed_at,
                            );
                            verified_pair = Some((locator_binding, file_identity_binding));
                        }
                    }
                }
                Err(error) => {
                    failure = Some(local_directory_failure(error));
                }
            }
        } else {
            failure = Some(WorkspaceRootRevalidationFailure::UnsupportedPlatform);
        }
    }
    let verified = verified_pair.is_some();
    let reason_code = if verified {
        "ROOT_IDENTITY_REVALIDATED"
    } else {
        workspace_root_failure_code(
            failure.unwrap_or(WorkspaceRootRevalidationFailure::IdentityUnavailable),
        )
    };
    let outcome = if verified { "VERIFIED" } else { "UNAVAILABLE" };
    let expected_root_version = candidate.root.version;
    let mut root = candidate.root;
    let from_status = root.status.clone();
    // PAUSED is explicit owner intent. Loss of access to its folder changes the
    // ResourceLocation observation, not that intent; a later successful revalidation
    // must not silently resume observation for a paused root.
    let next_status = match (verified, from_status.as_str()) {
        (true, "UNAVAILABLE") => "ACTIVE",
        (true, status) => status,
        (false, "PAUSED") => "PAUSED",
        (false, _) => "UNAVAILABLE",
    };
    let root_changed = root.status != next_status;
    let observed_at = match verified_pair.as_ref() {
        Some(pair) => pair.0.observed_at.clone(),
        None => timestamp_now()?,
    };
    if root_changed {
        root.status = next_status.to_owned();
        root.updated_at = observed_at.clone();
        root.version = root
            .version
            .checked_add(1)
            .ok_or_else(|| "WorkspaceRoot version overflow".to_owned())?;
    }
    let mut location = candidate.location;
    location.availability = if verified { "AVAILABLE" } else { "UNAVAILABLE" }.to_owned();
    location.observed_at = observed_at.clone();

    // Each revalidation is a new observation. A stable ID derived only from the
    // outcome could replay an old AVAILABLE receipt after a later failed check and
    // prevent a subsequent successful observation from restoring current bindings.
    let request_id = random_id("rootrv")?;
    let correlation_id = request_id.replace("rootrv_", "cor_");
    let request = WorkspaceCreateRequest {
        principal_id: "service:workspace-root-revalidator".to_owned(),
        request_id: request_id.clone(),
        request_payload: json!({
            "operation": "workspace.root.revalidate.v1",
            "workspace_id": root.workspace_id,
            "workspace_root_id": root.workspace_root_id,
            "runtime_id": runtime_id,
            "runtime_incarnation_id": runtime_incarnation_id,
            "expected_root_version": expected_root_version,
            "outcome": outcome,
            "reason_code": reason_code,
        }),
    };
    let root_event = root_changed.then(|| EventDraft {
        event_id: format!("ev_{}_root", &request_id[7..]),
        workspace_id: root.workspace_id.clone(),
        entity_type: "WorkspaceRoot".to_owned(),
        entity_id: root.workspace_root_id.clone(),
        origin_runtime_id: runtime_id.to_owned(),
        entity_revision: root.version,
        hlc_timestamp: observed_at.clone(),
        correlation_id: correlation_id.clone(),
        causation_id: None,
        schema_version: 1,
        event_type: "workspace.root.status.changed.v1".to_owned(),
        payload: json!({
            "workspace_root_id": root.workspace_root_id,
            "from": from_status,
            "to": root.status,
            "reason_code": reason_code,
            "aggregate_version": root.version,
        }),
        recorded_at: observed_at.clone(),
    });
    let resource_id = candidate.resource.resource_id.clone();
    let resource_version = candidate.resource.version;
    let location_event = EventDraft {
        event_id: format!("ev_{}_location", &request_id[7..]),
        workspace_id: root.workspace_id.clone(),
        entity_type: "Resource".to_owned(),
        entity_id: resource_id.clone(),
        origin_runtime_id: runtime_id.to_owned(),
        entity_revision: resource_version,
        hlc_timestamp: observed_at.clone(),
        correlation_id,
        causation_id: None,
        schema_version: 1,
        event_type: "resource.location.changed.v1".to_owned(),
        payload: json!({
            "location_id": location.location_id,
            "resource_id": resource_id,
            "availability": location.availability,
            "observed_at": location.observed_at,
        }),
        recorded_at: observed_at,
    };
    store
        .commit_workspace_root_revalidation(WorkspaceRootRevalidationCommit {
            request,
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
            expected_root_version,
            root,
            resource: candidate.resource,
            location,
            locator_binding: verified_pair.as_ref().map(|pair| pair.0.clone()),
            file_identity_binding: verified_pair.map(|pair| pair.1),
            root_event,
            location_event: Some(location_event),
        })
        .map_err(|_| "WorkspaceRoot revalidation could not be committed atomically".to_owned())
}

fn local_directory_failure(error: LocalDirectoryError) -> WorkspaceRootRevalidationFailure {
    match error {
        LocalDirectoryError::UnsupportedPlatform => {
            WorkspaceRootRevalidationFailure::UnsupportedPlatform
        }
        LocalDirectoryError::InvalidSelection => WorkspaceRootRevalidationFailure::InvalidLocator,
        LocalDirectoryError::IdentityChanged => WorkspaceRootRevalidationFailure::IdentityChanged,
        LocalDirectoryError::NotDirectory | LocalDirectoryError::OpenFailed => {
            WorkspaceRootRevalidationFailure::IdentityUnavailable
        }
    }
}

fn workspace_root_failure_code(failure: WorkspaceRootRevalidationFailure) -> &'static str {
    match failure {
        WorkspaceRootRevalidationFailure::NoPriorBinding => "NO_PRIOR_BINDING",
        WorkspaceRootRevalidationFailure::LocatorBindingMissing => "LOCATOR_BINDING_MISSING",
        WorkspaceRootRevalidationFailure::FileIdentityBindingMissing => {
            "FILE_IDENTITY_BINDING_MISSING"
        }
        WorkspaceRootRevalidationFailure::BindingMismatch => "BINDING_MISMATCH",
        WorkspaceRootRevalidationFailure::UnsupportedPlatform => "UNSUPPORTED_PLATFORM",
        WorkspaceRootRevalidationFailure::InvalidLocator => "INVALID_LOCATOR",
        WorkspaceRootRevalidationFailure::IdentityChanged => "IDENTITY_CHANGED",
        WorkspaceRootRevalidationFailure::IdentityUnavailable => "IDENTITY_UNAVAILABLE",
    }
}

fn fail_startup(
    data_directory: &Path,
    state: &mut RuntimeLocalState,
    blocker: &str,
    message: &str,
) -> Result<(), String> {
    state.transition(RuntimeState::Degraded, vec![blocker.to_owned()]);
    persist_state(data_directory, state)?;
    print_state(state)?;
    Err(message.to_owned())
}

/// Persist a post-registration startup failure to both the local status record and the
/// authoritative Runtime catalog. The local status still becomes DEGRADED if the
/// durable transition itself fails, but the returned error makes that partial failure
/// explicit to the process supervisor/operator.
fn fail_registered_startup(
    data_directory: &Path,
    store: &impl RuntimeLifecycleStore,
    state: &mut RuntimeLocalState,
    incarnation_version: &mut u64,
    blocker: &str,
    message: &str,
) -> Result<(), String> {
    state.transition(RuntimeState::Degraded, vec![blocker.to_owned()]);
    let durable_result =
        persist_incarnation_transition(store, state, incarnation_version, RuntimeState::Degraded);
    persist_state(data_directory, state)?;
    print_state(state)?;
    match durable_result {
        Ok(()) => Err(message.to_owned()),
        Err(error) => Err(format!("{message}; {error}")),
    }
}

/// Owns the Operator thread so every early return joins it before storage and the
/// single-instance lock are released. Dropping a JoinHandle alone would detach a
/// request-serving thread into the next Runtime incarnation.
struct OperatorLifecycle {
    server: Option<OperatorServer>,
}

impl OperatorLifecycle {
    fn new(server: Option<OperatorServer>) -> Self {
        Self { server }
    }

    fn stop_admission(&mut self) -> Result<(), String> {
        match self.server.as_mut() {
            Some(server) => server.stop_admission(),
            None => Ok(()),
        }
    }

    fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.stop();
        }
    }
}

impl Drop for OperatorLifecycle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn persist_incarnation_transition(
    store: &impl RuntimeLifecycleStore,
    state: &RuntimeLocalState,
    incarnation_version: &mut u64,
    next_state: RuntimeState,
) -> Result<(), String> {
    let observed_at = timestamp_now()?;
    let recovery_state = runtime_state_name(next_state);
    let availability = match next_state {
        RuntimeState::Recovering => "RECOVERING",
        RuntimeState::Degraded => "DEGRADED",
        RuntimeState::Draining | RuntimeState::Stopping => "DRAINING",
        RuntimeState::Stopped => "OFFLINE",
        _ => return Err("unsupported durable Runtime transition".to_owned()),
    };
    let update = RuntimeIncarnationStateUpdate {
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
        expected_version: *incarnation_version,
        recovery_state: recovery_state.to_owned(),
        availability: availability.to_owned(),
        observed_at: observed_at.clone(),
        stopped_at: (next_state == RuntimeState::Stopped).then_some(observed_at),
    };
    let record = store
        .transition_local_incarnation(update)
        .map_err(|_| "durable Runtime lifecycle transition failed".to_owned())?;
    *incarnation_version = record.version;
    Ok(())
}

fn startup_policy_name(policy: StartupPolicy) -> &'static str {
    match policy {
        StartupPolicy::Manual => "MANUAL",
        StartupPolicy::LoginBackground => "LOGIN_BACKGROUND",
        StartupPolicy::AlwaysOnService => "ALWAYS_ON_SERVICE",
    }
}

fn timestamp_now() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| "could not format Runtime lifecycle timestamp".to_owned())
}

pub fn print_status(data_directory: &Path) -> Result<(), String> {
    let path = data_directory.join(STATE_FILE);
    match read_state(&path)? {
        Some(state) => {
            let running = is_runtime_running(data_directory)?;
            let view = RuntimeStatusView {
                state: if running {
                    runtime_state_name(state.recovery_state).to_owned()
                } else if state.last_shutdown_clean {
                    "STOPPED".to_owned()
                } else {
                    "NOT_RUNNING".to_owned()
                },
                runtime_id: Some(state.runtime_id),
                local_incarnation_id: Some(state.local_incarnation_id),
                startup_policy: Some(state.startup_policy),
                recovered_from_unclean_shutdown: Some(state.recovered_from_unclean_shutdown),
                last_shutdown_clean: Some(state.last_shutdown_clean),
                blockers: Some(state.blockers),
                version: Some(state.version),
                process_running: running,
                last_state: Some(state.recovery_state),
            };
            let serialized = serde_json::to_string(&view)
                .map_err(|_| "could not serialize Runtime lifecycle status".to_owned())?;
            println!("{serialized}");
            Ok(())
        }
        None => {
            println!("{{\"state\":\"NOT_STARTED\"}}");
            Ok(())
        }
    }
}

#[derive(Serialize)]
struct RuntimeStatusView {
    state: String,
    runtime_id: Option<String>,
    local_incarnation_id: Option<String>,
    startup_policy: Option<StartupPolicy>,
    recovered_from_unclean_shutdown: Option<bool>,
    last_shutdown_clean: Option<bool>,
    blockers: Option<Vec<String>>,
    version: Option<u64>,
    process_running: bool,
    last_state: Option<RuntimeState>,
}

fn runtime_state_name(state: RuntimeState) -> &'static str {
    match state {
        RuntimeState::Starting => "STARTING",
        RuntimeState::Recovering => "RECOVERING",
        RuntimeState::Ready => "READY",
        RuntimeState::Degraded => "DEGRADED",
        RuntimeState::Draining => "DRAINING",
        RuntimeState::Stopping => "STOPPING",
        RuntimeState::Stopped => "STOPPED",
    }
}

fn is_runtime_running(data_directory: &Path) -> Result<bool, String> {
    let lock_path = data_directory.join(LOCK_FILE);
    reject_symlink_if_present(&lock_path)?;
    let file = match open_runtime_lock_file(&lock_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("could not inspect Runtime process lock".to_owned()),
    };
    restrict_file_permissions(&file)?;
    match file.try_lock_exclusive() {
        Ok(()) => {
            FileExt::unlock(&file)
                .map_err(|_| "could not release Runtime status probe lock".to_owned())?;
            Ok(false)
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
        Err(_) => Err("could not determine whether Runtime is running".to_owned()),
    }
}

fn print_state(state: &RuntimeLocalState) -> Result<(), String> {
    let mut value = serde_json::to_value(state)
        .map_err(|_| "could not serialize Runtime lifecycle status".to_owned())?;
    if let Some(object) = value.as_object_mut() {
        object.remove("local_principal_id");
    }
    let serialized = serde_json::to_string(&value)
        .map_err(|_| "could not serialize Runtime lifecycle status".to_owned())?;
    println!("{serialized}");
    Ok(())
}

struct InstanceLock {
    file: File,
}

impl InstanceLock {
    fn acquire(data_directory: &Path) -> Result<Self, String> {
        let path = data_directory.join(LOCK_FILE);
        reject_symlink_if_present(&path)?;
        let file = open_runtime_lock_file(&path)
            .map_err(|_| "could not open the Runtime single-instance lock".to_owned())?;
        restrict_file_permissions(&file)?;
        file.try_lock_exclusive().map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                "another LiteCowork Runtime already owns this state directory".to_owned()
            } else {
                "could not acquire the Runtime single-instance lock".to_owned()
            }
        })?;
        Ok(Self { file })
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn read_state(path: &Path) -> Result<Option<RuntimeLocalState>, String> {
    let mut file = match open_runtime_state_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("could not read Runtime lifecycle state".to_owned()),
    };
    validate_private_regular_file(&file, "Runtime lifecycle state")?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "could not read Runtime lifecycle state".to_owned())?;
    let state: RuntimeLocalState = serde_json::from_slice(&bytes)
        .map_err(|_| "Runtime lifecycle state is malformed or unsupported".to_owned())?;
    if state.schema_version != STATE_SCHEMA_VERSION || state.version == 0 {
        return Err("Runtime lifecycle state version is unsupported".to_owned());
    }
    Ok(Some(state))
}

fn persist_state(data_directory: &Path, state: &RuntimeLocalState) -> Result<(), String> {
    let bytes = serde_json::to_vec(state)
        .map_err(|_| "could not serialize Runtime lifecycle state".to_owned())?;
    let mut temporary = NamedTempFile::new_in(data_directory)
        .map_err(|_| "could not create a private Runtime state update".to_owned())?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| "could not durably write Runtime lifecycle state".to_owned())?;
    let destination = data_directory.join(STATE_FILE);
    temporary
        .persist(&destination)
        .map_err(|_| "could not atomically replace Runtime lifecycle state".to_owned())?;
    sync_directory(data_directory)
}

fn ensure_private_state_directory(path: &Path) -> Result<(), String> {
    let existed = path.exists();
    fs::create_dir_all(path)
        .map_err(|_| "could not create the Runtime state directory".to_owned())?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "could not inspect the Runtime state directory".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Runtime state path must be a real directory".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let mode = metadata.permissions().mode();
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err("Runtime state directory must be owned by the current user".to_owned());
        }
        if existed && mode & 0o077 != 0 {
            return Err("Runtime state directory permissions are too broad".to_owned());
        }
        if !existed {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| "could not restrict Runtime state directory permissions".to_owned())?;
        }
    }
    Ok(())
}

fn restrict_file_permissions(file: &File) -> Result<(), String> {
    validate_current_user_regular_file(file, "Runtime lock")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = file
            .metadata()
            .map_err(|_| "could not inspect Runtime lock permissions".to_owned())?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Runtime lock permissions are too broad".to_owned());
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "could not restrict Runtime lock permissions".to_owned())?;
    }
    Ok(())
}

/// Opens the process lock without following a last-component symlink. The initial
/// lstat remains useful for a clear error, but security depends on the open flags and
/// validation of the descriptor actually returned by the kernel.
fn open_runtime_lock_file(path: &Path) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags, open};

        let descriptor = open(
            path,
            OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|error| std::io::Error::from_raw_os_error(error.raw_os_error()))?;
        let file = File::from(descriptor);
        validate_current_user_regular_file(&file, "Runtime lock").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "unsafe Runtime lock file",
            )
        })?;
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        validate_current_user_regular_file(&file, "Runtime lock").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "unsafe Runtime lock file",
            )
        })?;
        Ok(file)
    }
}

fn open_runtime_state_file(path: &Path) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags, open};

        let descriptor = open(
            path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|error| std::io::Error::from_raw_os_error(error.raw_os_error()))?;
        Ok(File::from(descriptor))
    }
    #[cfg(not(unix))]
    {
        File::open(path)
    }
}

fn validate_private_regular_file(file: &File, label: &str) -> Result<(), String> {
    validate_current_user_regular_file(file, label)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = file
            .metadata()
            .map_err(|_| format!("could not inspect {label} permissions"))?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(format!("{label} permissions are too broad"));
        }
    }
    Ok(())
}

fn validate_current_user_regular_file(file: &File, label: &str) -> Result<(), String> {
    let metadata = file
        .metadata()
        .map_err(|_| format!("could not inspect {label} file"))?;
    if !metadata.is_file() {
        return Err(format!("{label} must be a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err(format!("{label} must be owned by the current user"));
        }
    }
    Ok(())
}

fn reject_symlink_if_present(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Runtime lock path must be a regular file".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("could not inspect Runtime lock path".to_owned()),
    }
}

fn random_id(prefix: &str) -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| "secure random generation failed".to_owned())?;
    Ok(format!("{prefix}_{}", hex::encode(bytes)))
}

fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "could not sync Runtime state directory".to_owned())?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeLocalState, RuntimeState, StartupPolicy, ensure_private_state_directory,
        persist_state,
    };

    #[test]
    fn existing_state_without_local_principal_fails_closed_instead_of_rotating_identity() {
        let directory = tempfile::tempdir().expect("temporary Runtime directory");
        #[cfg(unix)]
        std::fs::set_permissions(
            directory.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .expect("restrict temporary Runtime directory");
        ensure_private_state_directory(directory.path()).expect("private Runtime directory");
        let legacy_state = RuntimeLocalState {
            schema_version: 1,
            runtime_id: "rt_existing".to_owned(),
            local_principal_id: None,
            os_principal_binding_established: true,
            local_incarnation_id: "rli_previous".to_owned(),
            startup_policy: StartupPolicy::Manual,
            recovery_state: RuntimeState::Stopped,
            recovered_from_unclean_shutdown: false,
            last_shutdown_clean: true,
            blockers: Vec::new(),
            version: 1,
        };
        persist_state(directory.path(), &legacy_state).expect("persist existing state");

        let error = RuntimeLocalState::begin(directory.path())
            .expect_err("existing installation must not receive a replacement Principal");

        assert!(error.contains("missing its local Principal identity"));
        assert!(error.contains("explicit local recovery is required"));
    }
}

#[cfg(all(test, unix))]
mod lifecycle_file_tests {
    use super::{RuntimeLocalState, RuntimeState, StartupPolicy, read_state};
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    #[test]
    fn lifecycle_state_open_rejects_a_symlink_target() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("state-target.json");
        let link = directory.path().join("runtime-state.json");
        let state = RuntimeLocalState {
            schema_version: 1,
            runtime_id: "rt_test".to_owned(),
            local_principal_id: None,
            os_principal_binding_established: false,
            local_incarnation_id: "rli_test".to_owned(),
            startup_policy: StartupPolicy::Manual,
            recovery_state: RuntimeState::Stopped,
            recovered_from_unclean_shutdown: false,
            last_shutdown_clean: true,
            blockers: Vec::new(),
            version: 1,
        };
        fs::write(
            &target,
            serde_json::to_vec(&state).expect("serialize state"),
        )
        .expect("write target state");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
            .expect("make target state private");
        symlink(&target, &link).expect("create state symlink");

        assert!(read_state(&link).is_err());
    }
}
