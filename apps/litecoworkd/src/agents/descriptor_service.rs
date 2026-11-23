//! Runtime-owned AgentControlDescriptor service (specs/002 T05).
//!
//! The service is the only producer of a served descriptor. It selects a trusted
//! module by the stored provider identity, spends a per-module probe budget, runs
//! one bounded probe outside any storage transaction, validates and seals the
//! descriptor and appends it to the Runtime-local store with a compare-and-swap on
//! the previously observed digest. Reads return only the latest unexpired
//! revision; the daemon, not the UI, derives freshness.

use crate::agents::control_descriptor::AgentControlDescriptor;
use crate::agents::module_registry::{AgentModuleManifest, AgentModuleRegistry};
use crate::agents::modules::{default_module_registry, opencode};
use crate::agents::native_provider::NativeProviderKey;
use crate::agents::opencode_profile_probe::OpenCodeProfileProbe;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use storage_core::{
    AgentAuthObservationKey, AgentAuthObservationRecord, AgentAuthObservationReservation,
    AgentAuthObservationScope, AgentAuthObservationState, AgentAuthObservationStore,
    AgentControlDescriptorRecord, AgentDescriptorGate, AgentDescriptorStore, StoreError,
};

/// One bounded observation of the exact module version. Implemented by the live
/// OpenCode probe in production and by a fixture in tests.
pub(crate) trait DescriptorProbe: Send + Sync {
    fn probe(&self) -> Option<OpenCodeProfileProbe>;
}

pub(crate) struct LiveOpenCodeProbe;

impl DescriptorProbe for LiveOpenCodeProbe {
    fn probe(&self) -> Option<OpenCodeProfileProbe> {
        let executable = crate::agents::resolve_opencode_executable()?;
        let cwd = std::env::current_dir().ok()?;
        Some(crate::agents::probe_opencode_profile(
            &executable,
            &cwd,
            crate::agents::opencode_native_environment(),
        ))
    }
}

/// In-memory sliding-window probe budget. It is per daemon process: a restart resets
/// it, which is acceptable because a restart also invalidates no stored descriptor
/// and a probe is itself bounded and owner-triggered.
#[derive(Default)]
pub(crate) struct ProbeBudget {
    windows: Mutex<HashMap<String, VecDeque<i64>>>,
}

