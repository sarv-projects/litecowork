//! Pure lifecycle decisions for durable Conversation turns.
//!
//! This module does not admit an AgentSession or dispatch to a provider. Callers must
//! establish Conversation/Workspace ownership and native-session eligibility before
//! applying a transition, then persist the returned value with its domain event.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PresentationPreference {
    Auto,
    Simple,
    Rich,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConversationTurnStatus {
    Open,
    Running,
    WaitingUser,
    WaitingDependency,
    Completed,
    Failed,
    CancelRequested,
    Cancelled,
}

impl ConversationTurnStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationTurn {
    pub turn_id: String,
    pub conversation_id: String,
    pub user_message_id: String,
    pub agent_session_id: Option<String>,
    pub status: ConversationTurnStatus,
    pub retry_ordinal: u32,
    pub presentation_preference: PresentationPreference,
    pub created_at: String,
    pub settled_at: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TurnCommand {
    Start { agent_session_id: String },
    WaitForUser,
    WaitForDependency,
    RequestAgain,
    Resume { agent_session_id: String },
    Complete { settled_at: String },
    Fail { settled_at: String },
    RequestCancel,
    SettleCancelled { settled_at: String },
    Retry { agent_session_id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TurnTransitionError {
    InvalidIdentity,
    InvalidVersion,
    StaleVersion,
    InvalidTransition,
    InvalidSessionId,
    SessionMustBeFresh,
    InvalidTimestamp,
    VersionExhausted,
    RetryOrdinalExhausted,
}

/// Apply one state transition using compare-and-swap version semantics.
///
/// The input is not mutated. A storage transaction must persist the returned value and
/// the corresponding event atomically; a successful pure decision is not a dispatch or
/// provider acknowledgement.
pub fn transition(
    current: &ConversationTurn,
    expected_version: u64,
    command: TurnCommand,
) -> Result<ConversationTurn, TurnTransitionError> {
    validate(current)?;
    if expected_version != current.version {
        return Err(TurnTransitionError::StaleVersion);
    }

    let mut next = current.clone();
    next.version = current
        .version
        .checked_add(1)
        .ok_or(TurnTransitionError::VersionExhausted)?;

    match command {
        TurnCommand::Start { agent_session_id } => {
            require_status(current, ConversationTurnStatus::Open)?;
            next.agent_session_id = Some(valid_new_session_id(current, agent_session_id)?);
            next.status = ConversationTurnStatus::Running;
        }
        TurnCommand::WaitForUser => {
            require_status(current, ConversationTurnStatus::Running)?;
            next.status = ConversationTurnStatus::WaitingUser;
        }
        TurnCommand::WaitForDependency => {
            require_status(current, ConversationTurnStatus::WaitingUser)?;
            next.status = ConversationTurnStatus::WaitingDependency;
        }
        TurnCommand::Resume { agent_session_id } => {
            require_status(current, ConversationTurnStatus::WaitingDependency)?;
            next.agent_session_id = Some(valid_new_session_id(current, agent_session_id)?);
            next.status = ConversationTurnStatus::Running;
        }
        TurnCommand::RequestAgain => {
            require_status(current, ConversationTurnStatus::WaitingDependency)?;
            next.status = ConversationTurnStatus::WaitingUser;
        }
        TurnCommand::Complete { settled_at } => {
            require_any_status(
                current,
                &[
                    ConversationTurnStatus::Running,
                    ConversationTurnStatus::CancelRequested,
                ],
            )?;
            next.status = ConversationTurnStatus::Completed;
            next.settled_at = Some(valid_timestamp(settled_at)?);
        }
        TurnCommand::Fail { settled_at } => {
            require_any_status(
                current,
                &[
                    ConversationTurnStatus::Open,
                    ConversationTurnStatus::Running,
                    ConversationTurnStatus::WaitingUser,
                    ConversationTurnStatus::WaitingDependency,
                    ConversationTurnStatus::CancelRequested,
                ],
            )?;
            next.status = ConversationTurnStatus::Failed;
            next.settled_at = Some(valid_timestamp(settled_at)?);
        }
        TurnCommand::RequestCancel => {
            require_any_status(
                current,
                &[
                    ConversationTurnStatus::Running,
                    ConversationTurnStatus::WaitingUser,
                    ConversationTurnStatus::WaitingDependency,
                ],
            )?;
            next.status = ConversationTurnStatus::CancelRequested;
        }
        TurnCommand::SettleCancelled { settled_at } => {
            require_status(current, ConversationTurnStatus::CancelRequested)?;
            next.status = ConversationTurnStatus::Cancelled;
            next.settled_at = Some(valid_timestamp(settled_at)?);
        }
        TurnCommand::Retry { agent_session_id } => {
            require_status(current, ConversationTurnStatus::Failed)?;
            next.retry_ordinal = current
                .retry_ordinal
                .checked_add(1)
                .ok_or(TurnTransitionError::RetryOrdinalExhausted)?;
            next.agent_session_id = Some(valid_new_session_id(current, agent_session_id)?);
            next.status = ConversationTurnStatus::Running;
            next.settled_at = None;
        }
    }

    validate(&next)?;
    Ok(next)
}

pub fn cancel_open(
    current: &ConversationTurn,
    expected_version: u64,
    settled_at: String,
) -> Result<ConversationTurn, TurnTransitionError> {
    validate(current)?;
    if expected_version != current.version {
        return Err(TurnTransitionError::StaleVersion);
    }
    require_status(current, ConversationTurnStatus::Open)?;
    let mut next = current.clone();
    next.version = current
        .version
        .checked_add(1)
        .ok_or(TurnTransitionError::VersionExhausted)?;
    next.status = ConversationTurnStatus::Cancelled;
    next.settled_at = Some(valid_timestamp(settled_at)?);
    validate(&next)?;
    Ok(next)
}

pub fn validate(turn: &ConversationTurn) -> Result<(), TurnTransitionError> {
    let valid_id = |value: &str| {
        !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
    };
    if !valid_id(&turn.turn_id)
        || !valid_id(&turn.conversation_id)
        || !valid_id(&turn.user_message_id)
        || turn.version == 0
        || turn
            .agent_session_id
            .as_ref()
            .is_some_and(|id| !valid_id(id))
        || turn.created_at.trim().is_empty()
    {
        return Err(TurnTransitionError::InvalidIdentity);
    }
    if turn.status.is_terminal() != turn.settled_at.is_some() {
        return Err(TurnTransitionError::InvalidTimestamp);
    }
    if let Some(timestamp) = &turn.settled_at
        && (timestamp.trim().is_empty() || timestamp.chars().any(char::is_control))
    {
        return Err(TurnTransitionError::InvalidTimestamp);
    }
    if turn.status == ConversationTurnStatus::Running && turn.agent_session_id.is_none() {
        return Err(TurnTransitionError::InvalidSessionId);
    }
    Ok(())
}

fn valid_session_id(value: String) -> Result<String, TurnTransitionError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err(TurnTransitionError::InvalidSessionId)
    } else {
        Ok(value)
    }
}

fn valid_new_session_id(
    current: &ConversationTurn,
    value: String,
) -> Result<String, TurnTransitionError> {
    let value = valid_session_id(value)?;
    if current.agent_session_id.as_deref() == Some(value.as_str()) {
        return Err(TurnTransitionError::SessionMustBeFresh);
    }
    Ok(value)
}

fn valid_timestamp(value: String) -> Result<String, TurnTransitionError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        Err(TurnTransitionError::InvalidTimestamp)
    } else {
        Ok(value)
    }
}

