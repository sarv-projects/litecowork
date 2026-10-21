//! Combined Trust + ApprovalUse + Invocation + Effect dispatch admission.
//!
//! One authoritative transaction rechecks the live owner/lease, grant,
//! activation, the exact Trust decision and the exact Approval, then
//! transitions the Invocation, links and starts its Effect, appends the single
//! `ApprovalUse`, and writes the state snapshots, events, audit record and
//! idempotency receipt together. A rejected admission writes nothing, and no
//! provider call may happen before this transaction commits.
//!
//! The schema still closes Invocation creation and the first `DISPATCHED`
//! transition behind `sqlite-v15.sql`. This adapter therefore fails closed on
//! the current schema until the lead-owned migration that replaces those two
//! guards is applied; see `implementation/reviews/E04-S03.md`.

use super::*;
use domain_effects::{EffectRecord, EffectState, ExecutionMethod};
use domain_invocations::{CapabilityInvocationRecord, InvocationStatus};
use serde::Serialize;
use serde_json::json;
use storage_core::{
    AdmitCapabilityDispatchCommit, CapabilityDispatchStore, CommittedCapabilityDispatch,
    EffectEvidenceEventContext,
};


#[derive(Clone)]
pub struct SqliteCapabilityDispatchStore {
    store: SqliteWorkspaceStore,
}

impl SqliteCapabilityDispatchStore {
    pub fn new(store: SqliteWorkspaceStore) -> Self {
        Self { store }
    }

    fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let (reply, receive) = mpsc::channel();
        self.store.execute_command(
            Command::Operation {
                operation: Box::new(move |connection| {
                    let _ = reply.send(operation(connection));
                }),
            },
            receive,
        )
    }
}

impl CapabilityDispatchStore for SqliteCapabilityDispatchStore {
    fn admit_capability_dispatch(
        &self,
        mut commit: AdmitCapabilityDispatchCommit,
    ) -> Result<CommittedCapabilityDispatch, StoreError> {
        commit.events.invocation.recorded_at =
            canonicalize_utc_timestamp(&commit.events.invocation.recorded_at)?;
        commit.events.effect.recorded_at =
            canonicalize_utc_timestamp(&commit.events.effect.recorded_at)?;
        commit.events.approval_use.recorded_at =
            canonicalize_utc_timestamp(&commit.events.approval_use.recorded_at)?;
        validate_commit(&commit)?;

        let principal = runtime_principal(&commit.runtime_id, &commit.runtime_incarnation_id);
        let fingerprint = digest(&canonical_json(&json!({
            "workspace_id": commit.workspace_id,
            "invocation_id": commit.invocation_id,
            "expected_invocation_version": commit.expected_invocation_version,
            "effect_id": commit.effect_id,
            "expected_effect_version": commit.expected_effect_version,
            "approval_id": commit.approval_id,
            "approval_use_id": commit.approval_use_id,
            "trust_decision": commit.trust_decision,
        }))?);

        // First pass: authorize, replay-check and run every admission check so
        // the exact resulting records can be hashed into their state blobs.
        let probe = commit.clone();
        let probe_principal = principal.clone();
        let probe_request_id = commit.request_id.clone();
        let probe_fingerprint = fingerprint.clone();
        let (prior, admission) = self.run(move |connection| {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_runtime(
                &transaction,
                &probe.workspace_id,
                &probe.runtime_id,
                &probe.runtime_incarnation_id,
            )?;
            let prior = load_replay(
                &transaction,
                &probe_principal,
                &probe_request_id,
                &probe_fingerprint,
            )?;
            let admission = if prior.is_none() {
                Some(verify_admission(&transaction, &probe)?)
            } else {
                None
            };
            transaction.commit().map_err(map_database_error)?;
            Ok((prior, admission))
        })?;
        if let Some(prior) = prior {
            return Ok(prior);
        }
        let admission = admission.ok_or_else(|| {
            StoreError::Integrity("Capability dispatch admission state is missing".to_owned())
        })?;

        let invocation_state = put_aggregate_state(
            &self.store,
            &commit.workspace_id,
            &admission.next_invocation,
            "application/vnd.litecowork.capability-invocation+json",
            admission.next_invocation.version,
        )?;
        let effect_state = put_aggregate_state(
            &self.store,
            &commit.workspace_id,
            &admission.next_effect,
            "application/vnd.litecowork.effect+json",
            admission.next_effect.version,
        )?;
        let approval_use_state = put_aggregate_state(
            &self.store,
            &commit.workspace_id,
            &approval_use_record(&commit, &admission),
            "application/vnd.litecowork.approval-use+json",
            1,
        )?;

        self.run(move |connection| {
            admit_transaction(
                connection,
                commit,
                principal,
                fingerprint,
                admission,
                invocation_state,
                effect_state,
                approval_use_state,
            )
        })
    }
}

/// The exact persisted facts one admission decision was based on, plus the
/// records the admitted transition produces.
struct AdmissionState {
    invocation: CapabilityInvocationRecord,
    effect: EffectRecord,
    next_invocation: CapabilityInvocationRecord,
    next_effect: EffectRecord,
    approval_action_digest: String,
    approval_scope_digest: String,
    approval_target: Value,
    approval_kind: String,
}

fn validate_commit(commit: &AdmitCapabilityDispatchCommit) -> Result<(), StoreError> {
    let texts = [
        commit.workspace_id.as_str(),
        commit.runtime_id.as_str(),
        commit.runtime_incarnation_id.as_str(),
        commit.request_id.as_str(),
        commit.invocation_id.as_str(),
        commit.effect_id.as_str(),
        commit.approval_id.as_str(),
        commit.approval_use_id.as_str(),
    ];
    if texts.iter().any(|value| {
        value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control)
    }) || commit.expected_invocation_version == 0
        || commit.expected_effect_version == 0
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_COMMAND_INVALID".to_owned(),
        ));
    }
    for context in [&commit.events.effect, &commit.events.approval_use] {
        validate_dispatch_event_context(context)?;
    }
    validate_invocation_event_context(&commit.events.invocation)?;
    let decision = &commit.trust_decision;
    if decision.decision != "ALLOW" && decision.decision != "REQUIRE_APPROVAL" {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_TRUST_DECISION_DENIED".to_owned(),
        ));
    }
    if decision.reason_code.trim().is_empty()
        || decision.reason_code.len() > 256
        || decision.policy_id.trim().is_empty()
        || decision.policy_id.len() > 256
        || decision.policy_revision == 0
        || !valid_digest(&decision.policy_content_digest)
        || !valid_digest(&decision.request_digest)
        || !valid_digest(&decision.action_digest)
        || !valid_digest(&decision.scope_digest)
        || decision.invocation_id != commit.invocation_id
        || decision.effect_id != commit.effect_id
        || decision.capability_grant_id.trim().is_empty()
        || decision.activation_id.trim().is_empty()
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_TRUST_DECISION_INVALID".to_owned(),
        ));
    }
    Ok(())
}

fn validate_dispatch_event_context(context: &EffectEvidenceEventContext) -> Result<(), StoreError> {
    if context.event_id.trim().is_empty()
        || context.origin_runtime_id.trim().is_empty()
        || context.origin_runtime_incarnation_id.trim().is_empty()
        || context.hlc_timestamp.trim().is_empty()
        || context.correlation_id.trim().is_empty()
        || context
            .causation_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || [
            context.event_id.as_str(),
            context.origin_runtime_id.as_str(),
            context.origin_runtime_incarnation_id.as_str(),
            context.hlc_timestamp.as_str(),
            context.correlation_id.as_str(),
        ]
        .iter()
        .any(|value| value.chars().any(char::is_control))
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_EVENT_CONTEXT_INVALID".to_owned(),
        ));
    }
    Ok(())
}

/// The Invocation event context carries the same identity fields as the Effect
/// context, so it is validated through the same closed shape.
fn validate_invocation_event_context(
    context: &storage_core::CapabilityInvocationEventContext,
) -> Result<(), StoreError> {
    validate_dispatch_event_context(&EffectEvidenceEventContext {
        event_id: context.event_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        origin_runtime_incarnation_id: context.origin_runtime_incarnation_id.clone(),
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        recorded_at: context.recorded_at.clone(),
    })
}