impl ProbeBudget {
    pub(crate) fn admit(&self, module_id: &str, now_epoch: i64, max_per_minute: u32) -> bool {
        let Ok(mut windows) = self.windows.lock() else {
            return false;
        };
        let window = windows.entry(module_id.to_owned()).or_default();
        while window.front().is_some_and(|at| *at <= now_epoch - 60) {
            window.pop_front();
        }
        if window.len() >= max_per_minute as usize {
            return false;
        }
        window.push_back(now_epoch);
        true
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RefreshError {
    Throttled,
    /// The module could not observe its exact pinned version.
    Unavailable,
    Store,
}

pub(crate) struct AgentDescriptorService {
    registry: AgentModuleRegistry,
    probe: Arc<dyn DescriptorProbe>,
    budget: ProbeBudget,
    auth: Arc<crate::agents::auth_service::AgentAuthService>,
}

impl AgentDescriptorService {
    pub(crate) fn new(registry: AgentModuleRegistry, probe: Arc<dyn DescriptorProbe>) -> Self {
        Self {
            registry,
            probe,
            budget: ProbeBudget::default(),
            auth: crate::agents::auth_service::AgentAuthService::shared(),
        }
    }

    /// The process-wide production service. The probe budget is shared by all
    /// routes, so concurrent refreshes cannot each spend a private budget.
    pub(crate) fn shared() -> Arc<Self> {
        static SERVICE: OnceLock<Arc<AgentDescriptorService>> = OnceLock::new();
        SERVICE
            .get_or_init(|| {
                Arc::new(Self::new(
                    default_module_registry(),
                    Arc::new(LiveOpenCodeProbe),
                ))
            })
            .clone()
    }

    /// Selects a module from the stored provider identity. A provider without a
    /// trusted module (Codex, Claude Code today) has no descriptor.
    pub(crate) fn module_for_provider(&self, provider_key: &str) -> Option<&AgentModuleManifest> {
        match NativeProviderKey::from_profile_key(provider_key)? {
            NativeProviderKey::OpenCode => self.registry.get(opencode::OPENCODE_MODULE_ID),
            NativeProviderKey::ClaudeCode | NativeProviderKey::Codex => None,
        }
    }

    pub(crate) fn gate(
        manifest: &AgentModuleManifest,
        runtime_id: &str,
        runtime_incarnation_id: &str,
    ) -> AgentDescriptorGate {
        AgentDescriptorGate {
            registry_agent_id: manifest.registry_agent_ids[0].clone(),
            runtime_id: runtime_id.to_owned(),
            runtime_incarnation_id: runtime_incarnation_id.to_owned(),
            module_version: manifest.module_version.clone(),
        }
    }

    /// The latest unexpired descriptor, re-validated after load.
    pub(crate) fn current<S: AgentDescriptorStore>(
        &self,
        store: &S,
        manifest: &AgentModuleManifest,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        now: &str,
    ) -> Result<Option<AgentControlDescriptor>, StoreError> {
        let gate = Self::gate(manifest, runtime_id, runtime_incarnation_id);
        let Some(record) = store.current_agent_control_descriptor(&gate, now)? else {
            return Ok(None);
        };
        let descriptor: AgentControlDescriptor = serde_json::from_str(&record.descriptor_json)
            .map_err(|_| StoreError::CorruptSchema("stored descriptor is invalid".to_owned()))?;
        descriptor
            .validate()
            .map_err(|_| StoreError::CorruptSchema("stored descriptor is invalid".to_owned()))?;
        if descriptor.descriptor_digest != record.descriptor_digest {
            return Err(StoreError::CorruptSchema(
                "stored descriptor digest does not match its body".to_owned(),
            ));
        }
        Ok(Some(descriptor))
    }

    /// Runs one bounded probe under the module budget and appends the new revision.
    /// The probe runs before any transaction is opened.
    pub(crate) fn refresh<S: AgentDescriptorStore + AgentAuthObservationStore>(
        &self,
        store: &S,
        manifest: &AgentModuleManifest,
        agent_profile_id: &str,
        runtime_id: &str,
        runtime_incarnation_id: &str,
        now: time::OffsetDateTime,
    ) -> Result<AgentControlDescriptor, RefreshError> {
        if !self.budget.admit(
            &manifest.module_id,
            now.unix_timestamp(),
            manifest.probe_budget_per_minute,
        ) {
            return Err(RefreshError::Throttled);
        }
        let probe = self.probe.probe().ok_or(RefreshError::Unavailable)?;
        let mut descriptor = opencode::build_descriptor(
            &probe,
            runtime_id,
            runtime_incarnation_id,
            now,
            manifest.descriptor_ttl_seconds,
        )
        .map_err(|_| RefreshError::Unavailable)?;
        // Auth state comes only from the module-reported observation (T07).
        self.auth
            .overlay(
                store,
                &mut descriptor,
                agent_profile_id,
                now.unix_timestamp(),
            )
            .map_err(|_| RefreshError::Store)?;
        let gate = Self::gate(manifest, runtime_id, runtime_incarnation_id);
        let prior = store
            .latest_agent_control_descriptor(&gate)
            .map_err(|_| RefreshError::Store)?
            .map(|record| record.descriptor_digest);
        if prior.as_deref() == Some(descriptor.descriptor_digest.as_str()) {
            // Two refreshes sealed in the same second observe identical content and so
            // share a digest; the stored revision already is this descriptor.
            return Ok(descriptor);
        }
        let descriptor_json =
            serde_json::to_string(&descriptor).map_err(|_| RefreshError::Store)?;
        store
            .put_agent_control_descriptor(
                AgentControlDescriptorRecord {
                    registry_agent_id: gate.registry_agent_id,
                    runtime_id: runtime_id.to_owned(),
                    runtime_incarnation_id: runtime_incarnation_id.to_owned(),
                    module_id: manifest.module_id.clone(),
                    module_version: manifest.module_version.clone(),
                    descriptor_api_version: descriptor.descriptor_api_version,
                    descriptor_digest: descriptor.descriptor_digest.clone(),
                    descriptor_json,
                    observed_at: descriptor.observed_at.clone(),
                    expires_at: descriptor.expires_at.clone(),
                },
                prior,
            )
            .map_err(|_| RefreshError::Store)?;
        Ok(descriptor)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::agents::opencode_profile_probe::{OpenCodeProbeReadiness, OpenCodeProfileProbe};

    pub(crate) struct FixtureProbe(pub Option<&'static str>);

    impl DescriptorProbe for FixtureProbe {
        fn probe(&self) -> Option<OpenCodeProfileProbe> {
            let mut probe = OpenCodeProfileProbe::initial();
            probe.protocol_version = self.0.map(str::to_owned);
            probe.probe_readiness = OpenCodeProbeReadiness::Complete;
            probe.host_process_stopped = true;
            Some(probe)
        }
    }

    #[derive(Default)]
    pub(crate) struct MemoryStore {
        rows: Mutex<Vec<AgentControlDescriptorRecord>>,
    }

    impl AgentDescriptorStore for MemoryStore {
        fn put_agent_control_descriptor(
            &self,
            record: AgentControlDescriptorRecord,
            expected_prior_digest: Option<String>,
        ) -> Result<(), StoreError> {
            let mut rows = self.rows.lock().unwrap();
            let prior = rows.last().map(|row| row.descriptor_digest.clone());
            if prior != expected_prior_digest {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            rows.push(record);
            Ok(())
        }

        fn latest_agent_control_descriptor(
            &self,
            _gate: &AgentDescriptorGate,
        ) -> Result<Option<AgentControlDescriptorRecord>, StoreError> {
            Ok(self.rows.lock().unwrap().last().cloned())
        }

        fn current_agent_control_descriptor(
            &self,
            gate: &AgentDescriptorGate,
            now: &str,
        ) -> Result<Option<AgentControlDescriptorRecord>, StoreError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .last()
                .filter(|row| {
                    row.expires_at.as_str() > now
                        && row.runtime_incarnation_id == gate.runtime_incarnation_id
                })
                .cloned())
        }
    }

    impl AgentAuthObservationStore for MemoryStore {
        fn reserve_agent_auth_observation(
            &self,
            _key: AgentAuthObservationKey,
            _observed_at: &str,
            _expires_at: &str,
        ) -> Result<AgentAuthObservationReservation, StoreError> {
            Err(StoreError::Invalid(
                "auth observations unused in this fixture".to_owned(),
            ))
        }

        fn settle_agent_auth_observation(
            &self,
            _key: AgentAuthObservationKey,
            _generation: u64,
            _state: AgentAuthObservationState,
            _observed_at: &str,
            _expires_at: &str,
        ) -> Result<bool, StoreError> {
            Ok(false)
        }

        fn current_agent_auth_observation(
            &self,
            _key: AgentAuthObservationKey,
            _now: &str,
        ) -> Result<Option<AgentAuthObservationRecord>, StoreError> {
            Ok(None)
        }

        fn list_agent_auth_observations(
            &self,
            _scope: AgentAuthObservationScope,
            _now: &str,
        ) -> Result<Vec<AgentAuthObservationRecord>, StoreError> {
            Ok(Vec::new())
        }
    }

    fn service(version: Option<&'static str>) -> AgentDescriptorService {
        AgentDescriptorService::new(default_module_registry(), Arc::new(FixtureProbe(version)))
    }

    fn at(seconds: i64) -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_800_000_000 + seconds).unwrap()
    }

    fn rfc(seconds: i64) -> String {
        at(seconds)
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    }

    #[test]
    fn descriptor_service_selects_a_module_only_for_the_opencode_provider_identity() {
        let service = service(Some("2.0.32"));
        assert!(service.module_for_provider("OPENCODE").is_some());
        assert!(service.module_for_provider("opencode").is_some());
        assert!(service.module_for_provider("CODEX").is_none());
        assert!(service.module_for_provider("CLAUDE_CODE").is_none());
        assert!(service.module_for_provider("something-else").is_none());
    }

    #[test]
    fn descriptor_service_refresh_stores_a_fresh_descriptor_that_expires_at_the_ttl() {
        let service = service(Some("2.0.32"));
        let store = MemoryStore::default();
        let manifest = service.module_for_provider("OPENCODE").unwrap().clone();
        let built = service
            .refresh(&store, &manifest, "profile", "rt", "rti", at(0))
            .unwrap();
        let fresh = service
            .current(&store, &manifest, "rt", "rti", &rfc(899))
            .unwrap();
        assert_eq!(fresh.unwrap().descriptor_digest, built.descriptor_digest);
        assert!(
            service
                .current(&store, &manifest, "rt", "rti", &rfc(900))
                .unwrap()
                .is_none()
        );
        // A refresh after expiry appends a new revision against the stale prior digest.
        let again = service
            .refresh(&store, &manifest, "profile", "rt", "rti", at(1000))
            .unwrap();
        assert_ne!(again.descriptor_digest, built.descriptor_digest);
    }

    #[test]
    fn descriptor_service_refuses_an_unpinned_version_and_stores_nothing() {
        let service = service(Some("2.0.28"));
        let store = MemoryStore::default();
        let manifest = service.module_for_provider("OPENCODE").unwrap().clone();
        assert_eq!(
            service
                .refresh(&store, &manifest, "profile", "rt", "rti", at(0))
                .unwrap_err(),
            RefreshError::Unavailable
        );
        assert!(store.rows.lock().unwrap().is_empty());
    }

    #[test]
    fn descriptor_service_throttles_the_fourth_refresh_in_a_minute_per_module() {
        let service = service(Some("2.0.32"));
        let store = MemoryStore::default();
        let manifest = service.module_for_provider("OPENCODE").unwrap().clone();
        for second in 0..3 {
            service
                .refresh(&store, &manifest, "profile", "rt", "rti", at(second))
                .unwrap();
        }
        assert_eq!(
            service
                .refresh(&store, &manifest, "profile", "rt", "rti", at(10))
                .unwrap_err(),
            RefreshError::Throttled
        );
        // Another module's budget is independent of this one.
        assert!(
            service
                .budget
                .admit("another-module", at(10).unix_timestamp(), 3)
        );
        // The window slides after sixty seconds.
        service
            .refresh(&store, &manifest, "profile", "rt", "rti", at(61))
            .unwrap();
    }

    #[test]
    fn descriptor_service_rejects_a_tampered_stored_descriptor() {
        let service = service(Some("2.0.32"));
        let store = MemoryStore::default();
        let manifest = service.module_for_provider("OPENCODE").unwrap().clone();
        service
            .refresh(&store, &manifest, "profile", "rt", "rti", at(0))
            .unwrap();
        store
            .rows
            .lock()
            .unwrap()
            .last_mut()
            .unwrap()
            .descriptor_json = "{\"module_id\":\"opencode\"}".to_owned();
        assert!(
            service
                .current(&store, &manifest, "rt", "rti", &rfc(1))
                .is_err()
        );
    }

    #[test]
    fn agent_lifecycle_recovery_restart_revalidates_the_descriptor_until_reprobed() {
        let service = service(Some("2.0.32"));
        let store = MemoryStore::default();
        let manifest = service.module_for_provider("OPENCODE").unwrap().clone();
        service
            .refresh(&store, &manifest, "profile", "rt", "incarnation-1", at(0))
            .unwrap();
        // A new RuntimeIncarnation never inherits the previous incarnation's descriptor.
        assert!(
            service
                .current(&store, &manifest, "rt", "incarnation-2", &rfc(1))
                .unwrap()
                .is_none()
        );
        service
            .refresh(&store, &manifest, "profile", "rt", "incarnation-2", at(2))
            .unwrap();
        assert!(
            service
                .current(&store, &manifest, "rt", "incarnation-2", &rfc(3))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn agent_lifecycle_recovery_failed_probe_leaves_the_prior_descriptor_intact() {
        let good = service(Some("2.0.32"));
        let store = MemoryStore::default();
        let manifest = good.module_for_provider("OPENCODE").unwrap().clone();
        let prior = good
            .refresh(&store, &manifest, "profile", "rt", "rti", at(0))
            .unwrap();
        // The probe observes nothing usable (crash, timeout or unpinned version).
        let failing = service(None);
        assert_eq!(
            failing
                .refresh(&store, &manifest, "profile", "rt", "rti", at(5))
                .unwrap_err(),
            RefreshError::Unavailable
        );
        assert_eq!(store.rows.lock().unwrap().len(), 1, "no partial row");
        let current = good
            .current(&store, &manifest, "rt", "rti", &rfc(6))
            .unwrap()
            .unwrap();
        assert_eq!(current.descriptor_digest, prior.descriptor_digest);
    }
}
