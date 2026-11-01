//! Runtime shutdown-boundary tests.
//!
//! A Runtime incarnation that cannot finish its bounded Operator drain must not
//! report a clean shutdown, and the durable record it leaves behind must make
//! the next incarnation perform recovery. These tests cover that boundary
//! directly, including the case where the drain was interrupted after DRAINING
//! had already been persisted.

use super::{
    RuntimeLocalState, RuntimeState, StartupPolicy, advance_to_stopping_after_operator_drain,
    ensure_private_state_directory, persist_state, read_state,
};

#[cfg(unix)]
fn private_directory() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary Runtime directory");
    std::fs::set_permissions(
        directory.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .expect("restrict temporary Runtime directory");
    directory
}

#[cfg(not(unix))]
fn private_directory() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary Runtime directory")
}

fn draining_state(version: u64) -> RuntimeLocalState {
    RuntimeLocalState {
        schema_version: 1,
        runtime_id: "rt_draining_test".to_owned(),
        local_principal_id: Some("principal_draining_test".to_owned()),
        os_principal_binding_established: true,
        local_incarnation_id: "rli_draining_previous".to_owned(),
        startup_policy: StartupPolicy::Manual,
        recovery_state: RuntimeState::Draining,
        recovered_from_unclean_shutdown: false,
        last_shutdown_clean: false,
        blockers: vec!["TASK_ATTEMPT_ADMISSION_NOT_INTEGRATED".to_owned()],
        version,
    }
}

#[test]
fn a_durable_draining_state_is_reported_as_an_unclean_restart() {
    let directory = private_directory();
    ensure_private_state_directory(directory.path()).expect("private Runtime directory");
    persist_state(directory.path(), &draining_state(4)).expect("persist the DRAINING state");

    let stored = read_state(&directory.path().join("runtime-state.json"))
        .expect("the DRAINING state is readable")
        .expect("the DRAINING state exists");
    assert_eq!(stored.recovery_state, RuntimeState::Draining);
    assert!(!stored.last_shutdown_clean);

    let next = RuntimeLocalState::begin(directory.path()).expect("begin the next incarnation");

    assert_eq!(next.runtime_id, "rt_draining_test");
    assert_eq!(
        next.local_principal_id.as_deref(),
        Some("principal_draining_test")
    );
    assert_ne!(
        next.local_incarnation_id, "rli_draining_previous",
        "an unclean restart must create a new incarnation"
    );
    assert!(
        next.recovered_from_unclean_shutdown,
        "a durable DRAINING record must make the next incarnation recover"
    );
    assert!(!next.last_shutdown_clean);
    assert_eq!(next.recovery_state, RuntimeState::Starting);
    assert_eq!(next.version, 5);
}

#[test]
fn a_failed_drain_never_reports_a_stopped_incarnation() {
    let mut state = draining_state(9);
    let blockers = state.blockers.clone();

    let result = advance_to_stopping_after_operator_drain(
        &mut state,
        blockers,
        Err("drain failed".to_owned()),
    );

    assert_eq!(result, Err("drain failed".to_owned()));
    assert_eq!(
        state.recovery_state,
        RuntimeState::Draining,
        "a failed drain must leave the incarnation at DRAINING"
    );
    assert!(
        !state.last_shutdown_clean,
        "a failed drain must not report a clean shutdown"
    );
}