/// Every admission precondition. A failure here writes nothing.
fn verify_admission(
    transaction: &Transaction<'_>,
    commit: &AdmitCapabilityDispatchCommit,
) -> Result<AdmissionState, StoreError> {
    let invocation = load_invocation(transaction, &commit.workspace_id, &commit.invocation_id)?;
    if invocation.status != InvocationStatus::Created
        || invocation.version != commit.expected_invocation_version
        || invocation.effect_id.is_some()
    {
        return Err(StoreError::Conflict {
            expected: Some(commit.expected_invocation_version),
            actual: Some(invocation.version),
        });
    }
    if invocation.scope_kind != "ATTEMPT_EXECUTION"
        || invocation.task_id.is_none()
        || invocation.attempt_id.is_none()
    {
        // An Effect is admissible only inside an admitted Attempt scope.
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_SCOPE_NOT_ATTEMPT_EXECUTION".to_owned(),
        ));
    }

    let effect = load_effect(transaction, &commit.workspace_id, &commit.effect_id)?;
    if effect.state != EffectState::Proposed
        || effect.version != commit.expected_effect_version
        || effect.capability_invocation_id != invocation.invocation_id
        || effect.task_id != invocation.task_id.clone().unwrap_or_default()
        || effect.attempt_id != invocation.attempt_id.clone().unwrap_or_default()
        || effect.request_digest != invocation.request_digest
        || effect.operation != invocation.operation
        || effect.execution_method.as_str() != invocation.execution_method
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_EFFECT_PROVENANCE_MISMATCH".to_owned(),
        ));
    }

    // Live owner/lease authority for the originating AgentSession.
    let live: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM live_agent_session_invocation_authority WHERE agent_session_id=?1)",
            params![invocation.agent_session_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !live {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_SESSION_NOT_LIVE".to_owned(),
        ));
    }

    // The grant that authorizes this exact operation and capability identity.
    let grant: Option<(
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    )> = transaction
        .query_row(
            "SELECT scope_kind,COALESCE(task_id,''),COALESCE(attempt_id,''),capability_ref_json,
                    allowed_operations_json,expires_at,status
             FROM capability_grants WHERE capability_grant_id=?1",
            params![invocation.capability_grant_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        grant_scope_kind,
        grant_task_id,
        grant_attempt_id,
        grant_capability_ref,
        grant_operations,
        grant_expires_at,
        grant_status,
    )) = grant
    else {
        return Err(StoreError::NotFound);
    };
    if grant_status != "ACTIVE"
        || grant_scope_kind != invocation.scope_kind
        || grant_task_id != invocation.task_id.clone().unwrap_or_default()
        || grant_attempt_id != invocation.attempt_id.clone().unwrap_or_default()
        || grant_capability_ref
            != invocation_capability_ref_json(
                transaction,
                &commit.workspace_id,
                &commit.invocation_id,
            )?
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_GRANT_MISMATCH".to_owned(),
        ));
    }
    let operations: Vec<String> = serde_json::from_str(&grant_operations).map_err(|_| {
        StoreError::Integrity("CapabilityGrant operations JSON is corrupt".to_owned())
    })?;
    if !operations
        .iter()
        .any(|operation| operation == &invocation.operation)
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_OPERATION_NOT_GRANTED".to_owned(),
        ));
    }
    if grant_expires_at
        .as_deref()
        .is_some_and(|expires| !timestamp_is_future(expires))
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_GRANT_EXPIRED".to_owned(),
        ));
    }

    // The activation that makes the capability available on this Runtime.
    let activation: Option<(String, String, String, String, String, String)> = transaction
        .query_row(
            "SELECT a.scope_kind,COALESCE(a.task_id,''),COALESCE(a.attempt_id,''),
                    a.capability_ref_json,a.runtime_id,a.status
             FROM capability_activations a WHERE a.activation_id=?1",
            params![invocation.activation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((
        activation_scope_kind,
        activation_task_id,
        activation_attempt_id,
        activation_capability_ref,
        activation_runtime,
        activation_status,
    )) = activation
    else {
        return Err(StoreError::NotFound);
    };
    if activation_status != "ACTIVE"
        || activation_scope_kind != invocation.scope_kind
        || activation_task_id != invocation.task_id.clone().unwrap_or_default()
        || activation_attempt_id != invocation.attempt_id.clone().unwrap_or_default()
        || activation_capability_ref
            != invocation_capability_ref_json(
                transaction,
                &commit.workspace_id,
                &commit.invocation_id,
            )?
        || activation_runtime != commit.runtime_id
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_ACTIVATION_MISMATCH".to_owned(),
        ));
    }

    // The exact Trust decision must name this request, grant, activation, scope
    // and action binding.
    let decision = &commit.trust_decision;
    if decision.request_digest != invocation.request_digest
        || decision.capability_grant_id != invocation.capability_grant_id
        || decision.activation_id != invocation.activation_id
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_TRUST_DECISION_BINDING_MISMATCH".to_owned(),
        ));
    }
    let scope_digest = invocation_scope_digest(&commit.workspace_id, &invocation)?;
    if decision.scope_digest != scope_digest {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_TRUST_SCOPE_MISMATCH".to_owned(),
        ));
    }
    let target_digest = digest(&canonical_json(&effect.target)?);
    let action_digest = digest(&canonical_json(&json!({
        "capability_ref": invocation_capability_ref(transaction, &commit.workspace_id, &commit.invocation_id)?,
        "operation": invocation.operation,
        "target_digest": target_digest,
        "execution_method": invocation.execution_method,
        "effect_id": effect.effect_id,
    }))?);
    if decision.action_digest != action_digest {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_TRUST_ACTION_MISMATCH".to_owned(),
        ));
    }

    // The exact Approval that authorizes this Effect, and its single use.
    let approval = load_approval(transaction, &commit.workspace_id, &commit.approval_id)?;
    if approval.status != "APPROVED"
        || approval.task_id != effect.task_id
        || approval.requested_by_attempt.as_deref() != Some(effect.attempt_id.as_str())
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_APPROVAL_NOT_APPROVED".to_owned(),
        ));
    }
    if approval
        .expires_at
        .as_deref()
        .is_some_and(|expires| !timestamp_is_future(expires))
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_APPROVAL_EXPIRED".to_owned(),
        ));
    }
    let expected_target = json!({
        "capability_ref": invocation_capability_ref(transaction, &commit.workspace_id, &commit.invocation_id)?,
        "target_digest": target_digest,
        "execution_method": invocation.execution_method,
        "effect_id": effect.effect_id,
    });
    if approval.kind != invocation.operation
        || approval.scope_digest != scope_digest
        || approval.target_ref != expected_target
    {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_APPROVAL_BINDING_MISMATCH".to_owned(),
        ));
    }
    let expected_action_digest = digest(&canonical_json(&json!({
        "task_id": approval.task_id,
        "attempt_id": approval.requested_by_attempt,
        "action": approval.kind,
        "target": approval.target_ref,
        "scope_digest": approval.scope_digest,
    }))?);
    if approval.action_digest != expected_action_digest {
        return Err(StoreError::Invalid(
            "CAPABILITY_DISPATCH_APPROVAL_ACTION_DIGEST_MISMATCH".to_owned(),
        ));
    }
    let consumed: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM approval_uses WHERE approval_id=?1)",
            params![commit.approval_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if consumed {
        return Err(StoreError::Invalid("APPROVAL_ALREADY_CONSUMED".to_owned()));
    }

    let mut next_invocation = invocation.clone();
    next_invocation.status = InvocationStatus::Dispatched;
    next_invocation.effect_id = Some(effect.effect_id.clone());
    next_invocation.updated_at = commit.events.invocation.recorded_at.clone();
    next_invocation.version = invocation
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("CapabilityInvocation version overflow".to_owned()))?;
    let mut next_effect = effect.clone();
    next_effect.state = EffectState::Started;
    next_effect.dispatch_ordinal = effect
        .dispatch_ordinal
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("Effect dispatch ordinal overflow".to_owned()))?;
    next_effect.updated_at = commit.events.effect.recorded_at.clone();
    next_effect.version = effect
        .version
        .checked_add(1)
        .ok_or_else(|| StoreError::Invalid("Effect version overflow".to_owned()))?;

    Ok(AdmissionState {
        invocation,
        effect,
        next_invocation,
        next_effect,
        approval_action_digest: approval.action_digest,
        approval_scope_digest: approval.scope_digest,
        approval_target: approval.target_ref,
        approval_kind: approval.kind,
    })
}

