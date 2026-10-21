//! Browser input control: one actor at a time, epoch fenced.
//!
//! `docs/ENVIRONMENTS.md` requires that exactly one `EnvironmentControlLease` owner
//! acts at a time, and `docs/CHROME-BROWSER-LLD.md` requires that a human takeover is
//! an atomic epoch transfer, that queued agent input is discarded, and that a return
//! to the agent re-observes the page under a new epoch instead of replaying stale
//! selectors.
//!
//! Two rules shape this module:
//!
//! * The actor is derived from the authenticated principal and the lease, never from
//!   the request body. A request that *claims* an actor is only checked for
//!   consistency; it never grants that actor.
//! * This module decides and records intent. It does not dispatch provider input, and
//!   it is not evidence that an action was applied.

use crate::browser_consent::BrowserMode;
use serde::{Deserialize, Serialize};

/// Who currently owns browser input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrowserControlOwner {
    Agent,
    User,
    None,
    /// A transfer has been requested and is waiting for a safe point.
    TransferPending,
}

/// The kind of principal that is acting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrincipalKind {
    /// An authenticated human owner.
    User,
    /// An authenticated agent session.
    AgentSession,
}

/// The authenticated caller.
///
/// The request body may echo an actor, but this principal is the authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedPrincipal {
    pub principal_id: String,
    pub kind: PrincipalKind,
    /// The agent session identity, when the principal is an agent session.
    pub agent_session_id: Option<String>,
}

impl AuthenticatedPrincipal {
    pub fn user(principal_id: &str) -> Self {
        Self {
            principal_id: principal_id.to_owned(),
            kind: PrincipalKind::User,
            agent_session_id: None,
        }
    }

    pub fn agent_session(principal_id: &str, agent_session_id: &str) -> Self {
        Self {
            principal_id: principal_id.to_owned(),
            kind: PrincipalKind::AgentSession,
            agent_session_id: Some(agent_session_id.to_owned()),
        }
    }
}

/// The actor a request claims. This is caller intent only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimedActor {
    User,
    Agent,
}

/// The exact provider mechanism an admitted action will use.
///
/// The method is recorded so that an action's execution method is never ambiguous.
/// There is deliberately no "arbitrary script" or "raw protocol" member: agents must
/// not receive a generic escape hatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserExecutionMethod {
    DomElement,
    AccessibilityNode,
    CoordinatePointer,
    KeyboardEvent,
    Navigation,
    Download,
}

/// The browser actions a caller may request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserActionKind {
    OpenTab,
    CloseTab,
    Navigate,
    Back,
    Forward,
    Reload,
    TakeControl,
    ReturnControl,
    Detach,
    CloseSession,
    Capture,
    Find,
    SetZoom,
    OpenExternal,
    ApproveDownload,
}

/// A browser command as received from a caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserCommand {
    pub request_id: String,
    pub session_id: String,
    pub tab_id: Option<String>,
    pub action: BrowserActionKind,
    /// The caller's claimed intent. It is checked, never trusted.
    pub claimed_actor: ClaimedActor,
    /// The control epoch the caller believes is current.
    pub expected_epoch: u64,
    /// The session revision the caller believes is current.
    pub expected_session_revision: u64,
    /// The generation of the tab the caller believes is current.
    pub expected_tab_generation: u64,
}

/// The Runtime-local browser control lease projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserControlLease {
    pub session_id: String,
    pub environment_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub mode: BrowserMode,
    pub owner: BrowserControlOwner,
    pub control_epoch: u64,
    pub session_revision: u64,
    pub observation_revision: u64,
    /// Unix epoch milliseconds after which the lease is no longer current.
    pub expires_at_ms: i64,
    /// Provider inputs that have been dispatched but not yet acknowledged.
    pub in_flight_inputs: u32,
    /// The agent session that owns the lease, when the owner is the agent.
    pub owner_agent_session_id: Option<String>,
    /// The principal that owns the lease, when the owner is the user.
    pub owner_principal_id: Option<String>,
}

/// An observation captured from a browser surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserObservation {
    pub session_id: String,
    pub control_epoch: u64,
    pub observation_revision: u64,
    pub captured_at_ms: i64,
    /// The identity of the provider the observation came from.
    pub provider_identity: String,
}

