//! Environment provider capability, budget admission, and lifecycle guards.
//!
//! `docs/ENVIRONMENTS.md` requires that:
//!
//! * `probe` reports explicit feature flags and limits, and unknown support is
//!   unavailable rather than assumed.
//! * `REQUIRE_PROVIDER_ENFORCED` blocks provisioning unless the provider enforces
//!   the requested cost cap, while `ALLOW_HOST_MONITORED` is explicitly a monitor
//!   that may overshoot and must never be described as a hard cap.
//! * Persistent Environment cost and wall-time ceilings are cumulative from creation
//!   and do not reset on a schedule.
//! * A persistent workload must be stopped or suspended and observed quiescent
//!   before its Workspace can become read-only; unknown provider state blocks archive
//!   instead of pretending a background process stopped.
//! * Sharing scope is not an authorization grant, and a private worktree and a
//!   retained browser have different reuse semantics.

use serde::{Deserialize, Serialize};

/// The lifetime of an Environment substrate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentLifetime {
    Attempt,
    TaskRetained,
    WorkspacePersistent,
}

/// Who may reuse an Environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentSharingScope {
    AttemptPrivate,
    TaskShared,
    CoworkerPrivate,
    WorkspaceShared,
    UserShared,
}

/// How an Environment may be reused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReuseBoundary {
    /// One Attempt only. A replacement Attempt never reuses the substrate.
    SingleAttempt,
    /// Attempts of the owning Task, subject to write and control fencing.
    OwningTask,
    /// A retained browser or desktop surface that survives between Tasks.
    RetainedSurface,
    /// Reserved for a user-level owner contract that v1 does not offer.
    Unavailable,
}

/// The reuse boundary implied by a lifetime and sharing scope.
pub fn reuse_boundary(
    lifetime: EnvironmentLifetime,
    scope: EnvironmentSharingScope,
    is_browser_or_desktop_surface: bool,
) -> ReuseBoundary {
    if scope == EnvironmentSharingScope::UserShared {
        return ReuseBoundary::Unavailable;
    }
    match (lifetime, scope) {
        (EnvironmentLifetime::Attempt, _) => ReuseBoundary::SingleAttempt,
        (EnvironmentLifetime::TaskRetained, _) => ReuseBoundary::OwningTask,
        (EnvironmentLifetime::WorkspacePersistent, _) => {
            if is_browser_or_desktop_surface {
                ReuseBoundary::RetainedSurface
            } else {
                ReuseBoundary::OwningTask
            }
        }
    }
}

/// Whether the provider enforces a limit itself or only observes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetEnforcement {
    ProviderEnforced,
    HostMonitored,
    Unavailable,
}

/// Whether a limit must be provider enforced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetEnforcementPolicy {
    RequireProviderEnforced,
    AllowHostMonitored,
}

/// The cumulative usage of a persistent Environment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentUsage {
    /// Cumulative cost in the pinned budget currency's minor units.
    pub cost_minor_units: u64,
    /// Cumulative wall time in milliseconds since creation.
    pub wall_time_ms: u64,
    /// Cumulative tokens consumed.
    pub tokens: u64,
}

/// The cumulative budget of a persistent Environment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentBudget {
    pub max_cost_minor_units: Option<u64>,
    pub max_wall_time_ms: Option<u64>,
    pub max_tokens: Option<u64>,
    pub max_child_attempts: Option<u64>,
}

/// Why a use of an Environment was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentUseRefusal {
    /// The provider does not enforce the requested cap and the policy requires it.
    BudgetEnforcementUnavailable,
    /// A cumulative ceiling has been reached.
    BudgetLimitReached,
    /// The requested use would exceed the child-attempt ceiling.
    ChildAttemptLimitReached,
}

