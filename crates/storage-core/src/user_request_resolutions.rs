//! Owner-authorized UserRequest response and dismissal contracts.
//!
//! These mutations record the user's immutable response and source status. They do not
//! dispatch provider input, resume an Invocation, or grant Task/Effect authority.

use crate::{DomainEvent, EventDraft, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const USER_REQUEST_IDEMPOTENCY_KEY_MAX_BYTES: usize = 200;
pub const USER_REQUEST_RESPONSE_MAX_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequestResolutionActor {
    pub principal_id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequestResponseRecord {
    pub request_id: String,
    pub response_digest: String,
    pub responded_by: UserRequestResolutionActor,
    pub response_channel_ref: Option<UserRequestResponseChannelRef>,
    pub responded_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequestResponseChannelRef {
    pub channel_binding_id: String,
    pub provider_event_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserRequestResolutionReceipt {
    pub request_id: String,
    pub workspace_id: String,
    pub status: String,
    pub version: u64,
    pub resolved_at: String,
    pub resolved_by: UserRequestResolutionActor,
    pub response_digest: Option<String>,
    pub event: DomainEvent,
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub enum UserRequestResponseInput {
    Value(Value),
    ChoiceId(String),
}

#[derive(Clone, Debug)]
pub struct RespondUserRequestCommit {
    pub owner_principal_id: String,
    pub actor: UserRequestResolutionActor,
    pub workspace_id: String,
    pub request_id: String,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub response_id: String,
    pub input: UserRequestResponseInput,
    pub event: EventDraft,
}

#[derive(Clone, Debug)]
pub struct DismissUserRequestCommit {
    pub owner_principal_id: String,
    pub actor: UserRequestResolutionActor,
    pub workspace_id: String,
    pub request_id: String,
    pub idempotency_key: String,
    pub expected_version: u64,
    pub event: EventDraft,
}

/// Durable owner actions on one exact pending UserRequest.
///
/// A successful response stores ordinary non-secret user data. Provider continuation is
/// deliberately a separate Runtime-local operation and must not be inferred by this API.
pub trait UserRequestResolutionStore: Send + Sync {
    fn respond_to_user_request(
        &self,
        command: RespondUserRequestCommit,
    ) -> Result<UserRequestResolutionReceipt, StoreError>;

    fn dismiss_user_request(
        &self,
        command: DismissUserRequestCommit,
    ) -> Result<UserRequestResolutionReceipt, StoreError>;
}