/// Why a browser command was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserControlError {
    /// The caller's claimed actor does not match the authenticated principal.
    ActorMismatch,
    /// The principal kind cannot perform this action.
    PrincipalNotPermitted,
    /// The lease belongs to a different session.
    SessionMismatch,
    /// The caller's expected epoch is not the live epoch.
    StaleEpoch,
    /// The lease expired.
    LeaseExpired,
    /// Another actor owns the input right now.
    NotTheLeaseOwner,
    /// A transfer is in progress and no new input may be admitted.
    TransferPending,
    /// The caller's expected session revision is not current.
    StaleSessionRevision,
    /// The caller's expected tab generation is not current.
    StaleTabGeneration,
    /// Provider inputs are still in flight, so the transfer cannot be atomic.
    InputsInFlight,
    /// The observation is missing, stale, or bound to another epoch.
    ObservationNotUsable,
    /// The observation revision did not advance past the last accepted one.
    ObservationRevisionNotAdvanced,
    /// The control epoch cannot be incremented.
    EpochExhausted,
    /// The browser mode does not allow agent input.
    ModeDoesNotAllowAgentInput,
}

impl BrowserControlError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::ActorMismatch => "BROWSER_ACTOR_MISMATCH",
            Self::PrincipalNotPermitted => "BROWSER_PRINCIPAL_NOT_PERMITTED",
            Self::SessionMismatch => "BROWSER_SESSION_MISMATCH",
            Self::StaleEpoch => "BROWSER_STALE_CONTROL_EPOCH",
            Self::LeaseExpired => "BROWSER_LEASE_EXPIRED",
            Self::NotTheLeaseOwner => "BROWSER_NOT_THE_LEASE_OWNER",
            Self::TransferPending => "BROWSER_CONTROL_TRANSFER_PENDING",
            Self::StaleSessionRevision => "BROWSER_STALE_SESSION_REVISION",
            Self::StaleTabGeneration => "BROWSER_STALE_TAB_GENERATION",
            Self::InputsInFlight => "BROWSER_INPUTS_IN_FLIGHT",
            Self::ObservationNotUsable => "BROWSER_OBSERVATION_NOT_USABLE",
            Self::ObservationRevisionNotAdvanced => "BROWSER_OBSERVATION_REVISION_NOT_ADVANCED",
            Self::EpochExhausted => "BROWSER_CONTROL_EPOCH_EXHAUSTED",
            Self::ModeDoesNotAllowAgentInput => "BROWSER_MODE_DISALLOWS_AGENT_INPUT",
        }
    }
}

/// A command that passed every control check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedBrowserAction {
    pub request_id: String,
    pub session_id: String,
    pub actor: BrowserControlOwner,
    pub control_epoch: u64,
    pub session_revision: u64,
    pub action: BrowserActionKind,
    pub execution_method: Option<BrowserExecutionMethod>,
}

/// The execution method an admitted action will use.
///
/// The mapping is fixed so that the recorded method is exact. Actions that change
/// browser state but do not address a page element have no element method.
pub fn execution_method_for(action: BrowserActionKind) -> Option<BrowserExecutionMethod> {
    match action {
        BrowserActionKind::Navigate
        | BrowserActionKind::Back
        | BrowserActionKind::Forward
        | BrowserActionKind::Reload
        | BrowserActionKind::OpenTab
        | BrowserActionKind::OpenExternal => Some(BrowserExecutionMethod::Navigation),
        BrowserActionKind::Capture => Some(BrowserExecutionMethod::AccessibilityNode),
        BrowserActionKind::Find => Some(BrowserExecutionMethod::DomElement),
        BrowserActionKind::ApproveDownload => Some(BrowserExecutionMethod::Download),
        BrowserActionKind::CloseTab
        | BrowserActionKind::TakeControl
        | BrowserActionKind::ReturnControl
        | BrowserActionKind::Detach
        | BrowserActionKind::CloseSession
        | BrowserActionKind::SetZoom => None,
    }
}

