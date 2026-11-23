//! Bounded prior-incarnation AgentHost process reconciliation (001-T08 / A-104).
//!
//! This runs after Conversation sessions have been settled and before the Operator
//! endpoint admits requests. Native process identity is Runtime-local and never
//! projected. Unknown identity or shutdown state leaves the host nonterminal and
//! causes the caller to keep the Runtime DEGRADED.

use storage_core::{AgentHostInstanceRecord, AgentHostStore};

use super::process_identity::{self, PreparedProcessRecovery};

const PAGE_SIZE: usize = 64;
const MAX_HOSTS_PER_START: usize = 1024;
const MAX_CAS_RETRIES: usize = 4;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostRecoveryReport {
    pub(crate) examined: usize,
    pub(crate) stopped: usize,
    pub(crate) failures: usize,
}

pub(crate) fn recover_prior_agent_hosts(
    store: &dyn AgentHostStore,
    runtime_id: &str,
    current_runtime_incarnation_id: &str,
    now: &impl Fn() -> String,
) -> HostRecoveryReport {
    let mut report = HostRecoveryReport::default();
    let mut cursor: Option<(String, String)> = None;

    loop {
        if report.examined >= MAX_HOSTS_PER_START {
            report.failures += 1;
            break;
        }
        let remaining = MAX_HOSTS_PER_START - report.examined;
        let page_limit = PAGE_SIZE.min(remaining);
        let page = match store.list_prior_agent_host_instances(
            runtime_id,
            current_runtime_incarnation_id,
            cursor.as_ref().map(|value| value.0.as_str()),
            cursor.as_ref().map(|value| value.1.as_str()),
            page_limit,
        ) {
            Ok(page) => page,
            Err(_) => {
                report.failures += 1;
                break;
            }
        };
        if page.is_empty() {
            break;
        }

        let page_len = page.len();
        for host in page {
            let next_cursor = (host.started_at.clone(), host.host_instance_id.clone());
            if cursor
                .as_ref()
                .is_some_and(|previous| next_cursor <= *previous)
                || host.runtime_id != runtime_id
                || host.runtime_incarnation_id == current_runtime_incarnation_id
            {
                report.failures += 1;
                return report;
            }
            cursor = Some(next_cursor);
            report.examined += 1;

            // A provider-owned process is outside LiteCowork's authority. Its stale
            // local binding is not proof that LiteCowork can or should stop it.
            if host.ownership != "LITECOWORK" {
                continue;
            }
            if is_terminal(&host.state) {
                continue;
            }

            // Process identities are captured only for the current native local-host
            // implementations. An unfamiliar mode fails closed instead of borrowing
            // another mode's cleanup assumptions.
            if !matches!(
                host.hosting_mode.as_str(),
                "LOCAL_PER_SESSION" | "LOCAL_SHARED_DAEMON" | "EXTERNAL_PROCESS"
            ) {
                let _ = mark_degraded(store, &host, now);
                report.failures += 1;
                continue;
            }

            let prepared =
                match process_identity::prepare_recovery(host.process_identity_ref.as_deref()) {
                    Ok(prepared) => prepared,
                    Err(_) => PreparedProcessRecovery::Blocked,
                };
            match prepared {
                PreparedProcessRecovery::Blocked => {
                    let _ = mark_degraded(store, &host, now);
                    report.failures += 1;
                }
                PreparedProcessRecovery::Gone => {
                    if stop_without_signal(store, &host, now) {
                        report.stopped += 1;
                    } else {
                        report.failures += 1;
                    }
                }
                PreparedProcessRecovery::Stop(scope) => {
                    if !commit_stopping(store, &host, now) {
                        report.failures += 1;
                        continue;
                    }
                    if scope.stop().is_err() {
                        // Preserve STOPPING: process exit was not confirmed.
                        report.failures += 1;
                        continue;
                    }
                    if commit_stopped(store, &host, now) {
                        report.stopped += 1;
                    } else {
                        report.failures += 1;
                    }
                }
            }
        }
        if page_len < page_limit {
            break;
        }
        if report.examined == MAX_HOSTS_PER_START {
            let more = store.list_prior_agent_host_instances(
                runtime_id,
                current_runtime_incarnation_id,
                cursor.as_ref().map(|value| value.0.as_str()),
                cursor.as_ref().map(|value| value.1.as_str()),
                1,
            );
            if !matches!(more, Ok(ref page) if page.is_empty()) {
                report.failures += 1;
            }
            break;
        }
    }
    report
}

fn is_terminal(state: &str) -> bool {
    matches!(state, "STOPPED" | "FAILED")
}

