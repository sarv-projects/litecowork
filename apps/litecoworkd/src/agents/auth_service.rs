//! Runtime-local native sign-in begin and auth observation (specs/002 T07).
//!
//! LiteCowork never captures a credential here. A module declares its own auth
//! methods in its descriptor; `begin` reserves an exact-scope durable observation
//! before launching a supported native flow. The provider call is outside SQLite.
//! Launch success means only AUTHENTICATING; a later module-reported outcome is
//! generation-fenced and time-bounded. Missing or expired state is UNKNOWN.

use crate::agents::control_descriptor::{
    AgentAuthMethodDescriptor, AgentAuthMethodKind, AgentAuthState, AgentControlDescriptor,
};
use std::sync::{Arc, OnceLock};
use storage_core::{
    AgentAuthObservationKey, AgentAuthObservationRecord, AgentAuthObservationReservation,
    AgentAuthObservationScope, AgentAuthObservationState, AgentAuthObservationStore, StoreError,
};

/// Longest a native sign-in may stay in flight before it reads back as `UNKNOWN`.
const AUTHENTICATING_WINDOW_SECONDS: i64 = 300;
/// Longest a completed module observation stays valid.
const OUTCOME_WINDOW_SECONDS: i64 = 900;

/// Launches the module's own native login. Implementations must not accept or
/// return any credential bytes.
pub(crate) trait NativeLoginLauncher: Send + Sync {
    fn supports(&self, module_id: &str, auth_method_id: &str) -> bool;
    fn launch(&self, module_id: &str, auth_method_id: &str) -> Result<(), LaunchError>;
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LaunchError {
    /// The exact method has no qualified launcher for this module on this OS.
    Unsupported,
    Failed,
}

/// Production launcher: V1 ships no module with a qualified native login launch, so
/// every launch is refused rather than guessed.
pub(crate) struct UnqualifiedLauncher;

impl NativeLoginLauncher for UnqualifiedLauncher {
    fn supports(&self, _module_id: &str, _auth_method_id: &str) -> bool {
        false
    }

