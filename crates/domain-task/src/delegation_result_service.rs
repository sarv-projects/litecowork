//! Domain service that validates native child reports and publishes a bounded,
//! provenance-bound ResultEnvelope through the durable storage port.

use crate::{DelegationEnvelopeError, ResultEnvelope, validate_result_envelope};
use domain_workspace::EventContext;
use serde_json::json;
use sha2::{Digest, Sha256};
use storage_core::{DelegationResultPublishCommit, DelegationResultStore, StoreError};

/// Identity established by the authenticated private Runtime/AgentSession boundary.
/// The storage adapter independently rechecks all IDs against the terminal Attempt
/// and its AgentSession before committing.
#[derive(Clone, Debug)]
pub struct AuthenticatedChildResultPublisher {
    pub workspace_id: String,
    pub task_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub agent_session_id: String,
    pub task_spec_revision: u64,
}

#[derive(Clone, Debug)]
pub struct PublishChildResult {
    pub publisher: AuthenticatedChildResultPublisher,
    pub result: ResultEnvelope,
    pub event: EventContext,
}

/// Authenticated parent scope used by the internal DelegationChannel result read.
/// The owner identity is established at the Workspace boundary; the storage adapter
/// independently proves the exact parent/child Attempt relationship and pinned revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildResultReadContext {
    pub owner_principal_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub parent_attempt_id: String,
    pub child_attempt_id: String,
    pub task_spec_revision: u64,
}

pub struct DelegationResultService<S> {
    store: S,
}

impl<S: DelegationResultStore> DelegationResultService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Validate a bounded native report, then atomically persist its immutable
    /// envelope and summary-only event. All report prose remains in the envelope blob.
    pub fn publish(
        &self,
        command: PublishChildResult,
    ) -> Result<storage_core::CommittedDelegationResult, StoreError> {
        validate_publisher(&command.publisher)?;
        if command.event.origin_runtime_id != command.publisher.runtime_id {
            return Err(invalid("authenticated result event Runtime mismatch"));
        }
        validate_result_envelope(
            &command.result,
            &command.result.child_attempt_id,
            &command.publisher.workspace_id,
            command.publisher.task_spec_revision,
        )
        .map_err(envelope_error)?;

        let envelope_json =
            serde_json::to_value(&command.result).map_err(|error| invalid(error.to_string()))?;
        let envelope_digest = envelope_digest(&envelope_json)?;
        let status = serde_json::to_value(command.result.status)
            .map_err(|error| invalid(error.to_string()))?
            .as_str()
            .ok_or_else(|| invalid("ResultEnvelope status serialization is invalid"))?
            .to_owned();
        let child_attempt_id = command.result.child_attempt_id.clone();
        let workspace_id = command.publisher.workspace_id.clone();
        let runtime_id = command.publisher.runtime_id.clone();
        let payload = json!({
            "child_attempt_id": &child_attempt_id,
            "task_id": &command.publisher.task_id,
            "status": &status,
            "revision_at_start": command.result.revision_at_start,
            "result_digest": envelope_digest,
            "aggregate_version": 1,
        });
        self.store
            .publish_delegation_result(DelegationResultPublishCommit {
                workspace_id: workspace_id.clone(),
                runtime_id: runtime_id.clone(),
                runtime_incarnation_id: command.publisher.runtime_incarnation_id,
                agent_session_id: command.publisher.agent_session_id,
                child_attempt_id: child_attempt_id.clone(),
                envelope_json,
                envelope_digest,
                event: storage_core::EventDraft {
                    event_id: command.event.event_id,
                    workspace_id,
                    entity_type: "DELEGATION_RESULT".to_owned(),
                    entity_id: child_attempt_id,
                    origin_runtime_id: runtime_id,
                    entity_revision: 1,
                    hlc_timestamp: command.event.hlc_timestamp,
                    correlation_id: command.event.correlation_id,
                    causation_id: command.event.causation_id,
                    schema_version: 1,
                    event_type: "delegation.result.published.v1".to_owned(),
                    payload,
                    recorded_at: command.event.recorded_at,
                },
            })
    }

    /// Read the immutable result through the Workspace-owner-scoped storage port.
    /// The parent supplies the TaskSpec revision pinned when the child was admitted;
    /// stale child output is rejected instead of being treated as current.
    pub fn result(
        &self,
        context: ChildResultReadContext,
    ) -> Result<Option<ResultEnvelope>, StoreError> {
        let committed =
            self.store
                .get_delegation_result(storage_core::DelegationResultReadQuery {
                    owner_principal_id: context.owner_principal_id,
                    workspace_id: context.workspace_id.clone(),
                    task_id: context.task_id,
                    parent_attempt_id: context.parent_attempt_id,
                    child_attempt_id: context.child_attempt_id.clone(),
                    task_spec_revision: context.task_spec_revision,
                })?;
        let Some(committed) = committed else {
            return Ok(None);
        };
        let result: ResultEnvelope = serde_json::from_value(committed.envelope_json.clone())
            .map_err(|error| StoreError::Integrity(error.to_string()))?;
        validate_result_envelope(
            &result,
            &context.child_attempt_id,
            &context.workspace_id,
            context.task_spec_revision,
        )
        .map_err(|error| {
            StoreError::Integrity(format!("stored ResultEnvelope is invalid: {error}"))
        })?;
        if envelope_digest(&committed.envelope_json)? != committed.envelope_digest {
            return Err(StoreError::Integrity(
                "stored ResultEnvelope digest does not match its canonical bytes".to_owned(),
            ));
        }
        Ok(Some(result))
    }
}