fn mark_degraded(
    store: &dyn AgentHostStore,
    host: &AgentHostInstanceRecord,
    now: &impl Fn() -> String,
) -> bool {
    for _ in 0..MAX_CAS_RETRIES {
        let current = match load_host(store, host) {
            Some(current) => current,
            None => return false,
        };
        match current.state.as_str() {
            "DEGRADED" | "STOPPING" => return true,
            "STOPPED" | "FAILED" => return true,
            "STARTING" | "READY" | "BUSY" => {
                let result = store.transition_agent_host_instance(
                    &host.runtime_id,
                    &host.runtime_incarnation_id,
                    &host.host_instance_id,
                    &current.state,
                    "DEGRADED",
                    &now(),
                    host.process_identity_ref.as_deref(),
                );
                if result.is_ok() {
                    return true;
                }
            }
            _ => return false,
        }
    }
    false
}

fn stop_without_signal(
    store: &dyn AgentHostStore,
    host: &AgentHostInstanceRecord,
    now: &impl Fn() -> String,
) -> bool {
    commit_stopping(store, host, now) && commit_stopped(store, host, now)
}

fn commit_stopping(
    store: &dyn AgentHostStore,
    host: &AgentHostInstanceRecord,
    now: &impl Fn() -> String,
) -> bool {
    for _ in 0..MAX_CAS_RETRIES {
        let current = match load_host(store, host) {
            Some(current) => current,
            None => return false,
        };
        match current.state.as_str() {
            "STOPPING" => return true,
            "STARTING" => {
                let _ = store.transition_agent_host_instance(
                    &host.runtime_id,
                    &host.runtime_incarnation_id,
                    &host.host_instance_id,
                    "STARTING",
                    "DEGRADED",
                    &now(),
                    host.process_identity_ref.as_deref(),
                );
            }
            "BUSY" => {
                let _ = store.transition_agent_host_instance(
                    &host.runtime_id,
                    &host.runtime_incarnation_id,
                    &host.host_instance_id,
                    "BUSY",
                    "READY",
                    &now(),
                    host.process_identity_ref.as_deref(),
                );
            }
            "READY" | "DEGRADED" => {
                let _ = store.transition_agent_host_instance(
                    &host.runtime_id,
                    &host.runtime_incarnation_id,
                    &host.host_instance_id,
                    &current.state,
                    "STOPPING",
                    &now(),
                    host.process_identity_ref.as_deref(),
                );
            }
            // Do not signal a process after a terminal row: the lifecycle record says
            // that shutdown was already confirmed, and a later PID is not authority.
            "STOPPED" | "FAILED" => return false,
            _ => return false,
        }
    }
    matches!(
        load_host(store, host)
            .as_ref()
            .map(|row| row.state.as_str()),
        Some("STOPPING")
    )
}

fn commit_stopped(
    store: &dyn AgentHostStore,
    host: &AgentHostInstanceRecord,
    now: &impl Fn() -> String,
) -> bool {
    for _ in 0..MAX_CAS_RETRIES {
        let current = match load_host(store, host) {
            Some(current) => current,
            None => return false,
        };
        match current.state.as_str() {
            "STOPPED" => return true,
            "STOPPING" => {
                if store
                    .transition_agent_host_instance(
                        &host.runtime_id,
                        &host.runtime_incarnation_id,
                        &host.host_instance_id,
                        "STOPPING",
                        "STOPPED",
                        &now(),
                        host.process_identity_ref.as_deref(),
                    )
                    .is_ok()
                {
                    return true;
                }
            }
            _ => return false,
        }
    }
    matches!(
        load_host(store, host)
            .as_ref()
            .map(|row| row.state.as_str()),
        Some("STOPPED")
    )
}

