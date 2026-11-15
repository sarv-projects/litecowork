use storage_core::{AgentHostInstanceRecord, AgentHostStore};

/// Failure to make the durable host lifecycle match its native process state.
#[derive(Debug)]
pub(crate) enum HostStartError<T> {
    InvalidStartingRecord,
    StoreUnavailable,
    ProcessStartFailed,
    StateUnconfirmed,
    CleanupPending(T),
}

fn mark_host_degraded(
    store: &dyn AgentHostStore,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    host_instance_id: &str,
    process_identity_ref: &str,
    now: &impl Fn() -> String,
) {
    let Ok(Some(host)) =
        store.get_agent_host_instance(runtime_id, runtime_incarnation_id, host_instance_id)
    else {
        return;
    };
    if matches!(host.state.as_str(), "STARTING" | "READY" | "BUSY") {
        let _ = store.transition_agent_host_instance(
            runtime_id,
            runtime_incarnation_id,
            host_instance_id,
            &host.state,
            "DEGRADED",
            &now(),
            Some(process_identity_ref),
        );
    }
}

fn prepare_host_stop(
    store: &dyn AgentHostStore,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    host_instance_id: &str,
    process_identity_ref: &str,
    now: &impl Fn() -> String,
) -> bool {
    let Ok(Some(host)) =
        store.get_agent_host_instance(runtime_id, runtime_incarnation_id, host_instance_id)
    else {
        return false;
    };
    match host.state.as_str() {
        "STOPPING" => true,
        "READY" => {
            if store
                .transition_agent_host_instance(
                    runtime_id,
                    runtime_incarnation_id,
                    host_instance_id,
                    "READY",
                    "STOPPING",
                    &now(),
                    Some(process_identity_ref),
                )
                .is_ok()
            {
                return true;
            }
            matches!(
                store.get_agent_host_instance(
                    runtime_id,
                    runtime_incarnation_id,
                    host_instance_id
                ),
                Ok(Some(ref current)) if current.state == "STOPPING"
            )
        }
        _ => false,
    }
}

fn mark_host_stopped(
    store: &dyn AgentHostStore,
    runtime_id: &str,
    runtime_incarnation_id: &str,
    host_instance_id: &str,
    process_identity_ref: &str,
    now: &impl Fn() -> String,
) -> bool {
    store
        .transition_agent_host_instance(
            runtime_id,
            runtime_incarnation_id,
            host_instance_id,
            "STOPPING",
            "STOPPED",
            &now(),
            Some(process_identity_ref),
        )
        .is_ok()
}