impl EnvironmentUseRefusal {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::BudgetEnforcementUnavailable => "ENVIRONMENT_BUDGET_ENFORCEMENT_UNAVAILABLE",
            Self::BudgetLimitReached => "ENVIRONMENT_BUDGET_LIMIT_REACHED",
            Self::ChildAttemptLimitReached => "ENVIRONMENT_CHILD_ATTEMPT_LIMIT_REACHED",
        }
    }
}

/// The result of admitting a use of an Environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvironmentUseAdmission {
    /// Whether the host-monitored ceiling may overshoot while the host is offline,
    /// suspended, or reporting charges late. It is never a hard cap.
    pub may_overshoot: bool,
    /// The remaining cumulative budget.
    pub remaining: EnvironmentBudget,
}

/// Decide whether an Environment may admit one more use.
///
/// The ceilings are cumulative from creation. There is no reset, no top-up, and no
/// in-place budget change, so a replacement Environment is the only way to continue
/// past a limit.
pub fn admit_environment_use(
    budget: &EnvironmentBudget,
    usage: EnvironmentUsage,
    enforcement: BudgetEnforcement,
    policy: BudgetEnforcementPolicy,
    child_attempts_used: u64,
) -> Result<EnvironmentUseAdmission, EnvironmentUseRefusal> {
    if enforcement == BudgetEnforcement::Unavailable
        || (policy == BudgetEnforcementPolicy::RequireProviderEnforced
            && enforcement != BudgetEnforcement::ProviderEnforced)
    {
        return Err(EnvironmentUseRefusal::BudgetEnforcementUnavailable);
    }
    let may_overshoot = enforcement == BudgetEnforcement::HostMonitored;
    if let Some(limit) = budget.max_cost_minor_units {
        if usage.cost_minor_units >= limit {
            return Err(EnvironmentUseRefusal::BudgetLimitReached);
        }
    }
    if let Some(limit) = budget.max_wall_time_ms {
        if usage.wall_time_ms >= limit {
            return Err(EnvironmentUseRefusal::BudgetLimitReached);
        }
    }
    if let Some(limit) = budget.max_tokens {
        if usage.tokens >= limit {
            return Err(EnvironmentUseRefusal::BudgetLimitReached);
        }
    }
    if let Some(limit) = budget.max_child_attempts {
        if child_attempts_used >= limit {
            return Err(EnvironmentUseRefusal::ChildAttemptLimitReached);
        }
    }
    Ok(EnvironmentUseAdmission {
        may_overshoot,
        remaining: EnvironmentBudget {
            max_cost_minor_units: budget
                .max_cost_minor_units
                .map(|limit| limit.saturating_sub(usage.cost_minor_units)),
            max_wall_time_ms: budget
                .max_wall_time_ms
                .map(|limit| limit.saturating_sub(usage.wall_time_ms)),
            max_tokens: budget.max_tokens.map(|limit| limit.saturating_sub(usage.tokens)),
            max_child_attempts: budget
                .max_child_attempts
                .map(|limit| limit.saturating_sub(child_attempts_used)),
        },
    })
}

/// A durable reference that keeps an Environment alive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentReferenceKind {
    ActiveAttempt,
    UnsettledInvocation,
    ActiveControlLease,
    UnresolvedEffect,
    CheckpointHold,
}

/// A durable reference to an Environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentReference {
    pub kind: EnvironmentReferenceKind,
    pub reference_id: String,
}

/// Why an Environment may not be archived or destroyed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentLifecycleRefusal {
    /// A durable reference still points at the Environment.
    Referenced(EnvironmentReferenceKind, String),
    /// The provider state is unknown, so a background process may still be running.
    ProviderStateUnknown,
    /// The Environment is not quiescent.
    NotQuiescent,
    /// The retention policy does not allow destruction.
    RetentionDisallowsDestroy,
    /// Required outputs are not committed.
    RequiredOutputsUncommitted,
}

