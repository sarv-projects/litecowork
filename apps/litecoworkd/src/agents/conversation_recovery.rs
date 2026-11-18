//! Daemon restart and orphan recovery sweep for Conversation turns (001-T08).
//!
//! V1 has no same-incarnation resume of a native session, so at startup (before the
//! Operator accepts commands) every Conversation attempt that was in flight under a
//! prior incarnation is settled FAILED with a typed reason and its AgentSession is
//! ended LOST. The USER message is never touched, no AGENT message is invented, and the
//! turn shows a failure that offers Retry. Unknown native state is never reported as
//! completed. The sweep is idempotent: settled turns and ended sessions are skipped.

use storage_core::conversation::ConversationDispatchState;
use storage_core::{AgentSessionStore, ConversationTurnStore};
use storage_sqlite::SqliteWorkspaceStore;

use super::supervisor::settle_attempt_failed;

/// Typed reason recorded on every turn settled by the sweep.
pub(crate) const RECOVERY_REASON_CODE: &str = "RUNTIME_RESTARTED";

const PAGE_SIZE: usize = 100;

/// Counts reported as startup recovery facts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ConversationRecoveryReport {
    /// Unsettled dispatch intents (UNCLAIMED or CLAIMED) whose attempt was settled.
    pub(crate) intents_settled: usize,
    /// Stranded STARTING sessions released LOST after their turn was already settled or
    /// whose intent was missing.
    pub(crate) starting_sessions_released: usize,
    /// Rows that could not be settled; the sweep reports them instead of hiding them.
    pub(crate) failures: usize,
}

/// Settles every prior-incarnation Conversation attempt. Safe to run twice.
pub(crate) fn recover_conversation_sessions(
    store: &SqliteWorkspaceStore,
    runtime_id: &str,
    principal_id: &str,
) -> ConversationRecoveryReport {
    let mut report = ConversationRecoveryReport::default();
    let correlation_id = format!("conversation-recovery-{runtime_id}");

    for state in [
        ConversationDispatchState::Claimed,
        ConversationDispatchState::Unclaimed,
    ] {
        // Settled intents leave the state, so re-reading the first page makes progress;
        // stop when a whole page fails to settle to avoid spinning.
        let mut seen: std::collections::HashSet<(String, u32)> = std::collections::HashSet::new();
        loop {
            let conv_store = storage_sqlite::SqliteConversationStore::new(store.clone());
            let intents = match conv_store.list_conversation_dispatch_intents(state, PAGE_SIZE) {
                Ok(intents) => intents,
                Err(_) => {
                    report.failures += 1;
                    break;
                }
            };
            if intents.is_empty() {
                break;
            }
            let mut progressed = false;
            for intent in intents {
                if !seen.insert((intent.turn_id.clone(), intent.retry_ordinal)) {
                    // Already attempted this pass and the intent is still listed.
                    continue;
                }
                match settle_attempt_failed(
                    store,
                    runtime_id,
                    principal_id,
                    &intent.workspace_id,
                    &intent.conversation_id,
                    &intent.turn_id,
                    None,
                    RECOVERY_REASON_CODE,
                    &correlation_id,
                    None,
                ) {
                    Ok(()) => {
                        report.intents_settled += 1;
                        progressed = true;
                    }
                    Err(_) => report.failures += 1,
                }
            }
            if !progressed {
                break;
            }
        }
    }

    // STARTING reservations that no intent explains (for example a legacy turn from
    // before the dispatch outbox) are released through the same settle path.
    match store.list_workspaces() {
        Ok(workspaces) => {
            for workspace in workspaces {
                let mut after: Option<String> = None;
                loop {
                    let sessions = match store.list_starting_conversation_agent_sessions(
                        &workspace.workspace_id,
                        after.as_deref(),
                        PAGE_SIZE,
                    ) {
                        Ok(sessions) => sessions,
                        Err(_) => {
                            report.failures += 1;
                            break;
                        }
                    };
                    let Some(last) = sessions.last() else { break };
                    after = Some(last.agent_session_id.clone());
                    for session in sessions {
                        let (Some(conversation_id), Some(turn_id)) = (
                            session.conversation_id.as_deref(),
                            session.conversation_turn_id.as_deref(),
                        ) else {
                            report.failures += 1;
                            continue;
                        };
                        match settle_attempt_failed(
                            store,
                            runtime_id,
                            principal_id,
                            &workspace.workspace_id,
                            conversation_id,
                            turn_id,
                            Some(&session.agent_session_id),
                            RECOVERY_REASON_CODE,
                            &correlation_id,
                            None,
                        ) {
                            Ok(()) => report.starting_sessions_released += 1,
                            Err(_) => report.failures += 1,
                        }
                    }
                }
            }
        }
        Err(_) => report.failures += 1,
    }
    report
}