#[allow(clippy::too_many_arguments)]
fn admit_transaction(
    connection: &mut Connection,
    commit: AdmitCapabilityDispatchCommit,
    principal: String,
    fingerprint: String,
    admission: AdmissionState,
    invocation_state: AggregateStateRef,
    effect_state: AggregateStateRef,
    approval_use_state: AggregateStateRef,
) -> Result<CommittedCapabilityDispatch, StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_database_error)?;
    authorize_runtime(
        &transaction,
        &commit.workspace_id,
        &commit.runtime_id,
        &commit.runtime_incarnation_id,
    )?;
    if let Some(prior) = load_replay(&transaction, &principal, &commit.request_id, &fingerprint)? {
        transaction.commit().map_err(map_database_error)?;
        return Ok(prior);
    }
    // Re-run every admission check against the current rows inside the write
    // transaction. A concurrent revocation, expiry or replacement fails here
    // before any row is written.
    verify_admission(&transaction, &commit)?;

    // The ApprovalUse is recorded before the Invocation advances to DISPATCHED so the
    // database guard that refuses a dispatch without a consumed approval for the exact
    // Effect observes it inside this transaction.
    transaction
        .execute(
            "INSERT INTO approval_uses(approval_use_id,approval_id,effect_id,capability_grant_id,request_digest,consumed_at)
             VALUES(?1,?2,?3,NULL,?4,?5)",
            params![
                commit.approval_use_id,
                commit.approval_id,
                admission.next_effect.effect_id,
                admission.next_invocation.request_digest,
                commit.events.approval_use.recorded_at,
            ],
        )
        .map_err(map_database_error)?;

    let changed = transaction
        .execute(
            "UPDATE capability_invocations SET status='DISPATCHED',effect_id=?1,updated_at=?2,version=?3
             WHERE workspace_id=?4 AND invocation_id=?5 AND status='CREATED' AND version=?6 AND effect_id IS NULL",
            params![
                admission.next_effect.effect_id,
                admission.next_invocation.updated_at,
                to_sql_i64(admission.next_invocation.version, "CapabilityInvocation version")?,
                commit.workspace_id,
                admission.next_invocation.invocation_id,
                to_sql_i64(admission.invocation.version, "CapabilityInvocation version")?,
            ],
        )
        .map_err(map_database_error)?;
    if changed != 1 {
        return Err(StoreError::Conflict {
            expected: Some(admission.invocation.version),
            actual: None,
        });
    }
    let started = transaction
        .execute(
            "UPDATE effects SET state='STARTED',dispatch_ordinal=?1,updated_at=?2,version=?3
             WHERE task_id=?4 AND effect_id=?5 AND state='PROPOSED' AND version=?6",
            params![
                to_sql_i64(
                    u64::from(admission.next_effect.dispatch_ordinal),
                    "Effect dispatch ordinal",
                )?,
                admission.next_effect.updated_at,
                to_sql_i64(admission.next_effect.version, "Effect version")?,
                admission.next_effect.task_id,
                admission.next_effect.effect_id,
                to_sql_i64(admission.effect.version, "Effect version")?,
            ],
        )
        .map_err(map_database_error)?;
    if started != 1 {
        return Err(StoreError::Conflict {
            expected: Some(admission.effect.version),
            actual: None,
        });
    }

    let invocation_event = invocation_dispatch_event(&commit.events.invocation, &admission)?;
    let effect_event =
        effect_started_event(&commit.events.effect, &commit.workspace_id, &admission)?;
    let approval_event = approval_use_event(&commit.events.approval_use, &commit, &admission)?;
    let invocation_event = insert_domain_event(&transaction, &invocation_event, &invocation_state)?;
    let effect_event = insert_domain_event(&transaction, &effect_event, &effect_state)?;
    let approval_event = insert_domain_event(&transaction, &approval_event, &approval_use_state)?;

    transaction
        .execute(
            "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at)
             VALUES(?1,'capability_invocation',?2,?3,1,?4,?5,?6)",
            params![
                commit.workspace_id,
                admission.next_invocation.invocation_id,
                to_sql_i64(admission.next_invocation.version, "CapabilityInvocation version")?,
                invocation_state.blob.digest,
                encode(&invocation_state.blob)?,
                commit.events.invocation.recorded_at,
            ],
        )
        .map_err(map_database_error)?;
    transaction
        .execute(
            "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at)
             VALUES(?1,'effect',?2,?3,1,?4,?5,?6)",
            params![
                commit.workspace_id,
                admission.next_effect.effect_id,
                to_sql_i64(admission.next_effect.version, "Effect version")?,
                effect_state.blob.digest,
                encode(&effect_state.blob)?,
                commit.events.effect.recorded_at,
            ],
        )
        .map_err(map_database_error)?;

    let principal_json = json!({
        "principal_id": commit.runtime_id,
        "kind": "RUNTIME",
    });
    let audit_digest = digest(&canonical_json(&json!({
        "invocation_id": admission.next_invocation.invocation_id,
        "effect_id": admission.next_effect.effect_id,
        "approval_id": commit.approval_id,
        "action_digest": admission.approval_action_digest,
        "scope_digest": admission.approval_scope_digest,
    }))?);
    transaction
        .execute(
            "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest)
             VALUES(?1,?2,?3,'CAPABILITY_DISPATCH_ADMITTED',?4,?5,?6,?7,?8,?9)",
            params![
                format!("{}:audit", commit.events.effect.event_id),
                commit.workspace_id,
                encode(&principal_json)?,
                encode(&json!({
                    "invocation_id": admission.next_invocation.invocation_id,
                    "effect_id": admission.next_effect.effect_id,
                }))?,
                commit.trust_decision.decision,
                commit.trust_decision.reason_code,
                commit.events.effect.correlation_id,
                commit.events.effect.recorded_at,
                audit_digest,
            ],
        )
        .map_err(map_database_error)?;

    let committed = CommittedCapabilityDispatch {
        invocation_id: admission.next_invocation.invocation_id.clone(),
        invocation_version: admission.next_invocation.version,
        effect_id: admission.next_effect.effect_id.clone(),
        effect_version: admission.next_effect.version,
        effect_state: admission.next_effect.state.as_str().to_owned(),
        dispatch_ordinal: admission.next_effect.dispatch_ordinal,
        approval_use_id: commit.approval_use_id.clone(),
        approval_id: commit.approval_id.clone(),
        events: vec![invocation_event, effect_event, approval_event],
        replayed: false,
    };
    let response = encode(&committed)?;
    transaction
        .execute(
            "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at)
             VALUES(?1,?2,?3,?4,?5,?6,NULL)",
            params![
                principal,
                commit.request_id,
                fingerprint,
                response,
                digest(response.as_bytes()),
                commit.events.effect.recorded_at,
            ],
        )
        .map_err(map_database_error)?;
    transaction.commit().map_err(map_database_error)?;
    Ok(committed)
}

/// The immutable single-use ApprovalUse record committed with the admission.
fn approval_use_record(
    commit: &AdmitCapabilityDispatchCommit,
    admission: &AdmissionState,
) -> Value {
    json!({
        "approval_use_id": commit.approval_use_id,
        "approval_id": commit.approval_id,
        "effect_id": admission.next_effect.effect_id,
        "capability_grant_id": Value::Null,
        "request_digest": admission.next_invocation.request_digest,
        "consumed_at": commit.events.approval_use.recorded_at,
    })
}

fn invocation_dispatch_event(
    context: &storage_core::CapabilityInvocationEventContext,
    admission: &AdmissionState,
) -> Result<EventDraft, StoreError> {
    Ok(EventDraft {
        event_id: context.event_id.clone(),
        workspace_id: admission.next_invocation.workspace_id.clone(),
        entity_type: "CapabilityInvocation".to_owned(),
        entity_id: admission.next_invocation.invocation_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: admission.next_invocation.version,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: "capability.invocation.status.changed.v1".to_owned(),
        payload: json!({
            "invocation_id": admission.next_invocation.invocation_id,
            "from": admission.invocation.status.as_str(),
            "to": admission.next_invocation.status.as_str(),
            "result_refs": admission.next_invocation.result_refs,
            "effect_id": admission.next_effect.effect_id,
            "aggregate_version": admission.next_invocation.version,
        }),
        recorded_at: context.recorded_at.clone(),
    })
}

