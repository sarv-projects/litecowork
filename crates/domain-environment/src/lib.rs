//! Pure Environment identity validation and lifecycle decisions.
//!
//! Canonical persistence types and the transactional `EnvironmentStore` port live in
//! `storage-core`. This crate does not call an EnvironmentProvider, authorize a Task,
//! create a lease, dispatch an agent, or persist state. Store adapters must still
//! enforce the supplied version/status compare-and-set and immutable-config checks.

use std::fmt;
use storage_core::{
    BudgetEnforcement, BudgetEnforcementPolicy, EnvironmentConfig, EnvironmentHealth,
    EnvironmentIdentity, EnvironmentLifetime, EnvironmentRecord, EnvironmentSharingScope,
    EnvironmentStatus, ExpectedState, LifecycleHolds, PinnedSourceResource, RuntimeIncarnationId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReadinessProof {
    pub environment_id: storage_core::EnvironmentId,
    pub runtime_id: storage_core::RuntimeId,
    pub runtime_incarnation_id: RuntimeIncarnationId,
    pub provider_kind: String,
    pub source_resources: Vec<PinnedSourceResource>,
    pub isolation: storage_core::IsolationSpec,
    pub health: EnvironmentHealth,
    pub budget_enforcement: BudgetEnforcement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderProvisionResult {
    Ready(ProviderReadinessProof),
    Failed,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderSuspendResult {
    Suspended,
    Failed,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderDestroyResult {
    ConfirmedAbsent,
    Failed,
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleDecision {
    Applied(EnvironmentRecord),
    /// The state is deliberately unchanged. Reconcile provider state by stable request
    /// identity before trying again; a timeout is not proof of success or cleanup.
    ReconciliationRequired(EnvironmentRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentError {
    EmptyProviderKind,
    InvalidOwner,
    UserSharedUnavailable,
    InvalidSourcePin,
    InvalidNetworkPolicy,
    InvalidBudgetCurrency,
    CostBudgetMissingCurrency,
    BudgetEnforcementUnavailable,
    StaleState {
        expected: ExpectedState,
        actual: ExpectedState,
    },
    InvalidTransition {
        from: EnvironmentStatus,
        operation: &'static str,
    },
    LifecycleHeld(LifecycleHolds),
    InvalidSharingScopeChange,
    SharingScopeChangeHeld(LifecycleHolds),
    RetentionDisallowsDestroy,
    RequiredOutputsUncommitted,
    InvalidProviderProof,
    ImmutableConfigurationChanged,
    InvalidPersistedRecord,
    VersionExhausted,
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for EnvironmentError {}

/// Constructs a new canonical record only after validating its ownership/configuration.
pub fn new_environment(
    identity: EnvironmentIdentity,
    config: EnvironmentConfig,
    created_at: impl Into<String>,
    updated_at: impl Into<String>,
) -> Result<EnvironmentRecord, EnvironmentError> {
    validate_configuration(&identity, &config)?;
    let created_at = created_at.into();
    let updated_at = updated_at.into();
    if created_at.trim().is_empty() || updated_at.trim().is_empty() {
        return Err(EnvironmentError::InvalidPersistedRecord);
    }
    EnvironmentRecord::from_persisted_parts(
        identity,
        config,
        EnvironmentStatus::New,
        EnvironmentHealth::Unknown,
        created_at,
        updated_at,
        1,
    )
    .map_err(|_| EnvironmentError::InvalidPersistedRecord)
}

/// Rehydrates persisted canonical fields and revalidates domain invariants. Storage
/// adapters must use this rather than deserializing an EnvironmentRecord unchecked.
pub fn restore_environment(
    identity: EnvironmentIdentity,
    config: EnvironmentConfig,
    status: EnvironmentStatus,
    health: EnvironmentHealth,
    created_at: impl Into<String>,
    updated_at: impl Into<String>,
    version: u64,
) -> Result<EnvironmentRecord, EnvironmentError> {
    validate_configuration(&identity, &config)?;
    let created_at = created_at.into();
    let updated_at = updated_at.into();
    if version == 0 || created_at.trim().is_empty() || updated_at.trim().is_empty() {
        return Err(EnvironmentError::InvalidPersistedRecord);
    }
    EnvironmentRecord::from_persisted_parts(
        identity, config, status, health, created_at, updated_at, version,
    )
    .map_err(|_| EnvironmentError::InvalidPersistedRecord)
}

/// Method-oriented pure lifecycle decisions over the canonical storage record.
pub trait EnvironmentLifecycle {
    fn expected_state(&self) -> ExpectedState;
    fn request_provision(
        &self,
        expected: ExpectedState,
    ) -> Result<LifecycleDecision, EnvironmentError>;
    fn apply_provision_result(
        &self,
        expected: ExpectedState,
        expected_runtime_incarnation_id: &RuntimeIncarnationId,
        result: ProviderProvisionResult,
    ) -> Result<LifecycleDecision, EnvironmentError>;
    fn request_suspend(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
    ) -> Result<LifecycleDecision, EnvironmentError>;
    fn apply_suspend_result(
        &self,
        expected: ExpectedState,
        result: ProviderSuspendResult,
    ) -> Result<LifecycleDecision, EnvironmentError>;
    fn request_resume(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
    ) -> Result<LifecycleDecision, EnvironmentError>;
    fn request_destroy(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
        retention_allows_destroy: bool,
        required_outputs_committed: bool,
    ) -> Result<LifecycleDecision, EnvironmentError>;
    fn apply_destroy_result(
        &self,
        expected: ExpectedState,
        result: ProviderDestroyResult,
    ) -> Result<LifecycleDecision, EnvironmentError>;
}

/// Storage adapters call this while holding the Environment aggregate write lock.
/// It ensures proposed state came from a current CAS and cannot rewrite the pinned
/// provider, owner, source, isolation, or budget configuration.
pub fn validate_transition_proposal(
    current: &EnvironmentRecord,
    expected: ExpectedState,
    proposed: &EnvironmentRecord,
) -> Result<(), EnvironmentError> {
    let actual = ExpectedState {
        version: current.version(),
        status: current.status(),
    };
    if actual != expected {
        return Err(EnvironmentError::StaleState { expected, actual });
    }
    if current.identity() != proposed.identity() || current.config() != proposed.config() {
        return Err(EnvironmentError::ImmutableConfigurationChanged);
    }
    if proposed.version()
        != expected
            .version
            .checked_add(1)
            .ok_or(EnvironmentError::VersionExhausted)?
    {
        return Err(EnvironmentError::InvalidPersistedRecord);
    }
    if !allowed_transition(expected.status, proposed.status()) {
        return Err(EnvironmentError::InvalidTransition {
            from: expected.status,
            operation: "persist transition proposal",
        });
    }
    Ok(())
}

/// Build the canonical state for F65. Storage must independently recheck the CAS,
/// Workspace/Coworker ownership, and live-use holds in the same write transaction.
pub fn change_sharing_scope(
    current: &EnvironmentRecord,
    expected: ExpectedState,
    target_scope: EnvironmentSharingScope,
    target_coworker_id: Option<storage_core::CoworkerId>,
    holds: LifecycleHolds,
    updated_at: impl Into<String>,
) -> Result<EnvironmentRecord, EnvironmentError> {
    let actual = current.expected_state();
    if actual != expected {
        return Err(EnvironmentError::StaleState { expected, actual });
    }
    if current.config().lifetime != EnvironmentLifetime::WorkspacePersistent
        || current.status() != EnvironmentStatus::Suspended
        || current.config().sharing_scope == EnvironmentSharingScope::UserShared
        || !matches!(
            current.config().sharing_scope,
            EnvironmentSharingScope::CoworkerPrivate | EnvironmentSharingScope::WorkspaceShared
        )
        || !matches!(
            target_scope,
            EnvironmentSharingScope::CoworkerPrivate | EnvironmentSharingScope::WorkspaceShared
        )
    {
        return Err(EnvironmentError::InvalidSharingScopeChange);
    }
    if holds != LifecycleHolds::default() {
        return Err(EnvironmentError::SharingScopeChangeHeld(holds));
    }
    let owner_coworker_id = match (target_scope, target_coworker_id) {
        (EnvironmentSharingScope::CoworkerPrivate, Some(id)) => Some(id),
        (EnvironmentSharingScope::WorkspaceShared, None) => None,
        _ => return Err(EnvironmentError::InvalidSharingScopeChange),
    };
    if target_scope == current.config().sharing_scope {
        return Err(EnvironmentError::InvalidSharingScopeChange);
    }

    let updated_at = updated_at.into();
    if updated_at.trim().is_empty() {
        return Err(EnvironmentError::InvalidPersistedRecord);
    }
    let version = current
        .version()
        .checked_add(1)
        .ok_or(EnvironmentError::VersionExhausted)?;
    let mut identity = current.identity().clone();
    identity.owner.coworker_id = owner_coworker_id;
    let mut config = current.config().clone();
    config.sharing_scope = target_scope;
    validate_configuration(&identity, &config)?;
    EnvironmentRecord::from_persisted_parts(
        identity,
        config,
        current.status(),
        current.health(),
        current.created_at().to_owned(),
        updated_at,
        version,
    )
    .map_err(|_| EnvironmentError::InvalidPersistedRecord)
}

fn allowed_transition(from: EnvironmentStatus, to: EnvironmentStatus) -> bool {
    use EnvironmentStatus::*;
    matches!(
        (from, to),
        (New, Provisioning)
            | (Provisioning, Ready | Failed)
            | (Ready, Busy | Checkpointing | Destroying)
            | (Busy, Ready | Checkpointing)
            | (Checkpointing, Ready | Suspended | Failed)
            | (Suspended, Provisioning | Destroying)
            | (Failed, Destroying)
            | (Destroying, Destroyed | Failed)
    )
}

impl EnvironmentLifecycle for EnvironmentRecord {
    fn expected_state(&self) -> ExpectedState {
        Lifecycle(self).expected_state()
    }
    fn request_provision(
        &self,
        expected: ExpectedState,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).request_provision(expected)
    }
    fn apply_provision_result(
        &self,
        expected: ExpectedState,
        expected_runtime_incarnation_id: &RuntimeIncarnationId,
        result: ProviderProvisionResult,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).apply_provision_result(expected, expected_runtime_incarnation_id, result)
    }
    fn request_suspend(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).request_suspend(expected, holds)
    }
    fn apply_suspend_result(
        &self,
        expected: ExpectedState,
        result: ProviderSuspendResult,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).apply_suspend_result(expected, result)
    }
    fn request_resume(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).request_resume(expected, holds)
    }
    fn request_destroy(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
        retention_allows_destroy: bool,
        required_outputs_committed: bool,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).request_destroy(
            expected,
            holds,
            retention_allows_destroy,
            required_outputs_committed,
        )
    }
    fn apply_destroy_result(
        &self,
        expected: ExpectedState,
        result: ProviderDestroyResult,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        Lifecycle(self).apply_destroy_result(expected, result)
    }
}

struct Lifecycle<'a>(&'a EnvironmentRecord);

impl std::ops::Deref for Lifecycle<'_> {
    type Target = EnvironmentRecord;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl Lifecycle<'_> {
    pub fn expected_state(&self) -> ExpectedState {
        ExpectedState {
            version: self.version(),
            status: self.status(),
        }
    }

    pub fn request_provision(
        &self,
        expected: ExpectedState,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        self.transition(
            EnvironmentStatus::New,
            EnvironmentStatus::Provisioning,
            "provision",
        )
    }

    pub fn apply_provision_result(
        &self,
        expected: ExpectedState,
        expected_runtime_incarnation_id: &RuntimeIncarnationId,
        result: ProviderProvisionResult,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        if self.status() != EnvironmentStatus::Provisioning {
            return Err(self.invalid_transition("apply provision result"));
        }
        match result {
            ProviderProvisionResult::Ready(proof) => {
                if !self.proof_matches(&proof, expected_runtime_incarnation_id) {
                    // A malformed/mismatched readiness report does not prove the
                    // provider failed to create the Environment. Keep admission
                    // pending until the provider state and locator are reconciled.
                    return Ok(LifecycleDecision::ReconciliationRequired(self.0.clone()));
                }
                self.transition_with_health(
                    EnvironmentStatus::Provisioning,
                    EnvironmentStatus::Ready,
                    proof.health,
                    "provider ready",
                )
            }
            ProviderProvisionResult::Failed => self.transition(
                EnvironmentStatus::Provisioning,
                EnvironmentStatus::Failed,
                "provider provisioning failed",
            ),
            ProviderProvisionResult::Ambiguous => {
                Ok(LifecycleDecision::ReconciliationRequired(self.0.clone()))
            }
        }
    }

    /// Caller must have closed new-use admission and supply a fresh authoritative hold
    /// snapshot. Holds cause denial; this decision does not wait for them to settle.
    pub fn request_suspend(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        if !matches!(
            self.status(),
            EnvironmentStatus::Ready | EnvironmentStatus::Busy
        ) {
            return Err(self.invalid_transition("suspend"));
        }
        ensure_no_holds(holds)?;
        self.transition(self.status(), EnvironmentStatus::Checkpointing, "suspend")
    }

    pub fn apply_suspend_result(
        &self,
        expected: ExpectedState,
        result: ProviderSuspendResult,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        if self.status() != EnvironmentStatus::Checkpointing {
            return Err(self.invalid_transition("apply suspend result"));
        }
        match result {
            ProviderSuspendResult::Suspended => self.transition(
                EnvironmentStatus::Checkpointing,
                EnvironmentStatus::Suspended,
                "provider confirmed suspension",
            ),
            ProviderSuspendResult::Failed => self.transition(
                EnvironmentStatus::Checkpointing,
                EnvironmentStatus::Failed,
                "provider suspension failed",
            ),
            ProviderSuspendResult::Ambiguous => {
                Ok(LifecycleDecision::ReconciliationRequired(self.0.clone()))
            }
        }
    }

    /// Resume reattaches/revalidates only. It never restores prior Task grants,
    /// Secrets, control leases, or ExecutionLeases.
    pub fn request_resume(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        if self.status() != EnvironmentStatus::Suspended {
            return Err(self.invalid_transition("resume"));
        }
        ensure_no_holds(holds)?;
        self.transition(
            EnvironmentStatus::Suspended,
            EnvironmentStatus::Provisioning,
            "resume",
        )
    }

    pub fn request_destroy(
        &self,
        expected: ExpectedState,
        holds: LifecycleHolds,
        retention_allows_destroy: bool,
        required_outputs_committed: bool,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        if !matches!(
            self.status(),
            EnvironmentStatus::Ready | EnvironmentStatus::Suspended | EnvironmentStatus::Failed
        ) {
            return Err(self.invalid_transition("destroy"));
        }
        ensure_no_holds(holds)?;
        if !retention_allows_destroy {
            return Err(EnvironmentError::RetentionDisallowsDestroy);
        }
        if !required_outputs_committed {
            return Err(EnvironmentError::RequiredOutputsUncommitted);
        }
        self.transition(self.status(), EnvironmentStatus::Destroying, "destroy")
    }

    pub fn apply_destroy_result(
        &self,
        expected: ExpectedState,
        result: ProviderDestroyResult,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        self.check_expected(expected)?;
        if self.status() != EnvironmentStatus::Destroying {
            return Err(self.invalid_transition("apply destroy result"));
        }
        match result {
            ProviderDestroyResult::ConfirmedAbsent => self.transition(
                EnvironmentStatus::Destroying,
                EnvironmentStatus::Destroyed,
                "provider confirmed absence",
            ),
            ProviderDestroyResult::Failed => self.transition(
                EnvironmentStatus::Destroying,
                EnvironmentStatus::Failed,
                "provider destruction failed",
            ),
            // Timeout/lost response/unknown state is not proof that cleanup completed.
            ProviderDestroyResult::Ambiguous => {
                Ok(LifecycleDecision::ReconciliationRequired(self.0.clone()))
            }
        }
    }

    fn proof_matches(
        &self,
        proof: &ProviderReadinessProof,
        expected_runtime_incarnation_id: &RuntimeIncarnationId,
    ) -> bool {
        proof.environment_id == self.identity().environment_id
            && proof.runtime_id == self.identity().runtime_id
            && &proof.runtime_incarnation_id == expected_runtime_incarnation_id
            && proof.provider_kind == self.config().provider_kind
            && proof.source_resources == self.config().source_resources
            && proof.isolation == self.config().isolation
            && proof.health == EnvironmentHealth::Healthy
            && proof.budget_enforcement == self.config().budget_enforcement
            && match self.config().budget_enforcement_policy {
                BudgetEnforcementPolicy::RequireProviderEnforced => {
                    proof.budget_enforcement == BudgetEnforcement::ProviderEnforced
                }
                BudgetEnforcementPolicy::AllowHostMonitored => matches!(
                    proof.budget_enforcement,
                    BudgetEnforcement::ProviderEnforced | BudgetEnforcement::HostMonitored
                ),
            }
    }

    fn check_expected(&self, expected: ExpectedState) -> Result<(), EnvironmentError> {
        let actual = self.expected_state();
        if actual != expected {
            return Err(EnvironmentError::StaleState { expected, actual });
        }
        Ok(())
    }

    fn transition(
        &self,
        from: EnvironmentStatus,
        to: EnvironmentStatus,
        operation: &'static str,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        let health = if to == EnvironmentStatus::Failed {
            EnvironmentHealth::Unknown
        } else {
            self.health()
        };
        self.transition_with_health(from, to, health, operation)
    }

    fn transition_with_health(
        &self,
        from: EnvironmentStatus,
        to: EnvironmentStatus,
        health: EnvironmentHealth,
        operation: &'static str,
    ) -> Result<LifecycleDecision, EnvironmentError> {
        if self.status() != from {
            return Err(self.invalid_transition(operation));
        }
        let version = self
            .version()
            .checked_add(1)
            .ok_or(EnvironmentError::VersionExhausted)?;
        let record = self
            .0
            .with_domain_state(to, health, version)
            .map_err(|_| EnvironmentError::VersionExhausted)?;
        Ok(LifecycleDecision::Applied(record))
    }

    fn invalid_transition(&self, operation: &'static str) -> EnvironmentError {
        EnvironmentError::InvalidTransition {
            from: self.status(),
            operation,
        }
    }
}