/// Decide whether a command may be admitted for the current lease.
///
/// The actor is derived from the authenticated principal and the lease owner. The
/// claimed actor in the request is only checked for consistency.
pub fn admit_action(
    command: &BrowserCommand,
    principal: &AuthenticatedPrincipal,
    lease: &BrowserControlLease,
    now_ms: i64,
) -> Result<AdmittedBrowserAction, BrowserControlError> {
    if command.session_id != lease.session_id {
        return Err(BrowserControlError::SessionMismatch);
    }
    if now_ms >= lease.expires_at_ms {
        return Err(BrowserControlError::LeaseExpired);
    }
    if command.expected_epoch != lease.control_epoch {
        return Err(BrowserControlError::StaleEpoch);
    }
    if command.expected_session_revision != lease.session_revision {
        return Err(BrowserControlError::StaleSessionRevision);
    }
    if lease.owner == BrowserControlOwner::TransferPending {
        return Err(BrowserControlError::TransferPending);
    }

    let derived = derive_actor(principal, lease)?;
    match (command.claimed_actor, derived) {
        (ClaimedActor::User, BrowserControlOwner::User) => {}
        (ClaimedActor::Agent, BrowserControlOwner::Agent) => {}
        _ => return Err(BrowserControlError::ActorMismatch),
    }

    if derived == BrowserControlOwner::Agent && !lease.mode.allows_agent_input() {
        return Err(BrowserControlError::ModeDoesNotAllowAgentInput);
    }

    Ok(AdmittedBrowserAction {
        request_id: command.request_id.clone(),
        session_id: lease.session_id.clone(),
        actor: derived,
        control_epoch: lease.control_epoch,
        session_revision: lease.session_revision,
        action: command.action,
        execution_method: execution_method_for(command.action),
    })
}

/// Derive the acting owner from the authenticated principal and the lease.
fn derive_actor(
    principal: &AuthenticatedPrincipal,
    lease: &BrowserControlLease,
) -> Result<BrowserControlOwner, BrowserControlError> {
    match lease.owner {
        BrowserControlOwner::None => Err(BrowserControlError::NotTheLeaseOwner),
        BrowserControlOwner::TransferPending => Err(BrowserControlError::TransferPending),
        BrowserControlOwner::User => {
            if principal.kind != PrincipalKind::User {
                return Err(BrowserControlError::NotTheLeaseOwner);
            }
            if lease.owner_principal_id.as_deref() != Some(principal.principal_id.as_str()) {
                return Err(BrowserControlError::NotTheLeaseOwner);
            }
            Ok(BrowserControlOwner::User)
        }
        BrowserControlOwner::Agent => {
            if principal.kind != PrincipalKind::AgentSession {
                return Err(BrowserControlError::NotTheLeaseOwner);
            }
            match (
                lease.owner_agent_session_id.as_deref(),
                principal.agent_session_id.as_deref(),
            ) {
                (Some(owner), Some(actor)) if owner == actor => Ok(BrowserControlOwner::Agent),
                _ => Err(BrowserControlError::NotTheLeaseOwner),
            }
        }
    }
}

/// Transfer browser input to the authenticated user.
///
/// The transfer is atomic: provider inputs that are still in flight block it, the
/// control epoch advances, and the queued agent inputs are discarded by the caller
/// because their epoch is now stale.
pub fn request_takeover(
    command: &BrowserCommand,
    principal: &AuthenticatedPrincipal,
    lease: &BrowserControlLease,
    now_ms: i64,
) -> Result<BrowserControlLease, BrowserControlError> {
    if command.session_id != lease.session_id {
        return Err(BrowserControlError::SessionMismatch);
    }
    if principal.kind != PrincipalKind::User {
        return Err(BrowserControlError::PrincipalNotPermitted);
    }
    if now_ms >= lease.expires_at_ms {
        return Err(BrowserControlError::LeaseExpired);
    }
    if command.expected_epoch != lease.control_epoch {
        return Err(BrowserControlError::StaleEpoch);
    }
    if lease.owner == BrowserControlOwner::TransferPending {
        return Err(BrowserControlError::TransferPending);
    }
    if lease.in_flight_inputs > 0 {
        return Err(BrowserControlError::InputsInFlight);
    }
    let control_epoch = lease
        .control_epoch
        .checked_add(1)
        .ok_or(BrowserControlError::EpochExhausted)?;
    let session_revision = lease
        .session_revision
        .checked_add(1)
        .ok_or(BrowserControlError::EpochExhausted)?;
    Ok(BrowserControlLease {
        owner: BrowserControlOwner::User,
        owner_principal_id: Some(principal.principal_id.clone()),
        owner_agent_session_id: None,
        control_epoch,
        session_revision,
        in_flight_inputs: 0,
        ..lease.clone()
    })
}