fn effect_started_event(
    context: &EffectEvidenceEventContext,
    workspace_id: &str,
    admission: &AdmissionState,
) -> Result<EventDraft, StoreError> {
    let target_digest = digest(&canonical_json(&admission.next_effect.target)?);
    Ok(EventDraft {
        event_id: context.event_id.clone(),
        workspace_id: workspace_id.to_owned(),
        entity_type: "effect".to_owned(),
        entity_id: admission.next_effect.effect_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: admission.next_effect.version,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: "effect.started.v1".to_owned(),
        payload: json!({
            "effect_id": admission.next_effect.effect_id,
            "task_id": admission.next_effect.task_id,
            "attempt_id": admission.next_effect.attempt_id,
            "from": admission.effect.state.as_str(),
            "to": admission.next_effect.state.as_str(),
            "operation": admission.next_effect.operation,
            "target_digest": target_digest,
            "dispatch_ordinal": admission.next_effect.dispatch_ordinal,
        }),
        recorded_at: context.recorded_at.clone(),
    })
}

fn approval_use_event(
    context: &EffectEvidenceEventContext,
    commit: &AdmitCapabilityDispatchCommit,
    admission: &AdmissionState,
) -> Result<EventDraft, StoreError> {
    Ok(EventDraft {
        event_id: context.event_id.clone(),
        workspace_id: commit.workspace_id.clone(),
        entity_type: "approval_use".to_owned(),
        entity_id: commit.approval_use_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: 1,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: "approval.use.consumed.v1".to_owned(),
        payload: json!({
            "approval_use_id": commit.approval_use_id,
            "approval_id": commit.approval_id,
            "effect_id": admission.next_effect.effect_id,
            "request_digest": admission.next_invocation.request_digest,
            "consumed_at": context.recorded_at,
        }),
        recorded_at: context.recorded_at.clone(),
    })
}

/// The Workspace-scoped digest of the Invocation scope, matching the canonical
/// Trust scope binding used when the Approval was requested.
fn invocation_scope_digest(
    workspace_id: &str,
    invocation: &CapabilityInvocationRecord,
) -> Result<String, StoreError> {
    Ok(digest(&canonical_json(&json!({
        "workspace_id": workspace_id,
        "scope": invocation_scope_value(invocation),
    }))?))
}

fn invocation_scope_value(invocation: &CapabilityInvocationRecord) -> Value {
    match invocation.scope_kind.as_str() {
        "CONVERSATION" => {
            json!({"kind": "CONVERSATION", "conversation_id": invocation.conversation_id})
        }
        "TASK_PLANNING" => json!({"kind": "TASK_PLANNING", "task_id": invocation.task_id}),
        _ => json!({
            "kind": "ATTEMPT_EXECUTION",
            "task_id": invocation.task_id,
            "attempt_id": invocation.attempt_id,
        }),
    }
}

fn invocation_capability_ref(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    invocation_id: &str,
) -> Result<Value, StoreError> {
    let raw: String = transaction
        .query_row(
            "SELECT capability_ref_json FROM capability_invocations WHERE workspace_id=?1 AND invocation_id=?2",
            params![workspace_id, invocation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)?;
    serde_json::from_str(&raw).map_err(|_| {
        StoreError::Integrity("CapabilityInvocation CapabilityRef JSON is corrupt".to_owned())
    })
}

fn invocation_capability_ref_json(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    invocation_id: &str,
) -> Result<String, StoreError> {
    transaction
        .query_row(
            "SELECT capability_ref_json FROM capability_invocations WHERE workspace_id=?1 AND invocation_id=?2",
            params![workspace_id, invocation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?
        .ok_or(StoreError::NotFound)
}

fn load_invocation(
    connection: &Connection,
    workspace: &str,
    invocation_id: &str,
) -> Result<CapabilityInvocationRecord, StoreError> {
    let raw: Option<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        Option<String>,
        i64,
    )> = connection
        .query_row(
            "SELECT invocation_id,workspace_id,scope_kind,conversation_id,task_id,attempt_id,
             agent_session_id,capability_grant_id,activation_id,capability_ref_json,operation,
             request_digest,execution_method,idempotency_key,status,
             provider_task_status,provider_task_created_at,provider_task_expires_at,
             provider_task_ttl_ms,provider_poll_after_ms,provider_updated_at,
             partial_result_refs_json,result_refs_json,effect_id,failure_json,created_at,
             updated_at,completed_at,version
             FROM capability_invocations WHERE workspace_id=?1 AND invocation_id=?2",
            params![workspace, invocation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                    row.get(17)?,
                    row.get(18)?,
                    row.get(19)?,
                    row.get(20)?,
                    row.get(21)?,
                    row.get(22)?,
                    row.get(23)?,
                    row.get(24)?,
                    row.get(25)?,
                    row.get(26)?,
                    row.get(27)?,
                    row.get(28)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some(raw) = raw else {
        return Err(StoreError::NotFound);
    };
    let parse_json = |value: String, field: &str| {
        serde_json::from_str(&value).map_err(|_| {
            StoreError::Integrity(format!("CapabilityInvocation {field} JSON is corrupt"))
        })
    };
    let record = CapabilityInvocationRecord {
        invocation_id: raw.0,
        workspace_id: raw.1,
        scope_kind: raw.2,
        conversation_id: raw.3,
        task_id: raw.4,
        attempt_id: raw.5,
        agent_session_id: raw.6,
        capability_grant_id: raw.7,
        activation_id: raw.8,
        capability_ref: parse_json(raw.9, "CapabilityRef")?,
        operation: raw.10,
        request_digest: raw.11,
        execution_method: raw.12,
        action_batch_id: None,
        action_batch_ordinal: None,
        action_batch_operation_count: None,
        action_batch_digest: None,
        idempotency_key: raw.13,
        status: InvocationStatus::parse(&raw.14).map_err(|error| {
            StoreError::Invalid(format!("CapabilityInvocation status is invalid: {error:?}"))
        })?,
        provider_task_status: raw.15,
        provider_task_created_at: raw.16,
        provider_task_expires_at: raw.17,
        provider_task_ttl_ms: raw
            .18
            .map(|value| {
                u64::try_from(value)
                    .map_err(|_| StoreError::Integrity("Invocation TTL is out of range".to_owned()))
            })
            .transpose()?,
        provider_poll_after_ms: raw
            .19
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    StoreError::Integrity("Invocation poll delay is out of range".to_owned())
                })
            })
            .transpose()?,
        provider_updated_at: raw.20,
        partial_result_refs: parse_json(raw.21, "partial-result refs")?,
        result_refs: parse_json(raw.22, "result refs")?,
        effect_id: raw.23,
        failure: raw
            .24
            .map(|value| parse_json(value, "failure"))
            .transpose()?,
        created_at: raw.25,
        updated_at: raw.26,
        completed_at: raw.27,
        version: u64::try_from(raw.28)
            .map_err(|_| StoreError::Integrity("Invocation version is out of range".to_owned()))?,
    };
    Ok(record)
}

