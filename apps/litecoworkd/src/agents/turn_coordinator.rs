//! The only entry the Operator uses to start native work for an admitted turn
//! (001-T20, SPEC L-07).
//!
//! The Operator admits a turn (the durable USER message, OPEN turn and dispatch intent
//! commit atomically first, AC-001) and returns 202. It then hands the turn to this
//! coordinator, which drives the `AgentSessionSupervisor` off the request path. The
//! supervisor claims the durable dispatch intent with the dispatching Runtime incarnation
//! before host/binding preflight or session reservation. A losing or ambiguous claim
//! cannot settle the turn; startup recovery owns any durable intent left behind (AC-022).

use std::{io::Write, sync::Arc};

use storage_core::conversation::ConversationTurn;

use super::supervisor::{AgentSessionSupervisor, DispatchFailureClass};

#[derive(Clone)]
pub(crate) struct AgentTurnCoordinator {
    supervisor: Arc<AgentSessionSupervisor>,
}

impl AgentTurnCoordinator {
    pub(crate) fn new(supervisor: Arc<AgentSessionSupervisor>) -> Self {
        Self { supervisor }
    }

    /// Starts dispatch for an admitted (or retried) turn without blocking the caller.
    /// A dispatch error is a closed class. The supervisor settles the turn or leaves its
    /// durable intent for startup recovery; this layer reports no error detail.
    pub(crate) fn dispatch_admitted_turn(
        &self,
        local_incarnation_id: &str,
        principal_id: &str,
        workspace_id: &str,
        conversation_id: &str,
        turn: ConversationTurn,
        correlation_id: &str,
    ) {
        let supervisor = Arc::clone(&self.supervisor);
        let local_incarnation_id = local_incarnation_id.to_owned();
        let principal_id = principal_id.to_owned();
        let workspace_id = workspace_id.to_owned();
        let conversation_id = conversation_id.to_owned();
        let correlation_id = correlation_id.to_owned();
        let turn_id = turn.turn_id.clone();
        tokio::spawn(async move {
            if let Err(error) = supervisor
                .dispatch_turn(
                    &local_incarnation_id,
                    &principal_id,
                    &workspace_id,
                    &conversation_id,
                    turn,
                    &correlation_id,
                )
                .await
            {
                let mut stderr = std::io::stderr().lock();
                let _ = write_dispatch_diagnostic(&mut stderr, &turn_id, &correlation_id, error);
            }
        });
    }
}

/// A structured diagnostic with only correlation identifiers and a closed error class.
pub(crate) fn dispatch_diagnostic(
    turn_id: &str,
    correlation_id: &str,
    class: DispatchFailureClass,
) -> String {
    serde_json::json!({
        "event": "conversation.dispatch.failed",
        "turn_id": turn_id,
        "correlation_id": correlation_id,
        "class": class.as_str(),
    })
    .to_string()
}

fn write_dispatch_diagnostic(
    output: &mut impl Write,
    turn_id: &str,
    correlation_id: &str,
    class: DispatchFailureClass,
) -> std::io::Result<()> {
    writeln!(
        output,
        "{}",
        dispatch_diagnostic(turn_id, correlation_id, class)
    )
}

#[cfg(test)]
mod tests {
    use super::{DispatchFailureClass, dispatch_diagnostic, write_dispatch_diagnostic};

    #[test]
    fn conversation_turn_dispatch_diagnostic_carries_ids_and_class_but_no_detail() {
        let line =
            dispatch_diagnostic("turn-1", "corr-1", DispatchFailureClass::SessionReservation);
        assert_eq!(
            line,
            r#"{"class":"SESSION_RESERVATION","correlation_id":"corr-1","event":"conversation.dispatch.failed","turn_id":"turn-1"}"#
        );
        assert!(!line.contains("secret") && !line.contains("ses_native_handle"));

        let provider_detail = "super-secret-value provider rejected request with credential detail";
        let provider_failure: Result<(), &str> = Err(provider_detail);
        let failure_class = provider_failure
            .map_err(|_| DispatchFailureClass::ProviderExecution)
            .expect_err("provider detail is discarded into a closed class");
        let unstructured = dispatch_diagnostic("turn-2", "corr-2", failure_class);
        assert!(
            !unstructured.contains(provider_detail),
            "diagnostics must never include arbitrary provider/error text"
        );
        assert!(unstructured.contains("PROVIDER_EXECUTION"));
    }

    #[test]
    fn conversation_turn_captured_dispatch_log_omits_provider_and_secret_details() {
        let provider_detail = "provider rejected secret-token=never-log-this";
        let failure_class = Result::<(), &str>::Err(provider_detail)
            .map_err(|_| DispatchFailureClass::ProviderExecution)
            .expect_err("provider details collapse into a closed class");
        let mut captured = Vec::new();

        write_dispatch_diagnostic(
            &mut captured,
            "turn-captured",
            "corr-captured",
            failure_class,
        )
        .expect("write structured dispatch diagnostic");

        let captured = String::from_utf8(captured).expect("diagnostic is UTF-8");
        assert_eq!(
            captured,
            "{\"class\":\"PROVIDER_EXECUTION\",\"correlation_id\":\"corr-captured\",\"event\":\"conversation.dispatch.failed\",\"turn_id\":\"turn-captured\"}\n"
        );
        assert!(!captured.contains(provider_detail));
        assert!(!captured.contains("never-log-this"));
    }
}
