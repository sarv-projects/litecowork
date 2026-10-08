use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    NotFound, AlreadyExists, Unauthorized, VersionConflict, VersionOverflow,
    InvalidDefinition, CoworkerArchived, CoworkerInactive, ArchiveBlocked,
    AutomationDisabled, AutomationNotPaused, RoutineArchived, ReconciliationRequired,
    TriggerUnsupported, TriggerSourceChanged, IdempotencyConflict, WorkspaceArchived, Storage,
}
impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for DomainError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkOrigin { Owner, Proactive }

pub fn check_origin_status(status: CoworkerStatus, origin: WorkOrigin) -> Result<(), DomainError> {
    match (status, origin) {
        (CoworkerStatus::Archived, _) => Err(DomainError::CoworkerArchived),
        (CoworkerStatus::Paused, WorkOrigin::Proactive) => Err(DomainError::CoworkerInactive),
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CoworkerLifecycleFacts {
    pub is_primary: bool,
    pub has_active_automation: bool,
    pub has_nonterminal_tasks: bool,
    pub resume_reconciled: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct AutomationLifecycleFacts {
    pub routine_active: bool,
    pub coworker_active: bool,
    pub trigger_dependencies_reconciled: bool,
}

pub fn next_version(actual: u64, expected: u64) -> Result<u64, DomainError> {
    if actual != expected { return Err(DomainError::VersionConflict); }
    actual.checked_add(1).ok_or(DomainError::VersionOverflow)
}

pub fn check_coworker_transition(from: CoworkerStatus, to: CoworkerStatus, facts: &CoworkerLifecycleFacts) -> Result<(), DomainError> {
    if from == CoworkerStatus::Archived { return Err(DomainError::CoworkerArchived); }
    if from == to { return Err(DomainError::InvalidDefinition); }
    if to == CoworkerStatus::Archived && (facts.is_primary || facts.has_active_automation || facts.has_nonterminal_tasks) {
        return Err(DomainError::ArchiveBlocked);
    }
    if from == CoworkerStatus::Paused && to == CoworkerStatus::Active && !facts.resume_reconciled {
        return Err(DomainError::ReconciliationRequired);
    }
    Ok(())
}

pub fn check_automation_transition(from: AutomationStatus, to: AutomationStatus, facts: &AutomationLifecycleFacts) -> Result<(), DomainError> {
    if from == AutomationStatus::Disabled { return Err(DomainError::AutomationDisabled); }
    if from == to { return Err(DomainError::InvalidDefinition); }
    if to == AutomationStatus::Enabled {
        if !facts.routine_active { return Err(DomainError::RoutineArchived); }
        if !facts.coworker_active { return Err(DomainError::CoworkerInactive); }
        if !facts.trigger_dependencies_reconciled { return Err(DomainError::ReconciliationRequired); }
    }
    Ok(())
}

pub fn validate_coworker_definition(definition: &CoworkerDefinition) -> Result<(), DomainError> {
    let context = &definition.context_policy;
    let profiles: HashSet<_> = definition.enabled_delegation_profile_ids.iter().collect();
    let kinds: HashSet<_> = context.allowed_context_kinds.iter().collect();
    if definition.name.trim().is_empty() || definition.name.chars().count() > 120
        || definition.role_description.chars().count() > 2000
        || !context.require_user_confirmation_for_memory || context.max_retrieved_items > 100
        || profiles.len() != definition.enabled_delegation_profile_ids.len()
        || profiles.iter().any(|id| id.is_empty())
        || kinds.len() != context.allowed_context_kinds.len()
        || definition.default_lead_agent_binding_id.as_ref().is_some_and(String::is_empty)
    { return Err(DomainError::InvalidDefinition); }
    Ok(())
}

pub fn validate_triggers(triggers: &[TriggerSpec]) -> Result<(), DomainError> {
    let mut ids = HashSet::new();
    if triggers.is_empty() { return Err(DomainError::InvalidDefinition); }
    for trigger in triggers {
        let runtime_present = trigger.runtime_id.as_ref().is_some_and(|id| !id.is_empty());
        if trigger.trigger_id.is_empty() || !ids.insert(&trigger.trigger_id)
            || (trigger.placement == TriggerPlacement::SpecificRuntime) != runtime_present
            || (trigger.placement != TriggerPlacement::SpecificRuntime && trigger.runtime_id.is_some())
        { return Err(DomainError::InvalidDefinition); }
    }
    Ok(())
}

/// Conservative identity preservation: retained IDs cannot switch kind or logical
/// source. Provider-owned filter/recurrence changes and host handoffs still require
/// validation and cursor reconciliation in the transaction port.
pub fn check_trigger_identity(previous: &[TriggerSpec], next: &[TriggerSpec]) -> Result<(), DomainError> {
    for after in next {
        if let Some(before) = previous.iter().find(|t| t.trigger_id == after.trigger_id) {
            let same = match (&before.trigger, &after.trigger) {
                (TriggerDefinition::Manual, TriggerDefinition::Manual) => true,
                (TriggerDefinition::Schedule { .. }, TriggerDefinition::Schedule { .. }) => true,
                (TriggerDefinition::OneShot { scheduled_at: a, .. }, TriggerDefinition::OneShot { scheduled_at: b, .. }) => a == b,
                (TriggerDefinition::Webhook { source_identity: a, .. }, TriggerDefinition::Webhook { source_identity: b, .. }) => a == b,
                (TriggerDefinition::ConnectorEvent { connection_id: a, provider_event_type: ak, .. }, TriggerDefinition::ConnectorEvent { connection_id: b, provider_event_type: bk, .. }) => a == b && ak == bk,
                _ => false,
            };
            if !same { return Err(DomainError::TriggerSourceChanged); }
        }
    }
    Ok(())
}

pub fn validate_automation_definition(definition: &AutomationDefinition) -> Result<(), DomainError> {
    validate_triggers(&definition.triggers)?;
    let retry = &definition.execution_policy.retry_policy;
    if definition.routine_id.is_empty() || definition.routine_revision == 0
        || definition.execution_policy.max_concurrent_occurrences == 0
        || !retry.multiplier.is_finite() || retry.multiplier <= 0.0
        || definition.coworker_ref.as_ref().is_some_and(|r| r.coworker_id.is_empty() || r.revision == 0)
    { return Err(DomainError::InvalidDefinition); }
    for trigger in &definition.triggers {
        let misfire = match &trigger.trigger {
            TriggerDefinition::Schedule { timezone, rrule_or_cron, recurrence_semantics_version, misfire_policy, .. } => {
                if timezone.is_empty() || rrule_or_cron.is_empty() || *recurrence_semantics_version == 0 { return Err(DomainError::InvalidDefinition); }
                Some(misfire_policy)
            }
            TriggerDefinition::OneShot { scheduled_at, misfire_policy } => {
                if scheduled_at.is_empty() { return Err(DomainError::InvalidDefinition); }
                Some(misfire_policy)
            }
            TriggerDefinition::Webhook { source_identity, auth_profile_ref, replay_window_ms, max_payload_bytes, .. } => {
                if source_identity.is_empty() || auth_profile_ref.is_empty() || *replay_window_ms == 0 || *max_payload_bytes == 0 { return Err(DomainError::InvalidDefinition); }
                None
            }
            TriggerDefinition::ConnectorEvent { connection_id, provider_event_type, .. } => {
                if connection_id.is_empty() || provider_event_type.is_empty() { return Err(DomainError::InvalidDefinition); }
                None
            }
            TriggerDefinition::Manual => None,
        };
        if matches!(misfire, Some(MisfirePolicy::CatchUpBounded { max_occurrences: 0 })) { return Err(DomainError::InvalidDefinition); }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct OwnerCommandScope {
    /// Transport-authenticated identity; a port rechecks current Workspace ownership.
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResponsibilityCommand {
    CreateCoworker { coworker_id: String, definition: CoworkerDefinition },
    ReviseCoworker { coworker_id: String, expected_version: u64, definition: CoworkerDefinition },
    SetCoworkerStatus { coworker_id: String, expected_version: u64, status: CoworkerStatus },
    CreateAutomation { automation_id: String, name: String, definition: AutomationDefinition },
    ReviseAutomation { automation_id: String, expected_version: u64, name: String, definition: AutomationDefinition },
    SetAutomationStatus { automation_id: String, expected_version: u64, status: AutomationStatus },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CommittedResponsibility { Coworker(Coworker), Automation(Automation) }
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum RevisionAppend { Coworker(CoworkerRevision), Automation(AutomationRevision) }
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResponsibilityEvent { pub kind: &'static str, pub payload: serde_json::Value }
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResponsibilityMutation {
    pub aggregate: CommittedResponsibility,
    pub expected_version: Option<u64>,
    pub append: Option<RevisionAppend>,
    pub event: ResponsibilityEvent,
}

/// A production adapter must authorize the principal before loading/replaying,
/// compare the exact command fingerprint for RequestId reuse, and atomically commit
/// mutation, immutable revision, event envelope, idempotent result, and cursor edits.
/// The owned, deterministic callback may run for preparation and again against fresh
/// commit state. It must perform only reads and stage one mutation through commit.
/// Immutable state blob I/O occurs between transactions. Error/panic rolls back everything;
/// no external network calls may happen inside this transaction. Replay does not run
/// the callback and returns the original result after current authorization succeeds.
pub trait ResponsibilityStore {
    fn transaction<F>(&mut self, scope: &OwnerCommandScope, fingerprint: &str, operation: F) -> Result<CommittedResponsibility, DomainError>
    where F: Fn(&mut dyn ResponsibilityTransaction) -> Result<CommittedResponsibility, DomainError> + Send + 'static;
}

pub trait ResponsibilityTransaction {
    fn now(&self) -> String;
    /// Reads must enforce the transaction Workspace, returning NotFound across scopes.
    fn coworker(&mut self, id: &str) -> Result<Option<(Coworker, CoworkerRevision)>, DomainError>;
    fn automation(&mut self, id: &str) -> Result<Option<(Automation, AutomationRevision)>, DomainError>;
    /// Validate exact avatar revision, bindings (enabled/lead-eligible), profiles,
    /// failover allowlist, budgets and external schema objects in this Workspace.
    fn validate_coworker_references(&mut self, definition: &CoworkerDefinition) -> Result<(), DomainError>;
    /// Validate exact active Routine revision and optional Coworker pin, current
    /// credentials/provider conformance, syntax/DST/filter/limits, same-Workspace
    /// references, and budgets. Paused definitions may retain blocked dependencies.
    /// Retained triggers MUST carry cursors forward atomically; changed host/source
    /// requires reconciled, fenced handoff. No removed trigger may discard pending work.
    fn validate_automation_references(&mut self, previous: Option<&AutomationRevision>, definition: &AutomationDefinition, enabling: bool) -> Result<(), DomainError>;
    fn coworker_lifecycle_facts(&mut self, id: &str) -> Result<CoworkerLifecycleFacts, DomainError>;
    fn automation_lifecycle_facts(&mut self, id: &str) -> Result<AutomationLifecycleFacts, DomainError>;
    /// Stage exactly one mutation for the transaction owner to persist after blob
    /// preparation and final revalidation. The owner CASes the
    /// aggregate, retain history, and reserve the command result in one transaction.
    fn commit(&mut self, mutation: ResponsibilityMutation) -> Result<CommittedResponsibility, DomainError>;
}

pub struct ResponsibilityService<S> { store: S }
impl<S: ResponsibilityStore> ResponsibilityService<S> {
    pub fn new(store: S) -> Self { Self { store } }
    pub fn into_store(self) -> S { self.store }
    pub fn execute(&mut self, scope: &OwnerCommandScope, command: ResponsibilityCommand) -> Result<CommittedResponsibility, DomainError> {
        if scope.principal_id.is_empty() || scope.workspace_id.is_empty() || scope.request_id.is_empty() { return Err(DomainError::Unauthorized); }
        let fingerprint = crate::identity::definition_digest(&command)?;
        let owned_scope = scope.clone();
        self.store.transaction(scope, &fingerprint, move |tx| decide(tx, &owned_scope, command.clone()))
    }
}

fn decide(tx: &mut dyn ResponsibilityTransaction, scope: &OwnerCommandScope, command: ResponsibilityCommand) -> Result<CommittedResponsibility, DomainError> {
    let now = tx.now();
    let author = PrincipalRef { principal_id: scope.principal_id.clone(), kind: PrincipalKind::User };
    match command {
        ResponsibilityCommand::CreateCoworker { coworker_id, definition } => {
            if coworker_id.is_empty() { return Err(DomainError::InvalidDefinition); }
            if tx.coworker(&coworker_id)?.is_some() { return Err(DomainError::AlreadyExists); }
            validate_coworker_definition(&definition)?;
            tx.validate_coworker_references(&definition)?;
            let aggregate = Coworker { coworker_id: coworker_id.clone(), workspace_id: scope.workspace_id.clone(), current_revision: 1, status: CoworkerStatus::Active, created_at: now.clone(), updated_at: now.clone(), version: 1 };
            let event = ResponsibilityEvent { kind: "coworker.created.v1", payload: serde_json::json!({"coworker_id": coworker_id, "workspace_id": scope.workspace_id, "current_revision": 1, "status": "ACTIVE", "aggregate_version": 1}) };
            let revision = CoworkerRevision { coworker_id, revision: 1, definition, authored_by: author, created_at: now };
            tx.commit(ResponsibilityMutation { aggregate: CommittedResponsibility::Coworker(aggregate), expected_version: None, append: Some(RevisionAppend::Coworker(revision)), event })
        }
        ResponsibilityCommand::ReviseCoworker { coworker_id, expected_version, definition } => {
            let (mut aggregate, _) = tx.coworker(&coworker_id)?.ok_or(DomainError::NotFound)?;
            aggregate.version = next_version(aggregate.version, expected_version)?;
            if aggregate.status == CoworkerStatus::Archived { return Err(DomainError::CoworkerArchived); }
            validate_coworker_definition(&definition)?;
            tx.validate_coworker_references(&definition)?;
            aggregate.current_revision = aggregate.current_revision.checked_add(1).ok_or(DomainError::VersionOverflow)?;
            aggregate.updated_at = now.clone();
            let revision = CoworkerRevision { coworker_id: coworker_id.clone(), revision: aggregate.current_revision, definition, authored_by: author.clone(), created_at: now };
            let event = ResponsibilityEvent { kind: "coworker.revised.v1", payload: serde_json::json!({"coworker_id": coworker_id, "revision": revision.revision, "revision_digest": crate::identity::definition_digest(&revision)?, "authored_by": author, "aggregate_version": aggregate.version}) };
            tx.commit(ResponsibilityMutation { aggregate: CommittedResponsibility::Coworker(aggregate), expected_version: Some(expected_version), append: Some(RevisionAppend::Coworker(revision)), event })
        }
        ResponsibilityCommand::SetCoworkerStatus { coworker_id, expected_version, status } => {
            let (mut aggregate, revision) = tx.coworker(&coworker_id)?.ok_or(DomainError::NotFound)?;
            aggregate.version = next_version(aggregate.version, expected_version)?;
            check_coworker_transition(aggregate.status, status, &tx.coworker_lifecycle_facts(&coworker_id)?)?;
            if status == CoworkerStatus::Active { tx.validate_coworker_references(&revision.definition)?; }
            let event = ResponsibilityEvent { kind: "coworker.status.changed.v1", payload: serde_json::json!({"coworker_id": coworker_id, "from": aggregate.status, "to": status, "aggregate_version": aggregate.version}) };
            aggregate.status = status;
            aggregate.updated_at = now;
            tx.commit(ResponsibilityMutation { aggregate: CommittedResponsibility::Coworker(aggregate), expected_version: Some(expected_version), append: None, event })
        }
        ResponsibilityCommand::CreateAutomation { automation_id, name, definition } => {
            if automation_id.is_empty() || name.trim().is_empty() { return Err(DomainError::InvalidDefinition); }
            if tx.automation(&automation_id)?.is_some() { return Err(DomainError::AlreadyExists); }
            validate_automation_definition(&definition)?;
            // Creation does not start trigger hosting; enable is an owner command.
            tx.validate_automation_references(None, &definition, false)?;
            let aggregate = Automation { automation_id: automation_id.clone(), workspace_id: scope.workspace_id.clone(), name, current_revision: 1, status: AutomationStatus::Paused, created_at: now.clone(), updated_at: now.clone(), version: 1 };
            let event = ResponsibilityEvent { kind: "automation.created.v1", payload: serde_json::json!({"automation_id": automation_id, "current_revision": 1, "status": "PAUSED", "aggregate_version": 1}) };
            let revision = AutomationRevision { automation_id, revision: 1, definition, authored_by: author, created_at: now };
            tx.commit(ResponsibilityMutation { aggregate: CommittedResponsibility::Automation(aggregate), expected_version: None, append: Some(RevisionAppend::Automation(revision)), event })
        }
        ResponsibilityCommand::ReviseAutomation { automation_id, expected_version, name, definition } => {
            let (mut aggregate, previous) = tx.automation(&automation_id)?.ok_or(DomainError::NotFound)?;
            aggregate.version = next_version(aggregate.version, expected_version)?;
            if aggregate.status == AutomationStatus::Disabled { return Err(DomainError::AutomationDisabled); }
            if aggregate.status != AutomationStatus::Paused { return Err(DomainError::AutomationNotPaused); }
            if name.trim().is_empty() { return Err(DomainError::InvalidDefinition); }
            validate_automation_definition(&definition)?;
            check_trigger_identity(&previous.definition.triggers, &definition.triggers)?;
            tx.validate_automation_references(Some(&previous), &definition, aggregate.status == AutomationStatus::Enabled)?;
            aggregate.name = name;
            aggregate.current_revision = aggregate.current_revision.checked_add(1).ok_or(DomainError::VersionOverflow)?;
            aggregate.updated_at = now.clone();
            let revision = AutomationRevision { automation_id: automation_id.clone(), revision: aggregate.current_revision, definition, authored_by: author.clone(), created_at: now };
            let event = ResponsibilityEvent { kind: "automation.revision.created.v1", payload: serde_json::json!({"automation_id": automation_id, "revision": revision.revision, "definition_digest": crate::identity::definition_digest(&revision)?, "authored_by": author, "coworker_ref": revision.definition.coworker_ref}) };
            tx.commit(ResponsibilityMutation { aggregate: CommittedResponsibility::Automation(aggregate), expected_version: Some(expected_version), append: Some(RevisionAppend::Automation(revision)), event })
        }
        ResponsibilityCommand::SetAutomationStatus { automation_id, expected_version, status } => {
            let (mut aggregate, revision) = tx.automation(&automation_id)?.ok_or(DomainError::NotFound)?;
            aggregate.version = next_version(aggregate.version, expected_version)?;
            check_automation_transition(aggregate.status, status, &tx.automation_lifecycle_facts(&automation_id)?)?;
            if status == AutomationStatus::Enabled { tx.validate_automation_references(Some(&revision), &revision.definition, true)?; }
            let event = ResponsibilityEvent { kind: "automation.status.changed.v1", payload: serde_json::json!({"automation_id": automation_id, "from": aggregate.status, "to": status, "aggregate_version": aggregate.version}) };
            aggregate.status = status;
            aggregate.updated_at = now;
            tx.commit(ResponsibilityMutation { aggregate: CommittedResponsibility::Automation(aggregate), expected_version: Some(expected_version), append: None, event })
        }
    }
}
