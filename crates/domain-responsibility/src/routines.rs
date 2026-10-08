//! Saved, revisioned Routine definitions.
//!
//! A Routine is a passive work template. This boundary deliberately does not
//! schedule work, create Tasks, or execute triggers. Automation and Task adapters
//! may pin the immutable revision returned here.

use crate::{ContractObject, PlacementPreference, PrincipalKind, PrincipalRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoutineStatus {
    Active,
    Archived,
}

/// Schema owned elsewhere remains an object until the owning contract validator is
/// integrated. Keeping it typed as JSON prevents this domain from inventing a second
/// schema for outputs, bindings, capabilities, budgets, or verification policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineRevisionInput {
    pub objective_template: String,
    pub instructions: String,
    pub input_schema: ContractObject,
    pub constraints: Vec<String>,
    pub non_goals: Vec<String>,
    pub required_outputs: Vec<ContractObject>,
    pub acceptance_criteria: Vec<ContractObject>,
    pub approvals_required: Vec<ContractObject>,
    pub input_bindings: Vec<ContractObject>,
    pub required_capabilities: Vec<ContractObject>,
    pub preferred_agent_binding_id: Option<String>,
    pub placement_preference: PlacementPreference,
    pub budget_ceiling: Option<ContractObject>,
    pub verification_policy: ContractObject,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineRevision {
    pub routine_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub definition: RoutineRevisionInput,
    pub authored_by: PrincipalRef,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routine {
    pub routine_id: String,
    pub workspace_id: String,
    pub name: String,
    pub current_revision: u64,
    pub status: RoutineStatus,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineError {
    NotFound,
    AlreadyExists,
    Unauthorized,
    VersionConflict,
    VersionOverflow,
    RevisionOverflow,
    InvalidDefinition,
    Archived,
    ArchiveBlocked,
    IdempotencyConflict,
    WorkspaceArchived,
    Storage,
}
impl std::fmt::Display for RoutineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RoutineError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineOwnerScope {
    /// Transport-authenticated principal; the store rechecks Workspace ownership.
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum RoutineCommand {
    Create {
        routine_id: String,
        name: String,
        revision: RoutineRevisionInput,
    },
    Revise {
        routine_id: String,
        expected_version: u64,
        revision: RoutineRevisionInput,
    },
    Archive {
        routine_id: String,
        expected_version: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineEvent {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineMutation {
    pub routine: Routine,
    pub expected_version: Option<u64>,
    pub append_revision: Option<RoutineRevision>,
    pub event: RoutineEvent,
}

pub trait RoutineTransaction {
    fn now(&self) -> String;
    fn routine(&mut self, id: &str) -> Result<Option<(Routine, RoutineRevision)>, RoutineError>;
    fn validate_references(
        &mut self,
        workspace_id: &str,
        revision: &RoutineRevisionInput,
    ) -> Result<(), RoutineError>;
    fn has_enabled_automation_references(&mut self, routine_id: &str) -> Result<bool, RoutineError>;
    fn commit(&mut self, mutation: RoutineMutation) -> Result<Routine, RoutineError>;
}

pub trait RoutineStore {
    /// Authorize before replay, bind RequestId to the full command fingerprint, and
    /// atomically persist the head, immutable revision, event, snapshot, and receipt.
    fn transaction<F>(
        &mut self,
        scope: &RoutineOwnerScope,
        fingerprint: &str,
        operation: F,
    ) -> Result<Routine, RoutineError>
    where
        F: Fn(&mut dyn RoutineTransaction) -> Result<Routine, RoutineError> + Send + 'static;
}

pub struct RoutineService<S> {
    store: S,
}
impl<S: RoutineStore> RoutineService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn into_store(self) -> S {
        self.store
    }

    pub fn execute(
        &mut self,
        scope: &RoutineOwnerScope,
        command: RoutineCommand,
    ) -> Result<Routine, RoutineError> {
        if [&scope.principal_id, &scope.workspace_id, &scope.request_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(RoutineError::Unauthorized);
        }
        let fingerprint = command_fingerprint(&command)?;
        let owned_scope = scope.clone();
        self.store.transaction(scope, &fingerprint, move |tx| {
            decide(tx, &owned_scope, command.clone())
        })
    }
}

pub fn validate_routine_revision(input: &RoutineRevisionInput) -> Result<(), RoutineError> {
    fn valid_text(value: &str, max: usize) -> bool {
        !value.trim().is_empty()
            && value.chars().count() <= max
            && !value.chars().any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    }
    if !valid_text(&input.objective_template, 4000)
        || !valid_text(&input.instructions, 32_000)
        || input.preferred_agent_binding_id.as_ref().is_some_and(|id| {
            id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
        })
        || input.constraints.len() > 100
        || input.non_goals.len() > 100
        || input.constraints.iter().chain(&input.non_goals).any(|s| !valid_text(s, 2000))
    {
        return Err(RoutineError::InvalidDefinition);
    }
    // Ensure values are finite and serializable without attempting to validate schemas
    // owned by other contracts here.
    canonical(&serde_json::to_value(input).map_err(|_| RoutineError::InvalidDefinition)?)?;
    Ok(())
}

fn command_fingerprint(command: &RoutineCommand) -> Result<String, RoutineError> {
    let bytes = canonical(&serde_json::to_value(command).map_err(|_| RoutineError::InvalidDefinition)?)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn canonical(value: &serde_json::Value) -> Result<Vec<u8>, RoutineError> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| RoutineError::InvalidDefinition)
}

fn decide(
    tx: &mut dyn RoutineTransaction,
    scope: &RoutineOwnerScope,
    command: RoutineCommand,
) -> Result<Routine, RoutineError> {
    let now = tx.now();
    let author = PrincipalRef {
        principal_id: scope.principal_id.clone(),
        kind: PrincipalKind::User,
    };
    match command {
        RoutineCommand::Create { routine_id, name, revision } => {
            if !valid_identity(&routine_id) || !valid_name(&name) {
                return Err(RoutineError::InvalidDefinition);
            }
            if tx.routine(&routine_id)?.is_some() {
                return Err(RoutineError::AlreadyExists);
            }
            validate_routine_revision(&revision)?;
            tx.validate_references(&scope.workspace_id, &revision)?;
            let routine = Routine {
                routine_id: routine_id.clone(),
                workspace_id: scope.workspace_id.clone(),
                name,
                current_revision: 1,
                status: RoutineStatus::Active,
                created_at: now.clone(),
                updated_at: now.clone(),
                version: 1,
            };
            let append_revision = RoutineRevision {
                routine_id: routine_id.clone(),
                revision: 1,
                definition: revision,
                authored_by: author,
                created_at: now,
            };
            let event = RoutineEvent {
                kind: "routine.created.v1".into(),
                payload: serde_json::json!({
                    "routine_id": routine_id,
                    "current_revision": 1,
                    "status": "ACTIVE",
                    "aggregate_version": 1
                }),
            };
            tx.commit(RoutineMutation {
                routine,
                expected_version: None,
                append_revision: Some(append_revision),
                event,
            })
        }
        RoutineCommand::Revise { routine_id, expected_version, revision } => {
            let (current, _) = tx.routine(&routine_id)?.ok_or(RoutineError::NotFound)?;
            if current.status == RoutineStatus::Archived {
                return Err(RoutineError::Archived);
            }
            if current.version != expected_version {
                return Err(RoutineError::VersionConflict);
            }
            validate_routine_revision(&revision)?;
            tx.validate_references(&scope.workspace_id, &revision)?;
            let next_version = expected_version.checked_add(1).ok_or(RoutineError::VersionOverflow)?;
            let next_revision = current.current_revision.checked_add(1).ok_or(RoutineError::RevisionOverflow)?;
            let routine = Routine {
                current_revision: next_revision,
                updated_at: now.clone(),
                version: next_version,
                ..current
            };
            let append_revision = RoutineRevision {
                routine_id: routine_id.clone(),
                revision: next_revision,
                definition: revision,
                authored_by: author,
                created_at: now,
            };
            let digest = revision_digest(&append_revision)?;
            let event = RoutineEvent {
                kind: "routine.revision.created.v1".into(),
                payload: serde_json::json!({
                    "routine_id": routine_id,
                    "revision": next_revision,
                    "definition_digest": digest,
                    "authored_by": append_revision.authored_by
                }),
            };
            tx.commit(RoutineMutation {
                routine,
                expected_version: Some(expected_version),
                append_revision: Some(append_revision),
                event,
            })
        }
        RoutineCommand::Archive { routine_id, expected_version } => {
            let (current, _) = tx.routine(&routine_id)?.ok_or(RoutineError::NotFound)?;
            if current.status == RoutineStatus::Archived {
                return Err(RoutineError::Archived);
            }
            if current.version != expected_version {
                return Err(RoutineError::VersionConflict);
            }
            if tx.has_enabled_automation_references(&routine_id)? {
                return Err(RoutineError::ArchiveBlocked);
            }
            let next_version = expected_version.checked_add(1).ok_or(RoutineError::VersionOverflow)?;
            let routine = Routine {
                status: RoutineStatus::Archived,
                updated_at: now,
                version: next_version,
                ..current
            };
            let event = RoutineEvent {
                kind: "routine.status.changed.v1".into(),
                payload: serde_json::json!({
                    "routine_id": routine_id,
                    "from": "ACTIVE",
                    "to": "ARCHIVED",
                    "aggregate_version": next_version
                }),
            };
            tx.commit(RoutineMutation {
                routine,
                expected_version: Some(expected_version),
                append_revision: None,
                event,
            })
        }
    }
}

fn revision_digest(revision: &RoutineRevision) -> Result<String, RoutineError> {
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(canonical(
        &serde_json::to_value(revision).map_err(|_| RoutineError::InvalidDefinition)?,
    )?))))
}

fn valid_identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn valid_name(value: &str) -> bool {
    valid_identity(value) && value.chars().count() <= 120
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn definition(objective: &str) -> RoutineRevisionInput {
        RoutineRevisionInput {
            objective_template: objective.into(),
            instructions: "Review the supplied project and return findings.".into(),
            input_schema: BTreeMap::new(),
            constraints: vec!["Do not execute the workflow automatically".into()],
            non_goals: vec![],
            required_outputs: vec![BTreeMap::from([("kind".into(), json!("REPORT"))])],
            acceptance_criteria: vec![],
            approvals_required: vec![],
            input_bindings: vec![],
            required_capabilities: vec![],
            preferred_agent_binding_id: None,
            placement_preference: PlacementPreference::Class(crate::PlacementClass::Auto),
            budget_ceiling: None,
            verification_policy: BTreeMap::new(),
        }
    }

    #[test]
    fn routine_revision_input_round_trips_and_rejects_blank_objective() {
        let input = definition("Review repository");
        let encoded = serde_json::to_vec(&input).unwrap();
        let decoded: RoutineRevisionInput = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, input);
        assert_eq!(validate_routine_revision(&input), Ok(()));
        assert_eq!(validate_routine_revision(&definition("  ")), Err(RoutineError::InvalidDefinition));
    }

    #[test]
    fn command_fingerprint_is_stable_for_equal_commands() {
        let a = RoutineCommand::Create { routine_id: "routine-1".into(), name: "Review".into(), revision: definition("Review repository") };
        let b = a.clone();
        assert_eq!(command_fingerprint(&a).unwrap(), command_fingerprint(&b).unwrap());
    }
}
