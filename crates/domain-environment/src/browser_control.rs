//! Pure browser input-control fencing decisions.
//!
//! These decisions must be evaluated and committed atomically by the Runtime's
//! EnvironmentControlLease owner before any provider input is dispatched. This module
//! does not persist a lease or authorize browser/site Effects by itself.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserControlActor {
    Agent { actor_id: String },
    User { actor_id: String },
}

impl BrowserControlActor {
    fn is_valid(&self) -> bool {
        match self {
            Self::Agent { actor_id } | Self::User { actor_id } => !actor_id.trim().is_empty(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserControlError {
    InvalidIdentity,
    InvalidEpoch,
    InvalidExpiry,
    Expired,
    TimeBeforeOwnerSince,
    StaleEpoch { expected: u64, actual: u64 },
    WrongOwner,
    ActionsInFlight,
    StaleObservation,
    ObservationIdentityMismatch,
    ObservationFromFuture,
    Pointer(super::browser_pointer::BrowserPointerError),
    EpochExhausted,
}

impl fmt::Display for BrowserControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BrowserControlError {}

/// Runtime-local projection used to make single-input-owner and epoch decisions.
/// The authoritative implementation must persist each returned value through the
/// EnvironmentControlLease transaction before acknowledging the control transfer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserControlLease {
    environment_id: String,
    task_id: String,
    attempt_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    agent_owner: Option<BrowserControlActor>,
    owner: BrowserControlActor,
    epoch: u64,
    owner_since_ms: u64,
    expires_at_ms: u64,
}

impl BrowserControlLease {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        environment_id: impl Into<String>,
        task_id: impl Into<String>,
        attempt_id: impl Into<String>,
        runtime_id: impl Into<String>,
        runtime_incarnation_id: impl Into<String>,
        owner: BrowserControlActor,
        epoch: u64,
        owner_since_ms: u64,
        expires_at_ms: u64,
    ) -> Result<Self, BrowserControlError> {
        let agent_owner = matches!(owner, BrowserControlActor::Agent { .. }).then(|| owner.clone());
        Self::new_with_agent_owner(
            environment_id,
            task_id,
            attempt_id,
            runtime_id,
            runtime_incarnation_id,
            owner,
            agent_owner,
            epoch,
            owner_since_ms,
            expires_at_ms,
        )
    }

    /// Rehydrates a durable lease after takeover while retaining its immutable original
    /// AgentSession owner. Without that binding, a restarted Runtime could not safely
    /// return USER control to the exact session that previously held it.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_agent_owner(
        environment_id: impl Into<String>,
        task_id: impl Into<String>,
        attempt_id: impl Into<String>,
        runtime_id: impl Into<String>,
        runtime_incarnation_id: impl Into<String>,
        owner: BrowserControlActor,
        agent_owner: Option<BrowserControlActor>,
        epoch: u64,
        owner_since_ms: u64,
        expires_at_ms: u64,
    ) -> Result<Self, BrowserControlError> {
        let environment_id = environment_id.into();
        let task_id = task_id.into();
        let attempt_id = attempt_id.into();
        let runtime_id = runtime_id.into();
        let runtime_incarnation_id = runtime_incarnation_id.into();
        if [
            environment_id.as_str(),
            task_id.as_str(),
            attempt_id.as_str(),
            runtime_id.as_str(),
            runtime_incarnation_id.as_str(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
            || !owner.is_valid()
        {
            return Err(BrowserControlError::InvalidIdentity);
        }
        if let Some(agent) = agent_owner.as_ref() {
            if !matches!(agent, BrowserControlActor::Agent { .. }) || !agent.is_valid() {
                return Err(BrowserControlError::InvalidIdentity);
            }
        } else {
            // A durable lease always belongs to an Attempt-bound AgentSession, even
            // while a human temporarily holds input. Without the immutable origin
            // owner, restart hydration could create a user lease that cannot be safely
            // returned to that exact session.
            return Err(BrowserControlError::WrongOwner);
        }
        if matches!(owner, BrowserControlActor::Agent { .. })
            && agent_owner.as_ref() != Some(&owner)
        {
            return Err(BrowserControlError::WrongOwner);
        }
        if epoch == 0 {
            return Err(BrowserControlError::InvalidEpoch);
        }
        if expires_at_ms <= owner_since_ms {
            return Err(BrowserControlError::InvalidExpiry);
        }
        Ok(Self {
            environment_id,
            task_id,
            attempt_id,
            runtime_id,
            runtime_incarnation_id,
            agent_owner,
            owner,
            epoch,
            owner_since_ms,
            expires_at_ms,
        })
    }

    pub fn owner(&self) -> &BrowserControlActor {
        &self.owner
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Rejects stale queued actions as well as wrong actors and expired leases.
    pub fn authorize_action(
        &self,
        actor: &BrowserControlActor,
        expected_epoch: u64,
        now_ms: u64,
    ) -> Result<(), BrowserControlError> {
        self.check_live_epoch(expected_epoch, now_ms)?;
        if actor != &self.owner {
            return Err(BrowserControlError::WrongOwner);
        }
        Ok(())
    }

    /// Checks the current control owner/epoch and maps a pointer against the frame
    /// displayed for that same epoch. This is a pre-dispatch admission check only: the
    /// serialized provider path must recheck the lease and persist the action/Effect
    /// immediately before sending input.
    #[allow(clippy::too_many_arguments)]
    pub fn map_pointer_for_current_owner(
        &self,
        actor: &BrowserControlActor,
        expected_epoch: u64,
        now_ms: u64,
        geometry: &super::browser_pointer::BrowserFrameGeometry,
        position: super::browser_pointer::PointerClientPosition,
        expected_session_id: &str,
        expected_tab_id: &str,
        expected_tab_generation: u64,
        expected_frame_sequence: u64,
        expected_geometry_revision: u64,
        max_frame_age_ms: u64,
    ) -> Result<super::browser_pointer::BrowserViewportPosition, BrowserControlError> {
        self.authorize_action(actor, expected_epoch, now_ms)?;
        super::browser_pointer::transform_browser_pointer(
            geometry,
            position,
            expected_session_id,
            expected_tab_id,
            expected_tab_generation,
            expected_epoch,
            expected_frame_sequence,
            expected_geometry_revision,
            now_ms,
            max_frame_age_ms,
        )
        .map_err(BrowserControlError::Pointer)
    }

    /// Transfers input to an authenticated user after old-owner actions have drained.
    /// The caller persists this returned epoch before accepting user input.
    pub fn take_control(
        &self,
        user: BrowserControlActor,
        expected_epoch: u64,
        now_ms: u64,
        actions_in_flight: usize,
    ) -> Result<Self, BrowserControlError> {
        self.check_live_epoch(expected_epoch, now_ms)?;
        if !matches!(self.owner, BrowserControlActor::Agent { .. })
            || !matches!(user, BrowserControlActor::User { .. })
            || !user.is_valid()
        {
            return Err(BrowserControlError::WrongOwner);
        }
        if actions_in_flight != 0 {
            return Err(BrowserControlError::ActionsInFlight);
        }
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or(BrowserControlError::EpochExhausted)?;
        let mut next = self.clone();
        next.owner = user;
        next.epoch = epoch;
        next.owner_since_ms = now_ms;
        Ok(next)
    }

    /// Returns control only after a fresh observation bound to this Environment,
    /// Runtime incarnation, and user epoch, with no old-owner actions in flight. The
    /// caller supplies the maximum age accepted by its observation policy.
    pub fn return_to_agent(
        &self,
        user: BrowserControlActor,
        agent: BrowserControlActor,
        expected_epoch: u64,
        now_ms: u64,
        actions_in_flight: usize,
        previous_observation_revision: u64,
        max_observation_age_ms: u64,
        observation: &BrowserObservation,
    ) -> Result<Self, BrowserControlError> {
        self.check_live_epoch(expected_epoch, now_ms)?;
        if self.owner != user
            || !matches!(user, BrowserControlActor::User { .. })
            || self.agent_owner.as_ref() != Some(&agent)
        {
            return Err(BrowserControlError::WrongOwner);
        }
        if actions_in_flight != 0 {
            return Err(BrowserControlError::ActionsInFlight);
        }
        if observation.environment_id != self.environment_id
            || observation.task_id != self.task_id
            || observation.attempt_id != self.attempt_id
            || observation.runtime_id != self.runtime_id
            || observation.runtime_incarnation_id != self.runtime_incarnation_id
        {
            return Err(BrowserControlError::ObservationIdentityMismatch);
        }
        if observation.control_epoch != self.epoch
            || observation.revision == 0
            || observation.revision <= previous_observation_revision
            // Millisecond timestamps cannot order two events sharing the same tick; a
            // capture at the takeover tick may predate the persisted ownership change.
            || observation.captured_at_ms <= self.owner_since_ms
        {
            return Err(BrowserControlError::StaleObservation);
        }
        if observation.captured_at_ms > now_ms {
            return Err(BrowserControlError::ObservationFromFuture);
        }
        if now_ms - observation.captured_at_ms > max_observation_age_ms {
            return Err(BrowserControlError::StaleObservation);
        }
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or(BrowserControlError::EpochExhausted)?;
        let mut next = self.clone();
        next.owner = agent;
        next.epoch = epoch;
        next.owner_since_ms = now_ms;
        Ok(next)
    }

    fn check_live_epoch(
        &self,
        expected_epoch: u64,
        now_ms: u64,
    ) -> Result<(), BrowserControlError> {
        if now_ms < self.owner_since_ms {
            return Err(BrowserControlError::TimeBeforeOwnerSince);
        }
        if now_ms >= self.expires_at_ms {
            return Err(BrowserControlError::Expired);
        }
        if self.epoch != expected_epoch {
            return Err(BrowserControlError::StaleEpoch {
                expected: self.epoch,
                actual: expected_epoch,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserObservation {
    environment_id: String,
    task_id: String,
    attempt_id: String,
    runtime_id: String,
    runtime_incarnation_id: String,
    control_epoch: u64,
    revision: u64,
    captured_at_ms: u64,
}

impl BrowserObservation {
    pub fn new(
        environment_id: impl Into<String>,
        task_id: impl Into<String>,
        attempt_id: impl Into<String>,
        runtime_id: impl Into<String>,
        runtime_incarnation_id: impl Into<String>,
        control_epoch: u64,
        revision: u64,
        captured_at_ms: u64,
    ) -> Result<Self, BrowserControlError> {
        let environment_id = environment_id.into();
        let task_id = task_id.into();
        let attempt_id = attempt_id.into();
        let runtime_id = runtime_id.into();
        let runtime_incarnation_id = runtime_incarnation_id.into();
        if [
            environment_id.as_str(),
            task_id.as_str(),
            attempt_id.as_str(),
            runtime_id.as_str(),
            runtime_incarnation_id.as_str(),
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(BrowserControlError::InvalidIdentity);
        }
        if control_epoch == 0 || revision == 0 {
            return Err(BrowserControlError::StaleObservation);
        }
        Ok(Self {
            environment_id,
            task_id,
            attempt_id,
            runtime_id,
            runtime_incarnation_id,
            control_epoch,
            revision,
            captured_at_ms,
        })
    }
}
