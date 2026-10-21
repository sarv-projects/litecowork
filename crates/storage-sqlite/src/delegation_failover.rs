//! Durable policy-failover admission for Task lead handoff.
//!
//! The canonical `task.lead_agent.changed.v1` event is the durable record of a consumed
//! trigger observation. A `POLICY_FAILOVER` commit must therefore carry an observation
//! that is fresh at the server-generated event time, match the current lead and pinned
//! policy, name an allowed trigger, and be reused by no other committed lead change. The
//! transaction re-reads all of that state rather than trusting the caller.
//!
//! No provider probe happens here. The observation source identity is the authenticated
//! Runtime service that produced it; this module does not turn an arbitrary caller into
//! that service. Trust, auth, endpoint and budget qualification for the replacement lead
//! session remain separate admission prerequisites.

use super::*;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub(super) const POLICY_FAILOVER_CAUSE: &str = "POLICY_FAILOVER";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredFailoverPolicy {
    pub(super) mode: String,
    pub(super) triggers: Vec<String>,
    pub(super) fallback_agent_binding_ids: Vec<String>,
    pub(super) max_lead_changes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredFailoverObservation {
    pub(super) trigger: String,
    pub(super) affected_agent_binding_id: String,
    pub(super) source: StoredObservationSource,
    pub(super) source_observation_ref: String,
    #[serde(default)]
    pub(super) runtime_incarnation_id: Option<String>,
    pub(super) observed_at: String,
    #[serde(default)]
    pub(super) expires_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredObservationSource {
    service_id: String,
}

impl StoredFailoverObservation {
    pub(super) fn parse(value: &Value) -> Result<Self, StoreError> {
        let observation: Self = serde_json::from_value(value.clone()).map_err(|_| {
            StoreError::Invalid("lead failover trigger observation is malformed".to_owned())
        })?;
        observation.validate_shape()?;
        Ok(observation)
    }

    pub(super) fn source_service_id(&self) -> &str {
        &self.source.service_id
    }

    /// The confirmed trigger that produced this observation.
    pub(super) fn trigger(&self) -> &str {
        &self.trigger
    }

    /// The binding whose lead the observation is about.
    pub(super) fn affected_agent_binding_id(&self) -> &str {
        &self.affected_agent_binding_id
    }

    /// The source-local identity of the observation. One observation may authorize at
    /// most one durable lead decision in a Workspace.
    pub(super) fn source_observation_ref(&self) -> &str {
        &self.source_observation_ref
    }

    fn validate_shape(&self) -> Result<(), StoreError> {
        if !matches!(
            self.trigger.as_str(),
            "AGENT_UNAVAILABLE" | "QUOTA_EXHAUSTED" | "RUNTIME_UNAVAILABLE"
        ) || !valid_id(&self.affected_agent_binding_id)
            || !valid_id(&self.source.service_id)
            || !valid_id(&self.source_observation_ref)
            || self
                .runtime_incarnation_id
                .as_deref()
                .is_some_and(|value| !valid_id(value))
            || (self.trigger == "RUNTIME_UNAVAILABLE" && self.runtime_incarnation_id.is_none())
            || self.expires_at.as_deref().is_none_or(str::is_empty)
        {
            return Err(StoreError::Invalid(
                "lead failover trigger observation is invalid".to_owned(),
            ));
        }
        Ok(())
    }

    /// The observation must be fresh at the server-generated event time: it was
    /// observed at or before that time and has not expired at it.
    pub(super) fn validate_observation_freshness(
        &self,
        recorded_at: &str,
    ) -> Result<OffsetDateTime, StoreError> {
        let recorded = parse_timestamp(recorded_at).ok_or_else(|| {
            StoreError::Invalid("lead failover event time is malformed".to_owned())
        })?;
        let observed = parse_timestamp(&self.observed_at).ok_or_else(|| {
            StoreError::Invalid("lead failover observation time is malformed".to_owned())
        })?;
        let expires = self
            .expires_at
            .as_deref()
            .and_then(parse_timestamp)
            .ok_or_else(|| {
                StoreError::Invalid("lead failover observation expiry is malformed".to_owned())
            })?;
        if observed > recorded || expires <= recorded || expires <= observed {
            return Err(StoreError::Invalid(
                "lead failover observation is stale or not yet valid".to_owned(),
            ));
        }
        Ok(recorded)
    }
}

/// Read and validate the pinned TaskSpec lead-failover policy. The stored policy is
/// the authority for every lead decision; a caller-supplied policy is never trusted.
pub(super) fn load_pinned_policy(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    task_id: &str,
    task_spec_revision: u64,
) -> Result<StoredFailoverPolicy, StoreError> {
    let policy_json: Option<String> = transaction
        .query_row(
            "SELECT lead_failover_policy_json FROM task_spec_revisions WHERE task_id = ?1 AND workspace_id = ?2 AND revision = ?3",
            params![
                task_id,
                workspace_id,
                to_sql_i64(task_spec_revision, "TaskSpec revision")?
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_database_error)?;
    let policy_json = policy_json.ok_or(StoreError::NotFound)?;
    let policy: StoredFailoverPolicy = serde_json::from_str(&policy_json).map_err(|_| {
        StoreError::Integrity("stored Task lead failover policy is invalid".to_owned())
    })?;
    validate_policy_shape(&policy)?;
    Ok(policy)
}

/// Revalidate the pinned policy, trigger, observation identity and ordered fallback
/// inside the same transaction that will write the Task, event and receipt. Returns the
/// only target binding the pinned order permits.
pub(super) fn validate_policy_failover_admission(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    task_id: &str,
    task_spec_revision: u64,
    current_lead_agent_binding_id: &str,
    recorded_at: &str,
    observation: &StoredFailoverObservation,
) -> Result<String, StoreError> {
    observation.validate_observation_freshness(recorded_at)?;
    if observation.affected_agent_binding_id != current_lead_agent_binding_id {
        return Err(StoreError::Invalid(
            "lead failover observation does not match the current lead".to_owned(),
        ));
    }
    let policy = load_pinned_policy(transaction, workspace_id, task_id, task_spec_revision)?;
    if policy.mode != "ALLOW_LISTED" {
        return Err(StoreError::Invalid(
            "lead failover policy does not permit an automatic change".to_owned(),
        ));
    }
    if !policy
        .triggers
        .iter()
        .any(|trigger| trigger == &observation.trigger)
    {
        return Err(StoreError::Invalid(
            "lead failover trigger is not allowed by the pinned policy".to_owned(),
        ));
    }

    let prior_changes: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'Task' AND entity_id = ?2 AND type = 'task.lead_agent.changed.v1' AND json_extract(payload_json, '$.cause') = 'POLICY_FAILOVER'",
            params![workspace_id, task_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    let prior_changes = u32::try_from(prior_changes).map_err(|_| {
        StoreError::Integrity("Task policy failover history is not representable".to_owned())
    })?;
    if prior_changes >= policy.max_lead_changes {
        return Err(StoreError::Invalid(
            "LEAD_FAILOVER_CHANGE_LIMIT_REACHED".to_owned(),
        ));
    }

    // One adapter/runtime observation authorizes at most one lead change in the
    // Workspace. Scoping the check to a single Task would let the same observation
    // be replayed to change the lead of several Tasks.
    let reused_observations: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM domain_events WHERE workspace_id = ?1 AND entity_type = 'Task' AND type = 'task.lead_agent.changed.v1' AND json_extract(payload_json, '$.trigger_observation.source.service_id') = ?2 AND json_extract(payload_json, '$.trigger_observation.source_observation_ref') = ?3",
            params![
                workspace_id,
                observation.source.service_id,
                observation.source_observation_ref
            ],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if reused_observations != 0 {
        return Err(StoreError::Invalid(
            "LEAD_FAILOVER_OBSERVATION_ALREADY_USED".to_owned(),
        ));
    }

    for fallback_id in &policy.fallback_agent_binding_ids {
        if fallback_id == current_lead_agent_binding_id {
            continue;
        }
        let eligible: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_bindings WHERE workspace_id = ?1 AND agent_binding_id = ?2 AND enabled = 1 AND lead_eligible = 1)",
                params![workspace_id, fallback_id],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if eligible {
            return Ok(fallback_id.clone());
        }
    }
    Err(StoreError::Invalid(
        "LEAD_FAILOVER_NO_ELIGIBLE_FALLBACK".to_owned(),
    ))
}

/// Validate the stored policy shape. The pinned policy is read from the TaskSpec
/// revision, never from a caller payload.
pub(super) fn validate_policy_shape(policy: &StoredFailoverPolicy) -> Result<(), StoreError> {
    let unique_triggers = policy
        .triggers
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let unique_fallbacks = policy
        .fallback_agent_binding_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_triggers.len() != policy.triggers.len()
        || policy.fallback_agent_binding_ids.len() > 3
        || unique_fallbacks.len() != policy.fallback_agent_binding_ids.len()
        || policy
            .fallback_agent_binding_ids
            .iter()
            .any(|value| !valid_id(value))
        || policy.triggers.iter().any(|value| {
            !matches!(
                value.as_str(),
                "AGENT_UNAVAILABLE" | "QUOTA_EXHAUSTED" | "RUNTIME_UNAVAILABLE"
            )
        })
    {
        return Err(StoreError::Integrity(
            "stored Task lead failover policy is invalid".to_owned(),
        ));
    }
    if policy.mode == "ALLOW_LISTED"
        && (policy.triggers.is_empty()
            || policy.fallback_agent_binding_ids.is_empty()
            || !(1..=3).contains(&policy.max_lead_changes)
            || policy.max_lead_changes as usize > policy.fallback_agent_binding_ids.len())
    {
        return Err(StoreError::Integrity(
            "stored Task lead failover policy is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return None;
    }
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_core::{BlobRef, EventDraft, TaskLeadHandoffCommit};

    const OWNER: &str = "owner-failover";
    const WORKSPACE: &str = "workspace-failover";
    const TASK: &str = "task-failover";
    const LEAD: &str = "binding-lead";
    const FALLBACK_1: &str = "binding-fallback-1";
    const FALLBACK_2: &str = "binding-fallback-2";
    const SERVICE: &str = "lead-failover";
    const NOW: &str = "2026-10-17T10:00:00Z";

    fn connection(policy: &str) -> Connection {
        new_connection(
            Connection::open_in_memory().expect("in-memory SQLite"),
            policy,
        )
    }

    fn fixture(connection: Connection, policy: &str) -> Connection {
        new_connection(connection, policy)
    }

    fn new_connection(connection: Connection, policy: &str) -> Connection {
        connection
            .execute_batch(&format!(
                "CREATE TABLE workspaces(workspace_id TEXT PRIMARY KEY, owner_principal_id TEXT, status TEXT);
                 CREATE TABLE tasks(
                   task_id TEXT PRIMARY KEY, workspace_id TEXT, conversation_id TEXT,
                   current_spec_revision INTEGER, current_plan_revision INTEGER, status TEXT,
                   resume_status TEXT, routine_id TEXT, routine_revision INTEGER, automation_id TEXT,
                   automation_occurrence_id TEXT, origin_coworker_id TEXT, origin_coworker_revision INTEGER,
                   lead_agent_binding_id TEXT, blocking_conditions_json TEXT, priority TEXT,
                   created_by_json TEXT, created_at TEXT, updated_at TEXT, completed_at TEXT, version INTEGER
                 );
                 CREATE TABLE task_spec_revisions(
                   task_id TEXT, workspace_id TEXT, revision INTEGER, parent_revisions_json TEXT,
                   objective TEXT, task_category TEXT, constraints_json TEXT, non_goals_json TEXT,
                   input_refs_json TEXT, workspace_instruction_revision INTEGER,
                   required_outputs_json TEXT, acceptance_criteria_json TEXT, approvals_required_json TEXT,
                   budget_json TEXT, delegation_budget_policy_json TEXT, lead_failover_policy_json TEXT,
                   deadline TEXT, source_message_refs_json TEXT, placement_preference TEXT,
                   preferred_lead_agent_binding_id TEXT, authored_by_json TEXT, created_at TEXT
                 );
                 CREATE TABLE agent_bindings(workspace_id TEXT, agent_binding_id TEXT, enabled INTEGER, lead_eligible INTEGER);
                 CREATE TABLE agent_sessions(workspace_id TEXT, task_id TEXT, scope_kind TEXT, status TEXT);
                 CREATE TABLE attempts(task_id TEXT, status TEXT);
                 CREATE TABLE effects(task_id TEXT, state TEXT);
                 CREATE TABLE steps(step_id TEXT, task_id TEXT, plan_revision INTEGER, logical_key TEXT,
                   title TEXT, objective TEXT, dependencies_json TEXT, required_capabilities_json TEXT,
                   acceptance_criteria_json TEXT, status TEXT, current_attempt_id TEXT,
                   created_at TEXT, updated_at TEXT, version INTEGER);
                 CREATE TABLE workspace_origin_sequences(workspace_id TEXT, origin_runtime_id TEXT, last_sequence INTEGER,
                   PRIMARY KEY(workspace_id, origin_runtime_id));
                 CREATE TABLE domain_events(
                   event_id TEXT PRIMARY KEY, workspace_id TEXT, entity_type TEXT, entity_id TEXT,
                   origin_runtime_id TEXT, origin_sequence INTEGER, entity_revision INTEGER, hlc_timestamp TEXT,
                   correlation_id TEXT, causation_id TEXT, schema_version INTEGER, type TEXT,
                   payload_json TEXT, aggregate_state_ref_json TEXT, recorded_at TEXT, payload_digest TEXT
                 );
                 CREATE TABLE request_dedup(
                   principal_id TEXT, request_id TEXT, request_digest TEXT, response_json TEXT,
                   response_digest TEXT, created_at TEXT, expires_at TEXT,
                   PRIMARY KEY(principal_id, request_id)
                 );
                 INSERT INTO workspaces VALUES ('{WORKSPACE}','{OWNER}','ACTIVE');
                 INSERT INTO tasks VALUES (
                   '{TASK}','{WORKSPACE}',NULL,1,NULL,'READY',NULL,NULL,NULL,NULL,NULL,
                   NULL,NULL,'{LEAD}','[]','NORMAL','{{\"kind\":\"USER\",\"principal_id\":\"{OWNER}\"}}',
                   '2026-10-17T09:00:00Z','2026-10-17T09:00:00Z',NULL,1
                 );
                 INSERT INTO task_spec_revisions VALUES (
                   '{TASK}','{WORKSPACE}',1,'[]','Task objective',NULL,'[]','[]','[]',NULL,
                   '[]','[]','[]',NULL,NULL,'{policy}',
                   NULL,'[]','\"AUTO\"','{LEAD}','{{\"kind\":\"USER\",\"principal_id\":\"{OWNER}\"}}',
                   '2026-10-17T09:00:00Z'
                 );
                 INSERT INTO agent_bindings VALUES ('{WORKSPACE}','{LEAD}',1,1);
                 INSERT INTO agent_bindings VALUES ('{WORKSPACE}','{FALLBACK_1}',1,1);
                 INSERT INTO agent_bindings VALUES ('{WORKSPACE}','{FALLBACK_2}',1,1);"
            ))
            .expect("create failover fixture");
        connection
    }

    fn allow_listed_policy(max_lead_changes: u32) -> String {
        format!(
            "{{\"mode\":\"ALLOW_LISTED\",\"triggers\":[\"QUOTA_EXHAUSTED\",\"RUNTIME_UNAVAILABLE\"],\"fallback_agent_binding_ids\":[\"{FALLBACK_1}\",\"{FALLBACK_2}\"],\"max_lead_changes\":{max_lead_changes}}}"
        )
    }

    fn observation(reference: &str) -> Value {
        json!({
            "trigger": "QUOTA_EXHAUSTED",
            "affected_agent_binding_id": LEAD,
            "source": {"service_id": SERVICE},
            "source_observation_ref": reference,
            "runtime_incarnation_id": "runtime-incarnation-1",
            "observed_at": "2026-10-17T09:59:00Z",
            "expires_at": "2026-10-17T10:01:00Z",
        })
    }

    fn commit(
        request_id: &str,
        target: &str,
        observation: Option<Value>,
        expected_version: u64,
    ) -> TaskLeadHandoffCommit {
        let request_payload = json!({
            "workspace_id": WORKSPACE,
            "task_id": TASK,
            "request_scope_id": SERVICE,
            "expected_task_version": expected_version,
            "expected_task_spec_revision": 1,
            "target_agent_binding_id": target,
            "cause": "POLICY_FAILOVER",
            "trigger_observation": observation,
        });
        let next_version = expected_version + 1;
        let mut payload = json!({
            "task_id": TASK,
            "to_agent_binding_id": target,
            "cause": "POLICY_FAILOVER",
            "actor": {"service_id": SERVICE},
            "task_spec_revision": 1,
            "aggregate_version": next_version,
        });
        if let Some(observation) = observation.clone() {
            payload["trigger_observation"] = observation;
        }
        TaskLeadHandoffCommit {
            workspace_id: WORKSPACE.to_owned(),
            task_id: TASK.to_owned(),
            request_scope_id: SERVICE.to_owned(),
            request_id: request_id.to_owned(),
            request_payload,
            expected_task_version: expected_version,
            expected_task_spec_revision: 1,
            target_agent_binding_id: target.to_owned(),
            cause: "POLICY_FAILOVER".to_owned(),
            actor: json!({"service_id": SERVICE}),
            trigger_observation: observation,
            event: EventDraft {
                event_id: format!("event-{request_id}"),
                workspace_id: WORKSPACE.to_owned(),
                entity_type: "Task".to_owned(),
                entity_id: TASK.to_owned(),
                origin_runtime_id: "runtime-failover".to_owned(),
                entity_revision: next_version,
                hlc_timestamp: NOW.to_owned(),
                correlation_id: format!("correlation-{request_id}"),
                causation_id: None,
                schema_version: 1,
                event_type: "task.lead_agent.changed.v1".to_owned(),
                payload,
                recorded_at: NOW.to_owned(),
            },
        }
    }

    fn state_ref(version: u64) -> AggregateStateRef {
        AggregateStateRef {
            blob: BlobRef {
                digest: format!("sha256:{}", "a".repeat(64)),
                size_bytes: 128,
                media_type: TASK_STATE_MEDIA_TYPE.to_owned(),
            },
            entity_revision: version,
            record_schema_version: 1,
        }
    }

    #[test]
    fn allow_listed_observation_commits_atomically_and_exact_replay_returns_receipt() {
        let mut connection = connection(&allow_listed_policy(2));
        let committed = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("failover-1", FALLBACK_1, Some(observation("quota-1")), 1),
            state_ref(2),
        )
        .expect("fresh allowed observation commits");
        assert_eq!(committed.target_agent_binding_id, FALLBACK_1);
        assert_eq!(committed.previous_lead_agent_binding_id, LEAD);
        assert_eq!(committed.aggregate_version, 2);

        let payload: String = connection
            .query_row("SELECT payload_json FROM domain_events", [], |row| {
                row.get(0)
            })
            .expect("event payload");
        let payload: Value = serde_json::from_str(&payload).expect("valid event JSON");
        assert_eq!(payload["cause"], "POLICY_FAILOVER");
        assert_eq!(
            payload["trigger_observation"]["source_observation_ref"],
            "quota-1"
        );

        let replay = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("failover-1", FALLBACK_1, Some(observation("quota-1")), 1),
            state_ref(2),
        )
        .expect("exact replay");
        assert!(replay.replayed);
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("event count");
        assert_eq!(events, 1);
    }

    #[test]
    fn stale_expired_future_or_wrong_lead_observations_are_rejected_without_mutation() {
        let mut connection = connection(&allow_listed_policy(2));
        for (index, mut report) in [
            observation("expired"),
            observation("future"),
            observation("wrong-lead"),
            observation("wrong-trigger"),
        ]
        .into_iter()
        .enumerate()
        {
            match index {
                0 => report["expires_at"] = json!("2026-10-17T10:00:00Z"),
                1 => report["observed_at"] = json!("2026-10-17T10:00:01Z"),
                2 => report["affected_agent_binding_id"] = json!(FALLBACK_2),
                3 => report["trigger"] = json!("AGENT_UNAVAILABLE"),
                _ => {}
            }
            let error = commit_task_lead_handoff_transaction(
                &mut connection,
                commit(&format!("invalid-{index}"), FALLBACK_1, Some(report), 1),
                state_ref(2),
            )
            .expect_err("invalid observation is rejected");
            assert!(matches!(error, StoreError::Invalid(_)));
        }
        let lead: String = connection
            .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
                row.get(0)
            })
            .expect("lead");
        assert_eq!(lead, LEAD);
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        let receipts: i64 = connection
            .query_row("SELECT COUNT(*) FROM request_dedup", [], |row| row.get(0))
            .expect("receipts");
        assert_eq!(events, 0);
        assert_eq!(receipts, 0);
    }

    #[test]
    fn unavailable_or_out_of_order_fallbacks_cannot_be_selected() {
        let mut connection = connection(&allow_listed_policy(2));
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit(
                "out-of-order",
                FALLBACK_2,
                Some(observation("quota-order")),
                1,
            ),
            state_ref(2),
        )
        .expect_err("second fallback is not next while the first is eligible");
        assert!(matches!(error, StoreError::Invalid(_)));

        connection
            .execute(
                "UPDATE agent_bindings SET enabled = 0 WHERE agent_binding_id IN (?1, ?2)",
                params![FALLBACK_1, FALLBACK_2],
            )
            .expect("disable fallbacks");
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit(
                "no-fallback",
                FALLBACK_1,
                Some(observation("quota-none")),
                1,
            ),
            state_ref(2),
        )
        .expect_err("no eligible fallback");
        assert!(matches!(error, StoreError::Invalid(_)));
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 0);
    }

    #[test]
    fn one_observation_cannot_authorize_two_lead_changes() {
        let mut connection = connection(&allow_listed_policy(2));
        commit_task_lead_handoff_transaction(
            &mut connection,
            commit("first", FALLBACK_1, Some(observation("quota-once")), 1),
            state_ref(2),
        )
        .expect("first change commits");
        let mut reused = observation("quota-once");
        reused["affected_agent_binding_id"] = json!(FALLBACK_1);
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("second", FALLBACK_2, Some(reused), 2),
            state_ref(3),
        )
        .expect_err("the observation was already consumed");
        assert!(
            matches!(&error, StoreError::Invalid(code) if code == "LEAD_FAILOVER_OBSERVATION_ALREADY_USED"),
            "unexpected error: {error:?}"
        );
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 1);
    }

    #[test]
    fn one_observation_cannot_authorize_a_lead_change_on_another_task() {
        // A single adapter/runtime observation authorizes one lead change in the
        // Workspace. Replaying it against a second Task must be rejected.
        let mut connection = connection(&allow_listed_policy(2));
        commit_task_lead_handoff_transaction(
            &mut connection,
            commit("first", FALLBACK_1, Some(observation("quota-shared")), 1),
            state_ref(2),
        )
        .expect("first change commits");
        let policy = allow_listed_policy(2);
        connection
            .execute_batch(&format!(
                "INSERT INTO tasks VALUES (
                   'task-second','{WORKSPACE}',NULL,1,NULL,'READY',NULL,NULL,NULL,NULL,NULL,
                   NULL,NULL,'{LEAD}','[]','NORMAL','{{\"kind\":\"USER\",\"principal_id\":\"{OWNER}\"}}',
                   '2026-10-17T09:00:00Z','2026-10-17T09:00:00Z',NULL,1);
                 INSERT INTO task_spec_revisions VALUES (
                   'task-second','{WORKSPACE}',1,'[]','Second objective',NULL,'[]','[]','[]',NULL,
                   '[]','[]','[]',NULL,NULL,'{policy}',
                   NULL,'[]','\"AUTO\"','{LEAD}','{{\"kind\":\"USER\",\"principal_id\":\"{OWNER}\"}}',
                   '2026-10-17T09:00:00Z');"
            ))
            .expect("second Task fixture inserts");
        let mut second = commit(
            "second-task",
            FALLBACK_1,
            Some(observation("quota-shared")),
            1,
        );
        second.task_id = "task-second".to_owned();
        second.request_payload["task_id"] = json!("task-second");
        second.event.entity_id = "task-second".to_owned();
        second.event.payload["task_id"] = json!("task-second");
        let error = commit_task_lead_handoff_transaction(&mut connection, second, state_ref(2))
            .expect_err("the observation was already consumed in this Workspace");
        assert!(
            matches!(&error, StoreError::Invalid(code) if code == "LEAD_FAILOVER_OBSERVATION_ALREADY_USED"),
            "unexpected error: {error:?}"
        );
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 1);
    }

    #[test]
    fn a_running_task_with_an_active_attempt_fails_over_and_preserves_that_attempt() {
        // `TASK-RUNTIME.md` requires an automatic failover to preserve active Attempts
        // under their original bindings/leases. The Attempt row is untouched by the
        // lead change; only the Task's lead binding moves.
        let mut connection = connection(&allow_listed_policy(2));
        connection
            .execute_batch(&format!(
                "UPDATE tasks SET status='RUNNING', current_plan_revision=1 WHERE task_id='{TASK}';
                 INSERT INTO attempts VALUES ('{TASK}','RUNNING');
                 INSERT INTO agent_sessions VALUES ('{WORKSPACE}','{TASK}','ATTEMPT_EXECUTION','ACTIVE');
                 INSERT INTO steps VALUES (
                   'step-a','{TASK}',1,'scan','Scan','Scan the repository','[]','[]','[]','RUNNING',
                   'attempt-a','2026-10-17T09:00:00Z','2026-10-17T09:00:00Z',1);"
            ))
            .expect("running Task fixture");
        let committed = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("running", FALLBACK_1, Some(observation("quota-running")), 1),
            state_ref(2),
        )
        .expect("a running Task fails over while its Attempt keeps running");
        assert_eq!(committed.target_agent_binding_id, FALLBACK_1);
        let attempt_status: String = connection
            .query_row(
                "SELECT status FROM attempts WHERE task_id = ?1",
                params![TASK],
                |row| row.get(0),
            )
            .expect("preserved attempt");
        assert_eq!(attempt_status, "RUNNING");
        let lead: String = connection
            .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
                row.get(0)
            })
            .expect("lead");
        assert_eq!(lead, FALLBACK_1);
    }

    #[test]
    fn a_live_planning_session_still_blocks_an_automatic_failover() {
        // The prior planning session must be settled or safely checkpointed first.
        let mut connection = connection(&allow_listed_policy(2));
        connection
            .execute(
                "INSERT INTO agent_sessions VALUES (?1, ?2, 'TASK_PLANNING', 'ACTIVE')",
                params![WORKSPACE, TASK],
            )
            .expect("insert live planner");
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit(
                "live-planner",
                FALLBACK_1,
                Some(observation("quota-planner")),
                1,
            ),
            state_ref(2),
        )
        .expect_err("a live planning session blocks the failover");
        assert!(matches!(error, StoreError::Invalid(_)));
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 0);
    }

    #[test]
    fn an_unresolved_effect_still_blocks_an_automatic_failover() {
        let mut connection = connection(&allow_listed_policy(2));
        connection
            .execute("INSERT INTO effects VALUES (?1, 'PROPOSED')", params![TASK])
            .expect("insert unresolved effect");
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("effect", FALLBACK_1, Some(observation("quota-effect")), 1),
            state_ref(2),
        )
        .expect_err("an unresolved Effect blocks the failover");
        assert!(matches!(error, StoreError::Invalid(_)));
    }

    #[test]
    fn change_limit_and_policy_mode_are_rechecked_inside_the_transaction() {
        let mut limit_connection = connection(&allow_listed_policy(1));
        commit_task_lead_handoff_transaction(
            &mut limit_connection,
            commit("limit-1", FALLBACK_1, Some(observation("quota-limit-1")), 1),
            state_ref(2),
        )
        .expect("first and only allowed change commits");
        let mut limit_observation = observation("quota-limit-2");
        limit_observation["affected_agent_binding_id"] = json!(FALLBACK_1);
        let error = commit_task_lead_handoff_transaction(
            &mut limit_connection,
            commit("limit-2", FALLBACK_2, Some(limit_observation), 2),
            state_ref(3),
        )
        .expect_err("the pinned change budget is exhausted");
        assert!(
            matches!(&error, StoreError::Invalid(code) if code == "LEAD_FAILOVER_CHANGE_LIMIT_REACHED"),
            "unexpected error: {error:?}"
        );

        let mut ask_connection = connection(&allow_listed_policy(2));
        ask_connection
            .execute(
                "UPDATE task_spec_revisions SET lead_failover_policy_json = ?1",
                [json!({
                    "mode":"ASK",
                    "triggers":["QUOTA_EXHAUSTED"],
                    "fallback_agent_binding_ids":[FALLBACK_1],
                    "max_lead_changes":0
                })
                .to_string()],
            )
            .expect("replace policy with ASK");
        let error = commit_task_lead_handoff_transaction(
            &mut ask_connection,
            commit("ask", FALLBACK_1, Some(observation("quota-ask")), 1),
            state_ref(2),
        )
        .expect_err("ASK never admits an automatic change");
        assert!(matches!(error, StoreError::Invalid(_)));
    }

    #[test]
    fn an_active_attempt_does_not_block_policy_failover_but_a_live_planner_does() {
        // `TASK-RUNTIME.md` requires an automatic failover to preserve active Attempts
        // under their original bindings/leases. A live planning session still blocks it.
        let mut connection = connection(&allow_listed_policy(2));
        connection
            .execute("INSERT INTO attempts VALUES (?1, 'RUNNING')", [TASK])
            .expect("insert active Attempt");
        let committed = commit_task_lead_handoff_transaction(
            &mut connection,
            commit(
                "active-attempt",
                FALLBACK_1,
                Some(observation("quota-active")),
                1,
            ),
            state_ref(2),
        )
        .expect("an active Attempt is preserved by an automatic failover");
        assert_eq!(committed.target_agent_binding_id, FALLBACK_1);
        let mut planner_connection = new_connection(
            Connection::open_in_memory().expect("in-memory SQLite"),
            &allow_listed_policy(2),
        );
        planner_connection
            .execute(
                "INSERT INTO agent_sessions VALUES (?1, ?2, 'TASK_PLANNING', 'ACTIVE')",
                params![WORKSPACE, TASK],
            )
            .expect("insert live planner");
        let error = commit_task_lead_handoff_transaction(
            &mut planner_connection,
            commit(
                "live-planner",
                FALLBACK_1,
                Some(observation("quota-planner")),
                1,
            ),
            state_ref(2),
        )
        .expect_err("live planner blocks failover");
        assert!(matches!(error, StoreError::Invalid(_)));
    }

    #[test]
    fn policy_failover_is_version_fenced_against_task_and_spec_races() {
        let mut connection = connection(&allow_listed_policy(2));
        commit_task_lead_handoff_transaction(
            &mut connection,
            commit("race-1", FALLBACK_1, Some(observation("quota-race-1")), 1),
            state_ref(2),
        )
        .expect("first change commits");

        let mut stale_task = commit("race-2", FALLBACK_2, Some(observation("quota-race-2")), 1);
        let raced_observation = {
            let mut value = stale_task.trigger_observation.clone().unwrap();
            value["affected_agent_binding_id"] = json!(FALLBACK_1);
            value
        };
        stale_task.trigger_observation = Some(raced_observation.clone());
        stale_task.request_payload["trigger_observation"] = raced_observation.clone();
        stale_task.event.payload["trigger_observation"] = raced_observation;
        let error = commit_task_lead_handoff_transaction(&mut connection, stale_task, state_ref(2))
            .expect_err("a stale Task version is rejected");
        assert!(matches!(error, StoreError::Conflict { .. }));

        let mut stale_spec = commit("race-3", FALLBACK_2, Some(observation("quota-race-3")), 2);
        stale_spec.expected_task_spec_revision = 2;
        stale_spec.request_payload["expected_task_spec_revision"] = json!(2);
        stale_spec.event.payload["task_spec_revision"] = json!(2);
        let raced_spec_observation = {
            let mut value = stale_spec.trigger_observation.clone().unwrap();
            value["affected_agent_binding_id"] = json!(FALLBACK_1);
            value
        };
        stale_spec.trigger_observation = Some(raced_spec_observation.clone());
        stale_spec.request_payload["trigger_observation"] = raced_spec_observation.clone();
        stale_spec.event.payload["trigger_observation"] = raced_spec_observation;
        let error = commit_task_lead_handoff_transaction(&mut connection, stale_spec, state_ref(3))
            .expect_err("a stale TaskSpec revision is rejected distinctly");
        assert!(matches!(error, StoreError::Invalid(code) if code == "STALE_SPEC_REVISION"));
        let lead: String = connection
            .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
                row.get(0)
            })
            .expect("lead");
        assert_eq!(lead, FALLBACK_1);
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 1);
    }

    #[test]
    fn observation_consumption_and_exact_receipt_survive_process_restart() {
        let directory = tempfile::tempdir().expect("temporary failover database");
        let path = directory.path().join("state.sqlite3");
        {
            let mut connection = fixture(
                Connection::open(&path).expect("file-backed SQLite"),
                &allow_listed_policy(2),
            );
            commit_task_lead_handoff_transaction(
                &mut connection,
                commit(
                    "restart-1",
                    FALLBACK_1,
                    Some(observation("quota-restart")),
                    1,
                ),
                state_ref(2),
            )
            .expect("first change commits before restart");
        }
        let mut connection = Connection::open(&path).expect("reopen SQLite");
        let replay = commit_task_lead_handoff_transaction(
            &mut connection,
            commit(
                "restart-1",
                FALLBACK_1,
                Some(observation("quota-restart")),
                1,
            ),
            state_ref(2),
        )
        .expect("exact receipt survives restart");
        assert!(replay.replayed);

        let mut reused = observation("quota-restart");
        reused["affected_agent_binding_id"] = json!(FALLBACK_1);
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("restart-2", FALLBACK_2, Some(reused), 2),
            state_ref(3),
        )
        .expect_err("the consumed observation stays consumed after restart");
        assert!(
            matches!(&error, StoreError::Invalid(code) if code == "LEAD_FAILOVER_OBSERVATION_ALREADY_USED"),
            "unexpected error: {error:?}"
        );
        let events: i64 = connection
            .query_row("SELECT COUNT(*) FROM domain_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 1);
    }

    #[test]
    fn caller_supplied_observation_cannot_bypass_the_durable_change_history() {
        let mut connection = connection(&allow_listed_policy(2));
        // A crafted observation with a new reference is still rejected when the pinned
        // policy is not ALLOW_LISTED; the transaction never trusts the caller's JSON.
        connection
            .execute(
                "UPDATE task_spec_revisions SET lead_failover_policy_json = ?1",
                [json!({
                    "mode":"DISABLED",
                    "triggers":[],
                    "fallback_agent_binding_ids":[],
                    "max_lead_changes":0
                })
                .to_string()],
            )
            .expect("replace policy with DISABLED");
        let error = commit_task_lead_handoff_transaction(
            &mut connection,
            commit("disabled", FALLBACK_1, Some(observation("crafted")), 1),
            state_ref(2),
        )
        .expect_err("disabled policy blocks failover");
        assert!(matches!(error, StoreError::Invalid(_)));
        let lead: String = connection
            .query_row("SELECT lead_agent_binding_id FROM tasks", [], |row| {
                row.get(0)
            })
            .expect("lead");
        assert_eq!(lead, LEAD);
    }
}
