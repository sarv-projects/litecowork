//! Cause-specific safe-boundary evaluation for a Task lead handoff.
//!
//! The two causes have different boundaries, and the contract says so:
//!
//! * `OWNER_REQUEST` is an owner-initiated reassignment of a Task that has not started
//!   work, so it requires a settled Task: no accepted Plan, no live planning session,
//!   no active Attempt, and no unresolved Effect.
//! * `POLICY_FAILOVER` is a contract-driven change made while the Task is already
//!   running. `TASK-RUNTIME.md` requires it to settle or safely checkpoint the prior
//!   planning session and reconcile Invocations/Effects while **preserving active
//!   Attempts under their original bindings/leases**. An active Attempt therefore does
//!   not block an automatic failover; it keeps its own AgentBinding and ExecutionLease
//!   because `attempts.agent_binding_id` is Attempt-scoped.
//!
//! A live `TASK_PLANNING` session and an unresolved Effect still block both causes: the
//! prior planning session must be settled first, and `DELEGATION.md` says an ambiguous
//! provider state or an active unsafe Effect blocks the change rather than bypassing
//! recovery.

use rusqlite::{Connection, params};

/// Which command caused the handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LeadHandoffCause {
    OwnerRequest,
    PolicyFailover,
}

/// Validate the cause-specific safe boundary inside the handoff transaction.
///
/// The caller has already fenced the Task version, TaskSpec revision, and Workspace.
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_lead_handoff_boundary(
    connection: &Connection,
    workspace_id: &str,
    task_id: &str,
    task_status: &str,
    current_plan_revision: Option<u64>,
    completed_at: Option<&str>,
    current_lead_agent_binding_id: &str,
    target_agent_binding_id: &str,
    cause: LeadHandoffCause,
) -> Result<(), StoreError> {
    if completed_at.is_some() || current_lead_agent_binding_id == target_agent_binding_id {
        return Err(StoreError::Invalid(
            "Task is outside the safe lead handoff boundary".to_owned(),
        ));
    }
    let live_planning: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE workspace_id = ?1 AND task_id = ?2 AND scope_kind = 'TASK_PLANNING' AND status IN ('STARTING', 'ACTIVE', 'INTERRUPTING', 'CLOSING'))",
            params![workspace_id, task_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let unresolved_effect: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM effects WHERE task_id = ?1 AND state NOT IN ('OBSERVED', 'VERIFIED', 'FAILED'))",
            params![task_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if live_planning || unresolved_effect {
        return Err(StoreError::Invalid(
            "Task lead handoff requires settled planning and reconciled Effects".to_owned(),
        ));
    }
    match cause {
        LeadHandoffCause::OwnerRequest => {
            let active_attempt: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM attempts WHERE task_id = ?1 AND status NOT IN ('COMPLETED', 'FAILED', 'ABANDONED', 'CANCELLED'))",
                    params![task_id],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if task_status != "READY" || current_plan_revision.is_some() || active_attempt {
                return Err(StoreError::Invalid(
                    "Task lead handoff requires settled planning, Attempts, and Effects".to_owned(),
                ));
            }
        }
        LeadHandoffCause::PolicyFailover => {
            // An automatic failover may replace the lead of a Task that is already
            // executing. Active Attempts keep their own AgentBinding and ExecutionLease;
            // they are preserved rather than cancelled or reparented.
            if !matches!(task_status, "READY" | "RUNNING") {
                return Err(StoreError::Invalid(
                    "Task is outside the safe lead failover boundary".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

use super::{StoreError, map_database_error};

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE agent_sessions(workspace_id TEXT, task_id TEXT, scope_kind TEXT, status TEXT);
                 CREATE TABLE attempts(task_id TEXT, status TEXT);
                 CREATE TABLE effects(task_id TEXT, state TEXT);
                 INSERT INTO agent_sessions VALUES ('workspace-a','task-a','TASK_PLANNING','CLOSED');
                 INSERT INTO attempts VALUES ('task-a','RUNNING');
                 INSERT INTO effects VALUES ('task-a','VERIFIED');",
            )
            .expect("create boundary fixture");
        connection
    }

    fn check(
        connection: &Connection,
        status: &str,
        plan: Option<u64>,
        cause: LeadHandoffCause,
    ) -> Result<(), StoreError> {
        validate_lead_handoff_boundary(
            connection,
            "workspace-a",
            "task-a",
            status,
            plan,
            None,
            "binding-old",
            "binding-new",
            cause,
        )
    }

    #[test]
    fn owner_request_requires_a_settled_task() {
        let connection = connection();
        assert!(check(&connection, "READY", None, LeadHandoffCause::OwnerRequest).is_err());
        assert!(
            check(
                &connection,
                "READY",
                Some(1),
                LeadHandoffCause::OwnerRequest
            )
            .is_err()
        );
    }

    #[test]
    fn policy_failover_preserves_active_attempts_while_the_task_runs() {
        let connection = connection();
        // The fixture has a RUNNING Attempt. An automatic failover preserves it.
        assert!(
            check(
                &connection,
                "RUNNING",
                Some(1),
                LeadHandoffCause::PolicyFailover
            )
            .is_ok()
        );
        assert!(check(&connection, "READY", None, LeadHandoffCause::PolicyFailover).is_ok());
        // A Task that is no longer making progress is still outside the boundary.
        assert!(
            check(
                &connection,
                "VERIFYING",
                Some(1),
                LeadHandoffCause::PolicyFailover
            )
            .is_err()
        );
    }

    #[test]
    fn a_live_planner_or_unresolved_effect_blocks_both_causes() {
        let connection = connection();
        connection
            .execute(
                "INSERT INTO agent_sessions VALUES ('workspace-a','task-b','TASK_PLANNING','ACTIVE')",
                [],
            )
            .expect("insert live planner");
        connection
            .execute("UPDATE attempts SET task_id = 'task-b'", [])
            .expect("move attempt");
        connection
            .execute("UPDATE effects SET task_id = 'task-b'", [])
            .expect("move effect");
        connection
            .execute("INSERT INTO effects VALUES ('task-c','PROPOSED')", [])
            .expect("insert unresolved effect");
        assert!(
            validate_lead_handoff_boundary(
                &connection,
                "workspace-a",
                "task-b",
                "RUNNING",
                Some(1),
                None,
                "binding-old",
                "binding-new",
                LeadHandoffCause::PolicyFailover,
            )
            .is_err()
        );
        assert!(
            validate_lead_handoff_boundary(
                &connection,
                "workspace-a",
                "task-c",
                "RUNNING",
                Some(1),
                None,
                "binding-old",
                "binding-new",
                LeadHandoffCause::PolicyFailover,
            )
            .is_err()
        );
    }
}