/// Persists STARTING before native work and commits READY only after the caller's
/// process-start/readiness handshake succeeds. The caller performs native work only
/// after `create_agent_host_instance` returns, so no native call runs in a storage tx.
pub(crate) fn start_host_process<T>(
    store: &dyn AgentHostStore,
    host: AgentHostInstanceRecord,
    now: impl Fn() -> String,
    start: impl FnOnce() -> Result<T, ()>,
    process_identity_ref: impl FnOnce(&T) -> String,
    stop: impl FnOnce(T) -> Result<(), T>,
) -> Result<T, HostStartError<T>> {
    if host.state != "STARTING" || host.idle_since.is_some() || host.ownership != "LITECOWORK" {
        return Err(HostStartError::InvalidStartingRecord);
    }

    let runtime_id = host.runtime_id.clone();
    let runtime_incarnation_id = host.runtime_incarnation_id.clone();
    let host_instance_id = host.host_instance_id.clone();
    if store.create_agent_host_instance(host).is_err() {
        return Err(HostStartError::StoreUnavailable);
    }

    let mark_failed = || {
        store
            .transition_agent_host_instance(
                &runtime_id,
                &runtime_incarnation_id,
                &host_instance_id,
                "STARTING",
                "FAILED",
                &now(),
                None,
            )
            .is_ok()
    };

    let process = match start() {
        Ok(process) => process,
        Err(()) => {
            if mark_failed() {
                return Err(HostStartError::ProcessStartFailed);
            }
            return Err(HostStartError::StateUnconfirmed);
        }
    };
    let process_identity_ref = process_identity_ref(&process);
    let ready_at = now();

    match store.transition_agent_host_instance(
        &runtime_id,
        &runtime_incarnation_id,
        &host_instance_id,
        "STARTING",
        "READY",
        &ready_at,
        Some(&process_identity_ref),
    ) {
        Ok(_) => Ok(process),
        Err(_) => {
            // READY may have committed even if its acknowledgement was lost. Observe
            // storage before cleanup and persist STOPPING before stopping any process
            // that is already durably READY.
            let stopping = prepare_host_stop(
                store,
                &runtime_id,
                &runtime_incarnation_id,
                &host_instance_id,
                &process_identity_ref,
                &now,
            );
            match stop(process) {
                Ok(()) if stopping => {
                    if mark_host_stopped(
                        store,
                        &runtime_id,
                        &runtime_incarnation_id,
                        &host_instance_id,
                        &process_identity_ref,
                        &now,
                    ) {
                        Err(HostStartError::StoreUnavailable)
                    } else {
                        Err(HostStartError::StateUnconfirmed)
                    }
                }
                Ok(()) if mark_failed() => Err(HostStartError::StoreUnavailable),
                Ok(()) => Err(HostStartError::StateUnconfirmed),
                Err(process) if stopping => Err(HostStartError::CleanupPending(process)),
                Err(process) => {
                    // If storage still shows STARTING, an unconfirmed failed startup
                    // cannot be represented as STOPPED. Degraded is the accepted
                    // conservative state until cleanup/recovery can be confirmed.
                    mark_host_degraded(
                        store,
                        &runtime_id,
                        &runtime_incarnation_id,
                        &host_instance_id,
                        &process_identity_ref,
                        &now,
                    );
                    Err(HostStartError::CleanupPending(process))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::start_host_process;
    use std::sync::{Arc, Mutex};
    use storage_core::{AgentHostInstanceRecord, AgentHostStore, StoreError};

    #[derive(Clone, Default)]
    struct RecordingHostStore {
        events: Arc<Mutex<Vec<String>>>,
        fail_ready_once: Arc<Mutex<bool>>,
        commit_ready_then_fail: Arc<Mutex<bool>>,
        host: Arc<Mutex<Option<AgentHostInstanceRecord>>>,
    }

    impl AgentHostStore for RecordingHostStore {
        fn create_agent_host_instance(
            &self,
            host: AgentHostInstanceRecord,
        ) -> Result<(), StoreError> {
            self.events
                .lock()
                .expect("recording lock")
                .push(format!("create:{}", host.state));
            *self.host.lock().expect("host lock") = Some(host);
            Ok(())
        }

        fn transition_agent_host_instance(
            &self,
            _runtime_id: &str,
            _runtime_incarnation_id: &str,
            _host_instance_id: &str,
            expected_state: &str,
            next_state: &str,
            _occurred_at: &str,
            process_identity_ref: Option<&str>,
        ) -> Result<AgentHostInstanceRecord, StoreError> {
            self.events.lock().expect("recording lock").push(format!(
                "transition:{expected_state}>{next_state}:{}",
                process_identity_ref.unwrap_or("none")
            ));
            if next_state == "READY" {
                let mut fail_ready = self.fail_ready_once.lock().expect("failure lock");
                if *fail_ready {
                    *fail_ready = false;
                    if *self
                        .commit_ready_then_fail
                        .lock()
                        .expect("ambiguous commit lock")
                    {
                        if let Some(host) = self.host.lock().expect("host lock").as_mut() {
                            host.state = "READY".to_owned();
                            host.process_identity_ref = process_identity_ref.map(str::to_owned);
                        }
                    }
                    return Err(StoreError::Busy);
                }
            }
            if let Some(host) = self.host.lock().expect("host lock").as_mut() {
                if host.state != expected_state {
                    return Err(StoreError::Conflict {
                        expected: None,
                        actual: None,
                    });
                }
                host.state = next_state.to_owned();
                host.process_identity_ref = process_identity_ref.map(str::to_owned);
                return Ok(host.clone());
            }
            Err(StoreError::NotFound)
        }

        fn get_agent_host_instance(
            &self,
            _runtime_id: &str,
            _runtime_incarnation_id: &str,
            _host_instance_id: &str,
        ) -> Result<Option<AgentHostInstanceRecord>, StoreError> {
            Ok(self.host.lock().expect("host lock").clone())
        }

        fn list_agent_host_instances(
            &self,
            _runtime_id: &str,
            _runtime_incarnation_id: &str,
        ) -> Result<Vec<AgentHostInstanceRecord>, StoreError> {
            Ok(self
                .host
                .lock()
                .expect("host lock")
                .clone()
                .into_iter()
                .collect())
        }
    }

    fn host_record(state: &str) -> AgentHostInstanceRecord {
        AgentHostInstanceRecord {
            host_instance_id: "host-test".to_owned(),
            runtime_id: "runtime-test".to_owned(),
            runtime_incarnation_id: "incarnation-test".to_owned(),
            agent_profile_id: "profile-test".to_owned(),
            endpoint_id: "endpoint-test".to_owned(),
            hosting_mode: "LOCAL_SHARED_DAEMON".to_owned(),
            state: state.to_owned(),
            process_identity_ref: None,
            ownership: "LITECOWORK".to_owned(),
            started_at: "2026-11-14T00:00:00Z".to_owned(),
            last_used_at: "2026-11-14T00:00:00Z".to_owned(),
            idle_since: None,
        }
    }

    #[test]
    fn host_readiness_is_committed_only_after_process_start_observes_ready() {
        let store = RecordingHostStore::default();
        let order = Arc::clone(&store.events);
        let process = start_host_process(
            &store,
            host_record("STARTING"),
            || {
                order
                    .lock()
                    .expect("recording lock")
                    .push("clock:ready".to_owned());
                "2026-11-14T00:00:01Z".to_owned()
            },
            || {
                order
                    .lock()
                    .expect("recording lock")
                    .push("spawn:ready".to_owned());
                Ok::<_, ()>("process-handle")
            },
            |process| format!("identity:{process}"),
            |_| Ok::<(), &str>(()),
        )
        .expect("ready process should be returned");

        assert_eq!(process, "process-handle");
        assert_eq!(
            *store.events.lock().expect("recording lock"),
            [
                "create:STARTING",
                "spawn:ready",
                "clock:ready",
                "transition:STARTING>READY:identity:process-handle"
            ]
        );
    }

    #[test]
    fn failed_native_start_settles_failed_without_publishing_ready() {
        let store = RecordingHostStore::default();
        let order = Arc::clone(&store.events);
        let result = start_host_process(
            &store,
            host_record("STARTING"),
            || "2026-11-14T00:00:02Z".to_owned(),
            || {
                order
                    .lock()
                    .expect("recording lock")
                    .push("spawn:failed".to_owned());
                Err(())
            },
            |_| "unused-process-ref".to_owned(),
            |_| Ok::<(), &str>(()),
        );

        assert!(matches!(
            result,
            Err(super::HostStartError::ProcessStartFailed)
        ));
        assert_eq!(
            *store.events.lock().expect("recording lock"),
            [
                "create:STARTING",
                "spawn:failed",
                "transition:STARTING>FAILED:none"
            ]
        );
    }

    #[test]
    fn unconfirmed_cleanup_after_ready_commit_failure_marks_host_degraded() {
        let store = RecordingHostStore::default();
        *store.fail_ready_once.lock().expect("failure lock") = true;
        let order = Arc::clone(&store.events);
        let result = start_host_process(
            &store,
            host_record("STARTING"),
            || "2026-11-14T00:00:04Z".to_owned(),
            || Ok::<_, ()>("process-handle"),
            |process| format!("identity:{process}"),
            |process| Err::<(), _>(process),
        );

        assert!(matches!(
            result,
            Err(super::HostStartError::CleanupPending("process-handle"))
        ));
        assert_eq!(
            *order.lock().expect("recording lock"),
            [
                "create:STARTING",
                "transition:STARTING>READY:identity:process-handle",
                "transition:STARTING>DEGRADED:identity:process-handle"
            ],
            "an unconfirmed child stop must not leave a host advertised as STARTING"
        );
    }

    #[test]
    fn unconfirmed_cleanup_reconciles_lost_ready_commit_ack_before_stopping() {
        let store = RecordingHostStore::default();
        *store.fail_ready_once.lock().expect("failure lock") = true;
        *store
            .commit_ready_then_fail
            .lock()
            .expect("ambiguous commit lock") = true;
        let order = Arc::clone(&store.events);
        let result = start_host_process(
            &store,
            host_record("STARTING"),
            || "2026-11-14T00:00:05Z".to_owned(),
            || Ok::<_, ()>("process-handle"),
            |process| format!("identity:{process}"),
            |process| {
                order
                    .lock()
                    .expect("recording lock")
                    .push("stop:unconfirmed".to_owned());
                Err::<(), _>(process)
            },
        );

        assert!(matches!(
            result,
            Err(super::HostStartError::CleanupPending("process-handle"))
        ));
        assert_eq!(
            *order.lock().expect("recording lock"),
            [
                "create:STARTING",
                "transition:STARTING>READY:identity:process-handle",
                "transition:READY>STOPPING:identity:process-handle",
                "stop:unconfirmed"
            ],
            "a committed READY host must enter STOPPING before process cleanup"
        );
    }

    #[test]
    fn observed_cleanup_after_lost_ready_ack_uses_stopping_before_stopped() {
        let store = RecordingHostStore::default();
        *store.fail_ready_once.lock().expect("failure lock") = true;
        *store
            .commit_ready_then_fail
            .lock()
            .expect("ambiguous commit lock") = true;
        let order = Arc::clone(&store.events);
        let result = start_host_process(
            &store,
            host_record("STARTING"),
            || "2026-11-14T00:00:06Z".to_owned(),
            || Ok::<_, ()>("process-handle"),
            |process| format!("identity:{process}"),
            |process| {
                order
                    .lock()
                    .expect("recording lock")
                    .push("stop:observed-exit".to_owned());
                assert_eq!(process, "process-handle");
                Ok::<(), &str>(())
            },
        );

        assert!(matches!(result, Err(super::HostStartError::StoreUnavailable)));
        assert_eq!(
            *order.lock().expect("recording lock"),
            [
                "create:STARTING",
                "transition:STARTING>READY:identity:process-handle",
                "transition:READY>STOPPING:identity:process-handle",
                "stop:observed-exit",
                "transition:STOPPING>STOPPED:identity:process-handle"
            ]
        );
        assert_eq!(
            store
                .host
                .lock()
                .expect("host lock")
                .as_ref()
                .map(|host| host.state.as_str()),
            Some("STOPPED")
        );
    }

    #[test]
    fn invalid_host_record_is_rejected_before_storage_or_native_work() {
        let store = RecordingHostStore::default();
        let result = start_host_process(
            &store,
            host_record("READY"),
            || "2026-11-14T00:00:03Z".to_owned(),
            || panic!("invalid records must not start a native process"),
            |_| "unused-process-ref".to_owned(),
            |_| Ok::<(), &str>(()),
        );

        assert!(matches!(
            result,
            Err(super::HostStartError::InvalidStartingRecord)
        ));
        assert!(store.events.lock().expect("recording lock").is_empty());
    }
}
