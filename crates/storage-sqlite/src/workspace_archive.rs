use rusqlite::Transaction;
use storage_core::{StoreError, WorkspaceArchiveBlocker, WorkspaceArchiveBlockerKind};

/// Reads every durable authority/workload category that must be empty before an
/// ACTIVE Workspace can become read-only. The caller holds the immediate write
/// transaction used for the archive transition, fencing concurrent admissions.
pub(super) fn blockers(
    transaction: &Transaction<'_>,
    workspace_id: &str,
) -> Result<Vec<WorkspaceArchiveBlocker>, StoreError> {
    let checks = [
        (
            WorkspaceArchiveBlockerKind::NonterminalTasks,
            "SELECT COUNT(*) FROM tasks WHERE workspace_id = ?1 AND status NOT IN ('COMPLETED','FAILED','CANCELLED')",
        ),
        (
            WorkspaceArchiveBlockerKind::EnabledAutomations,
            "SELECT COUNT(*) FROM automations WHERE workspace_id = ?1 AND status <> 'DISABLED'",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveAutomationTriggers,
            "SELECT COUNT(*) FROM automation_cursors WHERE workspace_id = ?1",
        ),
        (
            WorkspaceArchiveBlockerKind::UnsettledAutomationOccurrences,
            "SELECT COUNT(*) FROM automation_occurrences WHERE workspace_id = ?1 AND status NOT IN ('COMPLETED','SKIPPED','FAILED')",
        ),
        (
            WorkspaceArchiveBlockerKind::UnsettledConversationTurns,
            "SELECT COUNT(*) FROM conversation_turns t JOIN conversations c ON c.conversation_id = t.conversation_id WHERE c.workspace_id = ?1 AND t.status NOT IN ('COMPLETED','FAILED','CANCELLED')",
        ),
        (
            WorkspaceArchiveBlockerKind::UnsettledAgentSessions,
            "SELECT COUNT(*) FROM agent_sessions WHERE workspace_id = ?1 AND status IN ('STARTING','ACTIVE','INTERRUPTING','CLOSING')",
        ),
        (
            WorkspaceArchiveBlockerKind::UnsettledInvocations,
            "SELECT COUNT(*) FROM capability_invocations WHERE workspace_id = ?1 AND status NOT IN ('SUCCEEDED','FAILED','CANCELLED')",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveCapabilityGrants,
            "SELECT COUNT(*) FROM capability_grants g WHERE g.status = 'ACTIVE' AND (EXISTS (SELECT 1 FROM conversations c WHERE c.conversation_id = g.conversation_id AND c.workspace_id = ?1) OR EXISTS (SELECT 1 FROM tasks t WHERE t.task_id = g.task_id AND t.workspace_id = ?1))",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveCapabilityActivations,
            "SELECT COUNT(*) FROM capability_activations WHERE workspace_id = ?1 AND status IN ('STARTING','ACTIVE','STOPPING')",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveSecretLeases,
            "SELECT COUNT(*) FROM secret_leases l JOIN tasks t ON t.task_id = l.task_id WHERE t.workspace_id = ?1 AND l.status = 'ACTIVE'",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveExecutionLeases,
            "SELECT COUNT(*) FROM execution_leases l JOIN tasks t ON t.task_id = l.task_id WHERE t.workspace_id = ?1 AND l.state IN ('ACTIVE','RELEASING')",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveControlLeases,
            "SELECT COUNT(*) FROM environment_control_leases l JOIN tasks t ON t.task_id = l.task_id WHERE t.workspace_id = ?1 AND l.state IN ('ACTIVE','RELEASING')",
        ),
        (
            WorkspaceArchiveBlockerKind::LivePersistentEnvironments,
            "SELECT COUNT(*) FROM environments WHERE owner_workspace_id = ?1 AND lifetime = 'WORKSPACE_PERSISTENT' AND status NOT IN ('SUSPENDED','DESTROYED')",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveWorkspaceRoots,
            "SELECT COUNT(*) FROM workspace_roots WHERE workspace_id = ?1 AND status = 'ACTIVE'",
        ),
        (
            WorkspaceArchiveBlockerKind::ActiveChannelHosts,
            "SELECT COUNT(*) FROM channel_host_assignments WHERE workspace_id = ?1 AND status IN ('ACTIVE','DRAINING')",
        ),
        (
            WorkspaceArchiveBlockerKind::UnresolvedEffects,
            "SELECT COUNT(*) FROM effects e JOIN tasks t ON t.task_id = e.task_id WHERE t.workspace_id = ?1 AND e.state NOT IN ('VERIFIED','FAILED')",
        ),
        (
            WorkspaceArchiveBlockerKind::PendingApprovals,
            "SELECT COUNT(*) FROM approvals a JOIN tasks t ON t.task_id = a.task_id WHERE t.workspace_id = ?1 AND a.status = 'PENDING'",
        ),
    ];

    let mut blockers = Vec::new();
    for (kind, query) in checks {
        let count: i64 = transaction
            .query_row(query, [workspace_id], |row| row.get(0))
            .map_err(|error| StoreError::Database(error.to_string()))?;
        if count < 0 {
            return Err(StoreError::Integrity(
                "Workspace archive blocker count is negative".to_owned(),
            ));
        }
        if count > 0 {
            blockers.push(WorkspaceArchiveBlocker {
                kind,
                count: u64::try_from(count).map_err(|_| {
                    StoreError::Integrity("Workspace archive blocker count overflow".to_owned())
                })?,
            });
        }
    }
    Ok(blockers)
}