fn require_status(
    turn: &ConversationTurn,
    required: ConversationTurnStatus,
) -> Result<(), TurnTransitionError> {
    if turn.status == required {
        Ok(())
    } else {
        Err(TurnTransitionError::InvalidTransition)
    }
}

fn require_any_status(
    turn: &ConversationTurn,
    required: &[ConversationTurnStatus],
) -> Result<(), TurnTransitionError> {
    if required.contains(&turn.status) {
        Ok(())
    } else {
        Err(TurnTransitionError::InvalidTransition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(status: ConversationTurnStatus) -> ConversationTurn {
        ConversationTurn {
            turn_id: "turn-1".into(),
            conversation_id: "conversation-1".into(),
            user_message_id: "message-1".into(),
            agent_session_id: matches!(status, ConversationTurnStatus::Running)
                .then(|| "session-1".into()),
            status,
            retry_ordinal: 0,
            presentation_preference: PresentationPreference::Auto,
            created_at: "2026-10-09T00:00:00Z".into(),
            settled_at: status.is_terminal().then(|| "2026-10-09T00:01:00Z".into()),
            version: 1,
        }
    }

    #[test]
    fn state_machine_supports_wait_resume_and_retains_retry_preference() {
        let open = turn(ConversationTurnStatus::Open);
        let running = transition(
            &open,
            1,
            TurnCommand::Start {
                agent_session_id: "s1".into(),
            },
        )
        .unwrap();
        let waiting_user = transition(&running, 2, TurnCommand::WaitForUser).unwrap();
        let waiting_dependency =
            transition(&waiting_user, 3, TurnCommand::WaitForDependency).unwrap();
        let resumed = transition(
            &waiting_dependency,
            4,
            TurnCommand::Resume {
                agent_session_id: "s2".into(),
            },
        )
        .unwrap();
        assert_eq!(resumed.status, ConversationTurnStatus::Running);
        assert_eq!(resumed.agent_session_id.as_deref(), Some("s2"));
        assert_eq!(
            resumed.presentation_preference,
            PresentationPreference::Auto
        );
        assert_eq!(resumed.version, 5);
    }

    #[test]
    fn rejected_provider_input_returns_waiting_dependency_to_waiting_user() {
        let mut waiting = turn(ConversationTurnStatus::WaitingDependency);
        waiting.agent_session_id = Some("closed-session".into());
        let next = transition(&waiting, 1, TurnCommand::RequestAgain).unwrap();
        assert_eq!(next.status, ConversationTurnStatus::WaitingUser);
        assert_eq!(next.agent_session_id, Some("closed-session".into()));
        assert_eq!(next.version, 2);
    }

    #[test]
    fn resumption_and_retry_require_a_fresh_agent_session() {
        let mut waiting = turn(ConversationTurnStatus::WaitingDependency);
        waiting.agent_session_id = Some("closed-session".into());
        assert_eq!(
            transition(
                &waiting,
                1,
                TurnCommand::Resume {
                    agent_session_id: "closed-session".into()
                }
            ),
            Err(TurnTransitionError::SessionMustBeFresh)
        );

        let mut failed = turn(ConversationTurnStatus::Failed);
        failed.agent_session_id = Some("closed-session".into());
        assert_eq!(
            transition(
                &failed,
                1,
                TurnCommand::Retry {
                    agent_session_id: "closed-session".into()
                }
            ),
            Err(TurnTransitionError::SessionMustBeFresh)
        );
    }

    #[test]
    fn retry_is_explicit_fresh_session_and_preserves_message_and_preference() {
        let mut failed = turn(ConversationTurnStatus::Failed);
        failed.settled_at = Some("2026-10-09T00:01:00Z".into());
        failed.presentation_preference = PresentationPreference::Rich;
        let retried = transition(
            &failed,
            1,
            TurnCommand::Retry {
                agent_session_id: "session-2".into(),
            },
        )
        .unwrap();
        assert_eq!(retried.status, ConversationTurnStatus::Running);
        assert_eq!(retried.retry_ordinal, 1);
        assert_eq!(retried.user_message_id, "message-1");
        assert_eq!(
            retried.presentation_preference,
            PresentationPreference::Rich
        );
        assert_eq!(retried.settled_at, None);
    }

    #[test]
    fn stale_versions_and_implicit_retry_are_rejected() {
        let open = turn(ConversationTurnStatus::Open);
        assert_eq!(
            transition(
                &open,
                0,
                TurnCommand::Start {
                    agent_session_id: "s1".into()
                }
            ),
            Err(TurnTransitionError::StaleVersion)
        );
        assert_eq!(
            transition(
                &open,
                1,
                TurnCommand::Retry {
                    agent_session_id: "s1".into()
                }
            ),
            Err(TurnTransitionError::InvalidTransition)
        );
    }

    #[test]
    fn cancel_requires_provider_settlement_and_racing_completion_is_preserved() {
        let running = transition(
            &turn(ConversationTurnStatus::Open),
            1,
            TurnCommand::Start {
                agent_session_id: "s1".into(),
            },
        )
        .unwrap();
        let cancelling = transition(&running, 2, TurnCommand::RequestCancel).unwrap();
        assert_eq!(
            transition(
                &cancelling,
                3,
                TurnCommand::SettleCancelled {
                    settled_at: "2026-10-09T00:02:00Z".into()
                }
            )
            .unwrap()
            .status,
            ConversationTurnStatus::Cancelled
        );
        assert_eq!(
            transition(
                &cancelling,
                3,
                TurnCommand::Complete {
                    settled_at: "2026-10-09T00:02:00Z".into()
                }
            )
            .unwrap()
            .status,
            ConversationTurnStatus::Completed
        );
    }

    #[test]
    fn open_turn_can_cancel_before_dispatch() {
        let cancelled = cancel_open(
            &turn(ConversationTurnStatus::Open),
            1,
            "2026-10-09T00:01:00Z".into(),
        )
        .unwrap();
        assert_eq!(cancelled.status, ConversationTurnStatus::Cancelled);
    }

    #[test]
    fn malformed_running_and_terminal_records_are_rejected() {
        let mut invalid_running = turn(ConversationTurnStatus::Running);
        invalid_running.agent_session_id = None;
        assert_eq!(
            validate(&invalid_running),
            Err(TurnTransitionError::InvalidSessionId)
        );
        let mut invalid_terminal = turn(ConversationTurnStatus::Completed);
        invalid_terminal.settled_at = None;
        assert_eq!(
            validate(&invalid_terminal),
            Err(TurnTransitionError::InvalidTimestamp)
        );
    }
}