fn ensure_no_holds(holds: LifecycleHolds) -> Result<(), EnvironmentError> {
    if holds.active_attempts > 0
        || holds.unsettled_invocations > 0
        || holds.unsettled_control_leases > 0
        || holds.checkpoint_holds > 0
        || holds.unresolved_effects > 0
    {
        Err(EnvironmentError::LifecycleHeld(holds))
    } else {
        Ok(())
    }
}

pub fn validate_configuration(
    identity: &EnvironmentIdentity,
    config: &EnvironmentConfig,
) -> Result<(), EnvironmentError> {
    if config.provider_kind.trim().is_empty() {
        return Err(EnvironmentError::EmptyProviderKind);
    }
    if config.name.trim().is_empty() {
        return Err(EnvironmentError::InvalidPersistedRecord);
    }
    if config.sharing_scope == EnvironmentSharingScope::UserShared {
        // DATA-MODEL has no durable principal owner/attachment; ENVIRONMENTS defers it in v1.
        return Err(EnvironmentError::UserSharedUnavailable);
    }
    match (config.lifetime, config.provision_preview_digest.as_deref()) {
        (EnvironmentLifetime::WorkspacePersistent, Some(digest)) if valid_sha256(digest) => {}
        (EnvironmentLifetime::WorkspacePersistent, _) => {
            return Err(EnvironmentError::InvalidPersistedRecord);
        }
        (_, None) => {}
        (_, Some(_)) => return Err(EnvironmentError::InvalidPersistedRecord),
    }
    if !config.network_policy.deny_private_networks {
        return Err(EnvironmentError::InvalidNetworkPolicy);
    }
    if config
        .source_resources
        .iter()
        .any(|source| !valid_sha256(&source.content_digest))
    {
        return Err(EnvironmentError::InvalidSourcePin);
    }
    if config.budget_ceiling.max_cost_minor_units.is_some()
        && config
            .budget_ceiling
            .currency
            .as_ref()
            .is_none_or(|currency| !valid_currency(currency))
    {
        return Err(EnvironmentError::CostBudgetMissingCurrency);
    }
    if config
        .budget_ceiling
        .currency
        .as_ref()
        .is_some_and(|currency| !valid_currency(currency))
    {
        return Err(EnvironmentError::InvalidBudgetCurrency);
    }
    if config.budget_enforcement == BudgetEnforcement::Unavailable
        || (config.budget_enforcement_policy == BudgetEnforcementPolicy::RequireProviderEnforced
            && config.budget_enforcement != BudgetEnforcement::ProviderEnforced)
    {
        return Err(EnvironmentError::BudgetEnforcementUnavailable);
    }

    let owner = &identity.owner;
    let owner_fields_match = match config.sharing_scope {
        EnvironmentSharingScope::AttemptPrivate => {
            owner.task_id.is_some()
                && owner.attempt_id.is_some()
                && owner.coworker_id.is_none()
                && owner.principal_id.is_none()
        }
        EnvironmentSharingScope::TaskShared => {
            owner.task_id.is_some()
                && owner.attempt_id.is_none()
                && owner.coworker_id.is_none()
                && owner.principal_id.is_none()
        }
        EnvironmentSharingScope::CoworkerPrivate => {
            owner.task_id.is_none()
                && owner.attempt_id.is_none()
                && owner.coworker_id.is_some()
                && owner.principal_id.is_none()
        }
        EnvironmentSharingScope::WorkspaceShared => {
            owner.task_id.is_none()
                && owner.attempt_id.is_none()
                && owner.coworker_id.is_none()
                && owner.principal_id.is_none()
        }
        EnvironmentSharingScope::UserShared => false,
    };
    if !owner_fields_match {
        return Err(EnvironmentError::InvalidOwner);
    }
    match config.lifetime {
        EnvironmentLifetime::Attempt if owner.task_id.is_none() || owner.attempt_id.is_none() => {
            return Err(EnvironmentError::InvalidOwner);
        }
        EnvironmentLifetime::TaskRetained
            if owner.task_id.is_none() || owner.attempt_id.is_some() =>
        {
            return Err(EnvironmentError::InvalidOwner);
        }
        EnvironmentLifetime::WorkspacePersistent
            if owner.task_id.is_some() || owner.attempt_id.is_some() =>
        {
            return Err(EnvironmentError::InvalidOwner);
        }
        _ => {}
    }
    if config.sharing_scope == EnvironmentSharingScope::CoworkerPrivate
        && config.lifetime != EnvironmentLifetime::WorkspacePersistent
    {
        return Err(EnvironmentError::InvalidOwner);
    }
    if config.isolation.write_scope.workspace_id != identity.owner_workspace_id {
        return Err(EnvironmentError::InvalidOwner);
    }
    Ok(())
}

