//! Suggestion projection types and the bounded, clock-owned expiry service.
//!
//! Expiry and owner visibility/resolution actions are settled through store ports that
//! atomically commit aggregate state, events, snapshots, and idempotency receipts.
//! TASK acceptance is coordinated by domain-task so the ordinary Task and Suggestion
//! resolution can share one storage transaction; proposal production remains separate.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuggestionKind {
    TaskOpportunity,
    RoutineOpportunity,
    AutomationOpportunity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuggestionAction {
    Task,
    OpenRoutineEditor,
    OpenAutomationEditor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuggestionStatus {
    Proposed,
    Accepted,
    Dismissed,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuggestionLatencyClass {
    Standard,
    Interactive,
    DeadlineSensitive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionVisibility {
    Visible,
    Snoozed,
    All,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionResourceRef {
    pub workspace_id: String,
    pub resource_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionGoalRef {
    pub goal_id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionServiceRef {
    pub service_id: String,
}

/// Values in proposal subcontracts are decoded as JSON objects at this projection
/// boundary; authoring/validation remains owned by the future Suggestion producer path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub suggestion_id: String,
    pub workspace_id: String,
    pub coworker_id: Option<String>,
    pub dedupe_key: String,
    pub kind: SuggestionKind,
    pub reason: String,
    pub source_refs: Vec<SuggestionResourceRef>,
    pub goal_refs: Vec<SuggestionGoalRef>,
    pub proposed_action: SuggestionAction,
    pub proposed_by: SuggestionServiceRef,
    pub proposed_task_spec: Option<Value>,
    pub estimated_cost: Option<Value>,
    pub latency_class_hint: Option<SuggestionLatencyClass>,
    pub status: SuggestionStatus,
    pub created_at: String,
    pub expires_at: String,
    pub snoozed_until: Option<String>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<super::PrincipalRef>,
    pub resolution_reason: Option<String>,
    pub result_task_id: Option<String>,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionPage {
    pub items: Vec<Suggestion>,
    pub next: Option<(String, String)>,
}

/// Persisted settings are versioned. `updated_at=None, version=0` is the virtual
/// unmuted default and must not be assigned a synthetic timestamp by a reader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionPreference {
    pub workspace_id: String,
    pub kind: SuggestionKind,
    pub muted: bool,
    pub updated_at: Option<String>,
    pub version: u64,
}

pub trait SuggestionPreferenceStore {
    fn list_kind_preferences(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<SuggestionPreference>, SuggestionServiceError>;

    fn set_kind_preference(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        kind: SuggestionKind,
        muted: bool,
        expected_version: u64,
        request_id: &str,
        as_of: &str,
    ) -> Result<SuggestionPreference, SuggestionServiceError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionServiceError {
    InvalidRequest,
    ClockUnavailable,
    Unauthorized,
    NotFound,
    VersionConflict,
    IdempotencyConflict,
    InvalidTransition,
    Expired,
    ExpiryPending,
    Storage,
}
impl std::fmt::Display for SuggestionServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SuggestionServiceError {}

/// One call's bounded expiry result. `more_due` means callers must not return an
/// actionable page until another bounded settlement call has drained the due set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestionExpiryBatch {
    pub as_of: String,
    pub settled_count: usize,
    pub more_due: bool,
}

pub const SUGGESTION_EXPIRY_BATCH_LIMIT: usize = 100;
pub const SUGGESTION_SERVICE_PRINCIPAL_ID: &str = "service:suggestion-service";

pub trait SuggestionClock: Send + Sync {
    fn now(&self) -> Result<String, SuggestionServiceError>;
}

/// Storage must authorize the owner, and each committed lifecycle mutation must be
/// atomic with its event, aggregate snapshot reference, and idempotency receipt.
pub trait SuggestionExpiryStore {
    fn settle_expired(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        service_principal_id: &str,
        as_of: &str,
        limit: usize,
    ) -> Result<(usize, bool), SuggestionServiceError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum SuggestionOwnerAction {
    Snooze { snoozed_until: Option<String> },
    Dismiss,
}

pub trait SuggestionOwnerActionStore {
    fn apply_owner_action(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        suggestion_id: &str,
        expected_version: u64,
        request_id: &str,
        as_of: &str,
        action: SuggestionOwnerAction,
    ) -> Result<Suggestion, SuggestionServiceError>;
}

pub struct SuggestionService<S, C> {
    store: S,
    clock: C,
}

impl<S, C> SuggestionService<S, C>
where
    S: SuggestionExpiryStore,
    C: SuggestionClock,
{
    pub fn new(store: S, clock: C) -> Self {
        Self { store, clock }
    }

    pub fn settle_expired(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<SuggestionExpiryBatch, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty() || workspace_id.trim().is_empty() {
            return Err(SuggestionServiceError::InvalidRequest);
        }
        let as_of = self.now()?;
        let (settled_count, more_due) = self.settle_at(owner_principal_id, workspace_id, &as_of)?;
        Ok(SuggestionExpiryBatch {
            as_of,
            settled_count,
            more_due,
        })
    }

    fn now(&self) -> Result<String, SuggestionServiceError> {
        let as_of = self.clock.now()?;
        if as_of.trim().is_empty() {
            return Err(SuggestionServiceError::ClockUnavailable);
        }
        Ok(as_of)
    }

    fn settle_at(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        as_of: &str,
    ) -> Result<(usize, bool), SuggestionServiceError> {
        self.store.settle_expired(
            owner_principal_id,
            workspace_id,
            SUGGESTION_SERVICE_PRINCIPAL_ID,
            as_of,
            SUGGESTION_EXPIRY_BATCH_LIMIT,
        )
    }
}

impl<S, C> SuggestionService<S, C>
where
    S: SuggestionExpiryStore + SuggestionOwnerActionStore,
    C: SuggestionClock,
{
    pub fn owner_action(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        suggestion_id: &str,
        expected_version: u64,
        request_id: &str,
        action: SuggestionOwnerAction,
    ) -> Result<Suggestion, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || suggestion_id.trim().is_empty()
            || expected_version == 0
            || request_id.trim().is_empty()
            || request_id.len() > 128
        {
            return Err(SuggestionServiceError::InvalidRequest);
        }
        let as_of = self.now()?;
        let (_, more_due) = self.settle_at(owner_principal_id, workspace_id, &as_of)?;
        if more_due {
            return Err(SuggestionServiceError::ExpiryPending);
        }
        self.store.apply_owner_action(
            owner_principal_id,
            workspace_id,
            suggestion_id,
            expected_version,
            request_id,
            &as_of,
            action,
        )
    }
}

impl<S, C> SuggestionService<S, C>
where
    S: SuggestionExpiryStore + SuggestionPreferenceStore,
    C: SuggestionClock,
{
    pub fn kind_preferences(
        &self,
        owner_principal_id: &str,
        workspace_id: &str,
    ) -> Result<Vec<SuggestionPreference>, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty() || workspace_id.trim().is_empty() {
            return Err(SuggestionServiceError::InvalidRequest);
        }
        self.store
            .list_kind_preferences(owner_principal_id, workspace_id)
    }

    pub fn set_kind_preference(
        &mut self,
        owner_principal_id: &str,
        workspace_id: &str,
        kind: SuggestionKind,
        muted: bool,
        expected_version: u64,
        request_id: &str,
    ) -> Result<SuggestionPreference, SuggestionServiceError> {
        if owner_principal_id.trim().is_empty()
            || workspace_id.trim().is_empty()
            || request_id.trim().is_empty()
            || request_id.len() > 128
            || !request_id.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(SuggestionServiceError::InvalidRequest);
        }
        let as_of = self.now()?;
        let (_, more_due) = self.settle_at(owner_principal_id, workspace_id, &as_of)?;
        if more_due {
            return Err(SuggestionServiceError::ExpiryPending);
        }
        self.store.set_kind_preference(
            owner_principal_id,
            workspace_id,
            kind,
            muted,
            expected_version,
            request_id,
            &as_of,
        )
    }
}