/// Decide whether a Workspace may be archived while an Environment exists.
///
/// Archive requires a positive quiescence observation. Unknown provider state blocks
/// the archive instead of pretending a background process stopped.
pub fn assert_environment_may_be_archived(
    references: &[EnvironmentReference],
    provider_state_known: bool,
    quiescent: bool,
) -> Result<(), EnvironmentLifecycleRefusal> {
    if let Some(reference) = references.first() {
        return Err(EnvironmentLifecycleRefusal::Referenced(
            reference.kind,
            reference.reference_id.clone(),
        ));
    }
    if !provider_state_known {
        return Err(EnvironmentLifecycleRefusal::ProviderStateUnknown);
    }
    if !quiescent {
        return Err(EnvironmentLifecycleRefusal::NotQuiescent);
    }
    Ok(())
}

/// The provider classes the local Runtime can actually offer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalProviderClass {
    /// A Task-scoped local workspace with an OS-enforced isolation provider.
    LocalWorkspace,
    /// A retained browser surface.
    Browser,
    /// A desktop surface.
    Desktop,
    /// A containerized substrate.
    Container,
}

/// The capabilities a provider reports from `probe`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentProviderCapabilities {
    pub provider_kind: String,
    pub provider_version: String,
    pub operating_system: String,
    pub operating_system_build: String,
    pub classes: Vec<LocalProviderClass>,
    pub supports_attempt_lifetime: bool,
    pub supports_task_retained_lifetime: bool,
    pub supports_workspace_persistent_lifetime: bool,
    pub supports_checkpoint: bool,
    pub supports_restore: bool,
    pub budget_enforcement: BudgetEnforcement,
    pub max_members: usize,
    pub notes: Vec<String>,
}

impl EnvironmentProviderCapabilities {
    /// Whether the provider can offer a lifetime at all.
    pub fn supports_lifetime(&self, lifetime: EnvironmentLifetime) -> bool {
        match lifetime {
            EnvironmentLifetime::Attempt => self.supports_attempt_lifetime,
            EnvironmentLifetime::TaskRetained => self.supports_task_retained_lifetime,
            EnvironmentLifetime::WorkspacePersistent => self.supports_workspace_persistent_lifetime,
        }
    }
}

/// Probe the local Environment provider.
///
/// The probe reports what this build can actually offer. A capability that is not
/// implemented here is reported as unsupported rather than advertised.
pub fn probe_environment_provider() -> EnvironmentProviderCapabilities {
    let mut notes = Vec::new();
    let mut classes = Vec::new();
    let mut supports_attempt_lifetime = false;
    let mut supports_task_retained_lifetime = false;
    let supports_workspace_persistent_lifetime = false;
    let supports_checkpoint = false;
    let supports_restore = false;

    #[cfg(target_os = "linux")]
    {
        let bubblewrap_present = ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap"]
            .iter()
            .any(|candidate| std::path::Path::new(candidate).exists());
        if bubblewrap_present {
            classes.push(LocalProviderClass::LocalWorkspace);
            supports_attempt_lifetime = true;
            supports_task_retained_lifetime = true;
            notes.push(
                "the local workspace provider is available but its isolation enforcement is not qualified on this host"
                    .to_owned(),
            );
        } else {
            notes.push(
                "no qualified bubblewrap binary is present, so no local workspace substrate can be offered"
                    .to_owned(),
            );
        }
        notes.push(
            "no container or desktop substrate provider is implemented in this build".to_owned(),
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        notes.push(
            "no local Environment provider is implemented for this platform".to_owned(),
        );
    }

    EnvironmentProviderCapabilities {
        provider_kind: "local-environment".to_owned(),
        provider_version: "1".to_owned(),
        operating_system: std::env::consts::OS.to_owned(),
        operating_system_build: crate::environment_isolation::operating_system_build(),
        classes,
        supports_attempt_lifetime,
        supports_task_retained_lifetime,
        supports_workspace_persistent_lifetime,
        supports_checkpoint,
        supports_restore,
        budget_enforcement: BudgetEnforcement::Unavailable,
        max_members: 16,
        notes,
    }
}

/// A provider request identity, used to make provisioning idempotent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequestIdentity {
    pub request_id: String,
    /// The digest of the exact request body, so a retry can be recognised as the same
    /// request rather than a new one.
    pub request_body_digest: String,
}