    fn launch(&self, _module_id: &str, _auth_method_id: &str) -> Result<(), LaunchError> {
        Err(LaunchError::Unsupported)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BeginOutcome {
    /// A native flow was launched now.
    Started {
        generation: u64,
        expires_at: i64,
    },
    /// A flow was already in flight; no second launch happened.
    InFlight {
        generation: u64,
        expires_at: i64,
    },
    SecretRequired,
    NativeConfigRequired,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BeginError {
    /// The method id is not declared by the module's fresh descriptor.
    MethodNotDeclared,
    /// The declared method kind has no qualified begin path in V1.
    Unsupported,
    LaunchFailed,
    Storage,
}

pub(crate) struct AgentAuthService {
    launcher: Arc<dyn NativeLoginLauncher>,
}

impl AgentAuthService {
    pub(crate) fn new(launcher: Arc<dyn NativeLoginLauncher>) -> Self {
        Self { launcher }
    }

    pub(crate) fn shared() -> Arc<Self> {
        static SERVICE: OnceLock<Arc<AgentAuthService>> = OnceLock::new();
        SERVICE
            .get_or_init(|| Arc::new(Self::new(Arc::new(UnqualifiedLauncher))))
            .clone()
    }

    /// Begins one declared method under an exact profile/Runtime/incarnation scope.
    /// The SQLite transaction reserves a generation and ends before process launch.
    pub(crate) fn begin<S: AgentAuthObservationStore>(
        &self,
        store: &S,
        agent_profile_id: &str,
        descriptor: &AgentControlDescriptor,
        auth_method_id: &str,
        now_epoch: i64,
    ) -> Result<BeginOutcome, BeginError> {
        let method: &AgentAuthMethodDescriptor = descriptor
            .auth
            .methods
            .iter()
            .find(|method| method.auth_method_id == auth_method_id)
            .ok_or(BeginError::MethodNotDeclared)?;
        match method.kind {
            AgentAuthMethodKind::SecretSlot => return Ok(BeginOutcome::SecretRequired),
            AgentAuthMethodKind::NativeConfigOnly => {
                return Ok(BeginOutcome::NativeConfigRequired);
            }
            AgentAuthMethodKind::ExternalHandoff | AgentAuthMethodKind::DeviceCode => {
                return Err(BeginError::Unsupported);
            }
            AgentAuthMethodKind::NativeFlow => {}
        }
        if !self
            .launcher
            .supports(&descriptor.module_id, auth_method_id)
        {
            return Err(BeginError::Unsupported);
        }
        let key = observation_key(agent_profile_id, descriptor, auth_method_id);
        let observed_at = timestamp(now_epoch)?;
        let expires_epoch = now_epoch.saturating_add(AUTHENTICATING_WINDOW_SECONDS);
        let expires_at = timestamp(expires_epoch)?;
        let generation = match store
            .reserve_agent_auth_observation(key.clone(), &observed_at, &expires_at)
            .map_err(|_| BeginError::Storage)?
        {
            AgentAuthObservationReservation::InFlight(record) => {
                return Ok(BeginOutcome::InFlight {
                    generation: record.generation,
                    expires_at: parse_timestamp(&record.expires_at)?,
                });
            }
            AgentAuthObservationReservation::Reserved { generation, .. } => generation,
        };

        // The durable reservation makes concurrent begin calls single-flight, but no
        // SQLite transaction is held while the native process is started.
        match self.launcher.launch(&descriptor.module_id, auth_method_id) {
            Ok(()) => Ok(BeginOutcome::Started {
                generation,
                expires_at: expires_epoch,
            }),
            Err(error) => {
                let settle_result =
                    self.record_outcome(store, &key, generation, AgentAuthState::Error, now_epoch);
                settle_result.map_err(|_| BeginError::Storage)?;
                match error {
                    LaunchError::Unsupported => Err(BeginError::Unsupported),
                    LaunchError::Failed => Err(BeginError::LaunchFailed),
                }
            }
        }
    }

    /// Records only a module-reported, closed outcome for the exact current begin
    /// generation. Late callbacks cannot overwrite a newer attempt.
    pub(crate) fn record_outcome<S: AgentAuthObservationStore>(
        &self,
        store: &S,
        key: &AgentAuthObservationKey,
        generation: u64,
        state: AgentAuthState,
        now_epoch: i64,
    ) -> Result<bool, StoreError> {
        let Some(state) = stored_state(state) else {
            return Ok(false);
        };
        let observed_at = timestamp(now_epoch)
            .map_err(|_| StoreError::Invalid("auth observation time is invalid".to_owned()))?;
        let expires_at = timestamp(now_epoch.saturating_add(OUTCOME_WINDOW_SECONDS))
            .map_err(|_| StoreError::Invalid("auth observation time is invalid".to_owned()))?;
        store.settle_agent_auth_observation(
            key.clone(),
            generation,
            state,
            &observed_at,
            &expires_at,
        )
    }

    /// A cancelled sign-in is a failed observation, never an authenticated one.
    pub(crate) fn cancel<S: AgentAuthObservationStore>(
        &self,
        store: &S,
        key: &AgentAuthObservationKey,
        generation: u64,
        now_epoch: i64,
    ) -> Result<bool, StoreError> {
        self.record_outcome(store, key, generation, AgentAuthState::NeedsAuth, now_epoch)
    }

    /// The latest matching unexpired method state. Missing or expired state is
    /// UNKNOWN; errors never become a positive auth result.
    pub(crate) fn state<S: AgentAuthObservationStore>(
        &self,
        store: &S,
        key: AgentAuthObservationKey,
        now_epoch: i64,
    ) -> Result<AgentAuthState, StoreError> {
        let now = timestamp(now_epoch)
            .map_err(|_| StoreError::Invalid("auth observation time is invalid".to_owned()))?;
        Ok(store
            .current_agent_auth_observation(key, &now)?
            .map_or(AgentAuthState::Unknown, |record| public_state(record.state)))
    }

    /// Applies an exact-scope observation to a fresh module descriptor and reseals
    /// it. A fresh explicit module state wins over older cached observations; absent
    /// module state can use a still-current observation. `AUTHENTICATING` remains
    /// visible while its reservation is live.
    pub(crate) fn overlay<S: AgentAuthObservationStore>(
        &self,
        store: &S,
        descriptor: &mut AgentControlDescriptor,
        agent_profile_id: &str,
        now_epoch: i64,
    ) -> Result<(), StoreError> {
        let scope = AgentAuthObservationScope {
            agent_profile_id: agent_profile_id.to_owned(),
            runtime_id: descriptor.installation.runtime_id.clone(),
            runtime_incarnation_id: descriptor.installation.runtime_incarnation_id.clone(),
            module_id: descriptor.module_id.clone(),
            module_version: descriptor.module_version.clone(),
        };
        let now = timestamp(now_epoch)
            .map_err(|_| StoreError::Invalid("auth observation time is invalid".to_owned()))?;
        let declared_methods: std::collections::HashSet<&str> = descriptor
            .auth
            .methods
            .iter()
            .map(|method| method.auth_method_id.as_str())
            .collect();
        let latest = store
            .list_agent_auth_observations(scope, &now)?
            .into_iter()
            .filter(|record| declared_methods.contains(record.key.auth_method_id.as_str()))
            .max_by(|left, right| {
                left.observed_at
                    .cmp(&right.observed_at)
                    .then_with(|| left.key.auth_method_id.cmp(&right.key.auth_method_id))
            });
        let Some(record) = latest else {
            return Ok(());
        };
        let module_state_is_unknown = descriptor.auth.state == AgentAuthState::Unknown;
        let observation_is_newer = record.observed_at >= descriptor.observed_at;
        let is_live_reservation = record.state == AgentAuthObservationState::Authenticating;
        if !(module_state_is_unknown || observation_is_newer || is_live_reservation) {
            return Ok(());
        }
        let state = public_state(record.state);
        if descriptor.auth.state == state {
            return Ok(());
        }
        descriptor.auth.state = state;
        descriptor.descriptor_digest = descriptor
            .computed_digest()
            .map_err(|_| StoreError::Integrity("auth descriptor reseal failed".to_owned()))?;
        descriptor
            .validate()
            .map_err(|_| StoreError::Integrity("auth descriptor validation failed".to_owned()))
    }
}

fn observation_key(
    agent_profile_id: &str,
    descriptor: &AgentControlDescriptor,
    auth_method_id: &str,
) -> AgentAuthObservationKey {
    AgentAuthObservationKey {
        agent_profile_id: agent_profile_id.to_owned(),
        runtime_id: descriptor.installation.runtime_id.clone(),
        runtime_incarnation_id: descriptor.installation.runtime_incarnation_id.clone(),
        module_id: descriptor.module_id.clone(),
        module_version: descriptor.module_version.clone(),
        auth_method_id: auth_method_id.to_owned(),
    }
}

fn stored_state(state: AgentAuthState) -> Option<AgentAuthObservationState> {
    match state {
        AgentAuthState::Unknown | AgentAuthState::Authenticating => None,
        AgentAuthState::NeedsAuth => Some(AgentAuthObservationState::NeedsAuth),
        AgentAuthState::Authenticated => Some(AgentAuthObservationState::Authenticated),
        AgentAuthState::Expired => Some(AgentAuthObservationState::Expired),
        AgentAuthState::Degraded => Some(AgentAuthObservationState::Degraded),
        AgentAuthState::Error => Some(AgentAuthObservationState::Error),
    }
}

fn public_state(state: AgentAuthObservationState) -> AgentAuthState {
    match state {
        AgentAuthObservationState::NeedsAuth => AgentAuthState::NeedsAuth,
        AgentAuthObservationState::Authenticating => AgentAuthState::Authenticating,
        AgentAuthObservationState::Authenticated => AgentAuthState::Authenticated,
        AgentAuthObservationState::Expired => AgentAuthState::Expired,
        AgentAuthObservationState::Degraded => AgentAuthState::Degraded,
        AgentAuthObservationState::Error => AgentAuthState::Error,
    }
}

fn timestamp(epoch: i64) -> Result<String, BeginError> {
    time::OffsetDateTime::from_unix_timestamp(epoch)
        .ok()
        .and_then(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .ok_or(BeginError::Storage)
}

fn parse_timestamp(value: &str) -> Result<i64, BeginError> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map(|value| value.unix_timestamp())
        .map_err(|_| BeginError::Storage)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::agents::modules::opencode;
    use crate::agents::opencode_profile_probe::{OpenCodeProbeReadiness, OpenCodeProfileProbe};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingLauncher(AtomicUsize, Result<(), LaunchError>);

    impl NativeLoginLauncher for CountingLauncher {
        fn supports(&self, _module: &str, _method: &str) -> bool {
            true
        }

        fn launch(&self, _module: &str, _method: &str) -> Result<(), LaunchError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            match &self.1 {
                Ok(()) => Ok(()),
                Err(LaunchError::Unsupported) => Err(LaunchError::Unsupported),
                Err(LaunchError::Failed) => Err(LaunchError::Failed),
            }
        }
    }

    #[derive(Default)]
    struct MemoryAuthStore {
        rows: Mutex<HashMap<AgentAuthObservationKey, AgentAuthObservationRecord>>,
    }

    impl AgentAuthObservationStore for MemoryAuthStore {
        fn reserve_agent_auth_observation(
            &self,
            key: AgentAuthObservationKey,
            observed_at: &str,
            expires_at: &str,
        ) -> Result<AgentAuthObservationReservation, StoreError> {
            let mut rows = self.rows.lock().unwrap();
            if let Some(record) = rows.get(&key)
                && record.state == AgentAuthObservationState::Authenticating
                && record.expires_at.as_str() > observed_at
            {
                return Ok(AgentAuthObservationReservation::InFlight(record.clone()));
            }
            let generation = rows.get(&key).map_or(1, |record| record.generation + 1);
            rows.insert(
                key.clone(),
                AgentAuthObservationRecord {
                    key,
                    state: AgentAuthObservationState::Authenticating,
                    generation,
                    observed_at: observed_at.to_owned(),
                    expires_at: expires_at.to_owned(),
                },
            );
            Ok(AgentAuthObservationReservation::Reserved {
                generation,
                expires_at: expires_at.to_owned(),
            })
        }

        fn settle_agent_auth_observation(
            &self,
            key: AgentAuthObservationKey,
            generation: u64,
            state: AgentAuthObservationState,
            observed_at: &str,
            expires_at: &str,
        ) -> Result<bool, StoreError> {
            let mut rows = self.rows.lock().unwrap();
            let Some(record) = rows.get_mut(&key) else {
                return Ok(false);
            };
            if record.generation != generation
                || record.state != AgentAuthObservationState::Authenticating
                || record.expires_at.as_str() <= observed_at
            {
                return Ok(false);
            }
            record.state = state;
            record.observed_at = observed_at.to_owned();
            record.expires_at = expires_at.to_owned();
            Ok(true)
        }

        fn current_agent_auth_observation(
            &self,
            key: AgentAuthObservationKey,
            now: &str,
        ) -> Result<Option<AgentAuthObservationRecord>, StoreError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .get(&key)
                .filter(|record| record.expires_at.as_str() > now)
                .cloned())
        }

        fn list_agent_auth_observations(
            &self,
            scope: AgentAuthObservationScope,
            now: &str,
        ) -> Result<Vec<AgentAuthObservationRecord>, StoreError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .values()
                .filter(|record| {
                    record.key.agent_profile_id == scope.agent_profile_id
                        && record.key.runtime_id == scope.runtime_id
                        && record.key.runtime_incarnation_id == scope.runtime_incarnation_id
                        && record.key.module_id == scope.module_id
                        && record.key.module_version == scope.module_version
                        && record.expires_at.as_str() > now
                })
                .cloned()
                .collect())
        }
    }

    fn descriptor(kind: Option<AgentAuthMethodKind>) -> AgentControlDescriptor {
        let mut probe = OpenCodeProfileProbe::initial();
        probe.protocol_version = Some("2.0.32".to_owned());
        probe.probe_readiness = OpenCodeProbeReadiness::Complete;
        probe.host_process_stopped = true;
        let mut descriptor = opencode::build_descriptor(
            &probe,
            "runtime",
            "incarnation",
            time::OffsetDateTime::from_unix_timestamp(1_800_000_000).expect("time"),
            900,
        )
        .expect("descriptor builds");
        if let Some(kind) = kind {
            descriptor.auth.methods.push(AgentAuthMethodDescriptor {
                auth_method_id: "native-login".to_owned(),
                display_name: "Sign in".to_owned(),
                kind,
                credential_slot_id: None,
                requires_restart: false,
            });
        }
        descriptor
    }

    fn service(result: Result<(), LaunchError>) -> (AgentAuthService, Arc<CountingLauncher>) {
        let launcher = Arc::new(CountingLauncher(AtomicUsize::new(0), result));
        (AgentAuthService::new(launcher.clone()), launcher)
    }

    fn key(profile: &str) -> AgentAuthObservationKey {
        AgentAuthObservationKey {
            agent_profile_id: profile.to_owned(),
            runtime_id: "runtime".to_owned(),
            runtime_incarnation_id: "incarnation".to_owned(),
            module_id: "opencode".to_owned(),
            module_version: crate::agents::modules::opencode::OPENCODE_MODULE_VERSION.to_owned(),
            auth_method_id: "native-login".to_owned(),
        }
    }

    #[test]
    fn agent_auth_undeclared_method_is_refused_without_a_launch_or_reservation() {
        let (service, launcher) = service(Ok(()));
        let store = MemoryAuthStore::default();
        let descriptor = descriptor(None);
        assert_eq!(
            service.begin(&store, "profile-a", &descriptor, "native-login", 100),
            Err(BeginError::MethodNotDeclared)
        );
        assert_eq!(launcher.0.load(Ordering::SeqCst), 0);
        assert!(store.rows.lock().unwrap().is_empty());
    }

    #[test]
    fn agent_auth_declared_native_flow_persists_and_returns_exact_in_flight_generation() {
        let (service, launcher) = service(Ok(()));
        let store = MemoryAuthStore::default();
        let descriptor = descriptor(Some(AgentAuthMethodKind::NativeFlow));
        assert_eq!(
            service.begin(&store, "profile-a", &descriptor, "native-login", 100),
            Ok(BeginOutcome::Started {
                generation: 1,
                expires_at: 400
            })
        );
        assert_eq!(
            service.begin(&store, "profile-a", &descriptor, "native-login", 150),
            Ok(BeginOutcome::InFlight {
                generation: 1,
                expires_at: 400
            })
        );
        assert_eq!(launcher.0.load(Ordering::SeqCst), 1);
        assert_eq!(
            service.state(&store, key("profile-a"), 150),
            Ok(AgentAuthState::Authenticating)
        );
    }

    #[test]
    fn agent_auth_expiry_and_new_runtime_scope_return_unknown() {
        let (service, _) = service(Ok(()));
        let store = MemoryAuthStore::default();
        let descriptor = descriptor(Some(AgentAuthMethodKind::NativeFlow));
        service
            .begin(&store, "profile-a", &descriptor, "native-login", 100)
            .expect("begin");
        let mut other_incarnation = key("profile-a");
        other_incarnation.runtime_incarnation_id = "next-incarnation".to_owned();
        assert_eq!(
            service.state(&store, other_incarnation, 150),
            Ok(AgentAuthState::Unknown)
        );
        assert_eq!(
            service.state(&store, key("profile-a"), 400),
            Ok(AgentAuthState::Unknown)
        );
    }

    #[test]
    fn agent_auth_cancel_and_failure_record_closed_states() {
        let (auth_service, _) = service(Ok(()));
        let store = MemoryAuthStore::default();
        let descriptor = descriptor(Some(AgentAuthMethodKind::NativeFlow));
        auth_service
            .begin(&store, "profile-a", &descriptor, "native-login", 100)
            .expect("begin");
        assert!(
            auth_service
                .cancel(&store, &key("profile-a"), 1, 120)
                .expect("cancel settles current generation")
        );
        assert_eq!(
            auth_service.state(&store, key("profile-a"), 121),
            Ok(AgentAuthState::NeedsAuth)
        );
        let (failing, _) = service(Err(LaunchError::Failed));
        assert_eq!(
            failing.begin(&store, "profile-b", &descriptor, "native-login", 100),
            Err(BeginError::LaunchFailed)
        );
        assert_eq!(
            failing.state(&store, key("profile-b"), 101),
            Ok(AgentAuthState::Error)
        );
    }

    #[test]
    fn agent_auth_unqualified_launcher_refuses_without_mutating_the_store() {
        let service = AgentAuthService::new(Arc::new(UnqualifiedLauncher));
        let store = MemoryAuthStore::default();
        let descriptor = descriptor(Some(AgentAuthMethodKind::NativeFlow));
        assert_eq!(
            service.begin(&store, "profile-a", &descriptor, "native-login", 100),
            Err(BeginError::Unsupported)
        );
        assert!(store.rows.lock().unwrap().is_empty());
    }

    #[test]
    fn agent_auth_non_native_kinds_never_launch() {
        let (service, launcher) = service(Ok(()));
        let store = MemoryAuthStore::default();
        assert_eq!(
            service.begin(
                &store,
                "profile-a",
                &descriptor(Some(AgentAuthMethodKind::SecretSlot)),
                "native-login",
                1
            ),
            Ok(BeginOutcome::SecretRequired)
        );
        assert_eq!(
            service.begin(
                &store,
                "profile-a",
                &descriptor(Some(AgentAuthMethodKind::DeviceCode)),
                "native-login",
                1
            ),
            Err(BeginError::Unsupported)
        );
        assert_eq!(launcher.0.load(Ordering::SeqCst), 0);
        assert!(store.rows.lock().unwrap().is_empty());
    }

    #[test]
    fn auth_overlay_uses_only_the_exact_profile_scope_and_reseals() {
        let (service, _) = service(Ok(()));
        let store = MemoryAuthStore::default();
        let descriptor = descriptor(Some(AgentAuthMethodKind::NativeFlow));
        let started = service
            .begin(&store, "profile-a", &descriptor, "native-login", 100)
            .expect("begin");
        let BeginOutcome::Started { generation, .. } = started else {
            panic!("first begin starts");
        };
        assert!(
            service
                .record_outcome(
                    &store,
                    &key("profile-a"),
                    generation,
                    AgentAuthState::Authenticated,
                    120
                )
                .expect("settle current generation")
        );
        let mut other_profile = descriptor.clone();
        service
            .overlay(&store, &mut other_profile, "profile-b", 121)
            .expect("other profile has no matching observation");
        assert_eq!(other_profile.auth.state, AgentAuthState::Unknown);
        let mut exact_profile = descriptor;
        let before = exact_profile.descriptor_digest.clone();
        service
            .overlay(&store, &mut exact_profile, "profile-a", 121)
            .expect("overlay exact profile observation");
        assert_eq!(exact_profile.auth.state, AgentAuthState::Authenticated);
        assert_ne!(exact_profile.descriptor_digest, before);
        exact_profile
            .validate()
            .expect("resealed descriptor validates");
    }
}
