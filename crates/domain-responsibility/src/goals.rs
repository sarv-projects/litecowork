//! Passive, revisioned Goal domain boundary.
//!
//! Goals record user intent and links to existing work. They never schedule Tasks,
//! choose agents, or grant authority. The storage adapter must recheck same-Workspace
//! Task/Routine references and atomically persist the aggregate, immutable revision,
//! event, and idempotency receipt.

use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::PrincipalRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineRevisionRef {
    pub routine_id: String,
    pub revision: u64,
}

/// Pins one immutable Artifact version from the same Workspace as its Goal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactVersionRef {
    pub workspace_id: String,
    pub artifact_id: String,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalRevisionInput {
    pub objective: String,
    pub success_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub horizon: Option<String>,
    pub related_task_ids: Vec<String>,
    pub related_routine_refs: Vec<RoutineRevisionRef>,
    #[serde(default)]
    pub related_artifact_refs: Vec<ArtifactVersionRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalRevision {
    pub goal_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub definition: GoalRevisionInput,
    pub authored_by: PrincipalRef,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub goal_id: String,
    pub workspace_id: String,
    pub coworker_id: Option<String>,
    pub current_revision: u64,
    pub status: GoalStatus,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalTaskOutcomeState {
    Verified,
    Incomplete,
    Unverified,
    Stale,
    Conflicted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalProgressAvailability {
    Complete,
    Partial,
}

/// Data needed to prove the full projection is not yet exposed by the local read model.
/// These codes make unknown dimensions explicit instead of presenting them as zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalProgressLimitation {
    VerificationRunReadModelUnavailable,
    TaskDependencyFreshnessUnavailable,
    ArtifactDependencyFreshnessUnavailable,
    ArtifactEvidenceReferenceUnresolved,
    EvidenceListTruncated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalArtifactEvidenceRefs {
    pub artifact_id: String,
    pub version: u64,
    /// Only IDs that resolve to committed Evidence in this Workspace are returned.
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalTaskContribution {
    pub task_id: String,
    /// Canonical Task status copied from the Task projection.
    pub task_status: String,
    pub outcome_state: GoalTaskOutcomeState,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalProgressProjection {
    pub computed_at: String,
    pub availability: GoalProgressAvailability,
    pub limitations: Vec<GoalProgressLimitation>,
    /// Null means this count cannot be established from the currently integrated readers.
    pub verified_task_count: Option<u64>,
    pub linked_task_count: u64,
    pub stale_source_count: Option<u64>,
    pub conflicted_source_count: Option<u64>,
    pub contributions: Vec<GoalTaskContribution>,
    pub artifact_evidence_refs: Vec<GoalArtifactEvidenceRefs>,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalError {
    NotFound,
    AlreadyExists,
    Unauthorized,
    VersionConflict,
    VersionOverflow,
    RevisionOverflow,
    InvalidDefinition,
    Archived,
    InvalidTransition,
    ReferenceUnavailable,
    WorkspaceArchived,
    IdempotencyConflict,
    Storage,
}
impl std::fmt::Display for GoalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for GoalError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalOwnerScope {
    /// Transport-authenticated principal; the adapter rechecks Workspace ownership.
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "command",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum GoalCommand {
    Create {
        goal_id: String,
        coworker_id: Option<String>,
        revision: GoalRevisionInput,
    },
    Revise {
        goal_id: String,
        expected_version: u64,
        revision: GoalRevisionInput,
    },
    SetStatus {
        goal_id: String,
        expected_version: u64,
        status: GoalStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalEvent {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalMutation {
    pub goal: Goal,
    pub expected_version: Option<u64>,
    pub append_revision: Option<GoalRevision>,
    pub event: GoalEvent,
}

/// A Goal progress projection must be computed from accepted Task/Evidence state by a
/// separate read-side projector. It must not be supplied by an agent or included in a
/// Goal mutation.
pub trait GoalTransaction {
    fn now(&self) -> String;
    fn goal(&mut self, goal_id: &str) -> Result<Option<(Goal, GoalRevision)>, GoalError>;
    fn validate_references(
        &mut self,
        workspace_id: &str,
        coworker_id: Option<&str>,
        revision: &GoalRevisionInput,
    ) -> Result<(), GoalError>;
    fn commit(&mut self, mutation: GoalMutation) -> Result<Goal, GoalError>;
}

pub trait GoalStore {
    /// Implementations re-authorize before replay, compare the fingerprint bound to
    /// RequestId, re-run the read-only decision against fresh transaction state, and
    /// atomically persist mutation, revision, event, and receipt.
    fn transaction<F>(
        &mut self,
        scope: &GoalOwnerScope,
        fingerprint: &str,
        operation: F,
    ) -> Result<Goal, GoalError>
    where
        F: Fn(&mut dyn GoalTransaction) -> Result<Goal, GoalError> + Send + 'static;
}

pub struct GoalService<S> {
    store: S,
}
impl<S: GoalStore> GoalService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    pub fn into_store(self) -> S {
        self.store
    }
    pub fn execute(
        &mut self,
        scope: &GoalOwnerScope,
        command: GoalCommand,
    ) -> Result<Goal, GoalError> {
        if scope.principal_id.trim().is_empty()
            || scope.workspace_id.trim().is_empty()
            || scope.request_id.trim().is_empty()
        {
            return Err(GoalError::Unauthorized);
        }
        let fingerprint = fingerprint(&command)?;
        let scope = scope.clone();
        let transaction_scope = scope.clone();
        self.store.transaction(&scope, &fingerprint, move |tx| {
            decide(tx, &transaction_scope, command.clone())
        })
    }
}

pub fn validate_goal_revision(input: &GoalRevisionInput) -> Result<(), GoalError> {
    fn reference(value: &str) -> bool {
        !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
    }
    fn strings(values: &[String], max_items: usize, max_chars: usize) -> bool {
        values.len() <= max_items
            && values.iter().all(|value| {
                !value.trim().is_empty()
                    && value.chars().count() <= max_chars
                    && !value.chars().any(char::is_control)
            })
    }
    if input.objective.trim().is_empty()
        || input.objective.chars().count() > 4000
        || input.objective.chars().any(char::is_control)
        || !strings(&input.success_criteria, 100, 2000)
        || !strings(&input.constraints, 100, 2000)
        || input.horizon.as_ref().is_some_and(|value| {
            value.is_empty() || value.len() > 64 || value.chars().any(char::is_control)
        })
        || input.related_task_ids.len() > 500
        || input.related_routine_refs.len() > 500
        || input.related_artifact_refs.len() > 500
        || input.related_task_ids.iter().any(|id| !reference(id))
        || input
            .related_routine_refs
            .iter()
            .any(|routine_ref| !reference(&routine_ref.routine_id) || routine_ref.revision == 0)
        || input.related_artifact_refs.iter().any(|artifact_ref| {
            !reference(&artifact_ref.workspace_id)
                || !reference(&artifact_ref.artifact_id)
                || artifact_ref.version == 0
        })
    {
        return Err(GoalError::InvalidDefinition);
    }
    let tasks: std::collections::HashSet<_> = input.related_task_ids.iter().collect();
    let routines: std::collections::HashSet<_> = input
        .related_routine_refs
        .iter()
        .map(|reference| (&reference.routine_id, reference.revision))
        .collect();
    let artifacts: std::collections::HashSet<_> = input
        .related_artifact_refs
        .iter()
        .map(|reference| (&reference.workspace_id, &reference.artifact_id))
        .collect();
    if tasks.len() != input.related_task_ids.len()
        || routines.len() != input.related_routine_refs.len()
        || artifacts.len() != input.related_artifact_refs.len()
    {
        return Err(GoalError::InvalidDefinition);
    }
    Ok(())
}

pub fn next_goal_version(actual: u64, expected: u64) -> Result<u64, GoalError> {
    if actual != expected {
        return Err(GoalError::VersionConflict);
    }
    actual.checked_add(1).ok_or(GoalError::VersionOverflow)
}

pub fn check_goal_transition(from: GoalStatus, to: GoalStatus) -> Result<(), GoalError> {
    use GoalStatus::*;
    let allowed = matches!(
        (from, to),
        (Active, Paused)
            | (Paused, Active)
            | (Active, Completed)
            | (Paused, Completed)
            | (Completed, Active)
            | (Active, Archived)
            | (Paused, Archived)
            | (Completed, Archived)
    );
    if from == Archived {
        Err(GoalError::Archived)
    } else if !allowed {
        Err(GoalError::InvalidTransition)
    } else {
        Ok(())
    }
}

fn decide(
    tx: &mut dyn GoalTransaction,
    scope: &GoalOwnerScope,
    command: GoalCommand,
) -> Result<Goal, GoalError> {
    let now = tx.now();
    if now.trim().is_empty() {
        return Err(GoalError::Storage);
    }
    let author = PrincipalRef {
        principal_id: scope.principal_id.clone(),
        kind: crate::PrincipalKind::User,
    };
    match command {
        GoalCommand::Create {
            goal_id,
            coworker_id,
            revision,
        } => {
            if goal_id.trim().is_empty()
                || goal_id.len() > 256
                || goal_id.chars().any(char::is_control)
                || coworker_id.as_ref().is_some_and(|id| {
                    id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
                })
            {
                return Err(GoalError::InvalidDefinition);
            }
            if tx.goal(&goal_id)?.is_some() {
                return Err(GoalError::AlreadyExists);
            }
            validate_goal_revision(&revision)?;
            tx.validate_references(&scope.workspace_id, coworker_id.as_deref(), &revision)?;
            let goal = Goal {
                goal_id: goal_id.clone(),
                workspace_id: scope.workspace_id.clone(),
                coworker_id,
                current_revision: 1,
                status: GoalStatus::Active,
                created_at: now.clone(),
                updated_at: now.clone(),
                version: 1,
            };
            let revision = GoalRevision {
                goal_id: goal_id.clone(),
                revision: 1,
                definition: revision,
                authored_by: author,
                created_at: now,
            };
            tx.commit(GoalMutation {
                goal,
                expected_version: None,
                append_revision: Some(revision),
                event: GoalEvent {
                    kind: "goal.created.v1".to_owned(),
                    payload: serde_json::json!({"goal_id": goal_id, "workspace_id": scope.workspace_id, "current_revision": 1, "status": "ACTIVE", "aggregate_version": 1}),
                },
            })
        }
        GoalCommand::Revise {
            goal_id,
            expected_version,
            revision: input,
        } => {
            let (mut goal, _) = tx.goal(&goal_id)?.ok_or(GoalError::NotFound)?;
            goal.version = next_goal_version(goal.version, expected_version)?;
            if goal.status == GoalStatus::Archived {
                return Err(GoalError::Archived);
            }
            validate_goal_revision(&input)?;
            tx.validate_references(&scope.workspace_id, goal.coworker_id.as_deref(), &input)?;
            goal.current_revision = goal
                .current_revision
                .checked_add(1)
                .ok_or(GoalError::RevisionOverflow)?;
            goal.updated_at = now.clone();
            let revision = GoalRevision {
                goal_id: goal_id.clone(),
                revision: goal.current_revision,
                definition: input,
                authored_by: author,
                created_at: now,
            };
            let digest = serde_json_canonicalizer::to_vec(&revision)
                .map(|bytes| format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))))
                .map_err(|_| GoalError::InvalidDefinition)?;
            let aggregate_version = goal.version;
            tx.commit(GoalMutation {
                goal,
                expected_version: Some(expected_version),
                append_revision: Some(revision.clone()),
                event: GoalEvent {
                    kind: "goal.revised.v1".to_owned(),
                    payload: serde_json::json!({"goal_id": goal_id, "revision": revision.revision, "revision_digest": digest, "authored_by": revision.authored_by, "aggregate_version": aggregate_version}),
                },
            })
        }
        GoalCommand::SetStatus {
            goal_id,
            expected_version,
            status,
        } => {
            let (mut goal, _) = tx.goal(&goal_id)?.ok_or(GoalError::NotFound)?;
            goal.version = next_goal_version(goal.version, expected_version)?;
            check_goal_transition(goal.status, status)?;
            let prior_status = goal.status;
            goal.status = status;
            goal.updated_at = now;
            tx.commit(GoalMutation {
                goal: goal.clone(),
                expected_version: Some(expected_version),
                append_revision: None,
                event: GoalEvent {
                    kind: "goal.status.changed.v1".to_owned(),
                    payload: serde_json::json!({"goal_id": goal_id, "from": prior_status, "to": status, "aggregate_version": goal.version}),
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revision() -> GoalRevisionInput {
        GoalRevisionInput {
            objective: "Ship the local desktop release".to_owned(),
            success_criteria: vec!["Owner can complete a verified local task".to_owned()],
            constraints: vec!["Local execution only".to_owned()],
            horizon: None,
            related_task_ids: vec!["task-1".to_owned()],
            related_routine_refs: vec![RoutineRevisionRef {
                routine_id: "routine-1".to_owned(),
                revision: 2,
            }],
            related_artifact_refs: vec![],
        }
    }

    #[test]
    fn goal_revision_validation_rejects_duplicate_or_unbounded_references() {
        assert_eq!(validate_goal_revision(&revision()), Ok(()));
        let mut duplicate = revision();
        duplicate.related_task_ids.push("task-1".to_owned());
        assert_eq!(
            validate_goal_revision(&duplicate),
            Err(GoalError::InvalidDefinition)
        );
        let mut invalid_revision = revision();
        invalid_revision.related_routine_refs[0].revision = 0;
        assert_eq!(
            validate_goal_revision(&invalid_revision),
            Err(GoalError::InvalidDefinition)
        );
        let mut invalid_text = revision();
        invalid_text.objective = "\nobjective".to_owned();
        assert_eq!(
            validate_goal_revision(&invalid_text),
            Err(GoalError::InvalidDefinition)
        );
    }

    #[test]
    fn goal_status_transitions_are_explicit_and_archive_is_terminal() {
        assert_eq!(
            check_goal_transition(GoalStatus::Active, GoalStatus::Paused),
            Ok(())
        );
        assert_eq!(
            check_goal_transition(GoalStatus::Completed, GoalStatus::Active),
            Ok(())
        );
        assert_eq!(
            check_goal_transition(GoalStatus::Archived, GoalStatus::Active),
            Err(GoalError::Archived)
        );
        assert_eq!(
            check_goal_transition(GoalStatus::Paused, GoalStatus::Paused),
            Err(GoalError::InvalidTransition)
        );
    }

    #[test]
    fn goal_version_uses_compare_and_swap_and_rejects_overflow() {
        assert_eq!(next_goal_version(4, 4), Ok(5));
        assert_eq!(next_goal_version(4, 3), Err(GoalError::VersionConflict));
        assert_eq!(
            next_goal_version(u64::MAX, u64::MAX),
            Err(GoalError::VersionOverflow)
        );
    }
}

fn fingerprint(command: &GoalCommand) -> Result<String, GoalError> {
    let bytes =
        serde_json_canonicalizer::to_vec(command).map_err(|_| GoalError::InvalidDefinition)?;
    Ok(format!(
        "sha256:{}",
        hex::encode(sha2::Sha256::digest(bytes))
    ))
}