fn load_effect(
    connection: &Connection,
    workspace_id: &str,
    effect_id: &str,
) -> Result<EffectRecord, StoreError> {
    let raw: Option<(String, String, String, Option<String>, String, String, Option<String>, String, String, String, String, Option<String>, Option<String>, Option<String>, String, String, i64, i64)> = connection
        .query_row(
            "SELECT effect_id,task_id,attempt_id,capability_ref_json,operation,target_json,
             idempotency_key,state,request_digest,capability_invocation_id,execution_method,
             result_ref_json,observed_state_json,verification_ref,created_at,updated_at,
             dispatch_ordinal,version
             FROM effects WHERE effect_id=?1 AND task_id IN (SELECT task_id FROM tasks WHERE workspace_id=?2)",
            params![effect_id, workspace_id],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?,
                    row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                    row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?, row.get(17)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some(raw) = raw else {
        return Err(StoreError::NotFound);
    };
    let parse_json = |value: String, field: &str| {
        serde_json::from_str(&value)
            .map_err(|_| StoreError::Integrity(format!("Effect {field} JSON is corrupt")))
    };
    let record = EffectRecord {
        effect_id: raw.0,
        task_id: raw.1,
        attempt_id: raw.2,
        capability_ref: raw
            .3
            .map(|value| parse_json(value, "CapabilityRef"))
            .transpose()?,
        operation: raw.4,
        target: parse_json(raw.5, "target")?,
        idempotency_key: raw.6,
        state: match raw.7.as_str() {
            "PROPOSED" => EffectState::Proposed,
            "STARTED" => EffectState::Started,
            "ACKNOWLEDGED" => EffectState::Acknowledged,
            "RECONCILING" => EffectState::Reconciling,
            "OBSERVED" => EffectState::Observed,
            "VERIFIED" => EffectState::Verified,
            "FAILED" => EffectState::Failed,
            "AMBIGUOUS" => EffectState::Ambiguous,
            _ => return Err(StoreError::Integrity("Effect state is invalid".to_owned())),
        },
        request_digest: raw.8,
        capability_invocation_id: raw.9,
        execution_method: match raw.10.as_str() {
            "STRUCTURED_API" => ExecutionMethod::StructuredApi,
            "STRUCTURED_BROWSER" => ExecutionMethod::StructuredBrowser,
            "ACCESSIBILITY_BROWSER" => ExecutionMethod::AccessibilityBrowser,
            "SCREEN_COMPUTER_USE" => ExecutionMethod::ScreenComputerUse,
            "DETERMINISTIC_LOCAL" => ExecutionMethod::DeterministicLocal,
            "NATIVE_AGENT_TOOL" => ExecutionMethod::NativeAgentTool,
            "UNKNOWN" => ExecutionMethod::Unknown,
            _ => {
                return Err(StoreError::Integrity(
                    "Effect execution method is invalid".to_owned(),
                ));
            }
        },
        dispatch_ordinal: u32::try_from(raw.16).map_err(|_| {
            StoreError::Integrity("Effect dispatch ordinal is out of range".to_owned())
        })?,
        result_ref: raw
            .11
            .map(|value| parse_json(value, "result ref"))
            .transpose()?,
        observed_state: raw
            .12
            .map(|value| parse_json(value, "observed state"))
            .transpose()?,
        verification_ref: raw.13,
        created_at: raw.14,
        updated_at: raw.15,
        version: u64::try_from(raw.17)
            .map_err(|_| StoreError::Integrity("Effect version is out of range".to_owned()))?,
    };
    Ok(record)
}

struct StoredApproval {
    approval_id: String,
    task_id: String,
    requested_by_attempt: Option<String>,
    kind: String,
    target_ref: Value,
    scope_digest: String,
    action_digest: String,
    status: String,
    expires_at: Option<String>,
}

fn load_approval(
    connection: &Connection,
    workspace_id: &str,
    approval_id: &str,
) -> Result<StoredApproval, StoreError> {
    let raw: Option<(
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    )> = connection
        .query_row(
            "SELECT a.approval_id,a.task_id,a.requested_by_attempt,a.kind,a.target_ref_json,
                    a.scope_digest,a.action_digest,a.status,a.expires_at
             FROM approvals a JOIN tasks t ON t.task_id=a.task_id
             WHERE a.approval_id=?1 AND t.workspace_id=?2",
            params![approval_id, workspace_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(map_database_error)?;
    let Some(raw) = raw else {
        return Err(StoreError::NotFound);
    };
    let target_ref = serde_json::from_str(&raw.4).map_err(|_| {
        StoreError::Integrity("Approval target reference JSON is corrupt".to_owned())
    })?;
    Ok(StoredApproval {
        approval_id: raw.0,
        task_id: raw.1,
        requested_by_attempt: raw.2,
        kind: raw.3,
        target_ref,
        scope_digest: raw.5,
        action_digest: raw.6,
        status: raw.7,
        expires_at: raw.8,
    })
}

fn put_aggregate_state<T: Serialize>(
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    record: &T,
    content_type: &str,
    revision: u64,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(record)?;
    let blob = store.inner.blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        content_type,
    )?;
    if blob.digest != digest(&bytes) || blob.size_bytes != bytes.len() as u64 {
        return Err(StoreError::Integrity(
            "Capability dispatch aggregate-state blob failed verification".to_owned(),
        ));
    }
    Ok(AggregateStateRef {
        blob,
        entity_revision: revision,
        record_schema_version: 1,
    })
}

fn runtime_principal(runtime: &str, incarnation: &str) -> String {
    format!("runtime:{runtime}:{incarnation}")
}

fn encode<T: Serialize>(value: &T) -> Result<String, StoreError> {
    let bytes = canonical_json(value)?;
    String::from_utf8(bytes).map_err(|error| StoreError::Invalid(error.to_string()))
}

fn authorize_runtime(
    connection: &Connection,
    workspace: &str,
    runtime: &str,
    incarnation: &str,
) -> Result<(), StoreError> {
    let authorized: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces w JOIN runtimes r ON r.runtime_id=?2
               JOIN runtime_incarnations i ON i.runtime_id=r.runtime_id AND i.runtime_incarnation_id=?3
                 AND i.recovery_state IN ('READY','DEGRADED')
               JOIN runtime_workspace_bindings b ON b.runtime_id=r.runtime_id AND b.workspace_id=w.workspace_id
                 AND b.status='ACTIVE' AND b.revoked_at IS NULL
               WHERE w.workspace_id=?1 AND w.status='ACTIVE' AND r.current_incarnation_id=i.runtime_incarnation_id
                 AND r.availability IN ('ONLINE','DEGRADED')
                 AND EXISTS(SELECT 1 FROM json_each(b.roles_json) role WHERE role.value='EXECUTOR'))",
            params![workspace, runtime, incarnation],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if authorized {
        Ok(())
    } else {
        Err(StoreError::Invalid(
            "Runtime incarnation is not authorized to dispatch CapabilityInvocations in this Workspace"
                .to_owned(),
        ))
    }
}

fn load_replay(
    transaction: &Transaction<'_>,
    principal: &str,
    request_id: &str,
    fingerprint: &str,
) -> Result<Option<CommittedCapabilityDispatch>, StoreError> {
    let prior: Option<(String, Option<String>)> = transaction
        .query_row(
            "SELECT request_digest,response_json FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            params![principal, request_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match prior {
        None => Ok(None),
        Some((digest, response)) if digest == fingerprint => response
            .map(|value| {
                let mut committed: CommittedCapabilityDispatch = serde_json::from_str(&value)
                    .map_err(|_| {
                        StoreError::Integrity("Capability dispatch receipt is corrupt".to_owned())
                    })?;
                committed.replayed = true;
                Ok(committed)
            })
            .transpose(),
        Some(_) => Err(StoreError::IdempotencyConflict),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Compare an RFC3339 timestamp against the store clock. An unparsable
/// timestamp is treated as not in the future and therefore fails closed.
fn timestamp_is_future(value: &str) -> bool {
    let Ok(parsed) =
        time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
    else {
        return false;
    };
    parsed > time::OffsetDateTime::now_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_core::{CapabilityDispatchEvents, TrustDispatchDecision};
    use std::time::Duration;

    const AT: &str = "2026-10-20T10:00:00Z";
    const FUTURE: &str = "2099-01-01T00:00:00Z";
    const CAPABILITY_REF: &str = "{\"capability_id\":\"mail.send\",\"identity_kind\":\"PACKAGE_COMPONENT\",\"source\":\"test:litespm\",\"package_version\":\"1.0.0\",\"digest\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}";

    #[derive(Clone)]
    struct TestKeys([u8; 32]);

    impl WorkspaceBlobKeyProvider for TestKeys {
        fn current_key(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new(self.0),
            })
        }

        fn key_by_version(
            &self,
            _workspace_id: &str,
            _purpose: BlobPurpose,
            version: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version,
                bytes: zeroize::Zeroizing::new(self.0),
            })
        }
    }

    fn state_database(directory: &tempfile::TempDir) -> std::path::PathBuf {
        directory.path().join("state").join("state.sqlite3")
    }

    fn test_store(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
        let blob_store = Arc::new(FileBlobStore::new(
            directory.path().join("blobs"),
            TestKeys([29_u8; 32]),
        ));
        SqliteWorkspaceStore::open(
            state_database(directory),
            blob_store,
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: Duration::from_secs(1),
            },
        )
        .expect("SQLite store opens")
    }

    /// The combined admission writer needs the lead-owned migration that
    /// replaces the two `sqlite-v15.sql` guards. The fixture applies the
    /// proposed replacement so the transaction itself can be exercised.
    fn apply_proposed_dispatch_ddl(connection: &Connection) {
        // The production schema now carries the dispatch admission itself: version
        // twenty-six drops the blanket creation block, allows CREATED to DISPATCHED in
        // the transition guard, and refuses a dispatch that is not bound to a consumed
        // approval for the exact Effect. Nothing is patched on top of the shipped
        // schema any more, so this helper verifies the shipped guards are in place.
        let guard: String = connection
            .query_row(
                "SELECT COALESCE((SELECT sql FROM sqlite_master WHERE type='trigger' AND name='capability_invocation_dispatch_requires_consumed_approval'), '')",
                [],
                |row| row.get(0),
            )
            .expect("read the dispatch approval guard");
        assert!(
            guard.contains("INVOCATION_DISPATCH_REQUIRES_CONSUMED_APPROVAL"),
            "the production schema must refuse a dispatch without a consumed approval"
        );
        let closed: String = connection
            .query_row(
                "SELECT COALESCE((SELECT sql FROM sqlite_master WHERE type='trigger' AND name='capability_invocation_creation_admission_closed'), '')",
                [],
                |row| row.get(0),
            )
            .expect("read the removed creation guard");
        assert!(
            closed.is_empty(),
            "the blanket creation block must be gone in the production schema"
        );
    }

    fn seed_fixture(directory: &tempfile::TempDir) -> Result<(), StoreError> {
        let connection =
            Connection::open(state_database(directory)).expect("open fixture database");
        connection
            .pragma_update(None, "foreign_keys", false)
            .expect("disable fixture foreign keys");
        connection
            .execute_batch("DROP TRIGGER environment_requires_issued_preview;")
            .expect("relax the unrelated Environment preview guard for the fixture");
        connection
            .execute_batch(&format!(
                "INSERT INTO workspaces(workspace_id,name,owner_principal_id,replication_policy,status,created_at,updated_at,version)
                 VALUES('ws','Workspace','owner','LOCAL_ONLY','ACTIVE','{AT}','{AT}',1);
                 INSERT INTO runtimes(runtime_id,device_identity_json,runtime_version,platform,architecture,roles_json,trust_zone,availability,startup_policy,current_incarnation_id,resource_capacity_json,last_seen,version)
                 VALUES('runtime','{{}}','0.1','linux','x86_64','[]','LOCAL','ONLINE','MANUAL','incarnation','{{}}','{AT}',1);
                 INSERT INTO runtime_incarnations(runtime_incarnation_id,runtime_id,process_started_at,litecowork_version,recovered_from_unclean_shutdown,recovery_state,ready_at,version)
                 VALUES('incarnation','runtime','{AT}','0.1',0,'READY','{AT}',1);
                 INSERT INTO runtime_workspace_bindings(runtime_workspace_binding_id,runtime_id,workspace_id,enrollment_mode,status,roles_json,created_at,activated_at,version)
                 VALUES('binding','runtime','ws','LOCAL_ENROLLMENT','ACTIVE','[\"EXECUTOR\"]','{AT}','{AT}',1);
                 INSERT INTO agent_bindings(agent_binding_id,workspace_id,agent_profile_id,runtime_id,endpoint_selection_policy_json,configuration_json,enabled,lead_eligible,created_at,version)
                 VALUES('binding-1','ws','profile-1','runtime','{{}}','{{}}',1,1,'{AT}',1);
                 INSERT INTO task_spec_revisions(task_id,workspace_id,revision,parent_revisions_json,objective,constraints_json,non_goals_json,input_refs_json,required_outputs_json,acceptance_criteria_json,approvals_required_json,lead_failover_policy_json,source_message_refs_json,placement_preference,authored_by_json,created_at)
                 VALUES('task-1','ws',1,'[]','objective','[]','[]','[]','[]','[]','[]','{{\"mode\":\"DISABLED\",\"triggers\":[],\"fallback_agent_binding_ids\":[],\"max_lead_changes\":0}}','[]','LOCAL','{{\"principal_id\":\"owner\",\"kind\":\"USER\"}}','{AT}');
                 INSERT INTO tasks(task_id,workspace_id,current_spec_revision,current_plan_revision,status,lead_agent_binding_id,blocking_conditions_json,priority,created_by_json,created_at,updated_at,version)
                 VALUES('task-1','ws',1,1,'RUNNING','binding-1','[]','NORMAL','{{\"principal_id\":\"owner\",\"kind\":\"USER\"}}','{AT}','{AT}',1);
                 INSERT INTO agent_sessions(agent_session_id,workspace_id,scope_kind,task_id,task_spec_revision,attempt_id,agent_binding_id,endpoint_id,runtime_id,runtime_incarnation_id,status,started_at,last_event_at,version)
                 VALUES('session-1','ws','ATTEMPT_EXECUTION','task-1',1,'attempt-1','binding-1','endpoint-1','runtime','incarnation','ACTIVE','{AT}','{AT}',1);
                 INSERT INTO steps(step_id,task_id,plan_revision,logical_key,title,objective,dependencies_json,required_capabilities_json,acceptance_criteria_json,status,current_attempt_id,created_at,updated_at,version)
                 VALUES('step-1','task-1',1,'key-1','title','objective','[]','[]','[]','RUNNING','attempt-1','{AT}','{AT}',1);
                 INSERT INTO environments(environment_id,runtime_id,provider_kind,class,lifetime,owner_workspace_id,owner_task_id,owner_attempt_id,sharing_scope,name,created_by_incarnation_id,status,health,budget_enforcement_policy,budget_enforcement,resource_limits_json,network_policy_json,budget_ceiling_json,backup_policy,created_at,updated_at,version)
                 VALUES('environment-1','runtime','LOCAL','DETERMINISTIC','ATTEMPT','ws','task-1','attempt-1','ATTEMPT_PRIVATE','Local','incarnation','READY','HEALTHY','ALLOW_HOST_MONITORED','HOST_MONITORED','{{}}','{{}}','{{}}','EXCLUDED','{AT}','{AT}',1);
                 INSERT INTO attempts(attempt_id,task_id,step_id,agent_binding_id,agent_session_id,runtime_id,runtime_incarnation_id,environment_id,capability_grant_ids_json,execution_lease_id,failover_class,status,started_at,created_at,version)
                 VALUES('attempt-1','task-1','step-1','binding-1','session-1','runtime','incarnation','environment-1','[]','lease-1','SAFE_PORTABLE','RUNNING','{AT}','{AT}',1);
                 INSERT INTO execution_leases(lease_id,task_id,step_id,attempt_id,runtime_id,runtime_incarnation_id,epoch,issuer_key_version,fencing_token_digest,state,acquired_at,renew_by,expires_at,version)
                 VALUES('lease-1','task-1','step-1','attempt-1','runtime','incarnation',7,1,'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc','ACTIVE','{AT}','{AT}','{FUTURE}',1);
                 INSERT INTO plan_revisions(task_id,revision,task_spec_revision,produced_by_agent_session_id,produced_by_attempt_id,steps_json,created_at)
                 VALUES('task-1',1,1,'session-1','attempt-1','[]','{AT}');
                 INSERT INTO capability_grants(capability_grant_id,scope_kind,task_id,attempt_id,capability_ref_json,allowed_operations_json,resource_scope_json,secret_refs_json,granted_by_json,expires_at,status,created_at,version)
                 VALUES('grant-1','ATTEMPT_EXECUTION','task-1','attempt-1','{CAPABILITY_REF}','[\"external.message.send\"]','{{\"operation_ids\":[\"external.message.send\"]}}','[]','{{\"principal_id\":\"owner\",\"kind\":\"USER\"}}','{FUTURE}','ACTIVE','{AT}',1);
                 INSERT INTO capability_activations(activation_id,workspace_id,scope_kind,task_id,attempt_id,capability_ref_json,runtime_id,runtime_incarnation_id,mode,status,health,created_at,updated_at,version)
                 VALUES('activation-1','ws','ATTEMPT_EXECUTION','task-1','attempt-1','{CAPABILITY_REF}','runtime','incarnation','MCP','ACTIVE','HEALTHY','{AT}','{AT}',1);"
            ))
            .map_err(|error| {
                StoreError::Database(format!("seed dispatch fixture failed: {error}"))
            })?;
        apply_proposed_dispatch_ddl(&connection);
        connection
            .execute_batch(&format!(
                "INSERT INTO capability_invocations(invocation_id,workspace_id,scope_kind,task_id,attempt_id,agent_session_id,activation_id,capability_ref_json,operation,request_digest,execution_method,capability_grant_id,status,partial_result_refs_json,result_refs_json,created_at,updated_at,version)
                 VALUES('inv-1','ws','ATTEMPT_EXECUTION','task-1','attempt-1','session-1','activation-1','{CAPABILITY_REF}','external.message.send','sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd','STRUCTURED_API','grant-1','CREATED','[]','[]','{AT}','{AT}',1);
                 INSERT INTO effects(effect_id,task_id,attempt_id,capability_ref_json,operation,target_json,idempotency_key,state,request_digest,capability_invocation_id,execution_method,dispatch_ordinal,created_at,updated_at,version)
                 VALUES('effect-1','task-1','attempt-1','{CAPABILITY_REF}','external.message.send','{{\"mailbox\":\"user@example.test\"}}',NULL,'PROPOSED','sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd','inv-1','STRUCTURED_API',0,'{AT}','{AT}',1);"
            ))
            .map_err(|error| {
                StoreError::Database(format!("seed invocation and effect failed: {error}"))
            })?;
        seed_approval(&connection)?;
        Ok(())
    }

    fn scope_digest() -> String {
        digest(
            &canonical_json(&json!({
                "workspace_id": "ws",
                "scope": {
                    "kind": "ATTEMPT_EXECUTION",
                    "task_id": "task-1",
                    "attempt_id": "attempt-1",
                },
            }))
            .expect("canonical scope"),
        )
    }

    fn target_ref() -> Value {
        json!({
            "capability_ref": serde_json::from_str::<Value>(CAPABILITY_REF).expect("capability ref"),
            "target_digest": digest(
                &canonical_json(&json!({"mailbox": "user@example.test"})).expect("canonical target")
            ),
            "execution_method": "STRUCTURED_API",
            "effect_id": "effect-1",
        })
    }

    fn seed_approval(connection: &Connection) -> Result<(), StoreError> {
        let target = target_ref();
        let scope = scope_digest();
        let action_digest = digest(
            &canonical_json(&json!({
                "task_id": "task-1",
                "attempt_id": "attempt-1",
                "action": "external.message.send",
                "target": target,
                "scope_digest": scope,
            }))
            .expect("canonical action"),
        );
        connection
            .execute(
                "INSERT INTO approvals(approval_id,task_id,requested_by_attempt,kind,action_summary,target_ref_json,scope_digest,action_digest,risk,required_assurance,status,requested_at,expires_at,version)
                 VALUES('approval-1','task-1','attempt-1','external.message.send','Approve operation',?1,?2,?3,'SENSITIVE','APPROVE_SENSITIVE','APPROVED',?4,?5,1)",
                params![
                    encode(&target).expect("encode target"),
                    scope,
                    action_digest,
                    AT,
                    FUTURE
                ],
            )
            .map_err(|error| {
                StoreError::Database(format!("seed approval failed: {error}"))
            })?;
        Ok(())
    }

    fn trust_decision() -> TrustDispatchDecision {
        let capability_ref = serde_json::from_str::<Value>(CAPABILITY_REF).expect("capability ref");
        let target_digest = digest(
            &canonical_json(&json!({"mailbox": "user@example.test"})).expect("canonical target"),
        );
        TrustDispatchDecision {
            decision: "REQUIRE_APPROVAL".to_owned(),
            reason_code: "ApprovalRequired".to_owned(),
            policy_id: "litecowork.standard".to_owned(),
            policy_revision: 1,
            policy_content_digest: format!("sha256:{}", "e".repeat(64)),
            request_digest: format!("sha256:{}", "d".repeat(64)),
            action_digest: digest(
                &canonical_json(&json!({
                    "capability_ref": capability_ref,
                    "operation": "external.message.send",
                    "target_digest": target_digest,
                    "execution_method": "STRUCTURED_API",
                    "effect_id": "effect-1",
                }))
                .expect("canonical action"),
            ),
            scope_digest: scope_digest(),
            invocation_id: "inv-1".to_owned(),
            capability_grant_id: "grant-1".to_owned(),
            activation_id: "activation-1".to_owned(),
            effect_id: "effect-1".to_owned(),
        }
    }

    fn commit(request_id: &str) -> AdmitCapabilityDispatchCommit {
        AdmitCapabilityDispatchCommit {
            workspace_id: "ws".to_owned(),
            runtime_id: "runtime".to_owned(),
            runtime_incarnation_id: "incarnation".to_owned(),
            request_id: request_id.to_owned(),
            invocation_id: "inv-1".to_owned(),
            expected_invocation_version: 1,
            effect_id: "effect-1".to_owned(),
            expected_effect_version: 1,
            approval_id: "approval-1".to_owned(),
            approval_use_id: format!("{request_id}-use"),
            trust_decision: trust_decision(),
            events: CapabilityDispatchEvents {
                invocation: storage_core::CapabilityInvocationEventContext {
                    event_id: format!("{request_id}-invocation"),
                    origin_runtime_id: "runtime".to_owned(),
                    origin_runtime_incarnation_id: "incarnation".to_owned(),
                    hlc_timestamp: AT.to_owned(),
                    correlation_id: "corr".to_owned(),
                    causation_id: None,
                    recorded_at: AT.to_owned(),
                },
                effect: EffectEvidenceEventContext {
                    event_id: format!("{request_id}-effect"),
                    origin_runtime_id: "runtime".to_owned(),
                    origin_runtime_incarnation_id: "incarnation".to_owned(),
                    hlc_timestamp: AT.to_owned(),
                    correlation_id: "corr".to_owned(),
                    causation_id: None,
                    recorded_at: AT.to_owned(),
                },
                approval_use: EffectEvidenceEventContext {
                    event_id: format!("{request_id}-approval"),
                    origin_runtime_id: "runtime".to_owned(),
                    origin_runtime_incarnation_id: "incarnation".to_owned(),
                    hlc_timestamp: AT.to_owned(),
                    correlation_id: "corr".to_owned(),
                    causation_id: None,
                    recorded_at: AT.to_owned(),
                },
            },
        }
    }

    fn count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_direct_dispatch_without_a_consumed_approval_is_refused_by_the_database() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");

        // The version twenty-six guard refuses a dispatch that is not bound to a
        // consumed approval for the exact Effect. A direct status update therefore
        // cannot reach DISPATCHED, even with live session, grant and activation
        // authority, because it never records the ApprovalUse.
        let error = connection
            .execute(
                "UPDATE capability_invocations SET status='DISPATCHED', effect_id='effect-1', version=2 WHERE invocation_id='inv-1'",
                [],
            )
            .expect_err("a direct dispatch without a consumed approval must be refused");
        assert!(
            error
                .to_string()
                .contains("INVOCATION_DISPATCH_REQUIRES_CONSUMED_APPROVAL"),
            "unexpected database rejection: {error}"
        );
        let status: String = connection
            .query_row(
                "SELECT status FROM capability_invocations WHERE invocation_id='inv-1'",
                [],
                |row| row.get(0),
            )
            .expect("invocation row");
        assert_eq!(status, "CREATED");
        assert_eq!(count(&connection, "approval_uses"), 0);
    }

    #[test]
    fn admission_commits_dispatch_effect_start_and_single_approval_use() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        let committed = dispatch
            .admit_capability_dispatch(commit("request-1"))
            .expect("admission commits");

        assert_eq!(committed.invocation_id, "inv-1");
        assert_eq!(committed.invocation_version, 2);
        assert_eq!(committed.effect_id, "effect-1");
        assert_eq!(committed.effect_version, 2);
        assert_eq!(committed.effect_state, "STARTED");
        assert_eq!(committed.dispatch_ordinal, 1);
        assert!(!committed.replayed);
        assert_eq!(committed.events.len(), 3);

        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        let invocation: (String, String, Option<String>, i64) = connection
            .query_row(
                "SELECT status,execution_method,effect_id,version FROM capability_invocations WHERE invocation_id='inv-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("invocation row");
        assert_eq!(invocation.0, "DISPATCHED");
        assert_eq!(invocation.1, "STRUCTURED_API");
        assert_eq!(invocation.2.as_deref(), Some("effect-1"));
        assert_eq!(invocation.3, 2);

        let effect: (String, i64, i64) = connection
            .query_row(
                "SELECT state,dispatch_ordinal,version FROM effects WHERE effect_id='effect-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("effect row");
        assert_eq!(effect.0, "STARTED");
        assert_eq!(effect.1, 1);
        assert_eq!(effect.2, 2);

        let approval_use: (String, String, Option<String>, String) = connection
            .query_row(
                "SELECT approval_id,effect_id,capability_grant_id,request_digest FROM approval_uses WHERE approval_use_id='request-1-use'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("approval use row");
        assert_eq!(approval_use.0, "approval-1");
        assert_eq!(approval_use.1, "effect-1");
        assert_eq!(approval_use.2, None);
        assert_eq!(approval_use.3, format!("sha256:{}", "d".repeat(64)));

        assert_eq!(count(&connection, "approval_uses"), 1);
        assert_eq!(count(&connection, "domain_events"), 3);
        assert_eq!(count(&connection, "audit_records"), 1);
        assert_eq!(count(&connection, "request_dedup"), 1);
        assert_eq!(count(&connection, "aggregate_snapshots"), 2);
    }

    #[test]
    fn exact_replay_returns_the_first_admission_and_appends_nothing() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        let first = dispatch
            .admit_capability_dispatch(commit("request-1"))
            .expect("first admission");
        let replay = dispatch
            .admit_capability_dispatch(commit("request-1"))
            .expect("replayed admission");

        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(replay.effect_version, first.effect_version);
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        assert_eq!(count(&connection, "approval_uses"), 1);
        assert_eq!(count(&connection, "domain_events"), 3);
        assert_eq!(count(&connection, "request_dedup"), 1);
    }

    #[test]
    fn changed_content_under_the_same_request_id_conflicts() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        dispatch
            .admit_capability_dispatch(commit("request-1"))
            .expect("first admission");
        let mut changed = commit("request-1");
        changed.approval_use_id = "request-1-other-use".to_owned();
        let result = dispatch.admit_capability_dispatch(changed);

        assert!(matches!(result, Err(StoreError::IdempotencyConflict)));
    }

    #[test]
    fn a_second_use_of_the_same_approval_is_rejected() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        // A prior consumption of the same Approval already exists. The unique
        // `approval_id` constraint permits at most one use.
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        connection
            .execute(
                "INSERT INTO approval_uses(approval_use_id,approval_id,effect_id,capability_grant_id,request_digest,consumed_at)
                 VALUES('prior-use','approval-1','effect-1',NULL,?1,?2)",
                params![format!("sha256:{}", "d".repeat(64)), AT],
            )
            .expect("seed a prior approval use");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        let result = dispatch.admit_capability_dispatch(commit("request-1"));

        match result {
            Err(StoreError::Invalid(message)) => {
                assert!(
                    message.contains("APPROVAL_ALREADY_CONSUMED"),
                    "unexpected rejection: {message}"
                );
            }
            other => panic!("expected an approval replay rejection, got {other:?}"),
        }
        assert_eq!(count(&connection, "approval_uses"), 1);
        assert_eq!(count(&connection, "domain_events"), 0);
        let invocation_status: String = connection
            .query_row(
                "SELECT status FROM capability_invocations WHERE invocation_id='inv-1'",
                [],
                |row| row.get(0),
            )
            .expect("invocation row");
        assert_eq!(invocation_status, "CREATED");
    }

    #[test]
    fn an_expired_approval_never_starts_the_effect() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        connection
            .execute(
                "UPDATE approvals SET expires_at='2020-01-01T00:00:00Z' WHERE approval_id='approval-1'",
                [],
            )
            .expect("expire the approval");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        let result = dispatch.admit_capability_dispatch(commit("request-1"));

        match result {
            Err(StoreError::Invalid(message)) => {
                assert!(
                    message.contains("CAPABILITY_DISPATCH_APPROVAL_EXPIRED"),
                    "unexpected rejection: {message}"
                );
            }
            other => panic!("expected an expired approval rejection, got {other:?}"),
        }
        assert_eq!(count(&connection, "approval_uses"), 0);
        assert_eq!(count(&connection, "domain_events"), 0);
        let effect_state: String = connection
            .query_row(
                "SELECT state FROM effects WHERE effect_id='effect-1'",
                [],
                |row| row.get(0),
            )
            .expect("effect row");
        assert_eq!(effect_state, "PROPOSED");
    }

    #[test]
    fn an_approval_for_another_action_never_starts_the_effect() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        connection
            .execute(
                "UPDATE approvals SET action_digest=?1 WHERE approval_id='approval-1'",
                params![format!("sha256:{}", "f".repeat(64))],
            )
            .expect("tamper with the approval binding");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        let result = dispatch.admit_capability_dispatch(commit("request-1"));

        assert!(matches!(result, Err(StoreError::Invalid(_))));
        assert_eq!(count(&connection, "approval_uses"), 0);
        assert_eq!(count(&connection, "domain_events"), 0);
    }

    #[test]
    fn a_revoked_grant_never_starts_the_effect() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        connection
            .execute(
                "UPDATE capability_grants SET status='REVOKED' WHERE capability_grant_id='grant-1'",
                [],
            )
            .expect("revoke the grant");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());

        let result = dispatch.admit_capability_dispatch(commit("request-1"));

        assert!(matches!(result, Err(StoreError::Invalid(_))));
        assert_eq!(count(&connection, "approval_uses"), 0);
        assert_eq!(count(&connection, "domain_events"), 0);
    }

    #[test]
    fn a_trust_decision_for_another_request_never_starts_the_effect() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());
        let mut tampered = commit("request-1");
        tampered.trust_decision.request_digest = format!("sha256:{}", "a".repeat(64));

        let result = dispatch.admit_capability_dispatch(tampered);

        assert!(matches!(result, Err(StoreError::Invalid(_))));
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        assert_eq!(count(&connection, "approval_uses"), 0);
        assert_eq!(count(&connection, "domain_events"), 0);
    }

    #[test]
    fn a_denied_trust_decision_never_starts_the_effect() {
        let directory = tempfile::tempdir().expect("temporary store");
        let store = test_store(&directory);
        seed_fixture(&directory).expect("seed dispatch fixture");
        let dispatch = SqliteCapabilityDispatchStore::new(store.clone());
        let mut denied = commit("request-1");
        denied.trust_decision.decision = "DENY".to_owned();

        let result = dispatch.admit_capability_dispatch(denied);

        match result {
            Err(StoreError::Invalid(message)) => {
                assert!(
                    message.contains("CAPABILITY_DISPATCH_TRUST_DECISION_DENIED"),
                    "unexpected rejection: {message}"
                );
            }
            other => panic!("expected a denied trust decision rejection, got {other:?}"),
        }
    }

    #[test]
    fn the_current_schema_admits_creation_but_refuses_dispatch_without_a_consumed_approval() {
        // Version 26 replaced the v15 blanket creation block, because the combined
        // Trust + ApprovalUse + Invocation + Effect writer now exists. Creation through
        // live authority is therefore admitted, but a raw status transition that skips
        // the ApprovalUse still cannot reach DISPATCHED at the database boundary.
        let directory = tempfile::tempdir().expect("temporary store");
        let _store = test_store(&directory);
        seed_fixture(&directory).expect("the v26 schema admits a live-authority Invocation");
        let connection =
            Connection::open(state_database(&directory)).expect("open fixture database");
        assert_eq!(count(&connection, "capability_invocations"), 1);
        let status: String = connection
            .query_row(
                "SELECT status FROM capability_invocations WHERE invocation_id='inv-1'",
                [],
                |row| row.get(0),
            )
            .expect("seeded Invocation status");
        assert_eq!(status, "CREATED");
        assert_eq!(
            count(&connection, "approval_uses"),
            0,
            "the fixture has not consumed an Approval yet"
        );

        let refused = connection.execute(
            "UPDATE capability_invocations SET status='DISPATCHED', version=version+1 WHERE invocation_id='inv-1'",
            [],
        );

        match refused {
            Err(rusqlite::Error::SqliteFailure(_, Some(message))) => assert!(
                message.contains("INVOCATION_DISPATCH_REQUIRES_CONSUMED_APPROVAL"),
                "unexpected rejection: {message}"
            ),
            other => panic!("a dispatch without a consumed Approval must be refused, got {other:?}"),
        }
        assert_eq!(
            count(&connection, "approval_uses"),
            0,
            "a refused dispatch must not leave an ApprovalUse behind"
        );
    }
}
