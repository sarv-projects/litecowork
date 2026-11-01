//! Skill, SkillProposal, and demonstration-capture persistence through the
//! bounded SQLite writer.
//!
//! Authority: [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md),
//! [`docs/CAPABILITY-INVOCATIONS.md`](../../../docs/CAPABILITY-INVOCATIONS.md),
//! [`docs/LEARNING.md`](../../../docs/LEARNING.md), and
//! [`docs/API.md`](../../../docs/API.md).
//!
//! The Skill, SkillProposal, and DemonstrationSession aggregates are stored in
//! the shared `aggregate_snapshots` projection with their canonical
//! `domain_events`, an idempotency receipt, and an audit record written in the
//! same transaction. No package-manager table, provider gateway, or second
//! planner is created here: LiteSPM owns package lifecycle and its wire
//! contract is not published, so nothing in this adapter installs, invokes, or
//! publishes a package.

use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use storage_core::{
    DemonstrationAction, DemonstrationSessionRecord, DemonstrationStatus, McpAppFrameRecord,
    SkillGuardError, SkillProposalDecision, SkillProposalRecord, SkillProposalReviewer,
    SkillProposalStatus, SkillRecord, evaluate_skill_proposal_review,
};


const SKILL_ENTITY: &str = "skill";
const SKILL_PROPOSAL_ENTITY: &str = "skill_proposal";
const DEMONSTRATION_ENTITY: &str = "demonstration_session";
const APP_FRAME_ENTITY: &str = "mcp_app_frame";

const SKILL_MEDIA_TYPE: &str = "application/vnd.litecowork.skill+json";
const SKILL_PROPOSAL_MEDIA_TYPE: &str = "application/vnd.litecowork.skill-proposal+json";
const DEMONSTRATION_MEDIA_TYPE: &str = "application/vnd.litecowork.demonstration+json";
const APP_FRAME_MEDIA_TYPE: &str = "application/vnd.litecowork.mcp-app-frame+json";

const MAX_PAGE_LIMIT: usize = 200;

/// The event context one mutation commits with.
#[derive(Clone, Debug)]
pub struct SkillEventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

/// One committed Skill revision with its canonical event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedSkill {
    pub skill: SkillRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// One committed SkillProposal with its canonical event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedSkillProposal {
    pub proposal: SkillProposalRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// One committed demonstration session with its canonical event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedDemonstrationSession {
    pub session: DemonstrationSessionRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// One committed host-issued MCP App frame with its canonical event.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedAppFrame {
    pub frame: McpAppFrameRecord,
    pub event: DomainEvent,
    pub replayed: bool,
}

/// The request that commits one host-issued MCP App frame.
#[derive(Clone, Debug)]
pub struct AppFrameCreateRequest {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub frame: McpAppFrameRecord,
}

/// A committed receipt that can be replayed with its `replayed` flag set.
trait Replayable {
    fn mark_replayed(&mut self);
}

impl Replayable for CommittedSkill {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

impl Replayable for CommittedSkillProposal {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

impl Replayable for CommittedDemonstrationSession {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

impl Replayable for CommittedAppFrame {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

/// A bounded page of one aggregate kind.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPage {
    pub items: Vec<SkillRecord>,
    pub next: Option<(String, String)>,
}

/// A bounded page of SkillProposals.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProposalPage {
    pub items: Vec<SkillProposalRecord>,
    pub next: Option<(String, String)>,
}

/// A bounded page of demonstration sessions.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DemonstrationSessionPage {
    pub items: Vec<DemonstrationSessionRecord>,
    pub next: Option<(String, String)>,
}

/// The request that commits one Skill revision.
#[derive(Clone, Debug)]
pub struct SkillMutationRequest {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_payload: Value,
    /// The optimistic concurrency check. `None` creates the Skill.
    pub expected_version: Option<u64>,
    pub skill: SkillRecord,
}

/// The request that creates one SkillProposal.
#[derive(Clone, Debug)]
pub struct SkillProposalCreateRequest {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub proposal: SkillProposalRecord,
}

/// The request that records one owner decision on a SkillProposal.
#[derive(Clone, Debug)]
pub struct SkillProposalReviewRequest {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub proposal_id: String,
    pub expected_version: u64,
    pub decision: SkillProposalDecision,
    pub reviewer: SkillProposalReviewer,
}

/// The request that creates one demonstration capture session.
#[derive(Clone, Debug)]
pub struct DemonstrationCreateRequest {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub session: DemonstrationSessionRecord,
}

/// The owner-controlled lifecycle change on one capture session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemonstrationTransition {
    Pause,
    Resume,
    Abort,
}

impl DemonstrationTransition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "PAUSE",
            Self::Resume => "RESUME",
            Self::Abort => "ABORT",
        }
    }
}

/// The request that applies one lifecycle change to a capture session.
#[derive(Clone, Debug)]
pub struct DemonstrationTransitionRequest {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_payload: Value,
    pub session_id: String,
    pub expected_version: u64,
    pub transition: DemonstrationTransition,
}

/// The request that appends one observed action to a capture session.
///
/// Only a qualified capture provider inside this Runtime may append
/// observations. The session's consent, Environment, status, and caps are all
/// re-checked inside the same transaction.
#[derive(Clone, Debug)]
pub struct DemonstrationAppendRequest {
    pub workspace_id: String,
    pub session_id: String,
    pub environment_id: String,
    pub action: DemonstrationAction,
    pub now: String,
}

/// The result of appending one observed action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DemonstrationAppendOutcome {
    Recorded { session: DemonstrationSessionRecord },
    Refused { reason: String },
}

/// The SQLite adapter for the E10 Skill aggregates.
#[derive(Clone)]
pub struct SqliteSkillStore {
    store: SqliteWorkspaceStore,
    context: SkillEventContext,
}

impl SqliteSkillStore {
    pub fn new(
        store: SqliteWorkspaceStore,
        mut context: SkillEventContext,
    ) -> Result<Self, StoreError> {
        if [
            &context.event_id,
            &context.origin_runtime_id,
            &context.hlc_timestamp,
            &context.correlation_id,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(StoreError::Invalid(
                "Skill event context is incomplete".to_owned(),
            ));
        }
        context.recorded_at = canonicalize_utc_timestamp(&context.recorded_at).map_err(|_| {
            StoreError::Invalid("Skill event context timestamp is invalid".to_owned())
        })?;
        Ok(Self { store, context })
    }