fn envelope_digest(envelope: &serde_json::Value) -> Result<String, StoreError> {
    let canonical =
        serde_json_canonicalizer::to_vec(envelope).map_err(|error| invalid(error.to_string()))?;
    Ok(format!("sha256:{}", lower_hex(&Sha256::digest(&canonical))))
}

fn validate_publisher(publisher: &AuthenticatedChildResultPublisher) -> Result<(), StoreError> {
    if [
        publisher.workspace_id.as_str(),
        publisher.task_id.as_str(),
        publisher.runtime_id.as_str(),
        publisher.runtime_incarnation_id.as_str(),
        publisher.agent_session_id.as_str(),
    ]
    .iter()
    .any(|value| {
        value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control)
    }) || publisher.task_spec_revision == 0
    {
        return Err(invalid("authenticated child result publisher is invalid"));
    }
    Ok(())
}

fn envelope_error(error: DelegationEnvelopeError) -> StoreError {
    invalid(format!("ResultEnvelope validation failed: {error}"))
}

fn invalid(message: impl Into<String>) -> StoreError {
    StoreError::Invalid(message.into())
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct CaptureStore {
        published: Arc<Mutex<Vec<DelegationResultPublishCommit>>>,
        result: Arc<Mutex<Option<storage_core::CommittedDelegationResult>>>,
        reads: Arc<Mutex<Vec<storage_core::DelegationResultReadQuery>>>,
    }

    impl DelegationResultStore for CaptureStore {
        fn publish_delegation_result(
            &self,
            commit: DelegationResultPublishCommit,
        ) -> Result<storage_core::CommittedDelegationResult, StoreError> {
            self.published.lock().unwrap().push(commit.clone());
            Err(StoreError::NotFound)
        }

        fn get_delegation_result(
            &self,
            query: storage_core::DelegationResultReadQuery,
        ) -> Result<Option<storage_core::CommittedDelegationResult>, StoreError> {
            self.reads.lock().unwrap().push(query);
            Ok(self.result.lock().unwrap().clone())
        }
    }

    fn command() -> PublishChildResult {
        PublishChildResult {
            publisher: AuthenticatedChildResultPublisher {
                workspace_id: "workspace-1".to_owned(),
                task_id: "task-1".to_owned(),
                runtime_id: "runtime-1".to_owned(),
                runtime_incarnation_id: "incarnation-1".to_owned(),
                agent_session_id: "session-1".to_owned(),
                task_spec_revision: 7,
            },
            result: ResultEnvelope {
                child_attempt_id: "child-1".to_owned(),
                status: crate::DelegationResultStatus::Blocked,
                summary: "untrusted details stay out of the event".to_owned(),
                output_refs: vec![],
                artifact_refs: vec![],
                evidence_refs: vec![],
                changed_resource_refs: vec![],
                unresolved: vec![],
                blockers: vec![],
                usage: None,
                confidence: None,
                verification_hints: vec![],
                revision_at_start: 7,
            },
            event: EventContext {
                event_id: "event-1".to_owned(),
                origin_runtime_id: "runtime-1".to_owned(),
                hlc_timestamp: "2026-10-17T12:00:00Z".to_owned(),
                correlation_id: "correlation-1".to_owned(),
                causation_id: None,
                recorded_at: "2026-10-17T12:00:00Z".to_owned(),
            },
        }
    }

    #[test]
    fn publication_validates_envelope_and_emits_only_safe_summary_fields() {
        let store = CaptureStore::default();
        let service = DelegationResultService::new(store.clone());
        assert!(matches!(
            service.publish(command()),
            Err(StoreError::NotFound)
        ));
        let captured = store.published.lock().unwrap();
        let commit = &captured[0];
        assert_eq!(commit.event.payload["status"], "BLOCKED");
        assert_eq!(commit.event.payload["task_id"], "task-1");
        assert_eq!(commit.event.entity_type, "DELEGATION_RESULT");
        assert!(commit.event.payload.get("summary").is_none());
        assert!(
            !commit
                .event
                .payload
                .to_string()
                .contains("untrusted details")
        );
        assert!(commit.envelope_digest.starts_with("sha256:"));
        assert_eq!(
            commit.envelope_json["summary"],
            "untrusted details stay out of the event"
        );
    }

    #[test]
    fn publication_rejects_wrong_task_revision_before_storage() {
        let store = CaptureStore::default();
        let service = DelegationResultService::new(store.clone());
        let mut command = command();
        command.publisher.task_spec_revision = 8;
        assert!(matches!(
            service.publish(command),
            Err(StoreError::Invalid(_))
        ));
        assert!(store.published.lock().unwrap().is_empty());
    }

    #[test]
    fn publication_rejects_event_from_another_runtime_before_storage() {
        let store = CaptureStore::default();
        let service = DelegationResultService::new(store.clone());
        let mut command = command();
        command.event.origin_runtime_id = "runtime-other".to_owned();
        assert!(matches!(
            service.publish(command),
            Err(StoreError::Invalid(_))
        ));
        assert!(store.published.lock().unwrap().is_empty());
    }

    #[test]
    fn result_reads_are_owner_workspace_scoped_and_task_revision_bound() {
        let store = CaptureStore::default();
        let command = command();
        let envelope_json = serde_json::to_value(&command.result).unwrap();
        let committed_digest = envelope_digest(&envelope_json).unwrap();
        *store.result.lock().unwrap() = Some(storage_core::CommittedDelegationResult {
            workspace_id: "workspace-1".to_owned(),
            task_id: "task-1".to_owned(),
            child_attempt_id: "child-1".to_owned(),
            published_by_agent_session_id: "session-1".to_owned(),
            runtime_id: "runtime-1".to_owned(),
            runtime_incarnation_id: "incarnation-1".to_owned(),
            envelope_json,
            envelope_digest: committed_digest,
            event: storage_core::DomainEvent {
                event_id: "event-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                entity_type: "DELEGATION_RESULT".to_owned(),
                entity_id: "child-1".to_owned(),
                origin_runtime_id: "runtime-1".to_owned(),
                origin_sequence: 1,
                entity_revision: 1,
                hlc_timestamp: "2026-10-17T12:00:00Z".to_owned(),
                correlation_id: "correlation-1".to_owned(),
                causation_id: None,
                schema_version: 1,
                event_type: "delegation.result.published.v1".to_owned(),
                payload: serde_json::Value::Null,
                aggregate_state_ref: storage_core::AggregateStateRef {
                    blob: storage_core::BlobRef {
                        digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
                        size_bytes: 0,
                        media_type: "application/vnd.litecow.delegation-result+json".to_owned(),
                    },
                    entity_revision: 1,
                    record_schema_version: 1,
                },
                recorded_at: "2026-10-17T12:00:00Z".to_owned(),
                payload_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            },
            replayed: false,
        });
        let service = DelegationResultService::new(store.clone());
        let result = service
            .result(ChildResultReadContext {
                owner_principal_id: "owner-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                task_id: "task-1".to_owned(),
                parent_attempt_id: "parent-1".to_owned(),
                child_attempt_id: "child-1".to_owned(),
                task_spec_revision: 7,
            })
            .unwrap()
            .unwrap();
        assert_eq!(result.status, crate::DelegationResultStatus::Blocked);
        assert_eq!(store.reads.lock().unwrap()[0].owner_principal_id, "owner-1");
        assert_eq!(store.reads.lock().unwrap()[0].workspace_id, "workspace-1");
        assert_eq!(store.reads.lock().unwrap()[0].task_id, "task-1");
        assert_eq!(store.reads.lock().unwrap()[0].parent_attempt_id, "parent-1");
        assert_eq!(store.reads.lock().unwrap()[0].task_spec_revision, 7);
        assert!(matches!(
            service.result(ChildResultReadContext {
                owner_principal_id: "owner-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                task_id: "task-1".to_owned(),
                parent_attempt_id: "parent-1".to_owned(),
                child_attempt_id: "child-1".to_owned(),
                task_spec_revision: 8,
            },),
            Err(StoreError::Integrity(_))
        ));
    }
}