/// Transfer browser input back to the agent.
///
/// The return requires a fresh observation that was captured under the current user
/// control epoch and that advances the observation revision. A stale or reused
/// observation never releases the lease back to the agent.
pub fn return_to_agent(
    command: &BrowserCommand,
    principal: &AuthenticatedPrincipal,
    lease: &BrowserControlLease,
    observation: Option<&BrowserObservation>,
    now_ms: i64,
    maximum_observation_age_ms: i64,
) -> Result<BrowserControlLease, BrowserControlError> {
    if command.session_id != lease.session_id {
        return Err(BrowserControlError::SessionMismatch);
    }
    if principal.kind != PrincipalKind::User {
        return Err(BrowserControlError::PrincipalNotPermitted);
    }
    if now_ms >= lease.expires_at_ms {
        return Err(BrowserControlError::LeaseExpired);
    }
    if command.expected_epoch != lease.control_epoch {
        return Err(BrowserControlError::StaleEpoch);
    }
    if lease.owner != BrowserControlOwner::User {
        return Err(BrowserControlError::NotTheLeaseOwner);
    }
    if lease.owner_principal_id.as_deref() != Some(principal.principal_id.as_str()) {
        return Err(BrowserControlError::NotTheLeaseOwner);
    }
    if lease.in_flight_inputs > 0 {
        return Err(BrowserControlError::InputsInFlight);
    }
    let observation = observation.ok_or(BrowserControlError::ObservationNotUsable)?;
    if observation.session_id != lease.session_id {
        return Err(BrowserControlError::ObservationNotUsable);
    }
    if observation.control_epoch != lease.control_epoch {
        return Err(BrowserControlError::ObservationNotUsable);
    }
    if observation.observation_revision <= lease.observation_revision {
        return Err(BrowserControlError::ObservationRevisionNotAdvanced);
    }
    if observation.captured_at_ms > now_ms {
        return Err(BrowserControlError::ObservationNotUsable);
    }
    if now_ms - observation.captured_at_ms > maximum_observation_age_ms {
        return Err(BrowserControlError::ObservationNotUsable);
    }
    if observation.provider_identity.is_empty() {
        return Err(BrowserControlError::ObservationNotUsable);
    }
    let control_epoch = lease
        .control_epoch
        .checked_add(1)
        .ok_or(BrowserControlError::EpochExhausted)?;
    let session_revision = lease
        .session_revision
        .checked_add(1)
        .ok_or(BrowserControlError::EpochExhausted)?;
    Ok(BrowserControlLease {
        owner: BrowserControlOwner::Agent,
        owner_principal_id: None,
        owner_agent_session_id: lease.owner_agent_session_id.clone(),
        control_epoch,
        session_revision,
        observation_revision: observation.observation_revision,
        in_flight_inputs: 0,
        ..lease.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lease() -> BrowserControlLease {
        BrowserControlLease {
            session_id: "session-1".to_owned(),
            environment_id: "env-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            task_id: Some("task-1".to_owned()),
            attempt_id: Some("attempt-1".to_owned()),
            mode: BrowserMode::ManagedChromium,
            owner: BrowserControlOwner::Agent,
            control_epoch: 4,
            session_revision: 9,
            observation_revision: 12,
            expires_at_ms: 10_000,
            in_flight_inputs: 0,
            owner_agent_session_id: Some("agent-session-1".to_owned()),
            owner_principal_id: None,
        }
    }

    fn command(actor: ClaimedActor) -> BrowserCommand {
        BrowserCommand {
            request_id: "request-1".to_owned(),
            session_id: "session-1".to_owned(),
            tab_id: Some("tab-1".to_owned()),
            action: BrowserActionKind::Navigate,
            claimed_actor: actor,
            expected_epoch: 4,
            expected_session_revision: 9,
            expected_tab_generation: 2,
        }
    }

    #[test]
    fn the_agent_may_act_while_it_owns_the_lease() {
        let admitted = admit_action(
            &command(ClaimedActor::Agent),
            &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
            &lease(),
            1_000,
        )
        .expect("admitted");
        assert_eq!(admitted.actor, BrowserControlOwner::Agent);
        assert_eq!(admitted.execution_method, Some(BrowserExecutionMethod::Navigation));
    }

    #[test]
    fn a_claimed_actor_never_overrides_the_authenticated_principal() {
        // The request claims the user, but the lease belongs to the agent session.
        assert_eq!(
            admit_action(
                &command(ClaimedActor::User),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease(),
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::ActorMismatch
        );
        // A different agent session is not the lease owner either.
        assert_eq!(
            admit_action(
                &command(ClaimedActor::Agent),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-2"),
                &lease(),
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::NotTheLeaseOwner
        );
    }

    #[test]
    fn a_stale_epoch_is_discarded() {
        let mut command = command(ClaimedActor::Agent);
        command.expected_epoch = 3;
        assert_eq!(
            admit_action(
                &command,
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease(),
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::StaleEpoch
        );
    }

    #[test]
    fn a_stale_session_revision_is_discarded() {
        let mut command = command(ClaimedActor::Agent);
        command.expected_session_revision = 8;
        assert_eq!(
            admit_action(
                &command,
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease(),
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::StaleSessionRevision
        );
    }

    #[test]
    fn an_expired_lease_admits_nothing() {
        assert_eq!(
            admit_action(
                &command(ClaimedActor::Agent),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease(),
                10_000,
            )
            .unwrap_err(),
            BrowserControlError::LeaseExpired
        );
    }

    #[test]
    fn a_manual_only_mode_refuses_agent_input() {
        let mut lease = lease();
        lease.mode = BrowserMode::ManualOnly;
        assert_eq!(
            admit_action(
                &command(ClaimedActor::Agent),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease,
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::ModeDoesNotAllowAgentInput
        );
    }

    #[test]
    fn takeover_advances_the_epoch_and_moves_ownership_to_the_user() {
        let user = AuthenticatedPrincipal::user("principal-1");
        let taken = request_takeover(
            &command(ClaimedActor::User),
            &user,
            &lease(),
            1_000,
        )
        .expect("takeover");
        assert_eq!(taken.owner, BrowserControlOwner::User);
        assert_eq!(taken.control_epoch, 5);
        assert_eq!(taken.owner_principal_id.as_deref(), Some("principal-1"));
        assert_eq!(taken.owner_agent_session_id, None);
    }

    #[test]
    fn takeover_is_refused_while_provider_inputs_are_in_flight() {
        let mut lease = lease();
        lease.in_flight_inputs = 1;
        assert_eq!(
            request_takeover(
                &command(ClaimedActor::User),
                &AuthenticatedPrincipal::user("principal-1"),
                &lease,
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::InputsInFlight
        );
    }

    #[test]
    fn takeover_is_refused_to_a_non_user_principal() {
        assert_eq!(
            request_takeover(
                &command(ClaimedActor::Agent),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease(),
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::PrincipalNotPermitted
        );
    }

    #[test]
    fn a_queued_agent_action_is_discarded_after_takeover() {
        let user = AuthenticatedPrincipal::user("principal-1");
        let taken = request_takeover(
            &command(ClaimedActor::User),
            &user,
            &lease(),
            1_000,
        )
        .expect("takeover");
        // The queued agent command still carries the pre-takeover epoch.
        assert_eq!(
            admit_action(
                &command(ClaimedActor::Agent),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &taken,
                1_100,
            )
            .unwrap_err(),
            BrowserControlError::StaleEpoch
        );
    }

    #[test]
    fn return_to_agent_requires_a_fresh_observation_under_the_current_epoch() {
        let user = AuthenticatedPrincipal::user("principal-1");
        let taken = request_takeover(&command(ClaimedActor::User), &user, &lease(), 1_000)
            .expect("takeover");
        let mut return_command = command(ClaimedActor::User);
        return_command.expected_epoch = taken.control_epoch;

        let observation = BrowserObservation {
            session_id: "session-1".to_owned(),
            control_epoch: taken.control_epoch,
            observation_revision: taken.observation_revision + 1,
            captured_at_ms: 1_100,
            provider_identity: "provider-1".to_owned(),
        };
        let returned = return_to_agent(
            &return_command,
            &user,
            &taken,
            Some(&observation),
            1_200,
            5_000,
        )
        .expect("returned");
        assert_eq!(returned.owner, BrowserControlOwner::Agent);
        assert_eq!(returned.control_epoch, taken.control_epoch + 1);
        assert_eq!(returned.observation_revision, observation.observation_revision);
    }

    #[test]
    fn return_to_agent_refuses_a_stale_or_reused_observation() {
        let user = AuthenticatedPrincipal::user("principal-1");
        let taken = request_takeover(&command(ClaimedActor::User), &user, &lease(), 1_000)
            .expect("takeover");
        let mut return_command = command(ClaimedActor::User);
        return_command.expected_epoch = taken.control_epoch;

        assert_eq!(
            return_to_agent(
                &return_command,
                &user,
                &taken,
                None,
                1_200,
                5_000
            )
            .unwrap_err(),
            BrowserControlError::ObservationNotUsable
        );

        let stale_revision = BrowserObservation {
            session_id: "session-1".to_owned(),
            control_epoch: taken.control_epoch,
            observation_revision: taken.observation_revision,
            captured_at_ms: 1_100,
            provider_identity: "provider-1".to_owned(),
        };
        assert_eq!(
            return_to_agent(
                &return_command,
                &user,
                &taken,
                Some(&stale_revision),
                1_200,
                5_000
            )
            .unwrap_err(),
            BrowserControlError::ObservationRevisionNotAdvanced
        );

        let old_epoch = BrowserObservation {
            control_epoch: taken.control_epoch - 1,
            ..stale_revision.clone()
        };
        assert_eq!(
            return_to_agent(
                &return_command,
                &user,
                &taken,
                Some(&old_epoch),
                1_200,
                5_000
            )
            .unwrap_err(),
            BrowserControlError::ObservationNotUsable
        );
    }

    #[test]
    fn return_to_agent_refuses_an_over_age_observation() {
        let user = AuthenticatedPrincipal::user("principal-1");
        let taken = request_takeover(&command(ClaimedActor::User), &user, &lease(), 1_000)
            .expect("takeover");
        let mut return_command = command(ClaimedActor::User);
        return_command.expected_epoch = taken.control_epoch;
        let observation = BrowserObservation {
            session_id: "session-1".to_owned(),
            control_epoch: taken.control_epoch,
            observation_revision: taken.observation_revision + 1,
            captured_at_ms: 1_100,
            provider_identity: "provider-1".to_owned(),
        };
        assert_eq!(
            return_to_agent(
                &return_command,
                &user,
                &taken,
                Some(&observation),
                7_000,
                1_000
            )
            .unwrap_err(),
            BrowserControlError::ObservationNotUsable
        );
    }

    #[test]
    fn a_different_user_cannot_return_control_to_the_agent() {
        let owner = AuthenticatedPrincipal::user("principal-1");
        let taken = request_takeover(&command(ClaimedActor::User), &owner, &lease(), 1_000)
            .expect("takeover");
        let mut return_command = command(ClaimedActor::User);
        return_command.expected_epoch = taken.control_epoch;
        let observation = BrowserObservation {
            session_id: "session-1".to_owned(),
            control_epoch: taken.control_epoch,
            observation_revision: taken.observation_revision + 1,
            captured_at_ms: 1_100,
            provider_identity: "provider-1".to_owned(),
        };
        assert_eq!(
            return_to_agent(
                &return_command,
                &AuthenticatedPrincipal::user("principal-2"),
                &taken,
                Some(&observation),
                1_200,
                5_000
            )
            .unwrap_err(),
            BrowserControlError::NotTheLeaseOwner
        );
    }

    #[test]
    fn a_transfer_pending_lease_admits_nothing() {
        let mut lease = lease();
        lease.owner = BrowserControlOwner::TransferPending;
        assert_eq!(
            admit_action(
                &command(ClaimedActor::Agent),
                &AuthenticatedPrincipal::agent_session("principal-1", "agent-session-1"),
                &lease,
                1_000,
            )
            .unwrap_err(),
            BrowserControlError::TransferPending
        );
    }

    #[test]
    fn the_recorded_execution_method_is_exact_for_every_action() {
        // The recorded method must stay exact. Control-only actions address no page
        // element and therefore record no element method at all.
        assert_eq!(
            execution_method_for(BrowserActionKind::Navigate),
            Some(BrowserExecutionMethod::Navigation)
        );
        assert_eq!(
            execution_method_for(BrowserActionKind::Find),
            Some(BrowserExecutionMethod::DomElement)
        );
        assert_eq!(
            execution_method_for(BrowserActionKind::Capture),
            Some(BrowserExecutionMethod::AccessibilityNode)
        );
        assert_eq!(
            execution_method_for(BrowserActionKind::ApproveDownload),
            Some(BrowserExecutionMethod::Download)
        );
        for control_only in [
            BrowserActionKind::TakeControl,
            BrowserActionKind::ReturnControl,
            BrowserActionKind::Detach,
            BrowserActionKind::CloseSession,
        ] {
            assert_eq!(execution_method_for(control_only), None, "{control_only:?}");
        }
    }
}
