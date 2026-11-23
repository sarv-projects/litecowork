//! The only entry the Operator uses to start native work for an admitted turn
//! (001-T20, SPEC L-07).
//!
//! The Operator admits a turn (the durable USER message, OPEN turn and dispatch intent
//! commit atomically first, AC-001) and returns 202. It then hands the turn to this
//! coordinator, which drives the `AgentSessionSupervisor` off the request path. The
//! supervisor claims the durable dispatch intent with the dispatching Runtime incarnation
//! before host/binding preflight or session reservation. A losing or ambiguous claim
//! cannot settle the turn; startup recovery owns any durable intent left behind (AC-022).

use std::{future::Future, io::Write, sync::Arc, time::Duration};

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
    /// A transient exact-claim/storage uncertainty is retried before any session/provider
    /// work. Other dispatch errors are a closed class; the supervisor settles the turn or
    /// leaves its durable intent for startup recovery. No error detail is reported.
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
            let result = dispatch_with_claim_retry(|| {
                let supervisor = Arc::clone(&supervisor);
                let local_incarnation_id = local_incarnation_id.clone();
                let principal_id = principal_id.clone();
                let workspace_id = workspace_id.clone();
                let conversation_id = conversation_id.clone();
                let turn = turn.clone();
                let correlation_id = correlation_id.clone();
                async move {
                    supervisor
                        .dispatch_turn(
                            &local_incarnation_id,
                            &principal_id,
                            &workspace_id,
                            &conversation_id,
                            turn,
                            &correlation_id,
                        )
                        .await
                }
            })
            .await;
            if let Err(error) = result {
                let mut stderr = std::io::stderr().lock();
                let _ = write_dispatch_diagnostic(&mut stderr, &turn_id, &correlation_id, error);
            }
        });
    }
}

const DISPATCH_CLAIM_RETRY_BASE: Duration = Duration::from_millis(100);
const DISPATCH_CLAIM_RETRY_MAX: Duration = Duration::from_secs(5);

fn dispatch_claim_retry_delay(retry_index: u32) -> Duration {
    let multiplier = 1_u32.checked_shl(retry_index.min(6)).unwrap_or(64);
    DISPATCH_CLAIM_RETRY_BASE
        .saturating_mul(multiplier)
        .min(DISPATCH_CLAIM_RETRY_MAX)
}

async fn dispatch_with_claim_retry<F, Fut>(mut dispatch: F) -> Result<(), DispatchFailureClass>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), DispatchFailureClass>>,
{
    let mut retry_index = 0_u32;
    loop {
        match dispatch().await {
            Err(DispatchFailureClass::DispatchIntentClaim) => {
                tokio::time::sleep(dispatch_claim_retry_delay(retry_index)).await;
                retry_index = retry_index.saturating_add(1);
            }
            result => return result,
        }
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
    use super::{
        DispatchFailureClass, dispatch_claim_retry_delay, dispatch_diagnostic,
        dispatch_with_claim_retry, write_dispatch_diagnostic,
    };

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

    #[test]
    fn conversation_dispatch_claim_retry_is_backed_off_and_does_not_retry_provider_failures() {
        use std::{future, time::Duration};

        assert_eq!(dispatch_claim_retry_delay(0), Duration::from_millis(100));
        assert_eq!(dispatch_claim_retry_delay(1), Duration::from_millis(200));
        assert_eq!(dispatch_claim_retry_delay(5), Duration::from_millis(3_200));
        assert_eq!(dispatch_claim_retry_delay(12), Duration::from_secs(5));

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("build test timer runtime");
        let mut claim_attempts = 0;
        let recovered = runtime.block_on(dispatch_with_claim_retry(|| {
            claim_attempts += 1;
            future::ready(if claim_attempts == 1 {
                Err(DispatchFailureClass::DispatchIntentClaim)
            } else {
                Ok(())
            })
        }));
        assert_eq!(recovered, Ok(()));
        assert_eq!(claim_attempts, 2);

        let mut provider_attempts = 0;
        let provider_failure = runtime.block_on(dispatch_with_claim_retry(|| {
            provider_attempts += 1;
            future::ready(Err(DispatchFailureClass::ProviderExecution))
        }));
        assert_eq!(
            provider_failure,
            Err(DispatchFailureClass::ProviderExecution)
        );
        assert_eq!(
            provider_attempts, 1,
            "provider failures are never auto-retried"
        );
    }
}