/// The outcome of an idempotent provider provisioning request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvisioningOutcome {
    /// The provider created the substrate.
    Created { provider_handle: String },
    /// The same request was already applied; the same handle is returned.
    AlreadyApplied { provider_handle: String },
    /// The provider state could not be determined. The caller must reconcile before
    /// retrying a non-idempotent allocation.
    ReconciliationRequired,
}

/// Decide how to treat a provisioning response.
///
/// A lost response is not proof that provisioning failed. The caller reconciles the
/// provider state instead of allocating a second substrate for the same request.
pub fn classify_provisioning_response(
    identity: &ProviderRequestIdentity,
    recorded: Option<&ProviderRequestIdentity>,
    existing_handle: Option<&str>,
    response_received: bool,
) -> ProvisioningOutcome {
    if let Some(recorded) = recorded {
        if recorded.request_id == identity.request_id
            && recorded.request_body_digest == identity.request_body_digest
        {
            return match existing_handle {
                Some(handle) => ProvisioningOutcome::AlreadyApplied {
                    provider_handle: handle.to_owned(),
                },
                None => ProvisioningOutcome::ReconciliationRequired,
            };
        }
    }
    if response_received {
        return ProvisioningOutcome::ReconciliationRequired;
    }
    ProvisioningOutcome::ReconciliationRequired
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> EnvironmentBudget {
        EnvironmentBudget {
            max_cost_minor_units: Some(1_000),
            max_wall_time_ms: Some(60_000),
            max_tokens: Some(10_000),
            max_child_attempts: Some(2),
        }
    }

    #[test]
    fn a_provider_enforced_budget_admits_a_use_within_the_ceiling() {
        let admission = admit_environment_use(
            &budget(),
            EnvironmentUsage {
                cost_minor_units: 100,
                wall_time_ms: 1_000,
                tokens: 500,
            },
            BudgetEnforcement::ProviderEnforced,
            BudgetEnforcementPolicy::RequireProviderEnforced,
            0,
        )
        .expect("admitted");
        assert!(!admission.may_overshoot);
        assert_eq!(admission.remaining.max_cost_minor_units, Some(900));
    }

    #[test]
    fn a_required_provider_enforced_budget_refuses_a_monitor_only_provider() {
        assert_eq!(
            admit_environment_use(
                &budget(),
                EnvironmentUsage::default(),
                BudgetEnforcement::HostMonitored,
                BudgetEnforcementPolicy::RequireProviderEnforced,
                0,
            )
            .unwrap_err(),
            EnvironmentUseRefusal::BudgetEnforcementUnavailable
        );
        assert_eq!(
            admit_environment_use(
                &budget(),
                EnvironmentUsage::default(),
                BudgetEnforcement::Unavailable,
                BudgetEnforcementPolicy::AllowHostMonitored,
                0,
            )
            .unwrap_err(),
            EnvironmentUseRefusal::BudgetEnforcementUnavailable
        );
    }

    #[test]
    fn a_host_monitored_budget_is_admitted_but_may_overshoot() {
        let admission = admit_environment_use(
            &budget(),
            EnvironmentUsage::default(),
            BudgetEnforcement::HostMonitored,
            BudgetEnforcementPolicy::AllowHostMonitored,
            0,
        )
        .expect("admitted");
        assert!(admission.may_overshoot);
    }

    #[test]
    fn a_reached_ceiling_stops_admitting_new_uses() {
        assert_eq!(
            admit_environment_use(
                &budget(),
                EnvironmentUsage {
                    cost_minor_units: 1_000,
                    wall_time_ms: 0,
                    tokens: 0,
                },
                BudgetEnforcement::ProviderEnforced,
                BudgetEnforcementPolicy::RequireProviderEnforced,
                0,
            )
            .unwrap_err(),
            EnvironmentUseRefusal::BudgetLimitReached
        );
        assert_eq!(
            admit_environment_use(
                &budget(),
                EnvironmentUsage::default(),
                BudgetEnforcement::ProviderEnforced,
                BudgetEnforcementPolicy::RequireProviderEnforced,
                2,
            )
            .unwrap_err(),
            EnvironmentUseRefusal::ChildAttemptLimitReached
        );
    }

    #[test]
    fn an_archive_is_blocked_by_a_durable_reference() {
        let references = vec![EnvironmentReference {
            kind: EnvironmentReferenceKind::ActiveAttempt,
            reference_id: "attempt-1".to_owned(),
        }];
        assert_eq!(
            assert_environment_may_be_archived(&references, true, true).unwrap_err(),
            EnvironmentLifecycleRefusal::Referenced(
                EnvironmentReferenceKind::ActiveAttempt,
                "attempt-1".to_owned()
            )
        );
    }

    #[test]
    fn unknown_provider_state_blocks_an_archive() {
        assert_eq!(
            assert_environment_may_be_archived(&[], false, true).unwrap_err(),
            EnvironmentLifecycleRefusal::ProviderStateUnknown
        );
        assert_eq!(
            assert_environment_may_be_archived(&[], true, false).unwrap_err(),
            EnvironmentLifecycleRefusal::NotQuiescent
        );
        assert_eq!(
            assert_environment_may_be_archived(&[], true, true),
            Ok(())
        );
    }

    #[test]
    fn private_worktree_and_retained_browser_semantics_are_separate() {
        assert_eq!(
            reuse_boundary(
                EnvironmentLifetime::Attempt,
                EnvironmentSharingScope::AttemptPrivate,
                false
            ),
            ReuseBoundary::SingleAttempt
        );
        assert_eq!(
            reuse_boundary(
                EnvironmentLifetime::WorkspacePersistent,
                EnvironmentSharingScope::WorkspaceShared,
                true
            ),
            ReuseBoundary::RetainedSurface
        );
        assert_eq!(
            reuse_boundary(
                EnvironmentLifetime::WorkspacePersistent,
                EnvironmentSharingScope::WorkspaceShared,
                false
            ),
            ReuseBoundary::OwningTask
        );
        assert_eq!(
            reuse_boundary(
                EnvironmentLifetime::WorkspacePersistent,
                EnvironmentSharingScope::UserShared,
                true
            ),
            ReuseBoundary::Unavailable
        );
    }

    #[test]
    fn the_provider_probe_reports_only_what_this_build_offers() {
        let capabilities = probe_environment_provider();
        assert!(!capabilities.notes.is_empty());
        assert!(!capabilities.supports_checkpoint);
        assert!(!capabilities.supports_restore);
        assert_eq!(capabilities.budget_enforcement, BudgetEnforcement::Unavailable);
    }

    #[test]
    fn a_lost_provisioning_response_requires_reconciliation() {
        let identity = ProviderRequestIdentity {
            request_id: "request-1".to_owned(),
            request_body_digest: "sha256:body".to_owned(),
        };
        assert_eq!(
            classify_provisioning_response(&identity, None, None, false),
            ProvisioningOutcome::ReconciliationRequired
        );
        assert_eq!(
            classify_provisioning_response(&identity, Some(&identity), Some("handle-1"), false),
            ProvisioningOutcome::AlreadyApplied {
                provider_handle: "handle-1".to_owned()
            }
        );
        assert_eq!(
            classify_provisioning_response(&identity, Some(&identity), None, false),
            ProvisioningOutcome::ReconciliationRequired
        );
    }
}