    /// The recorded instant the current context commits with.
    pub fn recorded_at(&self) -> String {
        self.context.recorded_at.clone()
    }

    /// A read-only handle for projections that never commit an event.
    ///
    /// The placeholder context is never written: read paths use neither the event
    /// identity nor the recorded timestamp.
    pub fn read_only(store: SqliteWorkspaceStore) -> Self {
        Self {
            store,
            context: SkillEventContext {
                event_id: "read-only".to_owned(),
                origin_runtime_id: "read-only".to_owned(),
                hlc_timestamp: "1970-01-01T00:00:00Z".to_owned(),
                correlation_id: "read-only".to_owned(),
                causation_id: None,
                recorded_at: "1970-01-01T00:00:00Z".to_owned(),
            },
        }
    }

    fn run<T, F>(&self, operation: F) -> Result<T, StoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    {
        let (reply, receive) = mpsc::channel();
        self.store
            .execute_command(
                Command::Operation {
                    operation: Box::new(move |connection| {
                        let _ = reply.send(Ok(operation(connection)));
                    }),
                },
                receive,
            )
            .map_err(|_| StoreError::ExecutorStopped)?
    }

    /// Commit one Skill revision.
    pub fn put_skill(&self, request: SkillMutationRequest) -> Result<CommittedSkill, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &request.principal_id)?;
            let request_digest = digest(canonical_json(&request.request_payload)?.as_slice());
            if let Some(replay) = find_receipt::<CommittedSkill>(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let current = load_aggregate::<SkillRecord>(
                &tx,
                &store,
                &request.workspace_id,
                SKILL_ENTITY,
                &request.skill.skill_id,
            )?;
            let next_version = match (&current, request.expected_version) {
                (None, None) => 1,
                (None, Some(_)) => {
                    return Err(StoreError::Conflict {
                        expected: None,
                        actual: None,
                    });
                }
                (Some(existing), Some(expected)) if existing.version == expected => {
                    existing.version + 1
                }
                (Some(existing), _) => {
                    return Err(StoreError::Conflict {
                        expected: request.expected_version,
                        actual: Some(existing.version),
                    });
                }
            };
            let mut skill = request.skill.clone();
            skill.workspace_id = request.workspace_id.clone();
            skill.version = next_version;
            skill
                .validate()
                .map_err(|error| {
                    StoreError::Invalid(format!(
                        "Skill record is invalid: {}",
                        error.reason_code()
                    ))
                })?;
            // One capability identity may name exactly one Skill in a Workspace.
            // A second Skill that claims the same identity is a duplicate, not a
            // revision, and is rejected before anything is persisted.
            let identity_taken: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM domain_events WHERE workspace_id=?1 AND entity_type='skill' AND json_extract(payload_json,'$.capability_id')=?2 AND entity_id<>?3)",
                    params![
                        request.workspace_id,
                        skill.capability_ref.capability_id,
                        skill.skill_id
                    ],
                    |row| row.get(0),
                )
                .map_err(map_database_error)?;
            if identity_taken {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            let state_ref = write_state(
                &store,
                &tx,
                &request.workspace_id,
                SKILL_ENTITY,
                &skill.skill_id,
                next_version,
                &skill,
                SKILL_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event_draft = event_draft(
                &context,
                &request.workspace_id,
                SKILL_ENTITY,
                &skill.skill_id,
                next_version,
                "skill.revised.v1",
                json!({
                    "skill_id": skill.skill_id,
                    "workspace_id": request.workspace_id,
                    "kind": skill.kind,
                    "status": skill.status,
                    "capability_id": skill.capability_ref.capability_id,
                    "manifest_digest": skill.current_revision_record().map(|revision| revision.manifest_digest.clone()),
                    "current_revision": skill.current_revision,
                    "aggregate_version": next_version,
                }),
            )?;
            let event = insert_domain_event(&tx, &event_draft, &state_ref)?;
            write_audit(
                &tx,
                &request.workspace_id,
                &request.principal_id,
                "SKILL_REVISION_COMMITTED",
                &json!({"skill_id": skill.skill_id}),
                &context,
                &json!({"skill_id": skill.skill_id, "version": next_version}),
            )?;
            let committed = CommittedSkill {
                skill,
                event,
                replayed: false,
            };
            let response = encode(&committed)?;
            write_receipt(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
                &response,
                &context.recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Read one Skill.
    pub fn get_skill(
        &self,
        principal_id: &str,
        workspace_id: &str,
        skill_id: &str,
    ) -> Result<Option<SkillRecord>, StoreError> {
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        let id = skill_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            load_aggregate::<SkillRecord>(&tx, &store, &workspace, SKILL_ENTITY, &id)
        })
    }

    /// List the bounded Skills of one Workspace.
    pub fn list_skills(
        &self,
        principal_id: &str,
        workspace_id: &str,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<SkillPage, StoreError> {
        if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(StoreError::Invalid("Skill page limit is invalid".to_owned()));
        }
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            let page = list_aggregate::<SkillRecord>(
                &tx,
                &store,
                &workspace,
                SKILL_ENTITY,
                after,
                limit,
            )?;
            Ok(SkillPage {
                items: page.items,
                next: page.next,
            })
        })
    }

    /// List the bounded Skills of one Workspace that match a predicate.
    ///
    /// The predicate runs while the page is built, so a filtered page neither
    /// skips a matching Skill nor returns one twice.
    pub fn list_skills_matching<F>(
        &self,
        principal_id: &str,
        workspace_id: &str,
        after: Option<(String, String)>,
        limit: usize,
        keep: F,
    ) -> Result<SkillPage, StoreError>
    where
        F: Fn(&SkillRecord) -> bool + Send + 'static,
    {
        if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(StoreError::Invalid("Skill page limit is invalid".to_owned()));
        }
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            let page = list_aggregate_filtered::<SkillRecord, F>(
                &tx,
                &store,
                &workspace,
                SKILL_ENTITY,
                after,
                limit,
                keep,
            )?;
            Ok(SkillPage {
                items: page.items,
                next: page.next,
            })
        })
    }

    /// Create one SkillProposal.
    pub fn create_skill_proposal(
        &self,
        request: SkillProposalCreateRequest,
    ) -> Result<CommittedSkillProposal, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &request.principal_id)?;
            let request_digest = digest(canonical_json(&request.request_payload)?.as_slice());
            if let Some(replay) = find_receipt::<CommittedSkillProposal>(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            if load_aggregate::<SkillProposalRecord>(
                &tx,
                &store,
                &request.workspace_id,
                SKILL_PROPOSAL_ENTITY,
                &request.proposal.proposal_id,
            )?
            .is_some()
            {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            let mut proposal = request.proposal.clone();
            proposal.workspace_id = request.workspace_id.clone();
            proposal.version = 1;
            proposal.validate().map_err(|error| {
                StoreError::Invalid(format!(
                    "SkillProposal record is invalid: {}",
                    error.reason_code()
                ))
            })?;
            verify_proposal_references(&tx, &request.workspace_id, &proposal)?;
            let state_ref = write_state(
                &store,
                &tx,
                &request.workspace_id,
                SKILL_PROPOSAL_ENTITY,
                &proposal.proposal_id,
                1,
                &proposal,
                SKILL_PROPOSAL_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event = insert_domain_event(
                &tx,
                &event_draft(
                    &context,
                    &request.workspace_id,
                    SKILL_PROPOSAL_ENTITY,
                    &proposal.proposal_id,
                    1,
                    "skill.proposal.created.v1",
                    json!({
                        "proposal_id": proposal.proposal_id,
                        "workspace_id": request.workspace_id,
                        "source": proposal.source,
                        "status": proposal.status,
                        "redaction_status": proposal.redaction.status,
                        "draft_digest": proposal.draft_digest,
                        "aggregate_version": 1,
                    }),
                )?,
                &state_ref,
            )?;
            write_audit(
                &tx,
                &request.workspace_id,
                &request.principal_id,
                "SKILL_PROPOSAL_CREATED",
                &json!({"proposal_id": proposal.proposal_id}),
                &context,
                &json!({"proposal_id": proposal.proposal_id, "status": proposal.status}),
            )?;
            let committed = CommittedSkillProposal {
                proposal,
                event,
                replayed: false,
            };
            let response = encode(&committed)?;
            write_receipt(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
                &response,
                &context.recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Read one SkillProposal.
    pub fn get_skill_proposal(
        &self,
        principal_id: &str,
        workspace_id: &str,
        proposal_id: &str,
    ) -> Result<Option<SkillProposalRecord>, StoreError> {
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        let id = proposal_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            load_aggregate::<SkillProposalRecord>(
                &tx,
                &store,
                &workspace,
                SKILL_PROPOSAL_ENTITY,
                &id,
            )
        })
    }

    /// List the bounded SkillProposals of one Workspace.
    ///
    /// The status filter is applied while paging, so a filtered page never
    /// skips a matching proposal and never repeats one across pages.
    pub fn list_skill_proposals(
        &self,
        principal_id: &str,
        workspace_id: &str,
        status: Option<SkillProposalStatus>,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<SkillProposalPage, StoreError> {
        if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(StoreError::Invalid(
                "SkillProposal page limit is invalid".to_owned(),
            ));
        }
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            let page = match status {
                Some(status) => list_aggregate_filtered::<SkillProposalRecord, _>(
                    &tx,
                    &store,
                    &workspace,
                    SKILL_PROPOSAL_ENTITY,
                    after,
                    limit,
                    move |proposal| proposal.status == status,
                )?,
                None => list_aggregate::<SkillProposalRecord>(
                    &tx,
                    &store,
                    &workspace,
                    SKILL_PROPOSAL_ENTITY,
                    after,
                    limit,
                )?,
            };
            Ok(SkillProposalPage {
                items: page.items,
                next: page.next,
            })
        })
    }

    /// Record one owner decision on a SkillProposal.
    pub fn review_skill_proposal(
        &self,
        request: SkillProposalReviewRequest,
    ) -> Result<CommittedSkillProposal, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &request.principal_id)?;
            let request_digest = digest(canonical_json(&request.request_payload)?.as_slice());
            if let Some(replay) = find_receipt::<CommittedSkillProposal>(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let current = load_aggregate::<SkillProposalRecord>(
                &tx,
                &store,
                &request.workspace_id,
                SKILL_PROPOSAL_ENTITY,
                &request.proposal_id,
            )?
            .ok_or(StoreError::NotFound)?;
            if current.version != request.expected_version {
                return Err(StoreError::Conflict {
                    expected: Some(request.expected_version),
                    actual: Some(current.version),
                });
            }
            let outcome = evaluate_skill_proposal_review(
                &current,
                &request.principal_id,
                &request.reviewer,
                request.decision,
                &context.recorded_at,
            )
            .map_err(skill_guard_error)?;
            let mut proposal = current.clone();
            proposal.status = outcome.status;
            proposal.approved_by = match request.decision {
                SkillProposalDecision::Approve => Some(request.principal_id.clone()),
                SkillProposalDecision::Reject => None,
            };
            proposal.updated_at = context.recorded_at.clone();
            proposal.version = current.version + 1;
            let state_ref = write_state(
                &store,
                &tx,
                &request.workspace_id,
                SKILL_PROPOSAL_ENTITY,
                &proposal.proposal_id,
                proposal.version,
                &proposal,
                SKILL_PROPOSAL_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event = insert_domain_event(
                &tx,
                &event_draft(
                    &context,
                    &request.workspace_id,
                    SKILL_PROPOSAL_ENTITY,
                    &proposal.proposal_id,
                    proposal.version,
                    "skill.proposal.reviewed.v1",
                    json!({
                        "proposal_id": proposal.proposal_id,
                        "workspace_id": request.workspace_id,
                        "status": proposal.status,
                        "decision": request.decision,
                        "reviewed_by": outcome.reviewed_by,
                        "publication_blocker": outcome.publication_blocker,
                        "activated_skill_id": outcome.activated_skill_id,
                        "aggregate_version": proposal.version,
                    }),
                )?,
                &state_ref,
            )?;
            write_audit(
                &tx,
                &request.workspace_id,
                &request.principal_id,
                "SKILL_PROPOSAL_REVIEWED",
                &json!({"proposal_id": proposal.proposal_id}),
                &context,
                &json!({
                    "proposal_id": proposal.proposal_id,
                    "status": proposal.status,
                    "publication_blocker": outcome.publication_blocker,
                }),
            )?;
            let committed = CommittedSkillProposal {
                proposal,
                event,
                replayed: false,
            };
            let response = encode(&committed)?;
            write_receipt(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
                &response,
                &context.recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Create one demonstration capture session.
    pub fn create_demonstration_session(
        &self,
        request: DemonstrationCreateRequest,
    ) -> Result<CommittedDemonstrationSession, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &request.principal_id)?;
            let request_digest = digest(canonical_json(&request.request_payload)?.as_slice());
            if let Some(replay) = find_receipt::<CommittedDemonstrationSession>(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let mut session = request.session.clone();
            session.workspace_id = request.workspace_id.clone();
            session.version = 1;
            session.validate().map_err(skill_guard_error)?;
            let state_ref = write_state(
                &store,
                &tx,
                &request.workspace_id,
                DEMONSTRATION_ENTITY,
                &session.session_id,
                1,
                &session,
                DEMONSTRATION_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event = insert_domain_event(
                &tx,
                &event_draft(
                    &context,
                    &request.workspace_id,
                    DEMONSTRATION_ENTITY,
                    &session.session_id,
                    1,
                    "demonstration.started.v1",
                    json!({
                        "demonstration_session_id": session.session_id,
                        "workspace_id": request.workspace_id,
                        "environment_id": session.environment_id,
                        "status": session.status,
                        "aggregate_version": 1,
                    }),
                )?,
                &state_ref,
            )?;
            write_audit(
                &tx,
                &request.workspace_id,
                &request.principal_id,
                "DEMONSTRATION_STARTED",
                &json!({"demonstration_session_id": session.session_id}),
                &context,
                &json!({"demonstration_session_id": session.session_id}),
            )?;
            let committed = CommittedDemonstrationSession {
                session,
                event,
                replayed: false,
            };
            let response = encode(&committed)?;
            write_receipt(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
                &response,
                &context.recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Read one demonstration capture session.
    pub fn get_demonstration_session(
        &self,
        principal_id: &str,
        workspace_id: &str,
        session_id: &str,
    ) -> Result<Option<DemonstrationSessionRecord>, StoreError> {
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        let id = session_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            load_aggregate::<DemonstrationSessionRecord>(
                &tx,
                &store,
                &workspace,
                DEMONSTRATION_ENTITY,
                &id,
            )
        })
    }

    /// List the bounded demonstration sessions of one Workspace.
    pub fn list_demonstration_sessions(
        &self,
        principal_id: &str,
        workspace_id: &str,
        after: Option<(String, String)>,
        limit: usize,
    ) -> Result<DemonstrationSessionPage, StoreError> {
        if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(StoreError::Invalid(
                "Demonstration page limit is invalid".to_owned(),
            ));
        }
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            let page = list_aggregate::<DemonstrationSessionRecord>(
                &tx,
                &store,
                &workspace,
                DEMONSTRATION_ENTITY,
                after,
                limit,
            )?;
            Ok(DemonstrationSessionPage {
                items: page.items,
                next: page.next,
            })
        })
    }

    /// Apply one owner-controlled lifecycle change to a capture session.
    pub fn transition_demonstration_session(
        &self,
        request: DemonstrationTransitionRequest,
    ) -> Result<CommittedDemonstrationSession, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &request.principal_id)?;
            let request_digest = digest(canonical_json(&request.request_payload)?.as_slice());
            if let Some(replay) = find_receipt::<CommittedDemonstrationSession>(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            let current = load_aggregate::<DemonstrationSessionRecord>(
                &tx,
                &store,
                &request.workspace_id,
                DEMONSTRATION_ENTITY,
                &request.session_id,
            )?
            .ok_or(StoreError::NotFound)?;
            if current.version != request.expected_version {
                return Err(StoreError::Conflict {
                    expected: Some(request.expected_version),
                    actual: Some(current.version),
                });
            }
            let mut session = current.clone();
            match request.transition {
                DemonstrationTransition::Pause => {
                    if session.status != DemonstrationStatus::Capturing {
                        return Err(StoreError::Invalid(
                            "Only a capturing demonstration can be paused".to_owned(),
                        ));
                    }
                    session.status = DemonstrationStatus::Paused;
                }
                DemonstrationTransition::Resume => {
                    if session.status != DemonstrationStatus::Paused {
                        return Err(StoreError::Invalid(
                            "Only a paused demonstration can be resumed".to_owned(),
                        ));
                    }
                    session.status = DemonstrationStatus::Capturing;
                }
                DemonstrationTransition::Abort => {
                    if session.status.is_terminal() {
                        return Err(StoreError::Invalid(
                            "A finished demonstration cannot be aborted".to_owned(),
                        ));
                    }
                    session.status = DemonstrationStatus::Aborted;
                    session.completed_at = Some(context.recorded_at.clone());
                }
            }
            session.updated_at = context.recorded_at.clone();
            session.version = current.version + 1;
            let state_ref = write_state(
                &store,
                &tx,
                &request.workspace_id,
                DEMONSTRATION_ENTITY,
                &session.session_id,
                session.version,
                &session,
                DEMONSTRATION_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event = insert_domain_event(
                &tx,
                &event_draft(
                    &context,
                    &request.workspace_id,
                    DEMONSTRATION_ENTITY,
                    &session.session_id,
                    session.version,
                    "demonstration.transitioned.v1",
                    json!({
                        "demonstration_session_id": session.session_id,
                        "workspace_id": request.workspace_id,
                        "transition": request.transition.as_str(),
                        "status": session.status,
                        "captured_action_count": session.captured_action_count,
                        "captured_trace_bytes": session.captured_trace_bytes,
                        "aggregate_version": session.version,
                    }),
                )?,
                &state_ref,
            )?;
            write_audit(
                &tx,
                &request.workspace_id,
                &request.principal_id,
                "DEMONSTRATION_TRANSITIONED",
                &json!({"demonstration_session_id": session.session_id}),
                &context,
                &json!({
                    "demonstration_session_id": session.session_id,
                    "transition": request.transition.as_str(),
                    "status": session.status,
                }),
            )?;
            let committed = CommittedDemonstrationSession {
                session,
                event,
                replayed: false,
            };
            let response = encode(&committed)?;
            write_receipt(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
                &response,
                &context.recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Append one observed action to a capture session.
    ///
    /// The session's consent, Environment, status, and caps are all re-checked
    /// inside the same transaction. A sensitive region under the
    /// `PAUSE_ON_DETECTION` policy pauses the session durably in that same
    /// transaction, so a later observation cannot slip past the pause.
    pub fn append_demonstration_action(
        &self,
        request: DemonstrationAppendRequest,
    ) -> Result<DemonstrationAppendOutcome, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            let current = load_aggregate::<DemonstrationSessionRecord>(
                &tx,
                &store,
                &request.workspace_id,
                DEMONSTRATION_ENTITY,
                &request.session_id,
            )?
            .ok_or(StoreError::NotFound)?;
            let elapsed_ms = elapsed_millis(&current.started_at, &request.now);
            let decision = storage_core::evaluate_capture_append(
                &current.capture_policy,
                current.status,
                &request.environment_id,
                &current.environment_id,
                current.captured_action_count,
                current.captured_trace_bytes,
                elapsed_ms,
                &request.action,
            );
            let outcome = match decision {
                storage_core::CaptureDecision::Recorded {
                    action_count,
                    trace_bytes,
                } => {
                    let mut session = current.clone();
                    session.actions.push(request.action.clone());
                    session.captured_action_count = action_count;
                    session.captured_trace_bytes = trace_bytes;
                    session.updated_at = context.recorded_at.clone();
                    session.version = current.version + 1;
                    let state_ref = write_state(
                        &store,
                        &tx,
                        &request.workspace_id,
                        DEMONSTRATION_ENTITY,
                        &session.session_id,
                        session.version,
                        &session,
                        DEMONSTRATION_MEDIA_TYPE,
                        &context.recorded_at,
                    )?;
                    let _event = insert_domain_event(
                        &tx,
                        &event_draft(
                            &context,
                            &request.workspace_id,
                            DEMONSTRATION_ENTITY,
                            &session.session_id,
                            session.version,
                            "demonstration.action.captured.v1",
                            json!({
                                "demonstration_session_id": session.session_id,
                                "workspace_id": request.workspace_id,
                                "sequence": request.action.sequence,
                                "kind": request.action.kind,
                                "captured_action_count": session.captured_action_count,
                                "captured_trace_bytes": session.captured_trace_bytes,
                                "aggregate_version": session.version,
                            }),
                        )?,
                        &state_ref,
                    )?;
                    tx.commit().map_err(map_database_error)?;
                    DemonstrationAppendOutcome::Recorded { session }
                }
                storage_core::CaptureDecision::RecordedWithSensitiveFieldsOmitted {
                    action_count,
                    trace_bytes,
                } => {
                    // The semantic step is kept; the observed value and the
                    // pointer hint never reach durable state.
                    let mut session = current.clone();
                    session
                        .actions
                        .push(request.action.without_sensitive_fields());
                    session.captured_action_count = action_count;
                    session.captured_trace_bytes = trace_bytes;
                    session.updated_at = context.recorded_at.clone();
                    session.version = current.version + 1;
                    let state_ref = write_state(
                        &store,
                        &tx,
                        &request.workspace_id,
                        DEMONSTRATION_ENTITY,
                        &session.session_id,
                        session.version,
                        &session,
                        DEMONSTRATION_MEDIA_TYPE,
                        &context.recorded_at,
                    )?;
                    let _event = insert_domain_event(
                        &tx,
                        &event_draft(
                            &context,
                            &request.workspace_id,
                            DEMONSTRATION_ENTITY,
                            &session.session_id,
                            session.version,
                            "demonstration.action.captured.v1",
                            json!({
                                "demonstration_session_id": session.session_id,
                                "workspace_id": request.workspace_id,
                                "sequence": request.action.sequence,
                                "kind": request.action.kind,
                                "sensitive_fields_omitted": true,
                                "captured_action_count": session.captured_action_count,
                                "captured_trace_bytes": session.captured_trace_bytes,
                                "aggregate_version": session.version,
                            }),
                        )?,
                        &state_ref,
                    )?;
                    tx.commit().map_err(map_database_error)?;
                    DemonstrationAppendOutcome::Recorded { session }
                }
                storage_core::CaptureDecision::Refused { reason } => {
                    if reason == storage_core::CaptureRefusal::SensitiveRegionPaused {
                        // The capture stops here and stays stopped until an
                        // explicit owner resume.
                        let mut session = current.clone();
                        session.status = DemonstrationStatus::Paused;
                        session.updated_at = context.recorded_at.clone();
                        session.version = current.version + 1;
                        let state_ref = write_state(
                            &store,
                            &tx,
                            &request.workspace_id,
                            DEMONSTRATION_ENTITY,
                            &session.session_id,
                            session.version,
                            &session,
                            DEMONSTRATION_MEDIA_TYPE,
                            &context.recorded_at,
                        )?;
                        let _event = insert_domain_event(
                            &tx,
                            &event_draft(
                                &context,
                                &request.workspace_id,
                                DEMONSTRATION_ENTITY,
                                &session.session_id,
                                session.version,
                                "demonstration.transitioned.v1",
                                json!({
                                    "demonstration_session_id": session.session_id,
                                    "workspace_id": request.workspace_id,
                                    "transition": "PAUSE",
                                    "reason_code": "SENSITIVE_REGION_DETECTED",
                                    "status": session.status,
                                    "captured_action_count": session.captured_action_count,
                                    "captured_trace_bytes": session.captured_trace_bytes,
                                    "aggregate_version": session.version,
                                }),
                            )?,
                            &state_ref,
                        )?;
                        write_audit(
                            &tx,
                            &request.workspace_id,
                            &context.origin_runtime_id,
                            "DEMONSTRATION_PAUSED_ON_SENSITIVE_REGION",
                            &json!({"demonstration_session_id": session.session_id}),
                            &context,
                            &json!({
                                "demonstration_session_id": session.session_id,
                                "reason_code": "SENSITIVE_REGION_DETECTED",
                            }),
                        )?;
                    }
                    tx.commit().map_err(map_database_error)?;
                    DemonstrationAppendOutcome::Refused {
                        reason: format!("{reason:?}"),
                    }
                }
            };
            Ok(outcome)
        })
    }

    /// Mark one capture session as converted into a SkillProposal.
    pub fn complete_demonstration_session(
        &self,
        principal_id: &str,
        workspace_id: &str,
        session_id: &str,
        expected_version: u64,
        trace_resource_id: Option<String>,
        skill_proposal_id: Option<String>,
    ) -> Result<CommittedDemonstrationSession, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        let id = session_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            let current = load_aggregate::<DemonstrationSessionRecord>(
                &tx,
                &store,
                &workspace,
                DEMONSTRATION_ENTITY,
                &id,
            )?
            .ok_or(StoreError::NotFound)?;
            if current.version != expected_version {
                return Err(StoreError::Conflict {
                    expected: Some(expected_version),
                    actual: Some(current.version),
                });
            }
            if current.status.is_terminal() {
                return Err(StoreError::Invalid(
                    "A finished demonstration cannot be completed again".to_owned(),
                ));
            }
            let mut session = current.clone();
            session.status = DemonstrationStatus::Converted;
            session.completed_at = Some(context.recorded_at.clone());
            session.updated_at = context.recorded_at.clone();
            session.trace_resource_id = trace_resource_id;
            session.skill_proposal_id = skill_proposal_id;
            session.version = current.version + 1;
            let state_ref = write_state(
                &store,
                &tx,
                &workspace,
                DEMONSTRATION_ENTITY,
                &session.session_id,
                session.version,
                &session,
                DEMONSTRATION_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event = insert_domain_event(
                &tx,
                &event_draft(
                    &context,
                    &workspace,
                    DEMONSTRATION_ENTITY,
                    &session.session_id,
                    session.version,
                    "demonstration.converted.v1",
                    json!({
                        "demonstration_session_id": session.session_id,
                        "workspace_id": workspace,
                        "status": session.status,
                        "trace_resource_id": session.trace_resource_id,
                        "skill_proposal_id": session.skill_proposal_id,
                        "aggregate_version": session.version,
                    }),
                )?,
                &state_ref,
            )?;
            write_audit(
                &tx,
                &workspace,
                &principal,
                "DEMONSTRATION_CONVERTED",
                &json!({"demonstration_session_id": session.session_id}),
                &context,
                &json!({
                    "demonstration_session_id": session.session_id,
                    "skill_proposal_id": session.skill_proposal_id,
                }),
            )?;
            let committed = CommittedDemonstrationSession {
                session,
                event,
                replayed: false,
            };
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Commit one host-issued MCP App frame.
    ///
    /// The frame descriptor and its host-issued nonce are written in the same
    /// transaction as the canonical event and the replay receipt. The nonce is
    /// never written into the event payload: a frame's nonce is host-local
    /// session material, not a replicated fact.
    pub fn put_app_frame(
        &self,
        request: AppFrameCreateRequest,
    ) -> Result<CommittedAppFrame, StoreError> {
        let context = self.context.clone();
        let store = self.store.clone();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &request.workspace_id, &request.principal_id)?;
            let request_digest = digest(canonical_json(&request.request_payload)?.as_slice());
            if let Some(replay) = find_receipt::<CommittedAppFrame>(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
            )? {
                tx.commit().map_err(map_database_error)?;
                return Ok(replay);
            }
            if load_aggregate::<McpAppFrameRecord>(
                &tx,
                &store,
                &request.workspace_id,
                APP_FRAME_ENTITY,
                &request.frame.frame_id,
            )?
            .is_some()
            {
                return Err(StoreError::Conflict {
                    expected: None,
                    actual: None,
                });
            }
            let mut frame = request.frame.clone();
            frame.workspace_id = request.workspace_id.clone();
            frame.version = 1;
            frame.validate().map_err(skill_guard_error)?;
            let state_ref = write_state(
                &store,
                &tx,
                &request.workspace_id,
                APP_FRAME_ENTITY,
                &frame.frame_id,
                1,
                &frame,
                APP_FRAME_MEDIA_TYPE,
                &context.recorded_at,
            )?;
            let event = insert_domain_event(
                &tx,
                &event_draft(
                    &context,
                    &request.workspace_id,
                    APP_FRAME_ENTITY,
                    &frame.frame_id,
                    1,
                    "mcp_app.frame.issued.v1",
                    json!({
                        "frame_id": frame.frame_id,
                        "workspace_id": request.workspace_id,
                        "server_identity": frame.descriptor.server_identity,
                        "resource_uri": frame.descriptor.resource_uri,
                        "content_digest": frame.descriptor.content_digest,
                        "origin": frame.descriptor.origin,
                        "expires_at": frame.descriptor.expires_at,
                        "aggregate_version": 1,
                    }),
                )?,
                &state_ref,
            )?;
            write_audit(
                &tx,
                &request.workspace_id,
                &request.principal_id,
                "MCP_APP_FRAME_ISSUED",
                &json!({"frame_id": frame.frame_id}),
                &context,
                &json!({"frame_id": frame.frame_id}),
            )?;
            let committed = CommittedAppFrame {
                frame,
                event,
                replayed: false,
            };
            let response = encode(&committed)?;
            write_receipt(
                &tx,
                &request.principal_id,
                &request.request_id,
                &request_digest,
                &response,
                &context.recorded_at,
            )?;
            tx.commit().map_err(map_database_error)?;
            Ok(committed)
        })
    }

    /// Read one host-issued MCP App frame.
    ///
    /// The read is owner-scoped: a frame belongs to the Workspace that issued
    /// it and is never visible from another Workspace.
    pub fn get_app_frame(
        &self,
        principal_id: &str,
        workspace_id: &str,
        frame_id: &str,
    ) -> Result<Option<McpAppFrameRecord>, StoreError> {
        let store = self.store.clone();
        let workspace = workspace_id.to_owned();
        let principal = principal_id.to_owned();
        let id = frame_id.to_owned();
        self.run(move |connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .map_err(map_database_error)?;
            authorize_owner(&tx, &workspace, &principal)?;
            load_aggregate::<McpAppFrameRecord>(&tx, &store, &workspace, APP_FRAME_ENTITY, &id)
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn authorize_owner(
    tx: &Transaction<'_>,
    workspace_id: &str,
    owner_principal_id: &str,
) -> Result<(), StoreError> {
    let active: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_id=?1 AND owner_principal_id=?2 AND status='ACTIVE')",
            params![workspace_id, owner_principal_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if active {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

fn encode<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value)
        .map_err(|error| StoreError::Invalid(format!("Skill payload is not encodable: {error}")))
}

fn skill_guard_error(error: SkillGuardError) -> StoreError {
    StoreError::Invalid(format!(
        "Skill guard rejected the request: {}",
        error.reason_code()
    ))
}

/// The elapsed milliseconds between two RFC 3339 instants.
///
/// An unparsable instant fails closed: the capture's own bookkeeping is broken,
/// so recording stops instead of running with an unbounded duration.
fn elapsed_millis(started_at: &str, now: &str) -> u64 {
    let Ok(start) =
        time::OffsetDateTime::parse(started_at, &time::format_description::well_known::Rfc3339)
    else {
        return u64::MAX;
    };
    let Ok(end) =
        time::OffsetDateTime::parse(now, &time::format_description::well_known::Rfc3339)
    else {
        return u64::MAX;
    };
    let millis = (end - start).whole_milliseconds();
    if millis <= 0 {
        0
    } else {
        millis as u64
    }
}

/// Write the aggregate-state blob and its snapshot row for one aggregate.
fn write_state<T: Serialize>(
    store: &SqliteWorkspaceStore,
    tx: &Transaction<'_>,
    workspace_id: &str,
    entity_type: &str,
    entity_id: &str,
    revision: u64,
    value: &T,
    media_type: &str,
    recorded_at: &str,
) -> Result<AggregateStateRef, StoreError> {
    let bytes = canonical_json(value)?;
    let blob = store.inner.blobs.put(
        workspace_id,
        BlobPurpose::AggregateState,
        &bytes,
        media_type,
    )?;
    if store
        .inner
        .blobs
        .get(workspace_id, BlobPurpose::AggregateState, &blob)?
        != bytes
    {
        return Err(StoreError::Integrity(
            "Skill aggregate state failed blob verification".to_owned(),
        ));
    }
    tx.execute(
        "INSERT INTO aggregate_snapshots(workspace_id,entity_type,entity_id,through_revision,projection_schema_version,state_digest,blob_ref_json,created_at) VALUES (?1,?2,?3,?4,1,?5,?6,?7)",
        params![
            workspace_id,
            entity_type,
            entity_id,
            to_sql_i64(revision, "Skill aggregate revision")?,
            blob.digest,
            encode(&blob)?,
            recorded_at
        ],
    )
    .map_err(map_database_error)?;
    Ok(AggregateStateRef {
        blob,
        entity_revision: revision,
        record_schema_version: 1,
    })
}

/// Load the current aggregate state of one entity.
fn load_aggregate<T: for<'de> Deserialize<'de>>(
    tx: &Transaction<'_>,
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    entity_type: &str,
    entity_id: &str,
) -> Result<Option<T>, StoreError> {
    let row: Option<(i64, String)> = tx
        .query_row(
            "SELECT through_revision, blob_ref_json FROM aggregate_snapshots WHERE workspace_id=?1 AND entity_type=?2 AND entity_id=?3 ORDER BY through_revision DESC LIMIT 1",
            params![workspace_id, entity_type, entity_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((_revision, blob_json)) = row else {
        return Ok(None);
    };
    let blob: BlobRef = serde_json::from_str(&blob_json).map_err(|_| {
        StoreError::Integrity("Skill aggregate blob reference is malformed".to_owned())
    })?;
    let bytes = store
        .inner
        .blobs
        .get(workspace_id, BlobPurpose::AggregateState, &blob)?;
    let value = serde_json::from_slice(&bytes).map_err(|_| {
        StoreError::Integrity("Skill aggregate state is malformed".to_owned())
    })?;
    Ok(Some(value))
}

/// A bounded, keyset-paginated page of one aggregate kind.
pub(super) struct AggregatePage<T> {
    items: Vec<T>,
    /// The keyset key of every returned item, in the same order.
    keys: Vec<(String, String)>,
    /// The exclusive cursor that continues past this page.
    next: Option<(String, String)>,
    /// True when the underlying projection holds rows beyond this page.
    has_more: bool,
}

/// A bounded, keyset-paginated page of one aggregate kind with a predicate.
///
/// The predicate runs while the page is built, so a filtered page neither
/// skips a matching aggregate nor returns one twice.
pub(super) fn list_aggregate_filtered<T, F>(
    tx: &Transaction<'_>,
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    entity_type: &str,
    after: Option<(String, String)>,
    limit: usize,
    keep: F,
) -> Result<AggregatePage<T>, StoreError>
where
    T: for<'de> Deserialize<'de>,
    F: Fn(&T) -> bool + Send + 'static,
{
    let mut cursor = after;
    let mut items: Vec<T> = Vec::with_capacity(limit);
    let mut keys: Vec<(String, String)> = Vec::with_capacity(limit);
    let mut has_more = false;
    // The scan is bounded so a filter that matches nothing cannot walk the
    // whole Workspace on every request.
    let scan_budget = limit.saturating_mul(8).max(64);
    let mut scanned = 0usize;
    'scan: while items.len() < limit && scanned < scan_budget {
        let page = list_aggregate::<T>(tx, store, workspace_id, entity_type, cursor.clone(), limit)?;
        scanned = scanned.saturating_add(page.items.len());
        let page_has_more = page.has_more;
        let page_next = page.next.clone();
        for (value, key) in page.items.into_iter().zip(page.keys.into_iter()) {
            cursor = Some(key.clone());
            if keep(&value) {
                items.push(value);
                keys.push(key);
                if items.len() == limit {
                    has_more = page_has_more;
                    break 'scan;
                }
            }
        }
        if !page_has_more {
            break;
        }
        match page_next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(AggregatePage {
        items,
        keys,
        next: if has_more { cursor } else { None },
        has_more,
    })
}

fn list_aggregate<T: for<'de> Deserialize<'de>>(
    tx: &Transaction<'_>,
    store: &SqliteWorkspaceStore,
    workspace_id: &str,
    entity_type: &str,
    after: Option<(String, String)>,
    limit: usize,
) -> Result<AggregatePage<T>, StoreError> {
    let after_created = after.as_ref().map(|value| value.0.clone());
    let after_id = after.as_ref().map(|value| value.1.clone());
    let mut statement = tx
        .prepare(
            "SELECT entity_id, rev, created FROM (SELECT entity_id, MAX(through_revision) AS rev, MIN(created_at) AS created FROM aggregate_snapshots WHERE workspace_id=?1 AND entity_type=?2 GROUP BY entity_id) WHERE (?3 IS NULL OR created > ?3 OR (created = ?3 AND entity_id > ?4)) ORDER BY created ASC, entity_id ASC LIMIT ?5",
        )
        .map_err(map_database_error)?;
    let rows = statement
        .query_map(
            params![
                workspace_id,
                entity_type,
                after_created.as_deref(),
                after_id.as_deref(),
                (limit + 1) as i64
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(map_database_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_database_error)?;
    let has_more = rows.len() > limit;
    let mut items = Vec::with_capacity(rows.len().min(limit));
    let mut keys = Vec::with_capacity(rows.len().min(limit));
    let mut last: Option<(String, String)> = None;
    for (entity_id, _revision) in rows.iter().take(limit) {
        let created: String = tx
            .query_row(
                "SELECT MIN(created_at) FROM aggregate_snapshots WHERE workspace_id=?1 AND entity_type=?2 AND entity_id=?3",
                params![workspace_id, entity_type, entity_id],
                |row| row.get(0),
            )
            .map_err(map_database_error)?;
        if let Some(value) = load_aggregate::<T>(tx, store, workspace_id, entity_type, entity_id)? {
            items.push(value);
            keys.push((created.clone(), entity_id.clone()));
            last = Some((created, entity_id.clone()));
        }
    }
    let next = if has_more { last } else { None };
    Ok(AggregatePage {
        items,
        keys,
        next,
        has_more,
    })
}

/// Prove that a proposal only references same-Workspace work.
///
/// A proposal that names a Task, Artifact, or Resource from another Workspace —
/// or one that does not exist at all — is rejected before it is stored.
fn verify_proposal_references(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal: &SkillProposalRecord,
) -> Result<(), StoreError> {
    let task_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE task_id=?1 AND workspace_id=?2)",
            params![proposal.source_task_id, workspace_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !task_exists {
        return Err(StoreError::NotFound);
    }
    let artifact_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE artifact_id=?1 AND workspace_id=?2)",
            params![proposal.source_artifact_id, workspace_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !artifact_exists {
        return Err(StoreError::NotFound);
    }
    let resource_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resources WHERE resource_id=?1 AND workspace_id=?2)",
            params![proposal.draft_resource_ref, workspace_id],
            |row| row.get(0),
        )
        .map_err(map_database_error)?;
    if !resource_exists {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

fn find_receipt<T: for<'de> Deserialize<'de> + Replayable>(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
) -> Result<Option<T>, StoreError> {
    let prior: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT request_digest,response_json,response_digest FROM request_dedup WHERE principal_id=?1 AND request_id=?2",
            params![principal_id, request_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    let Some((stored_digest, response, response_digest)) = prior else {
        return Ok(None);
    };
    if stored_digest != request_digest {
        return Err(StoreError::IdempotencyConflict);
    }
    let response = response.ok_or_else(|| {
        StoreError::Integrity("Skill request receipt has no response".to_owned())
    })?;
    if response_digest.as_deref() != Some(digest(response.as_bytes()).as_str()) {
        return Err(StoreError::Integrity(
            "Skill request receipt digest mismatch".to_owned(),
        ));
    }
    let mut committed: T = serde_json::from_str(&response).map_err(|_| {
        StoreError::Integrity("Skill request receipt is malformed".to_owned())
    })?;
    committed.mark_replayed();
    Ok(Some(committed))
}

fn write_receipt(
    tx: &Transaction<'_>,
    principal_id: &str,
    request_id: &str,
    request_digest: &str,
    response: &str,
    recorded_at: &str,
) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO request_dedup(principal_id,request_id,request_digest,response_json,response_digest,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,NULL)",
        params![principal_id, request_id, request_digest, response, digest(response.as_bytes()), recorded_at],
    )
    .map_err(map_database_error)?;
    Ok(())
}

fn write_audit(
    tx: &Transaction<'_>,
    workspace_id: &str,
    principal_id: &str,
    action: &str,
    resource_ref: &Value,
    context: &SkillEventContext,
    payload: &Value,
) -> Result<(), StoreError> {
    let audit_digest = digest(canonical_json(payload)?.as_slice());
    tx.execute(
        "INSERT INTO audit_records(audit_record_id,workspace_id,principal_json,action,resource_ref_json,decision,reason_code,correlation_id,occurred_at,payload_digest) VALUES (?1,?2,?3,?4,?5,'ALLOW','OWNER_AUTHORIZED',?6,?7,?8)",
        params![
            format!("{}:audit", context.event_id),
            workspace_id,
            encode(&json!({"principal_id": principal_id, "kind": "USER"}))?,
            action,
            encode(resource_ref)?,
            context.correlation_id,
            context.recorded_at,
            audit_digest
        ],
    )
    .map_err(map_database_error)?;
    Ok(())
}

fn event_draft(
    context: &SkillEventContext,
    workspace_id: &str,
    entity_type: &str,
    entity_id: &str,
    entity_revision: u64,
    event_type: &str,
    payload: Value,
) -> Result<EventDraft, StoreError> {
    Ok(EventDraft {
        event_id: context.event_id.clone(),
        workspace_id: workspace_id.to_owned(),
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_owned(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload,
        recorded_at: context.recorded_at.clone(),
    })
}