fn load_host(
    store: &dyn AgentHostStore,
    host: &AgentHostInstanceRecord,
) -> Option<AgentHostInstanceRecord> {
    store
        .get_agent_host_instance(
            &host.runtime_id,
            &host.runtime_incarnation_id,
            &host.host_instance_id,
        )
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::recover_prior_agent_hosts;
    use std::sync::{Arc, Mutex};
    use storage_core::{AgentHostInstanceRecord, AgentHostStore, StoreError};

    #[derive(Clone, Default)]
    struct MemoryHostStore(Arc<Mutex<Vec<AgentHostInstanceRecord>>>);

    impl AgentHostStore for MemoryHostStore {
        fn create_agent_host_instance(
            &self,
            host: AgentHostInstanceRecord,
        ) -> Result<(), StoreError> {
            self.0.lock().unwrap().push(host);
            Ok(())
        }

        fn transition_agent_host_instance(
            &self,
            runtime_id: &str,
            runtime_incarnation_id: &str,
            host_instance_id: &str,
            expected_state: &str,
            next_state: &str,
            _occurred_at: &str,
            process_identity_ref: Option<&str>,
        ) -> Result<AgentHostInstanceRecord, StoreError> {
            let mut rows = self.0.lock().unwrap();
            let host = rows
                .iter_mut()
                .find(|host| {
                    host.runtime_id == runtime_id
                        && host.runtime_incarnation_id == runtime_incarnation_id
                        && host.host_instance_id == host_instance_id
                })
                .ok_or(StoreError::NotFound)?;
            if host.state != expected_state {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            host.state = next_state.to_owned();
            if let Some(value) = process_identity_ref {
                host.process_identity_ref = Some(value.to_owned());
            }
            Ok(host.clone())
        }

        fn get_agent_host_instance(
            &self,
            runtime_id: &str,
            runtime_incarnation_id: &str,
            host_instance_id: &str,
        ) -> Result<Option<AgentHostInstanceRecord>, StoreError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .iter()
                .find(|host| {
                    host.runtime_id == runtime_id
                        && host.runtime_incarnation_id == runtime_incarnation_id
                        && host.host_instance_id == host_instance_id
                })
                .cloned())
        }

        fn list_agent_host_instances(
            &self,
            _runtime_id: &str,
            _runtime_incarnation_id: &str,
        ) -> Result<Vec<AgentHostInstanceRecord>, StoreError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn list_prior_agent_host_instances(
            &self,
            runtime_id: &str,
            current_runtime_incarnation_id: &str,
            after_started_at: Option<&str>,
            after_host_instance_id: Option<&str>,
            limit: usize,
        ) -> Result<Vec<AgentHostInstanceRecord>, StoreError> {
            let after = after_started_at.zip(after_host_instance_id);
            let mut rows = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|host| {
                    host.runtime_id == runtime_id
                        && host.runtime_incarnation_id != current_runtime_incarnation_id
                        && after.is_none_or(|(started_at, host_id)| {
                            (host.started_at.as_str(), host.host_instance_id.as_str())
                                > (started_at, host_id)
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            rows.sort_by(|left, right| {
                (&left.started_at, &left.host_instance_id)
                    .cmp(&(&right.started_at, &right.host_instance_id))
            });
            rows.truncate(limit);
            Ok(rows)
        }
    }

    fn host(state: &str, owner: &str, identity: Option<&str>) -> AgentHostInstanceRecord {
        AgentHostInstanceRecord {
            host_instance_id: format!("host-{state}-{owner}"),
            runtime_id: "runtime-test".to_owned(),
            runtime_incarnation_id: "old-incarnation".to_owned(),
            agent_profile_id: "profile-test".to_owned(),
            endpoint_id: "endpoint-test".to_owned(),
            hosting_mode: "LOCAL_PER_SESSION".to_owned(),
            state: state.to_owned(),
            process_identity_ref: identity.map(str::to_owned),
            ownership: owner.to_owned(),
            started_at: format!("2026-11-19T00:00:0{}Z", state.len()),
            last_used_at: "2026-11-19T00:00:00Z".to_owned(),
            idle_since: None,
        }
    }

    #[test]
    fn missing_process_identity_degrades_host_and_reports_recovery_blocker() {
        let store = MemoryHostStore::default();
        store
            .create_agent_host_instance(host("BUSY", "LITECOWORK", None))
            .unwrap();
        let report = recover_prior_agent_hosts(&store, "runtime-test", "new-incarnation", &|| {
            "2026-11-19T01:00:00Z".to_owned()
        });
        assert_eq!(report.examined, 1);
        assert_eq!(report.stopped, 0);
        assert_eq!(report.failures, 1);
        let host = store.0.lock().unwrap()[0].clone();
        assert_eq!(host.state, "DEGRADED");
    }

    #[test]
    fn provider_owned_and_already_terminal_hosts_are_not_signalled_or_changed() {
        let store = MemoryHostStore::default();
        store
            .create_agent_host_instance(host("READY", "EXTERNAL", None))
            .unwrap();
        store
            .create_agent_host_instance(host("STOPPED", "LITECOWORK", None))
            .unwrap();
        let report = recover_prior_agent_hosts(&store, "runtime-test", "new-incarnation", &|| {
            "2026-11-19T01:00:00Z".to_owned()
        });
        assert_eq!(report.failures, 0);
        let hosts = store.0.lock().unwrap();
        assert_eq!(hosts[0].state, "READY");
        assert_eq!(hosts[1].state, "STOPPED");
    }
}
