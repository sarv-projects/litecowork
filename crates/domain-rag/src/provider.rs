//! The qualified retrieval provider boundary.
//!
//! LiteCowork Core owns Resource scope, revision, provenance, authorization, and
//! deletion. A retrieval provider only answers an already authorized query with
//! segments that name the exact immutable revision they came from. A provider that is
//! absent, unconfigured, offline, or serving an incompatible index generation fails
//! closed: it never returns partial or stale evidence.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::grounding::RetrievalCitation;
use crate::limits::RetrievalBudget;
use crate::scope::RetrievalScope;

/// Canonical provider identity of the served deterministic local lexical index.
pub const LOCAL_LEXICAL_PROVIDER_ID: &str = "litecowork.local-lexical";
/// Parser identity of the only qualified local extraction parser. It matches the
/// `resource_text_indexes.parser_id` storage constraint.
pub const LOCAL_LEXICAL_PARSER_ID: &str = "litecowork.plain-text.v1";

/// Whether a provider can serve a request right now.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderAvailability {
    /// The provider contract is pinned and the provider can serve this request.
    Qualified,
    /// No provider is registered for this Workspace.
    NotConfigured,
    /// A provider exists but is offline or failed its qualification.
    Unavailable,
    /// The provider exists but cannot serve this request, for example because the
    /// index generation or embedding model does not match the stored index.
    Incompatible,
}

/// Where the provider runs and where its content would leave the device.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderPlacement {
    /// The provider and its index stay on this device.
    Local,
    /// The provider is a remote service.
    Cloud,
}

/// Which retrieval strategies the provider actually combines.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetrievalMode {
    /// Deterministic token equality over the local encrypted text index.
    LexicalOnly,
    /// Lexical recall plus vector recall plus a rerank stage.
    Hybrid,
}

/// A truthful description of the provider that served, or refused, a request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: String,
    pub provider_version: String,
    pub availability: ProviderAvailability,
    pub retrieval_mode: RetrievalMode,
    pub placement: ProviderPlacement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One authorized retrieval request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RetrievalRequest {
    pub workspace_id: String,
    pub query: String,
    pub scope: RetrievalScope,
    pub budget: RetrievalBudget,
}

/// The bounded result of one provider search.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RetrievalPage {
    pub citations: Vec<RetrievalCitation>,
    /// Number of candidates the provider examined before the scope and authorization
    /// filters were applied.
    pub scanned_candidates: usize,
    /// True when the provider stopped at a budget boundary instead of exhausting the
    /// authorized scope.
    pub truncated: bool,
}

/// Why a provider could not serve a request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetrievalError {
    /// No provider is registered for this Workspace.
    NotConfigured { reason_code: String, reason: String },
    /// The provider exists but cannot serve the request.
    Unavailable { reason_code: String, reason: String },
    /// The provider cannot compare the request with the stored index.
    Incompatible { reason_code: String, reason: String },
    /// The request itself is not valid.
    InvalidRequest { reason_code: String, reason: String },
    /// The request exceeded a hard bound.
    BudgetExceeded { reason_code: String, reason: String },
    /// The provider could not verify its own evidence.
    Integrity { reason_code: String, reason: String },
}

impl RetrievalError {
    pub fn reason_code(&self) -> &str {
        match self {
            Self::NotConfigured { reason_code, .. }
            | Self::Unavailable { reason_code, .. }
            | Self::Incompatible { reason_code, .. }
            | Self::InvalidRequest { reason_code, .. }
            | Self::BudgetExceeded { reason_code, .. }
            | Self::Integrity { reason_code, .. } => reason_code,
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            Self::NotConfigured { reason, .. }
            | Self::Unavailable { reason, .. }
            | Self::Incompatible { reason, .. }
            | Self::InvalidRequest { reason, .. }
            | Self::BudgetExceeded { reason, .. }
            | Self::Integrity { reason, .. } => reason,
        }
    }
}

impl fmt::Display for RetrievalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.reason())
    }
}

impl std::error::Error for RetrievalError {}

/// The provider port used by Core. Implementations are provider specific; Core never
/// assumes a wire protocol, endpoint, or embedding API.
pub trait RetrievalProvider: Send + Sync {
    /// The truthful status of this provider right now.
    fn status(&self) -> ProviderStatus;

    /// Searches the already authorized scope and returns exact revision-bound segments.
    fn search(&self, request: &RetrievalRequest) -> Result<RetrievalPage, RetrievalError>;
}