fn valid_currency(currency: &str) -> bool {
    currency.len() == 3 && currency.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_core::{
        AttemptId, BudgetCeiling, CoworkerId, EnvironmentClass, EnvironmentCreateRequest,
        EnvironmentId, EnvironmentLifecycleRequest, EnvironmentListRequest, EnvironmentOwner,
        EnvironmentRequestIdentity, EventDraft, FilesystemIsolation, IsolationSpec, NetworkMode,
        NetworkPolicy, PrincipalId, ProcessIsolation, ProviderBindingCommit, ResourceId,
        ResourceLimits, ResourceRevisionId, ResourceScope, RuntimeId, TaskId, WorkspaceId,
    };

    fn id<T>(
        value: &str,
        constructor: impl FnOnce(String) -> Result<T, storage_core::StoreError>,
    ) -> T {
        constructor(value.to_owned()).unwrap()
    }

    fn identity() -> EnvironmentIdentity {
        EnvironmentIdentity {
            environment_id: id("env-1", EnvironmentId::new),
            runtime_id: id("runtime-1", RuntimeId::new),
            owner_workspace_id: id("workspace-1", WorkspaceId::new),
            created_by_incarnation_id: None,
            owner: EnvironmentOwner {
                task_id: None,
                attempt_id: None,
                coworker_id: None,
                principal_id: None,
            },
        }
    }

    fn config() -> EnvironmentConfig {
        EnvironmentConfig {
            name: "Workspace Environment".to_owned(),
            provider_kind: "local-workspace".to_owned(),
            class: EnvironmentClass::LocalWorkspace,
            lifetime: EnvironmentLifetime::WorkspacePersistent,
            sharing_scope: EnvironmentSharingScope::WorkspaceShared,
            source_resources: vec![PinnedSourceResource {
                resource_id: id("resource-1", ResourceId::new),
                revision_id: id("revision-1", ResourceRevisionId::new),
                content_digest: format!("sha256:{}", "a".repeat(64)),
            }],
            resource_limits: ResourceLimits {
                cpu_millis: 1000,
                memory_bytes: 1024,
                storage_bytes: 4096,
                max_processes: Some(4),
                max_lifetime_seconds: Some(3600),
            },
            network_policy: NetworkPolicy {
                mode: NetworkMode::None,
                allowed_domains: vec![],
                max_response_bytes: 0,
                max_download_bytes: 0,
                deny_private_networks: true,
            },
            budget_ceiling: BudgetCeiling {
                max_wall_time_ms: Some(60000),
                max_cost_minor_units: None,
                currency: None,
                max_tokens: None,
                max_child_attempts: None,
                max_concurrency: Some(1),
            },
            budget_enforcement_policy: BudgetEnforcementPolicy::RequireProviderEnforced,
            budget_enforcement: BudgetEnforcement::ProviderEnforced,
            provision_preview_digest: Some(format!("sha256:{}", "b".repeat(64))),
            retention_expires_at: None,
            backup_policy: storage_core::EnvironmentBackupPolicy::Excluded,
            isolation: IsolationSpec {
                filesystem: FilesystemIsolation::PrivateCopy,
                process: ProcessIsolation::Container,
                network: NetworkMode::None,
                write_scope: ResourceScope {
                    workspace_id: id("workspace-1", WorkspaceId::new),
                    resource_ids: vec![],
                },
            },
        }
    }

    fn record() -> EnvironmentRecord {
        new_environment(
            identity(),
            config(),
            "2026-10-09T00:00:00Z",
            "2026-10-09T00:00:00Z",
        )
        .unwrap()
    }
    fn event_draft() -> EventDraft {
        EventDraft {
            event_id: "event-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            entity_type: "Environment".to_owned(),
            entity_id: "env-1".to_owned(),
            origin_runtime_id: "runtime-1".to_owned(),
            entity_revision: 1,
            hlc_timestamp: "2026-10-09T00:00:00Z".to_owned(),
            correlation_id: "request-1".to_owned(),
            causation_id: None,
            schema_version: 1,
            event_type: "environment.created.v1".to_owned(),
            payload: serde_json::json!({"status":"PROVISIONING"}),
            recorded_at: "2026-10-09T00:00:00Z".to_owned(),
        }
    }
    fn request_identity() -> EnvironmentRequestIdentity {
        EnvironmentRequestIdentity::new(
            id("principal-1", PrincipalId::new),
            "request-1",
            br#"{"environment_id":"env-1"}"#.to_vec(),
            format!("sha256:{}", "c".repeat(64)),
        )
        .unwrap()
    }
    fn applied(decision: Result<LifecycleDecision, EnvironmentError>) -> EnvironmentRecord {
        match decision.unwrap() {
            LifecycleDecision::Applied(record) => record,
            LifecycleDecision::ReconciliationRequired(_) => panic!("unexpected reconciliation"),
        }
    }
    fn provisioning() -> EnvironmentRecord {
        let record = record();
        applied(record.request_provision(record.expected_state()))
    }
    fn proof(record: &EnvironmentRecord) -> ProviderReadinessProof {
        ProviderReadinessProof {
            environment_id: record.identity().environment_id.clone(),
            runtime_id: record.identity().runtime_id.clone(),
            runtime_incarnation_id: incarnation(),
            provider_kind: record.config().provider_kind.clone(),
            source_resources: record.config().source_resources.clone(),
            isolation: record.config().isolation.clone(),
            health: EnvironmentHealth::Healthy,
            budget_enforcement: BudgetEnforcement::ProviderEnforced,
        }
    }
    fn incarnation() -> RuntimeIncarnationId {
        id("incarnation-1", RuntimeIncarnationId::new)
    }
    fn ready() -> EnvironmentRecord {
        let pending = provisioning();
        applied(pending.apply_provision_result(
            pending.expected_state(),
            &incarnation(),
            ProviderProvisionResult::Ready(proof(&pending)),
        ))
    }
    fn suspended() -> EnvironmentRecord {
        let ready = ready();
        let checkpointing =
            applied(ready.request_suspend(ready.expected_state(), LifecycleHolds::default()));
        applied(checkpointing.apply_suspend_result(
            checkpointing.expected_state(),
            ProviderSuspendResult::Suspended,
        ))
    }
    fn destroying() -> EnvironmentRecord {
        let ready = ready();
        applied(ready.request_destroy(
            ready.expected_state(),
            LifecycleHolds::default(),
            true,
            true,
        ))
    }

    #[test]
    fn configuration_enforces_scope_owner_lifetime_and_workspace() {
        let mut invalid = identity();
        let mut cfg = config();
        cfg.sharing_scope = EnvironmentSharingScope::TaskShared;
        cfg.lifetime = EnvironmentLifetime::TaskRetained;
        cfg.provision_preview_digest = None;
        assert_eq!(
            validate_configuration(&invalid, &cfg),
            Err(EnvironmentError::InvalidOwner)
        );
        invalid.owner.task_id = Some(id("task-1", TaskId::new));
        assert_eq!(validate_configuration(&invalid, &cfg), Ok(()));
        invalid.owner.attempt_id = Some(id("attempt-1", AttemptId::new));
        assert_eq!(
            validate_configuration(&invalid, &cfg),
            Err(EnvironmentError::InvalidOwner)
        );
        cfg.sharing_scope = EnvironmentSharingScope::AttemptPrivate;
        cfg.lifetime = EnvironmentLifetime::Attempt;
        assert_eq!(validate_configuration(&invalid, &cfg), Ok(()));
        cfg.isolation.write_scope.workspace_id = id("other", WorkspaceId::new);
        assert_eq!(
            validate_configuration(&invalid, &cfg),
            Err(EnvironmentError::InvalidOwner)
        );
    }

    #[test]
    fn coworker_and_user_scope_rules_are_explicit() {
        let mut owner = identity();
        owner.owner.coworker_id = Some(id("coworker-1", CoworkerId::new));
        let mut cfg = config();
        cfg.sharing_scope = EnvironmentSharingScope::CoworkerPrivate;
        cfg.provision_preview_digest = Some(format!("sha256:{}", "b".repeat(64)));
        assert_eq!(validate_configuration(&owner, &cfg), Ok(()));
        cfg.lifetime = EnvironmentLifetime::TaskRetained;
        cfg.provision_preview_digest = None;
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::InvalidOwner)
        );
        cfg = config();
        cfg.sharing_scope = EnvironmentSharingScope::UserShared;
        owner.owner = EnvironmentOwner {
            task_id: None,
            attempt_id: None,
            coworker_id: None,
            principal_id: Some(id("principal", PrincipalId::new)),
        };
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::UserSharedUnavailable)
        );
    }

    #[test]
    fn sharing_scope_change_requires_suspended_persistent_environment_without_holds() {
        let mut owner = identity();
        owner.owner.task_id = None;
        owner.owner.coworker_id = Some(id("coworker-1", CoworkerId::new));
        let mut cfg = config();
        cfg.lifetime = EnvironmentLifetime::WorkspacePersistent;
        cfg.sharing_scope = EnvironmentSharingScope::CoworkerPrivate;
        cfg.provision_preview_digest = Some(format!("sha256:{}", "b".repeat(64)));
        let suspended = restore_environment(
            owner,
            cfg,
            EnvironmentStatus::Suspended,
            EnvironmentHealth::Healthy,
            "2026-10-09T00:00:00Z",
            "2026-10-09T00:00:00Z",
            4,
        )
        .unwrap();

        let changed = change_sharing_scope(
            &suspended,
            ExpectedState {
                version: 4,
                status: EnvironmentStatus::Suspended,
            },
            EnvironmentSharingScope::WorkspaceShared,
            None,
            LifecycleHolds::default(),
            "2026-10-09T00:01:00Z",
        )
        .unwrap();

        assert_eq!(changed.version(), 5);
        assert_eq!(changed.status(), EnvironmentStatus::Suspended);
        assert_eq!(
            changed.config().sharing_scope,
            EnvironmentSharingScope::WorkspaceShared
        );
        assert_eq!(changed.identity().owner.coworker_id, None);
        assert_eq!(changed.updated_at(), "2026-10-09T00:01:00Z");

        let coworker = change_sharing_scope(
            &changed,
            changed.expected_state(),
            EnvironmentSharingScope::CoworkerPrivate,
            Some(id("coworker-2", CoworkerId::new)),
            LifecycleHolds::default(),
            "2026-10-09T00:02:00Z",
        )
        .unwrap();
        assert_eq!(
            coworker.config().sharing_scope,
            EnvironmentSharingScope::CoworkerPrivate
        );
        assert_eq!(
            coworker
                .identity()
                .owner
                .coworker_id
                .as_ref()
                .unwrap()
                .as_str(),
            "coworker-2"
        );
    }

    #[test]
    fn sharing_scope_change_rejects_active_status_invalid_target_and_each_hold() {
        let suspended = persistent_coworker_environment(EnvironmentStatus::Suspended);
        let expected = suspended.expected_state();
        let held = [
            LifecycleHolds {
                active_attempts: 1,
                ..LifecycleHolds::default()
            },
            LifecycleHolds {
                unsettled_invocations: 1,
                ..LifecycleHolds::default()
            },
            LifecycleHolds {
                unsettled_control_leases: 1,
                ..LifecycleHolds::default()
            },
            LifecycleHolds {
                checkpoint_holds: 1,
                ..LifecycleHolds::default()
            },
            LifecycleHolds {
                unresolved_effects: 1,
                ..LifecycleHolds::default()
            },
        ];
        for holds in held {
            assert!(
                change_sharing_scope(
                    &suspended,
                    expected,
                    EnvironmentSharingScope::WorkspaceShared,
                    None,
                    holds,
                    "2026-10-09T00:01:00Z",
                )
                .is_err()
            );
        }
        assert!(
            change_sharing_scope(
                &persistent_coworker_environment(EnvironmentStatus::Ready),
                expected,
                EnvironmentSharingScope::WorkspaceShared,
                None,
                LifecycleHolds::default(),
                "2026-10-09T00:01:00Z",
            )
            .is_err()
        );
        assert!(
            change_sharing_scope(
                &suspended,
                expected,
                EnvironmentSharingScope::UserShared,
                None,
                LifecycleHolds::default(),
                "2026-10-09T00:01:00Z",
            )
            .is_err()
        );
        assert!(
            change_sharing_scope(
                &suspended,
                expected,
                EnvironmentSharingScope::CoworkerPrivate,
                Some(id("coworker-2", CoworkerId::new)),
                LifecycleHolds::default(),
                "2026-10-09T00:01:00Z",
            )
            .is_err()
        );
    }

    fn persistent_coworker_environment(status: EnvironmentStatus) -> EnvironmentRecord {
        let mut owner = identity();
        owner.owner.task_id = None;
        owner.owner.coworker_id = Some(id("coworker-1", CoworkerId::new));
        let mut cfg = config();
        cfg.lifetime = EnvironmentLifetime::WorkspacePersistent;
        cfg.sharing_scope = EnvironmentSharingScope::CoworkerPrivate;
        cfg.provision_preview_digest = Some(format!("sha256:{}", "b".repeat(64)));
        restore_environment(
            owner,
            cfg,
            status,
            EnvironmentHealth::Healthy,
            "2026-10-09T00:00:00Z",
            "2026-10-09T00:00:00Z",
            4,
        )
        .unwrap()
    }

    #[test]
    fn cost_budget_requires_valid_currency_and_enforcement() {
        let owner = identity();
        let mut cfg = config();
        cfg.budget_ceiling.max_cost_minor_units = Some(1);
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::CostBudgetMissingCurrency)
        );
        cfg.budget_ceiling.currency = Some("usd".into());
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::CostBudgetMissingCurrency)
        );
        cfg.budget_ceiling.currency = Some("USD".into());
        cfg.budget_enforcement = BudgetEnforcement::HostMonitored;
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::BudgetEnforcementUnavailable)
        );
        cfg.budget_enforcement_policy = BudgetEnforcementPolicy::AllowHostMonitored;
        assert_eq!(validate_configuration(&owner, &cfg), Ok(()));
    }

    #[test]
    fn source_digest_and_private_network_denial_are_validated() {
        let owner = identity();
        let mut cfg = config();
        cfg.source_resources[0].content_digest = "sha256:bad".to_owned();
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::InvalidSourcePin)
        );
        cfg = config();
        cfg.network_policy.deny_private_networks = false;
        assert_eq!(
            validate_configuration(&owner, &cfg),
            Err(EnvironmentError::InvalidNetworkPolicy)
        );
    }

    #[test]
    fn restore_revalidates_configuration_and_positive_version() {
        let owner = identity();
        let cfg = config();
        assert!(
            restore_environment(
                owner.clone(),
                cfg.clone(),
                EnvironmentStatus::Ready,
                EnvironmentHealth::Healthy,
                "2026-10-09T00:00:00Z",
                "2026-10-09T00:00:00Z",
                0
            )
            .is_err()
        );
        let mut invalid = cfg;
        invalid.sharing_scope = EnvironmentSharingScope::UserShared;
        assert_eq!(
            restore_environment(
                owner,
                invalid,
                EnvironmentStatus::Ready,
                EnvironmentHealth::Healthy,
                "2026-10-09T00:00:00Z",
                "2026-10-09T00:00:00Z",
                1
            ),
            Err(EnvironmentError::UserSharedUnavailable)
        );
    }

    #[test]
    fn environment_creation_request_rejects_persistent_without_preview_transaction() {
        let pending = provisioning();
        let error = EnvironmentCreateRequest::new(request_identity(), pending, event_draft())
            .expect_err("persistent creation has no typed preview-consumption contract");
        assert!(error.to_string().contains("typed preview consumption"));

        let mut scoped_identity = identity();
        scoped_identity.owner.task_id = Some(id("task-1", TaskId::new));
        let mut scoped_config = config();
        scoped_config.lifetime = EnvironmentLifetime::TaskRetained;
        scoped_config.sharing_scope = EnvironmentSharingScope::TaskShared;
        scoped_config.provision_preview_digest = None;
        let scoped = new_environment(
            scoped_identity,
            scoped_config,
            "2026-10-09T00:00:00Z",
            "2026-10-09T00:00:00Z",
        )
        .unwrap();
        let scoped = applied(scoped.request_provision(scoped.expected_state()));
        let scoped = scoped.with_updated_at("2026-10-09T00:00:05Z").unwrap();
        let mut mismatched_event = event_draft();
        mismatched_event.recorded_at = "2026-10-09T00:00:06Z".to_owned();
        assert!(
            EnvironmentCreateRequest::new(request_identity(), scoped.clone(), mismatched_event,)
                .is_err()
        );
        assert!(scoped.with_updated_at(" ").is_err());

        let mut matching_event = event_draft();
        matching_event.recorded_at = scoped.updated_at().to_owned();
        let request = EnvironmentCreateRequest::new(request_identity(), scoped, matching_event)
            .expect("task-retained creation is supported by this port");
        assert_eq!(request.identity().principal_id().as_str(), "principal-1");
        assert_eq!(request.request_id(), "request-1");
        assert_eq!(
            request.identity().canonical_request_body(),
            br#"{"environment_id":"env-1"}"#
        );
    }

    #[test]
    fn canonical_record_retains_all_added_v11_environment_metadata() {
        let mut identity = identity();
        identity.created_by_incarnation_id =
            Some(id("runtime-incarnation-1", RuntimeIncarnationId::new));
        let mut config = config();
        config.name = "Pinned workspace environment".to_owned();
        config.retention_expires_at = Some("2026-10-12T00:00:00Z".to_owned());
        config.backup_policy = storage_core::EnvironmentBackupPolicy::IncludeCheckpoints;
        let record = new_environment(
            identity,
            config,
            "2026-10-09T00:00:00Z",
            "2026-10-09T00:00:01Z",
        )
        .unwrap();

        let expected_preview_digest = format!("sha256:{}", "b".repeat(64));
        assert_eq!(record.config().name, "Pinned workspace environment");
        assert_eq!(
            record.config().provision_preview_digest.as_deref(),
            Some(expected_preview_digest.as_str())
        );
        assert_eq!(
            record.config().retention_expires_at.as_deref(),
            Some("2026-10-12T00:00:00Z")
        );
        assert_eq!(
            record.config().backup_policy,
            storage_core::EnvironmentBackupPolicy::IncludeCheckpoints
        );
        assert_eq!(
            record
                .identity()
                .created_by_incarnation_id
                .as_ref()
                .unwrap()
                .as_str(),
            "runtime-incarnation-1"
        );
        assert_eq!(record.created_at(), "2026-10-09T00:00:00Z");
        assert_eq!(record.updated_at(), "2026-10-09T00:00:01Z");
        let next = applied(record.request_provision(record.expected_state()));
        assert_eq!(next.created_at(), record.created_at());
        assert_eq!(next.updated_at(), record.updated_at());
        assert_eq!(next.identity(), record.identity());
        assert_eq!(next.config(), record.config());
    }

    #[test]
    fn environment_list_request_requires_explicit_positive_limit() {
        let workspace = id("workspace-1", WorkspaceId::new);
        assert!(EnvironmentListRequest::new(workspace.clone(), None, 0).is_err());
        let request =
            EnvironmentListRequest::new(workspace, Some(id("env-5", EnvironmentId::new)), 25)
                .unwrap();
        assert_eq!(request.limit(), 25);
        assert_eq!(request.after_environment_id().unwrap().as_str(), "env-5");
    }

    #[test]
    fn ready_lifecycle_request_requires_matching_runtime_local_binding() {
        let pending = provisioning();
        let ready = applied(pending.apply_provision_result(
            pending.expected_state(),
            &incarnation(),
            ProviderProvisionResult::Ready(proof(&pending)),
        ));
        let expected = pending.expected_state();
        assert!(
            EnvironmentLifecycleRequest::new(
                request_identity(),
                ready.identity().environment_id.clone(),
                ready.identity().owner_workspace_id.clone(),
                expected,
                ready.clone(),
                None,
                event_draft(),
            )
            .is_err()
        );

        let binding = ProviderBindingCommit::new(
            ready.identity().environment_id.clone(),
            ready.identity().runtime_id.clone(),
            incarnation(),
            ready.config().provider_kind.clone(),
            "opaque:runtime-local-ref",
            "2026-10-09T00:00:05Z",
            Some("2026-10-10T00:00:00Z".to_owned()),
        )
        .unwrap();
        let stamped_ready = ready.with_updated_at("2026-10-09T00:00:06Z").unwrap();
        assert!(
            EnvironmentLifecycleRequest::new(
                request_identity(),
                stamped_ready.identity().environment_id.clone(),
                stamped_ready.identity().owner_workspace_id.clone(),
                expected,
                stamped_ready.clone(),
                Some(binding.clone()),
                event_draft(),
            )
            .is_err()
        );
        let mut matching_event = event_draft();
        matching_event.recorded_at = stamped_ready.updated_at().to_owned();
        let request = EnvironmentLifecycleRequest::new(
            request_identity(),
            stamped_ready.identity().environment_id.clone(),
            stamped_ready.identity().owner_workspace_id.clone(),
            expected,
            stamped_ready,
            Some(binding),
            matching_event,
        )
        .expect("READY must be paired with a local binding");
        assert!(request.provider_binding().is_some());
    }

    #[test]
    fn provision_success_requires_exact_provider_source_isolation_and_budget_proof() {
        let pending = provisioning();
        let original_config = pending.config().clone();
        let invalid_proofs = [
            {
                let mut p = proof(&pending);
                p.environment_id = id("other-env", EnvironmentId::new);
                p
            },
            {
                let mut p = proof(&pending);
                p.runtime_id = id("other-runtime", RuntimeId::new);
                p
            },
            {
                let mut p = proof(&pending);
                p.runtime_incarnation_id = id("stale-incarnation", RuntimeIncarnationId::new);
                p
            },
            {
                let mut p = proof(&pending);
                p.provider_kind = "other-provider".to_owned();
                p
            },
            {
                let mut p = proof(&pending);
                p.source_resources.clear();
                p
            },
            {
                let mut p = proof(&pending);
                p.isolation.network = NetworkMode::Default;
                p
            },
            {
                let mut p = proof(&pending);
                p.health = EnvironmentHealth::Degraded;
                p
            },
            {
                let mut p = proof(&pending);
                p.budget_enforcement = BudgetEnforcement::HostMonitored;
                p
            },
        ];
        for bad_proof in invalid_proofs {
            let decision = pending
                .apply_provision_result(
                    pending.expected_state(),
                    &incarnation(),
                    ProviderProvisionResult::Ready(bad_proof),
                )
                .unwrap();
            assert_eq!(
                decision,
                LifecycleDecision::ReconciliationRequired(pending.clone())
            );
            assert_eq!(pending.status(), EnvironmentStatus::Provisioning);
            assert_eq!(pending.config(), &original_config);
        }
    }

    #[test]
    fn definite_provision_failure_is_terminal_for_readiness() {
        let pending = provisioning();
        let failed = applied(pending.apply_provision_result(
            pending.expected_state(),
            &incarnation(),
            ProviderProvisionResult::Failed,
        ));
        assert_eq!(failed.status(), EnvironmentStatus::Failed);
        assert_eq!(failed.health(), EnvironmentHealth::Unknown);
    }

    #[test]
    fn ambiguous_provision_does_not_claim_ready_or_failed() {
        let pending = provisioning();
        assert_eq!(
            pending
                .apply_provision_result(
                    pending.expected_state(),
                    &incarnation(),
                    ProviderProvisionResult::Ambiguous
                )
                .unwrap(),
            LifecycleDecision::ReconciliationRequired(pending.clone())
        );
        assert_eq!(pending.status(), EnvironmentStatus::Provisioning);
    }

    #[test]
    fn stale_status_and_version_cas_rejects_commands() {
        let ready = ready();
        assert!(matches!(
            ready.request_suspend(
                ExpectedState {
                    version: ready.version() - 1,
                    status: ready.status()
                },
                LifecycleHolds::default()
            ),
            Err(EnvironmentError::StaleState { .. })
        ));
        assert!(matches!(
            ready.request_suspend(
                ExpectedState {
                    version: ready.version(),
                    status: EnvironmentStatus::Busy
                },
                LifecycleHolds::default()
            ),
            Err(EnvironmentError::StaleState { .. })
        ));
    }

    #[test]
    fn storage_transition_validator_rejects_config_rewrite_and_illegal_edges() {
        let current = ready();
        let expected = current.expected_state();
        let valid = match current
            .request_suspend(expected, LifecycleHolds::default())
            .unwrap()
        {
            LifecycleDecision::Applied(record) => record,
            LifecycleDecision::ReconciliationRequired(_) => unreachable!(),
        };
        assert_eq!(
            validate_transition_proposal(&current, expected, &valid),
            Ok(())
        );

        let mut changed_config = current.config().clone();
        changed_config.provider_kind = "other-provider".to_owned();
        let rewritten = restore_environment(
            current.identity().clone(),
            changed_config,
            EnvironmentStatus::Checkpointing,
            current.health(),
            current.created_at(),
            current.updated_at(),
            current.version() + 1,
        )
        .unwrap();
        assert_eq!(
            validate_transition_proposal(&current, expected, &rewritten),
            Err(EnvironmentError::ImmutableConfigurationChanged)
        );

        let illegal = restore_environment(
            current.identity().clone(),
            current.config().clone(),
            EnvironmentStatus::Destroyed,
            current.health(),
            current.created_at(),
            current.updated_at(),
            current.version() + 1,
        )
        .unwrap();
        assert!(matches!(
            validate_transition_proposal(&current, expected, &illegal),
            Err(EnvironmentError::InvalidTransition { .. })
        ));
    }

    fn hold(index: usize) -> LifecycleHolds {
        let mut holds = LifecycleHolds::default();
        match index {
            0 => holds.active_attempts = 1,
            1 => holds.unsettled_invocations = 1,
            2 => holds.unsettled_control_leases = 1,
            3 => holds.checkpoint_holds = 1,
            4 => holds.unresolved_effects = 1,
            _ => unreachable!(),
        }
        holds
    }

    #[test]
    fn every_active_or_unsettled_hold_denies_suspend_and_destroy() {
        let ready = ready();
        for index in 0..5 {
            let holds = hold(index);
            assert_eq!(
                ready.request_suspend(ready.expected_state(), holds),
                Err(EnvironmentError::LifecycleHeld(holds))
            );
            assert_eq!(
                ready.request_destroy(ready.expected_state(), holds, true, true),
                Err(EnvironmentError::LifecycleHeld(holds))
            );
        }
    }

    #[test]
    fn resume_and_all_provider_result_paths_obey_state_machine() {
        let suspended = suspended();
        let hold = LifecycleHolds {
            unresolved_effects: 1,
            ..LifecycleHolds::default()
        };
        assert_eq!(
            suspended.request_resume(suspended.expected_state(), hold),
            Err(EnvironmentError::LifecycleHeld(hold))
        );
        let checkpointing =
            applied(ready().request_suspend(ready().expected_state(), LifecycleHolds::default()));
        let failed = applied(checkpointing.apply_suspend_result(
            checkpointing.expected_state(),
            ProviderSuspendResult::Failed,
        ));
        assert_eq!(failed.status(), EnvironmentStatus::Failed);
        assert!(matches!(
            failed.request_resume(failed.expected_state(), LifecycleHolds::default()),
            Err(EnvironmentError::InvalidTransition { .. })
        ));
        let busy = restore_environment(
            identity(),
            config(),
            EnvironmentStatus::Busy,
            EnvironmentHealth::Healthy,
            "2026-10-09T00:00:00Z",
            "2026-10-09T00:00:00Z",
            4,
        )
        .unwrap();
        assert!(matches!(
            busy.request_destroy(busy.expected_state(), LifecycleHolds::default(), true, true),
            Err(EnvironmentError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn suspend_has_checkpointing_intermediate_and_ambiguous_result_stays_there() {
        let ready = ready();
        let checkpointing =
            applied(ready.request_suspend(ready.expected_state(), LifecycleHolds::default()));
        assert_eq!(checkpointing.status(), EnvironmentStatus::Checkpointing);
        assert_eq!(checkpointing.version(), ready.version() + 1);
        assert_eq!(
            checkpointing
                .apply_suspend_result(
                    checkpointing.expected_state(),
                    ProviderSuspendResult::Ambiguous
                )
                .unwrap(),
            LifecycleDecision::ReconciliationRequired(checkpointing.clone())
        );
        let suspended = applied(checkpointing.apply_suspend_result(
            checkpointing.expected_state(),
            ProviderSuspendResult::Suspended,
        ));
        assert_eq!(suspended.status(), EnvironmentStatus::Suspended);
        assert_eq!(suspended.version(), checkpointing.version() + 1);
    }

    #[test]
    fn resume_revalidates_as_provisioning_and_carries_no_execution_authority() {
        let suspended = suspended();
        let provisioning = applied(
            suspended.request_resume(suspended.expected_state(), LifecycleHolds::default()),
        );
        assert_eq!(provisioning.status(), EnvironmentStatus::Provisioning);
        let resumed = applied(provisioning.apply_provision_result(
            provisioning.expected_state(),
            &incarnation(),
            ProviderProvisionResult::Ready(proof(&provisioning)),
        ));
        assert_eq!(resumed.status(), EnvironmentStatus::Ready);
        assert_eq!(resumed.config(), suspended.config());
    }

    #[test]
    fn destroy_denies_retention_and_uncommitted_output_holds() {
        let ready = ready();
        assert_eq!(
            ready.request_destroy(
                ready.expected_state(),
                LifecycleHolds::default(),
                false,
                true
            ),
            Err(EnvironmentError::RetentionDisallowsDestroy)
        );
        assert_eq!(
            ready.request_destroy(
                ready.expected_state(),
                LifecycleHolds::default(),
                true,
                false
            ),
            Err(EnvironmentError::RequiredOutputsUncommitted)
        );
    }

    #[test]
    fn ambiguous_destroy_keeps_destroying_and_requires_reconciliation() {
        let destroying = destroying();
        let before = destroying.clone();
        assert_eq!(
            destroying
                .apply_destroy_result(
                    destroying.expected_state(),
                    ProviderDestroyResult::Ambiguous
                )
                .unwrap(),
            LifecycleDecision::ReconciliationRequired(before.clone())
        );
        assert_eq!(before.status(), EnvironmentStatus::Destroying);
        assert_ne!(before.status(), EnvironmentStatus::Destroyed);
    }

    #[test]
    fn only_positive_absence_proof_marks_destroyed_and_terminal() {
        let destroying = destroying();
        let destroyed = applied(destroying.apply_destroy_result(
            destroying.expected_state(),
            ProviderDestroyResult::ConfirmedAbsent,
        ));
        assert_eq!(destroyed.status(), EnvironmentStatus::Destroyed);
        assert_eq!(destroyed.version(), destroying.version() + 1);
        assert!(matches!(
            destroyed.apply_destroy_result(
                destroyed.expected_state(),
                ProviderDestroyResult::ConfirmedAbsent
            ),
            Err(EnvironmentError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn definite_cleanup_failure_is_visible_and_not_destroyed() {
        let destroying = destroying();
        let failed = applied(
            destroying
                .apply_destroy_result(destroying.expected_state(), ProviderDestroyResult::Failed),
        );
        assert_eq!(failed.status(), EnvironmentStatus::Failed);
        assert_eq!(failed.health(), EnvironmentHealth::Unknown);
    }
}
