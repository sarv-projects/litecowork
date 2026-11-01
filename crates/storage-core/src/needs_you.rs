use crate::StoreError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NeedsYouItemKind {
    Approval,
    UserRequest,
    TaskBlocker,
}

impl NeedsYouItemKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "APPROVAL",
            Self::UserRequest => "USER_REQUEST",
            Self::TaskBlocker => "TASK_BLOCKER",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NeedsYouStatus {
    Open,
    Resolved,
}

impl NeedsYouStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Resolved => "RESOLVED",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsYouFilter {
    pub status: Option<NeedsYouStatus>,
    pub kind: Option<NeedsYouItemKind>,
    pub task_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsYouItem {
    pub item_id: String,
    pub kind: NeedsYouItemKind,
    pub status: NeedsYouStatus,
    pub approval_id: Option<String>,
    pub user_request_id: Option<String>,
    pub task_id: Option<String>,
    pub blocker_id: Option<String>,
    pub title: String,
    pub safe_summary: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// Opaque page position plus the normalized query identity it is valid for.
/// The Operator transport may encode this value as an opaque string, but storage
/// rechecks every binding before it uses the position.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsYouCursor {
    pub workspace_id: String,
    pub filter: NeedsYouFilter,
    pub limit: usize,
    pub after_created_at: String,
    pub after_item_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsYouPageRequest {
    pub workspace_id: String,
    pub filter: NeedsYouFilter,
    pub limit: usize,
    pub cursor: Option<NeedsYouCursor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NeedsYouPage {
    pub items: Vec<NeedsYouItem>,
    pub next_cursor: Option<NeedsYouCursor>,
}

/// Read-only Workspace projection over canonical Approval, UserRequest, and Task
/// blocker records. It does not create an inbox aggregate or synthesize blocker history.
pub trait NeedsYouReadStore: Send + Sync {
    fn list_needs_you_page(&self, request: NeedsYouPageRequest)
    -> Result<NeedsYouPage, StoreError>;
}
