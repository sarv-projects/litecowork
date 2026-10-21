//! Skill definitions, SkillProposal review, demonstration capture, and MCP App
//! sandbox contracts.
//!
//! Authority:
//! - [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md) — LiteSPM owns
//!   package lifecycle; LiteCowork owns Task-scoped use; Skill text never grants
//!   capabilities or secrets.
//! - [`docs/CAPABILITY-INVOCATIONS.md`](../../../docs/CAPABILITY-INVOCATIONS.md) —
//!   MCP Skill manifest conformance, manifest digest pinning, MCP App frame/CSP/bridge.
//! - [`docs/LEARNING.md`](../../../docs/LEARNING.md) — SkillProposal intake, redaction,
//!   explicit owner review, LiteSPM-owned publication.
//! - [`docs/NETWORK-SECURITY.md`](../../../docs/NETWORK-SECURITY.md) — MCP App sandbox,
//!   exact declared CSP domains, no invented fallback domains.
//! - [`docs/API.md`](../../../docs/API.md) — `/v1/skills`, `/v1/skill-proposals`,
//!   `/v1/demonstrations`, `/v1/discover/*` wire shapes.
//!
//! This module owns deterministic rules only. It never classifies free text, never
//! contacts a package manager, never grants authority, and never publishes a package.
//! LiteSPM's package discovery/installation wire contract is not published in
//! [`docs/CAPABILITY-FABRIC.md`](../../../docs/CAPABILITY-FABRIC.md); every path that
//! would need it is reported as *not qualified* instead of being simulated here.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coworker_learning::redact_sensitive_content;
use crate::is_sha256_digest;

/// Maximum number of files one pinned Skill manifest may list.
pub const SKILL_MANIFEST_MAX_FILES: usize = 512;
/// Maximum total byte size of the files one pinned Skill manifest may list.
pub const SKILL_MANIFEST_MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum number of retained revisions for one Skill.
pub const SKILL_MAX_REVISIONS: usize = 64;
/// Maximum byte length of a Skill title.
pub const SKILL_TITLE_MAX_BYTES: usize = 200;
/// Maximum byte length of a Skill summary.
pub const SKILL_SUMMARY_MAX_BYTES: usize = 2_000;
/// Maximum number of capabilities one Skill revision may declare.
pub const SKILL_DECLARED_CAPABILITY_MAX: usize = 64;
/// Maximum number of typed placeholders one SkillProposal may carry.
pub const SKILL_PLACEHOLDER_MAX: usize = 32;
/// Maximum byte length of one placeholder name.
pub const SKILL_PLACEHOLDER_NAME_MAX_BYTES: usize = 64;
/// Upper bound accepted for a demonstration capture duration policy.
pub const CAPTURE_MAX_POLICY_DURATION_MS: u32 = 1_800_000;
/// Upper bound accepted for a demonstration capture action-count policy.
pub const CAPTURE_MAX_POLICY_ACTIONS: u32 = 1_000;
/// Upper bound accepted for a demonstration capture trace-byte policy.
pub const CAPTURE_MAX_POLICY_TRACE_BYTES: u32 = 5_242_880;
/// Maximum byte length of one MCP App bridge message payload.
pub const APP_BRIDGE_MAX_PAYLOAD_BYTES: usize = 64 * 1024;
/// Maximum byte length of one declared MCP App CSP domain.
pub const APP_CSP_DOMAIN_MAX_BYTES: usize = 255;

/// Canonical blocker code persisted when a pinned Skill no longer matches its
/// approved manifest digest.
pub const SKILL_DRIFT_BLOCKER_CODE: &str = "SKILL_DRIFT_DETECTED";

/// Canonical reason code used whenever the LiteSPM package lifecycle is required
/// but its published contract is unavailable.
pub const PACKAGE_SERVICE_NOT_QUALIFIED: &str = "LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED";

/// Every deterministic rejection this module can produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillGuardError {
    EmptyIdentifier,
    IdentifierTooLong,
    InvalidUri,
    InvalidDigest,
    DuplicateManifestResource,
    TooManyManifestResources,
    ManifestResourceTooLarge,
    ManifestTooLarge,
    EmptyManifest,
    DynamicManifestNotActivatable,
    TitleTooLong,
    SummaryTooLong,
    TooManyDeclaredCapabilities,
    TooManyRevisions,
    RevisionNotFound,
    SkillRevoked,
    SkillSuspended,
    SkillNotActive,
    NoCapabilityGrant,
    GrantScopeMismatch,
    GrantDigestMismatch,
    GrantExpired,
    GrantStatusMismatch,
    CapabilityNotGranted,
    ProposalNotInReview,
    ProposalAlreadyTerminal,
    ProposalRedactionNotPassed,
    ReviewerNotOwner,
    ReviewerNotHuman,
    ReviewerAssuranceTooLow,
    DriftDetected,
    CapturePolicyInvalid,
    CaptureConsentMissing,
    CaptureCapReached,
    CaptureDurationExceeded,
    CaptureEnvironmentClassNotAllowed,
    CaptureEnvironmentMismatch,
    CaptureSessionNotCapturing,
    CaptureSessionTerminal,
    SensitiveRegionDetected,
    TraceTooLarge,
    TooManyActions,
    InvalidEnvironment,
    AppOriginMismatch,
    AppNonceMismatch,
    AppMethodNotAllowed,
    AppPayloadTooLarge,
    AppCredentialInPayload,
    AppFrameExpired,
    AppFrameUnknown,
    AppCspDomainInvalid,
    PackageServiceNotQualified,
    SkillStatusNotAllowed,
    SkillIdentityDuplicate,
}

impl SkillGuardError {
    /// Stable machine-readable reason code.
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::EmptyIdentifier => "EMPTY_IDENTIFIER",
            Self::IdentifierTooLong => "IDENTIFIER_TOO_LONG",
            Self::InvalidUri => "INVALID_URI",
            Self::InvalidDigest => "INVALID_DIGEST",
            Self::DuplicateManifestResource => "DUPLICATE_MANIFEST_RESOURCE",
            Self::TooManyManifestResources => "TOO_MANY_MANIFEST_RESOURCES",
            Self::ManifestResourceTooLarge => "MANIFEST_RESOURCE_TOO_LARGE",
            Self::ManifestTooLarge => "MANIFEST_TOO_LARGE",
            Self::EmptyManifest => "EMPTY_MANIFEST",
            Self::DynamicManifestNotActivatable => "DYNAMIC_MANIFEST_NOT_ACTIVATABLE",
            Self::TitleTooLong => "TITLE_TOO_LONG",
            Self::SummaryTooLong => "SUMMARY_TOO_LONG",
            Self::TooManyDeclaredCapabilities => "TOO_MANY_DECLARED_CAPABILITIES",
            Self::TooManyRevisions => "TOO_MANY_REVISIONS",
            Self::RevisionNotFound => "REVISION_NOT_FOUND",
            Self::SkillRevoked => "SKILL_REVOKED",
            Self::SkillSuspended => "SKILL_SUSPENDED",
            Self::SkillNotActive => "SKILL_NOT_ACTIVE",
            Self::NoCapabilityGrant => "NO_CAPABILITY_GRANT",
            Self::GrantScopeMismatch => "GRANT_SCOPE_MISMATCH",
            Self::GrantDigestMismatch => "GRANT_DIGEST_MISMATCH",
            Self::GrantExpired => "GRANT_EXPIRED",
            Self::GrantStatusMismatch => "GRANT_STATUS_MISMATCH",
            Self::CapabilityNotGranted => "CAPABILITY_NOT_GRANTED",
            Self::ProposalNotInReview => "PROPOSAL_NOT_IN_REVIEW",
            Self::ProposalAlreadyTerminal => "PROPOSAL_ALREADY_TERMINAL",
            Self::ProposalRedactionNotPassed => "PROPOSAL_REDACTION_NOT_PASSED",
            Self::ReviewerNotOwner => "REVIEWER_NOT_OWNER",
            Self::ReviewerNotHuman => "REVIEWER_NOT_HUMAN",
            Self::ReviewerAssuranceTooLow => "REVIEWER_ASSURANCE_TOO_LOW",
            Self::DriftDetected => SKILL_DRIFT_BLOCKER_CODE,
            Self::CapturePolicyInvalid => "CAPTURE_POLICY_INVALID",
            Self::CaptureConsentMissing => "CAPTURE_CONSENT_MISSING",
            Self::CaptureCapReached => "CAPTURE_CAP_REACHED",
            Self::CaptureDurationExceeded => "CAPTURE_DURATION_EXCEEDED",
            Self::CaptureEnvironmentClassNotAllowed => "CAPTURE_ENVIRONMENT_CLASS_NOT_ALLOWED",
            Self::CaptureEnvironmentMismatch => "CAPTURE_ENVIRONMENT_MISMATCH",
            Self::CaptureSessionNotCapturing => "CAPTURE_SESSION_NOT_CAPTURING",
            Self::CaptureSessionTerminal => "CAPTURE_SESSION_TERMINAL",
            Self::SensitiveRegionDetected => "SENSITIVE_REGION_DETECTED",
            Self::TraceTooLarge => "TRACE_TOO_LARGE",
            Self::TooManyActions => "TOO_MANY_ACTIONS",
            Self::InvalidEnvironment => "INVALID_ENVIRONMENT",
            Self::AppOriginMismatch => "APP_ORIGIN_MISMATCH",
            Self::AppNonceMismatch => "APP_NONCE_MISMATCH",
            Self::AppMethodNotAllowed => "APP_METHOD_NOT_ALLOWED",
            Self::AppPayloadTooLarge => "APP_PAYLOAD_TOO_LARGE",
            Self::AppCredentialInPayload => "APP_CREDENTIAL_IN_PAYLOAD",
            Self::AppFrameExpired => "APP_FRAME_EXPIRED",
            Self::AppFrameUnknown => "APP_FRAME_UNKNOWN",
            Self::AppCspDomainInvalid => "APP_CSP_DOMAIN_INVALID",
            Self::PackageServiceNotQualified => PACKAGE_SERVICE_NOT_QUALIFIED,
            Self::SkillStatusNotAllowed => "SKILL_STATUS_NOT_ALLOWED",
            Self::SkillIdentityDuplicate => "SKILL_IDENTITY_DUPLICATE",
        }
    }
}

impl std::fmt::Display for SkillGuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason_code())
    }
}

impl std::error::Error for SkillGuardError {}

/// True when `value` is a bounded, printable identifier.
pub fn is_valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(char::is_control)
}

/// True when `value` is an absolute URI with a scheme and no whitespace.
pub fn is_valid_uri(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 2_048
        && !value.chars().any(|character| character.is_control() || character.is_whitespace())
        && value
            .split_once(':')
            .is_some_and(|(scheme, _)| {
                !scheme.is_empty()
                    && scheme.starts_with(|c: char| c.is_ascii_lowercase())
                    && scheme
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"+-.".contains(&byte))
            })
}

// ---------------------------------------------------------------------------
// MCP Skill manifest conformance
// ---------------------------------------------------------------------------

/// Whether a manifest pins its content or advertises a dynamic tree.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillManifestKind {
    /// Every supporting file is listed with its digest and size.
    Pinned,
    /// The server advertises `dynamic`; it has no content integrity.
    Dynamic,
}

/// One file listed by a pinned Skill manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillManifestResource {
    pub uri: String,
    pub digest: String,
    pub size_bytes: u64,
}

/// The advertised manifest of one MCP Skill.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    /// Host-assigned authenticated MCP server identity.
    pub origin_id: String,
    /// The exact advertised `SKILL.md` URI.
    pub skill_uri: String,
    pub kind: SkillManifestKind,
    /// Every supporting file, each listed exactly once.
    pub resources: Vec<SkillManifestResource>,
}

impl SkillManifest {
    /// A dynamic manifest is displayable but never activatable: it carries no
    /// content integrity and therefore no content-bound approval.
    pub const fn is_activatable(&self) -> bool {
        matches!(self.kind, SkillManifestKind::Pinned)
    }
}

/// Validate a pinned or dynamic Skill manifest against the MCP Skills contract.
///
/// A hostile manifest is rejected rather than repaired: duplicated entries, an
/// oversized tree, a malformed digest, or a malformed URI all fail closed.
pub fn validate_skill_manifest(manifest: &SkillManifest) -> Result<(), SkillGuardError> {
    if !is_valid_identifier(&manifest.origin_id) {
        return Err(SkillGuardError::EmptyIdentifier);
    }
    if !is_valid_uri(&manifest.skill_uri) {
        return Err(SkillGuardError::InvalidUri);
    }
    match manifest.kind {
        SkillManifestKind::Dynamic => {
            if !manifest.resources.is_empty() {
                return Err(SkillGuardError::EmptyManifest);
            }
            Ok(())
        }
        SkillManifestKind::Pinned => {
            if manifest.resources.is_empty() {
                return Err(SkillGuardError::EmptyManifest);
            }
            if manifest.resources.len() > SKILL_MANIFEST_MAX_FILES {
                return Err(SkillGuardError::TooManyManifestResources);
            }
            let mut total: u64 = 0;
            let mut seen: Vec<&str> = Vec::with_capacity(manifest.resources.len());
            for resource in &manifest.resources {
                if !is_valid_uri(&resource.uri) {
                    return Err(SkillGuardError::InvalidUri);
                }
                if seen.iter().any(|uri| *uri == resource.uri.as_str()) {
                    return Err(SkillGuardError::DuplicateManifestResource);
                }
                seen.push(resource.uri.as_str());
                if !is_sha256_digest(&resource.digest) {
                    return Err(SkillGuardError::InvalidDigest);
                }
                if resource.size_bytes == 0 || resource.size_bytes > SKILL_MANIFEST_MAX_TOTAL_BYTES
                {
                    return Err(SkillGuardError::ManifestResourceTooLarge);
                }
                total = total.saturating_add(resource.size_bytes);
                if total > SKILL_MANIFEST_MAX_TOTAL_BYTES {
                    return Err(SkillGuardError::ManifestTooLarge);
                }
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Skill definition and revision
// ---------------------------------------------------------------------------

/// The kind of Skill LiteCowork is holding a definition for.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillKind {
    /// A LiteCowork-owned procedural definition produced by an approved proposal.
    LocalProcedure,
    /// A Skill published by an MCP Skills extension server.
    McpSkill,
    /// A component of a LiteSPM-managed package.
    PackageComponent,
}

/// The lifecycle state of a Skill definition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillStatus {
    /// Held for review; never loadable.
    Draft,
    /// Loadable only through a scoped CapabilityGrant.
    Active,
    /// Temporarily not loadable; the definition and its history are retained.
    Suspended,
    /// Permanently not loadable.
    Revoked,
}

impl SkillStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Active => "ACTIVE",
            Self::Suspended => "SUSPENDED",
            Self::Revoked => "REVOKED",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Revoked)
    }
}

/// Where a Skill definition came from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "origin_kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillOrigin {
    /// Derived from an owner-approved SkillProposal in this Workspace.
    LocalProposal {
        proposal_id: String,
    },
    /// Published by an MCP Skills extension server.
    McpServer {
        server_identity: String,
        skill_uri: String,
    },
    /// A component of a LiteSPM-managed package.
    LiteSpmPackage {
        package_id: String,
        package_version: String,
        package_digest: String,
    },
}

/// The normalized capability identity of one Skill.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillCapabilityRef {
    pub capability_id: String,
    pub identity_kind: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    pub digest: String,
    pub component: String,
}

/// One immutable revision of a Skill definition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRevision {
    pub revision: u64,
    pub title: String,
    pub summary: String,
    /// Digest over the pinned manifest (`{origin_id, skill_uri, resources}`).
    pub manifest_digest: String,
    /// Digest of the stored instruction content.
    pub content_digest: String,
    /// Resource reference holding the instruction content. Never inline text.
    pub content_ref: String,
    /// Capability names the procedure asks for. These are a request, never authority.
    pub declared_capabilities: Vec<String>,
    /// Additional declared requirements, for example an Environment class.
    pub declared_requirements: Vec<String>,
    pub origin: SkillOrigin,
    pub created_at: String,
    pub created_by: String,
}

/// The LiteCowork-owned record of one Skill.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRecord {
    pub skill_id: String,
    pub workspace_id: String,
    pub kind: SkillKind,
    pub status: SkillStatus,
    pub capability_ref: SkillCapabilityRef,
    pub current_revision: u64,
    pub revisions: Vec<SkillRevision>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

impl SkillRecord {
    /// The current immutable revision, if the record has one.
    pub fn current_revision_record(&self) -> Option<&SkillRevision> {
        self.revisions
            .iter()
            .find(|revision| revision.revision == self.current_revision)
    }

    /// Validate the record's own shape. Cross-record authority checks live in
    /// [`evaluate_skill_activation`].
    pub fn validate(&self) -> Result<(), SkillGuardError> {
        if !is_valid_identifier(&self.skill_id) || !is_valid_identifier(&self.workspace_id) {
            return Err(SkillGuardError::EmptyIdentifier);
        }
        if !is_valid_identifier(&self.capability_ref.capability_id)
            || !is_valid_identifier(&self.capability_ref.source)
            || !is_valid_identifier(&self.capability_ref.component)
        {
            return Err(SkillGuardError::EmptyIdentifier);
        }
        if !is_sha256_digest(&self.capability_ref.digest) {
            return Err(SkillGuardError::InvalidDigest);
        }
        if self.revisions.len() > SKILL_MAX_REVISIONS {
            return Err(SkillGuardError::TooManyRevisions);
        }
        if self.current_revision == 0 || self.revisions.is_empty() {
            return Err(SkillGuardError::RevisionNotFound);
        }
        let Some(current) = self.current_revision_record() else {
            return Err(SkillGuardError::RevisionNotFound);
        };
        validate_skill_revision(current)
    }
}

/// Validate one Skill revision's own shape.
pub fn validate_skill_revision(revision: &SkillRevision) -> Result<(), SkillGuardError> {
    if revision.revision == 0 {
        return Err(SkillGuardError::RevisionNotFound);
    }
    if revision.title.is_empty() || revision.title.len() > SKILL_TITLE_MAX_BYTES {
        return Err(SkillGuardError::TitleTooLong);
    }
    if revision.summary.len() > SKILL_SUMMARY_MAX_BYTES {
        return Err(SkillGuardError::SummaryTooLong);
    }
    if !is_sha256_digest(&revision.manifest_digest)
        || !is_sha256_digest(&revision.content_digest)
    {
        return Err(SkillGuardError::InvalidDigest);
    }
    if !is_valid_identifier(&revision.content_ref) {
        return Err(SkillGuardError::EmptyIdentifier);
    }
    if revision.declared_capabilities.len() > SKILL_DECLARED_CAPABILITY_MAX {
        return Err(SkillGuardError::TooManyDeclaredCapabilities);
    }
    if revision
        .declared_capabilities
        .iter()
        .any(|capability| !is_valid_identifier(capability))
    {
        return Err(SkillGuardError::EmptyIdentifier);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Skill activation guard
// ---------------------------------------------------------------------------

/// Decide whether one owner-initiated Skill definition write may be persisted.
///
/// A Skill definition is procedural text. Writing one never grants authority:
/// the Skill is only loadable through a scoped CapabilityGrant bound to the exact
/// capability reference *and* the exact manifest digest (see
/// [`evaluate_skill_activation`]).
///
/// The guard therefore fails closed on the lifecycle transition. A newly created
/// Skill is always `DRAFT`; a revision may narrow an existing Skill's status
/// (`ACTIVE` to `SUSPENDED` or `REVOKED`) but may never widen it. No API path can
/// therefore promote a definition straight into the loadable state, and a
/// definition that changes its manifest digest stops matching the grant that
/// pinned the previous revision.
///
/// The returned status is the status that may be persisted. It is never the
/// requested one when the request was refused.
pub fn evaluate_skill_definition_write(
    current: Option<&SkillRecord>,
    requested_status: SkillStatus,
) -> Result<SkillStatus, SkillGuardError> {
    match current {
        None => {
            if requested_status != SkillStatus::Draft {
                return Err(SkillGuardError::SkillStatusNotAllowed);
            }
            Ok(SkillStatus::Draft)
        }
        Some(current) => {
            if requested_status == SkillStatus::Active && current.status != SkillStatus::Active {
                return Err(SkillGuardError::SkillStatusNotAllowed);
            }
            Ok(requested_status)
        }
    }
}

/// The CapabilityGrant that authorizes loading one Skill.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillActivationGrant {
    pub grant_id: String,
    /// The exact `capability_ref_key_digest` the grant is bound to.
    pub capability_ref: String,
    /// The manifest digest the grant is bound to.
    pub manifest_digest: String,
    /// `CONVERSATION`, `TASK_PLANNING`, or `ATTEMPT_EXECUTION`.
    pub scope: String,
    /// `ACTIVE`, `REVOKED`, or `EXPIRED`.
    pub status: String,
    pub allowed_operations: Vec<String>,
    pub expires_at: Option<String>,
}

/// The proven authorization for loading one Skill revision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillActivation {
    pub skill_id: String,
    pub revision: u64,
    pub manifest_digest: String,
    pub grant_id: String,
    pub scope: String,
    pub content_ref: String,
    pub content_digest: String,
}

/// Prove that one Skill revision may be loaded now.
///
/// A Skill is procedural text. It never carries authority: the grant bound to the
/// exact capability reference *and* the exact manifest digest decides which
/// capabilities the procedure may use, and every capability the revision declares
/// must already be inside that grant.
pub fn evaluate_skill_activation(
    skill: &SkillRecord,
    grant: Option<&SkillActivationGrant>,
    capability_ref_key_digest: &str,
    now: &str,
) -> Result<SkillActivation, SkillGuardError> {
    skill.validate()?;
    match skill.status {
        SkillStatus::Revoked => return Err(SkillGuardError::SkillRevoked),
        SkillStatus::Suspended => return Err(SkillGuardError::SkillSuspended),
        SkillStatus::Draft => return Err(SkillGuardError::SkillNotActive),
        SkillStatus::Active => {}
    }
    let revision = skill
        .current_revision_record()
        .ok_or(SkillGuardError::RevisionNotFound)?;
    let Some(grant) = grant else {
        return Err(SkillGuardError::NoCapabilityGrant);
    };
    if grant.status != "ACTIVE" {
        return Err(SkillGuardError::GrantStatusMismatch);
    }
    if let Some(expires_at) = grant.expires_at.as_deref() {
        if expires_at <= now {
            return Err(SkillGuardError::GrantExpired);
        }
    }
    if grant.capability_ref != capability_ref_key_digest {
        return Err(SkillGuardError::GrantScopeMismatch);
    }
    if grant.manifest_digest != revision.manifest_digest {
        return Err(SkillGuardError::GrantDigestMismatch);
    }
    for capability in &revision.declared_capabilities {
        if !grant.allowed_operations.iter().any(|allowed| allowed == capability) {
            return Err(SkillGuardError::CapabilityNotGranted);
        }
    }
    Ok(SkillActivation {
        skill_id: skill.skill_id.clone(),
        revision: revision.revision,
        manifest_digest: revision.manifest_digest.clone(),
        grant_id: grant.grant_id.clone(),
        scope: grant.scope.clone(),
        content_ref: revision.content_ref.clone(),
        content_digest: revision.content_digest.clone(),
    })
}

// ---------------------------------------------------------------------------
// SkillProposal
// ---------------------------------------------------------------------------

/// Where a SkillProposal came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillProposalSource {
    /// A demonstrated workflow captured with explicit owner consent.
    Demonstration,
    /// A proposal produced by the learning pipeline.
    LearningCandidate,
    /// Authored directly by the owner.
    OwnerAuthored,
}

/// The lifecycle state of a SkillProposal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillProposalStatus {
    /// Captured but not yet reviewed.
    Draft,
    /// Waiting for an explicit owner decision.
    Review,
    /// The owner approved the draft. Publication is still LiteSPM-owned.
    Approved,
    /// The owner rejected the draft.
    Rejected,
    /// A LiteSPM-managed package published this draft.
    Published,
}

impl SkillProposalStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Review => "REVIEW",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Published => "PUBLISHED",
        }
    }

    /// A proposal that can no longer change.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Published)
    }

    /// A proposal on which no further owner review is possible.
    ///
    /// An approved proposal is closed for review even though package
    /// publication is still outstanding: publication is LiteSPM-owned and a
    /// second owner approval would not change anything.
    pub const fn is_review_closed(self) -> bool {
        matches!(self, Self::Approved | Self::Rejected | Self::Published)
    }
}

/// The result of scanning a proposed procedure body.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillRedactionStatus {
    /// The body has not been scanned yet.
    Pending,
    /// The body was scanned and carries no forbidden material.
    Passed,
    /// The body still carries forbidden material and must not be published.
    Failed,
}

/// The kind of a typed placeholder extracted from a captured demonstration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillPlaceholderKind {
    /// A filesystem path observed during the demonstration.
    Path,
    /// A URL observed during the demonstration.
    Url,
    /// A domain-specific identifier observed during the demonstration.
    Identifier,
    /// A literal value that the procedure must not carry.
    Literal,
}

impl SkillPlaceholderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Path => "PATH",
            Self::Url => "URL",
            Self::Identifier => "IDENTIFIER",
            Self::Literal => "LITERAL",
        }
    }
}

/// One typed placeholder that replaces an observed task-specific value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPlaceholder {
    pub name: String,
    pub kind: SkillPlaceholderKind,
    /// A non-secret example of the observed shape. Never the original secret.
    pub sample: Option<String>,
}

/// The summary of the redaction pass over a proposed procedure body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRedactionSummary {
    pub status: SkillRedactionStatus,
    pub redacted_spans: u32,
    /// Stable reason codes only. No matched secret bytes are ever retained.
    pub finding_codes: Vec<String>,
    /// Digest of the redacted body that was actually stored.
    pub redacted_digest: String,
}

/// The observed drift between an approved manifest and a later observation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillDrift {
    pub expected_manifest_digest: String,
    pub observed_manifest_digest: String,
    /// URIs whose digest changed. Names only; no content.
    pub drifted_resources: Vec<String>,
    pub detected_at: String,
}

/// The LiteCowork-owned SkillProposal record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProposalRecord {
    pub proposal_id: String,
    pub workspace_id: String,
    pub source: SkillProposalSource,
    pub source_task_id: String,
    pub source_artifact_id: String,
    /// Resource reference holding the redacted draft body.
    pub draft_resource_ref: String,
    /// Digest of the redacted draft body.
    pub draft_digest: String,
    pub redaction: SkillRedactionSummary,
    /// Typed placeholders that replaced task-specific values.
    pub placeholders: Vec<SkillPlaceholder>,
    /// Capability names the procedure asks for. Never authority.
    pub declared_capabilities: Vec<String>,
    pub status: SkillProposalStatus,
    pub proposed_by: String,
    pub approved_by: Option<String>,
    pub published_capability_ref: Option<String>,
    pub drift: Option<SkillDrift>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

impl SkillProposalRecord {
    /// Validate the record's own shape.
    pub fn validate(&self) -> Result<(), SkillGuardError> {
        if !is_valid_identifier(&self.proposal_id)
            || !is_valid_identifier(&self.workspace_id)
            || !is_valid_identifier(&self.source_task_id)
            || !is_valid_identifier(&self.source_artifact_id)
            || !is_valid_identifier(&self.draft_resource_ref)
        {
            return Err(SkillGuardError::EmptyIdentifier);
        }
        if !is_sha256_digest(&self.draft_digest) {
            return Err(SkillGuardError::InvalidDigest);
        }
        if self.placeholders.len() > SKILL_PLACEHOLDER_MAX {
            return Err(SkillGuardError::TooManyDeclaredCapabilities);
        }
        for placeholder in &self.placeholders {
            if placeholder.name.is_empty()
                || placeholder.name.len() > SKILL_PLACEHOLDER_NAME_MAX_BYTES
                || !placeholder
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                return Err(SkillGuardError::EmptyIdentifier);
            }
        }
        if self.declared_capabilities.len() > SKILL_DECLARED_CAPABILITY_MAX {
            return Err(SkillGuardError::TooManyDeclaredCapabilities);
        }
        Ok(())
    }
}

/// The authenticated owner who is deciding a proposal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProposalReviewer {
    pub principal_id: String,
    /// `USER` for a human owner; every other kind is a machine.
    pub principal_kind: String,
    /// `APPROVE_SAFE` or `APPROVE_HIGH`.
    pub assurance_level: String,
}

/// The decision an owner took on a proposal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillProposalDecision {
    Approve,
    Reject,
}

/// What one review decision produced.
///
/// An approval never installs a package, never activates a Skill, and never
/// grants authority. Publication stays with LiteSPM, and the LiteSPM package
/// contract is not published, so publication is reported as blocked.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProposalReviewOutcome {
    pub proposal_id: String,
    pub status: SkillProposalStatus,
    pub reviewed_by: String,
    pub reviewed_at: String,
    /// Always `None`: a draft can never enable itself.
    pub activated_skill_id: Option<String>,
    /// The reason publication cannot proceed yet.
    pub publication_blocker: Option<String>,
}

/// Decide one SkillProposal.
///
/// The reviewer must be the authenticated Workspace owner acting as a human with
/// sufficient assurance. A service, runtime, or agent may never approve a
/// proposal, and no path here approves automatically.
pub fn evaluate_skill_proposal_review(
    proposal: &SkillProposalRecord,
    owner_principal_id: &str,
    reviewer: &SkillProposalReviewer,
    decision: SkillProposalDecision,
    now: &str,
) -> Result<SkillProposalReviewOutcome, SkillGuardError> {
    proposal.validate()?;
    if reviewer.principal_id != owner_principal_id {
        return Err(SkillGuardError::ReviewerNotOwner);
    }
    if reviewer.principal_kind != "USER" {
        return Err(SkillGuardError::ReviewerNotHuman);
    }
    if !matches!(reviewer.assurance_level.as_str(), "APPROVE_SAFE" | "APPROVE_HIGH") {
        return Err(SkillGuardError::ReviewerAssuranceTooLow);
    }
    match proposal.status {
        SkillProposalStatus::Review => {}
        SkillProposalStatus::Approved
        | SkillProposalStatus::Rejected
        | SkillProposalStatus::Published => {
            return Err(SkillGuardError::ProposalAlreadyTerminal);
        }
        SkillProposalStatus::Draft => return Err(SkillGuardError::ProposalNotInReview),
    }
    // A proposal may only be approved when its redaction actually passed. A
    // pending or failed redaction is never approvable: an unredacted draft
    // must not reach a package or an owner's approval.
    if proposal.redaction.status != SkillRedactionStatus::Passed {
        return Err(SkillGuardError::ProposalRedactionNotPassed);
    }
    if proposal.drift.is_some() {
        return Err(SkillGuardError::DriftDetected);
    }
    let (status, publication_blocker) = match decision {
        SkillProposalDecision::Approve => (
            SkillProposalStatus::Approved,
            // LiteSPM owns package publication and its wire contract is not
            // published, so an approval cannot publish anything yet.
            Some(PACKAGE_SERVICE_NOT_QUALIFIED.to_owned()),
        ),
        SkillProposalDecision::Reject => (SkillProposalStatus::Rejected, None),
    };
    Ok(SkillProposalReviewOutcome {
        proposal_id: proposal.proposal_id.clone(),
        status,
        reviewed_by: reviewer.principal_id.clone(),
        reviewed_at: now.to_owned(),
        activated_skill_id: None,
        publication_blocker,
    })
}

/// Detect drift between an approved manifest digest and a later observation.
///
/// A changed manifest revokes the prior approval and requires a new review; the
/// canonical blocker code is [`SKILL_DRIFT_BLOCKER_CODE`].
pub fn detect_skill_drift(
    expected_manifest_digest: &str,
    observed_manifest_digest: &str,
    drifted_resources: Vec<String>,
    detected_at: &str,
) -> Option<SkillDrift> {
    if expected_manifest_digest == observed_manifest_digest {
        return None;
    }
    Some(SkillDrift {
        expected_manifest_digest: expected_manifest_digest.to_owned(),
        observed_manifest_digest: observed_manifest_digest.to_owned(),
        drifted_resources,
        detected_at: detected_at.to_owned(),
    })
}

// ---------------------------------------------------------------------------
// Demonstration capture
// ---------------------------------------------------------------------------

/// What a capture does when a sensitive region is detected.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SensitiveRegionPolicy {
    /// Stop appending observations and wait for an explicit owner resume.
    PauseOnDetection,
    /// Keep recording but drop the sensitive fields.
    OmitSensitiveFields,
}

impl SensitiveRegionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PauseOnDetection => "PAUSE_ON_DETECTION",
            Self::OmitSensitiveFields => "OMIT_SENSITIVE_FIELDS",
        }
    }
}

/// The immutable policy one demonstration capture runs under.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DemonstrationCapturePolicy {
    pub max_duration_ms: u32,
    pub max_actions: u32,
    pub max_trace_bytes: u32,
    /// `BROWSER` or `DESKTOP`.
    pub allowed_environment_class: String,
    pub sensitive_region_policy: SensitiveRegionPolicy,
}

impl DemonstrationCapturePolicy {
    /// Validate the policy bounds. A policy is immutable once a capture starts.
    pub fn validate(&self) -> Result<(), SkillGuardError> {
        if self.max_duration_ms == 0 || self.max_duration_ms > CAPTURE_MAX_POLICY_DURATION_MS {
            return Err(SkillGuardError::CapturePolicyInvalid);
        }
        if self.max_actions == 0 || self.max_actions > CAPTURE_MAX_POLICY_ACTIONS {
            return Err(SkillGuardError::CapturePolicyInvalid);
        }
        if self.max_trace_bytes == 0 || self.max_trace_bytes > CAPTURE_MAX_POLICY_TRACE_BYTES {
            return Err(SkillGuardError::CapturePolicyInvalid);
        }
        if !matches!(self.allowed_environment_class.as_str(), "BROWSER" | "DESKTOP") {
            return Err(SkillGuardError::CaptureEnvironmentClassNotAllowed);
        }
        Ok(())
    }
}

/// The explicit owner consent a capture requires.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DemonstrationCaptureConsent {
    pub granted_by_principal_id: String,
    pub granted_at: String,
    /// `SEMANTIC_CAPTURE`.
    pub scope: String,
}

impl DemonstrationCaptureConsent {
    /// Consent is present only when an authenticated owner granted semantic capture.
    pub fn is_present(&self) -> bool {
        !self.granted_by_principal_id.trim().is_empty()
            && self.scope == "SEMANTIC_CAPTURE"
            && !self.granted_at.trim().is_empty()
    }
}

/// The lifecycle state of one demonstration capture.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DemonstrationStatus {
    Created,
    Capturing,
    Paused,
    Review,
    Converted,
    Aborted,
}

impl DemonstrationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Capturing => "CAPTURING",
            Self::Paused => "PAUSED",
            Self::Review => "REVIEW",
            Self::Converted => "CONVERTED",
            Self::Aborted => "ABORTED",
        }
    }

    /// A capture that can no longer record.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Converted | Self::Aborted)
    }
}

/// One semantic action observed during a demonstration.
///
/// Raw screen coordinates are never the authority for a step: an action carries
/// the semantic target it acted on, and the coordinate is only an optional hint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DemonstrationAction {
    pub sequence: u32,
    /// The semantic action kind, for example `click` or `type`.
    pub kind: String,
    /// The semantic target the action acted on.
    pub target: String,
    /// The observed value, when the action produced one.
    pub value: Option<String>,
    /// True when the observation landed in a sensitive region.
    pub sensitive: bool,
    /// Optional pointer hint. Never the sole authority for the step.
    pub coordinate_hint: Option<String>,
}

impl DemonstrationAction {
    /// The same observation with its sensitive fields removed.
    ///
    /// The observed value and the pointer hint are dropped. The semantic kind
    /// and target stay, because a procedure is reconstructed from semantics and
    /// never from a pointer position.
    pub fn without_sensitive_fields(&self) -> Self {
        Self {
            sequence: self.sequence,
            kind: self.kind.clone(),
            target: self.target.clone(),
            value: None,
            sensitive: true,
            coordinate_hint: None,
        }
    }

    /// The number of bytes this observation occupies in the bounded trace.
    pub fn trace_bytes(&self) -> u32 {
        serde_json::to_vec(self).map(|bytes| bytes.len() as u32).unwrap_or(0)
    }
}

/// The bounded trace one capture produced.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DemonstrationTrace {
    pub actions: Vec<DemonstrationAction>,
    pub trace_bytes: u32,
    pub started_at: String,
    pub ended_at: String,
}

/// The durable record of one demonstration capture session.
///
/// The session holds the bounded, semantic observation list. It never holds a
/// credential, and the observed values are replaced by typed placeholders when
/// the draft is derived.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DemonstrationSessionRecord {
    pub session_id: String,
    pub workspace_id: String,
    pub environment_id: String,
    /// The Task whose workflow is being demonstrated.
    pub source_task_id: Option<String>,
    /// The Artifact that records the demonstrated result.
    pub source_artifact_id: Option<String>,
    pub status: DemonstrationStatus,
    pub capture_policy: DemonstrationCapturePolicy,
    pub consent: DemonstrationCaptureConsent,
    pub captured_action_count: u32,
    pub captured_trace_bytes: u32,
    pub started_at: String,
    pub completed_at: Option<String>,
    /// The Resource that holds the redacted trace, when one was published.
    pub trace_resource_id: Option<String>,
    /// The proposal this capture converted into.
    pub skill_proposal_id: Option<String>,
    /// The bounded semantic observation list.
    pub actions: Vec<DemonstrationAction>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

impl DemonstrationSessionRecord {
    /// Validate the session's own shape and the capture policy it is bound to.
    pub fn validate(&self) -> Result<(), SkillGuardError> {
        if !is_valid_identifier(&self.session_id) || !is_valid_identifier(&self.workspace_id) {
            return Err(SkillGuardError::EmptyIdentifier);
        }
        if !is_valid_identifier(&self.environment_id) {
            return Err(SkillGuardError::InvalidEnvironment);
        }
        self.capture_policy.validate()?;
        if !self.consent.is_present() {
            return Err(SkillGuardError::CaptureConsentMissing);
        }
        if self.actions.len() > CAPTURE_MAX_POLICY_ACTIONS as usize {
            return Err(SkillGuardError::TooManyActions);
        }
        Ok(())
    }

    /// The bounded trace this session observed.
    pub fn trace(&self) -> DemonstrationTrace {
        DemonstrationTrace {
            actions: self.actions.clone(),
            trace_bytes: self.captured_trace_bytes,
            started_at: self.started_at.clone(),
            ended_at: self.completed_at.clone().unwrap_or_else(|| self.updated_at.clone()),
        }
    }
}

/// Why one capture action was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureRefusal {
    /// The policy's action count was reached; recording stopped.
    ActionCapReached,
    /// The policy's trace byte budget was reached; recording stopped.
    TraceCapReached,
    /// The policy's duration was reached; recording stopped.
    DurationReached,
    /// A sensitive region was detected and the policy pauses on detection.
    SensitiveRegionPaused,
    /// The capture is not currently recording.
    NotCapturing,
    /// The capture environment does not match the session.
    EnvironmentMismatch,
}

/// The outcome of appending one observation to a capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureDecision {
    /// The observation was appended.
    Recorded { action_count: u32, trace_bytes: u32 },
    /// The observation was appended with its sensitive fields omitted.
    ///
    /// The `OMIT_SENSITIVE_FIELDS` policy keeps recording the semantic step
    /// while dropping the observed value and the pointer hint.
    RecordedWithSensitiveFieldsOmitted { action_count: u32, trace_bytes: u32 },
    /// The observation was refused; the reason is recorded.
    Refused { reason: CaptureRefusal },
}

/// Decide whether one observation may be appended to a capture.
///
/// The caps stop recording rather than truncating silently, and a sensitive
/// region follows the immutable policy.
pub fn evaluate_capture_append(
    policy: &DemonstrationCapturePolicy,
    status: DemonstrationStatus,
    environment_id: &str,
    session_environment_id: &str,
    action_count: u32,
    trace_bytes: u32,
    elapsed_ms: u64,
    action: &DemonstrationAction,
) -> CaptureDecision {
    if status != DemonstrationStatus::Capturing {
        return CaptureDecision::Refused {
            reason: CaptureRefusal::NotCapturing,
        };
    }
    if environment_id != session_environment_id {
        return CaptureDecision::Refused {
            reason: CaptureRefusal::EnvironmentMismatch,
        };
    }
    if action.sensitive && policy.sensitive_region_policy == SensitiveRegionPolicy::PauseOnDetection {
        // The capture stops here. The caller pauses the session durably and
        // waits for an explicit owner resume.
        return CaptureDecision::Refused {
            reason: CaptureRefusal::SensitiveRegionPaused,
        };
    }
    if action_count >= policy.max_actions {
        return CaptureDecision::Refused {
            reason: CaptureRefusal::ActionCapReached,
        };
    }
    if trace_bytes >= policy.max_trace_bytes {
        return CaptureDecision::Refused {
            reason: CaptureRefusal::TraceCapReached,
        };
    }
    if elapsed_ms >= u64::from(policy.max_duration_ms) {
        return CaptureDecision::Refused {
            reason: CaptureRefusal::DurationReached,
        };
    }
    let omit_sensitive_fields = action.sensitive;
    let observation = if omit_sensitive_fields {
        action.without_sensitive_fields()
    } else {
        action.clone()
    };
    let observation_bytes = observation.trace_bytes();
    if trace_bytes.saturating_add(observation_bytes) > policy.max_trace_bytes {
        return CaptureDecision::Refused {
            reason: CaptureRefusal::TraceCapReached,
        };
    }
    let action_count = action_count + 1;
    let trace_bytes = trace_bytes.saturating_add(observation_bytes);
    if omit_sensitive_fields {
        CaptureDecision::RecordedWithSensitiveFieldsOmitted {
            action_count,
            trace_bytes,
        }
    } else {
        CaptureDecision::Recorded {
            action_count,
            trace_bytes,
        }
    }
}

/// The redacted draft one completed capture produced.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillProposalDraft {
    pub title: String,
    pub summary: String,
    /// The redacted procedure body. Secret bytes are already removed.
    pub body: String,
    pub placeholders: Vec<SkillPlaceholder>,
    pub declared_capabilities: Vec<String>,
    pub redaction: SkillRedactionSummary,
}

/// Derive a reviewable draft from a completed capture.
///
/// The procedure is reconstructed from semantic actions, never from raw
/// coordinates. Observed values are replaced by typed placeholders, and the
/// reconstructed body is scanned by the shared deterministic redactor. A body
/// that still carries forbidden material produces a failed redaction summary
/// and the caller must not store it.
pub fn derive_skill_proposal_draft(
    trace: &DemonstrationTrace,
    policy: &DemonstrationCapturePolicy,
    title: &str,
) -> SkillProposalDraft {
    let mut body = String::new();
    let mut placeholders: Vec<SkillPlaceholder> = Vec::new();
    let mut declared_capabilities: Vec<String> = Vec::new();
    let mut counters: Vec<(SkillPlaceholderKind, usize)> = Vec::new();
    for action in &trace.actions {
        if action.sensitive {
            continue;
        }
        body.push_str(&action.kind);
        body.push(' ');
        body.push_str(&action.target);
        if let Some(value) = action.value.as_deref() {
            if !value.is_empty() {
                let kind = classify_placeholder_value(value);
                let index = match counters.iter_mut().find(|(k, _)| *k == kind) {
                    Some((_, count)) => {
                        *count += 1;
                        *count
                    }
                    None => {
                        counters.push((kind, 1));
                        1
                    }
                };
                let name = format!("{}_{}", kind.as_str(), index);
                if !placeholders.iter().any(|existing| existing.name == name) {
                    placeholders.push(SkillPlaceholder {
                        name,
                        kind,
                        sample: None,
                    });
                }
                body.push_str(&format!(" [{}]", kind.as_str()));
            }
        }
        body.push('\n');
        if !action.kind.is_empty()
            && !declared_capabilities
                .iter()
                .any(|capability| capability == &action.kind)
        {
            declared_capabilities.push(action.kind.clone());
        }
    }
    let report = redact_sensitive_content(&body);
    let status = if report.has_forbidden_content() {
        SkillRedactionStatus::Failed
    } else {
        SkillRedactionStatus::Passed
    };
    let mut finding_codes: Vec<String> = Vec::new();
    for finding in &report.findings {
        let code = finding.kind.as_str().to_owned();
        if !finding_codes.contains(&code) {
            finding_codes.push(code);
        }
    }
    SkillProposalDraft {
        title: title.to_owned(),
        summary: format!(
            "{} step demonstration captured under a {}/{} policy",
            trace.actions.len(),
            policy.allowed_environment_class,
            policy.sensitive_region_policy.as_str()
        ),
        body: report.redacted_text,
        placeholders,
        declared_capabilities,
        redaction: SkillRedactionSummary {
            status,
            redacted_spans: report.findings.len() as u32,
            finding_codes,
            redacted_digest: String::new(),
        },
    }
}

/// Classify one observed value into the typed placeholder that replaces it.
///
/// The classification only decides the placeholder *type*. The original value is
/// never kept, so a wrong classification cannot leak it.
fn classify_placeholder_value(value: &str) -> SkillPlaceholderKind {
    let lowered = value.trim().to_ascii_lowercase();
    if lowered.starts_with("https://") || lowered.starts_with("http://") {
        return SkillPlaceholderKind::Url;
    }
    if value.contains('/') || value.contains('\\') {
        return SkillPlaceholderKind::Path;
    }
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return SkillPlaceholderKind::Identifier;
    }
    SkillPlaceholderKind::Literal
}

// ---------------------------------------------------------------------------
// MCP App sandbox
// ---------------------------------------------------------------------------

/// The CSP domains an MCP App declared.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpAppCsp {
    /// Exact origins the App may connect to.
    pub connect_domains: Vec<String>,
    /// Exact origins the App may load resources from.
    pub resource_domains: Vec<String>,
}

/// The policy LiteCowork applies to one embedded MCP App frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpAppFramePolicy {
    pub frame_id: String,
    pub workspace_id: String,
    /// Host-authenticated MCP server identity.
    pub server_identity: String,
    /// The exact `ui://` resource URI.
    pub resource_uri: String,
    /// Digest computed over the fetched App bytes.
    pub content_digest: String,
    pub package_version: Option<String>,
    /// The exact origin the App frame is served from.
    pub origin: String,
    pub csp: McpAppCsp,
    /// Optional Task scope for the frame.
    pub task_id: Option<String>,
    /// The frame is torn down at this instant.
    pub expires_at: String,
}

/// The host-owned descriptor an embedded App frame is rendered from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpAppFrameDescriptor {
    pub frame_id: String,
    pub origin: String,
    /// The iframe `sandbox` attribute. Never `allow-same-origin`.
    pub sandbox: Vec<String>,
    /// The computed Content-Security-Policy value.
    pub csp: String,
    pub connect_domains: Vec<String>,
    pub resource_domains: Vec<String>,
    /// The only bridge methods the App may call.
    pub bridge_methods: Vec<String>,
    pub server_identity: String,
    pub resource_uri: String,
    pub content_digest: String,
    pub package_version: Option<String>,
    pub task_id: Option<String>,
    pub expires_at: String,
}

/// The durable record of one host-issued MCP App frame.
///
/// The record keeps the host-issued nonce next to the frame descriptor. The
/// nonce is bound to exactly one frame instance, so a message replayed from a
/// torn-down frame is refused even when its origin still matches. The nonce
/// never appears in a catalog row, an event payload, or an invocation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpAppFrameRecord {
    pub frame_id: String,
    pub workspace_id: String,
    pub descriptor: McpAppFrameDescriptor,
    /// The host-issued per-frame nonce.
    pub nonce: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

impl McpAppFrameRecord {
    /// Validate the record's own shape.
    pub fn validate(&self) -> Result<(), SkillGuardError> {
        if !is_valid_identifier(&self.frame_id) || !is_valid_identifier(&self.workspace_id) {
            return Err(SkillGuardError::EmptyIdentifier);
        }
        if self.nonce.is_empty() || self.nonce.len() > 256 {
            return Err(SkillGuardError::EmptyIdentifier);
        }
        if self.descriptor.frame_id != self.frame_id {
            return Err(SkillGuardError::AppFrameUnknown);
        }
        Ok(())
    }
}

/// Bridge methods an App may call. Everything else is refused.
pub const APP_BRIDGE_METHODS: [&str; 4] = [
    "tool.invoke",
    "resource.read",
    "navigation.request",
    "app.ready",
];

/// Credential-shaped keys an App may never send through the bridge.
const APP_CREDENTIAL_KEYS: [&str; 12] = [
    "authorization",
    "access_token",
    "accesstoken",
    "api_key",
    "apikey",
    "client_secret",
    "cookie",
    "credential",
    "credentials",
    "password",
    "refresh_token",
    "secret",
];

/// Build the host-owned descriptor for one embedded MCP App frame.
///
/// The sandbox permits scripts for the AppBridge and nothing else. The CSP
/// applies `default-src 'none'` and only the exact declared domains; when a
/// declaration is absent or invalid, every external connection and resource is
/// blocked and the host never invents a fallback domain.
pub fn build_app_frame(policy: &McpAppFramePolicy) -> Result<McpAppFrameDescriptor, SkillGuardError> {
    if !is_valid_identifier(&policy.frame_id) || !is_valid_identifier(&policy.workspace_id) {
        return Err(SkillGuardError::EmptyIdentifier);
    }
    if !is_valid_identifier(&policy.server_identity) {
        return Err(SkillGuardError::EmptyIdentifier);
    }
    if !is_valid_uri(&policy.resource_uri) {
        return Err(SkillGuardError::InvalidUri);
    }
    if !is_sha256_digest(&policy.content_digest) {
        return Err(SkillGuardError::InvalidDigest);
    }
    if !is_valid_uri(&policy.origin) {
        return Err(SkillGuardError::InvalidUri);
    }
    let connect_domains = validate_csp_domains(&policy.csp.connect_domains)?;
    let resource_domains = validate_csp_domains(&policy.csp.resource_domains)?;
    let mut csp = String::from("default-src 'none'");
    if !connect_domains.is_empty() {
        csp.push_str("; connect-src ");
        csp.push_str(&connect_domains.join(" "));
    }
    if !resource_domains.is_empty() {
        csp.push_str("; resource-src ");
        csp.push_str(&resource_domains.join(" "));
    }
    Ok(McpAppFrameDescriptor {
        frame_id: policy.frame_id.clone(),
        origin: policy.origin.clone(),
        sandbox: vec!["allow-scripts".to_owned()],
        csp,
        connect_domains,
        resource_domains,
        bridge_methods: APP_BRIDGE_METHODS.iter().map(|value| (*value).to_owned()).collect(),
        server_identity: policy.server_identity.clone(),
        resource_uri: policy.resource_uri.clone(),
        content_digest: policy.content_digest.clone(),
        package_version: policy.package_version.clone(),
        task_id: policy.task_id.clone(),
        expires_at: policy.expires_at.clone(),
    })
}

fn validate_csp_domains(domains: &[String]) -> Result<Vec<String>, SkillGuardError> {
    let mut result: Vec<String> = Vec::with_capacity(domains.len());
    for domain in domains {
        if domain.is_empty() || domain.len() > APP_CSP_DOMAIN_MAX_BYTES {
            return Err(SkillGuardError::AppCspDomainInvalid);
        }
        if !is_valid_uri(domain) {
            return Err(SkillGuardError::AppCspDomainInvalid);
        }
        if result.iter().any(|existing| existing == domain) {
            return Err(SkillGuardError::AppCspDomainInvalid);
        }
        result.push(domain.clone());
    }
    Ok(result)
}

/// One message an embedded App sent to the host bridge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpAppBridgeEnvelope {
    pub frame_id: String,
    /// The origin the message claims to come from.
    pub origin: String,
    /// The host-issued per-frame nonce.
    pub nonce: String,
    pub method: String,
    pub payload: Value,
}

/// An accepted bridge request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpAppBridgeRequest {
    pub frame_id: String,
    pub method: String,
    pub payload: Value,
}

/// Validate one App bridge message against the host-owned frame descriptor.
///
/// The App may never steer the host: the origin must match exactly, the nonce
/// must be the host-issued one, the method must be on the allowlist, and the
/// payload may never carry a credential.
pub fn validate_app_bridge_message(
    descriptor: &McpAppFrameDescriptor,
    envelope: &McpAppBridgeEnvelope,
    expected_nonce: &str,
    now: &str,
) -> Result<McpAppBridgeRequest, SkillGuardError> {
    if envelope.frame_id != descriptor.frame_id {
        return Err(SkillGuardError::AppFrameUnknown);
    }
    if descriptor.expires_at.as_str() <= now {
        return Err(SkillGuardError::AppFrameExpired);
    }
    if envelope.origin != descriptor.origin {
        return Err(SkillGuardError::AppOriginMismatch);
    }
    if expected_nonce.is_empty() || envelope.nonce != expected_nonce {
        return Err(SkillGuardError::AppNonceMismatch);
    }
    if !descriptor.bridge_methods.iter().any(|method| method == &envelope.method) {
        return Err(SkillGuardError::AppMethodNotAllowed);
    }
    let payload_bytes = serde_json::to_vec(&envelope.payload)
        .map_err(|_| SkillGuardError::AppPayloadTooLarge)?;
    if payload_bytes.len() > APP_BRIDGE_MAX_PAYLOAD_BYTES {
        return Err(SkillGuardError::AppPayloadTooLarge);
    }
    if payload_carries_credential(&envelope.payload) {
        return Err(SkillGuardError::AppCredentialInPayload);
    }
    Ok(McpAppBridgeRequest {
        frame_id: descriptor.frame_id.clone(),
        method: envelope.method.clone(),
        payload: envelope.payload.clone(),
    })
}

fn payload_carries_credential(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, child)| {
            let normalized = key.to_ascii_lowercase();
            APP_CREDENTIAL_KEYS
                .iter()
                .any(|candidate| normalized == *candidate)
                || payload_carries_credential(child)
        }),
        Value::Array(items) => items.iter().any(payload_carries_credential),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pinned_manifest() -> SkillManifest {
        SkillManifest {
            origin_id: "mcp-server-1".to_owned(),
            skill_uri: "https://mcp.example/skills/invoice/SKILL.md".to_owned(),
            kind: SkillManifestKind::Pinned,
            resources: vec![SkillManifestResource {
                uri: "https://mcp.example/skills/invoice/SKILL.md".to_owned(),
                digest: format!("sha256:{}", "a".repeat(64)),
                size_bytes: 512,
            }],
        }
    }

    fn revision() -> SkillRevision {
        SkillRevision {
            revision: 1,
            title: "Invoice cleanup".to_owned(),
            summary: "Clean up an invoice".to_owned(),
            manifest_digest: format!("sha256:{}", "b".repeat(64)),
            content_digest: format!("sha256:{}", "c".repeat(64)),
            content_ref: "resource:draft-1".to_owned(),
            declared_capabilities: vec!["connector.invoice.read".to_owned()],
            declared_requirements: Vec::new(),
            origin: SkillOrigin::LocalProposal {
                proposal_id: "proposal-1".to_owned(),
            },
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            created_by: "owner-1".to_owned(),
        }
    }

    fn skill() -> SkillRecord {
        SkillRecord {
            skill_id: "skill-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            kind: SkillKind::LocalProcedure,
            status: SkillStatus::Active,
            capability_ref: SkillCapabilityRef {
                capability_id: "local:skill-1".to_owned(),
                identity_kind: "MCP_SKILL".to_owned(),
                source: "litecowork:local".to_owned(),
                package_version: None,
                digest: format!("sha256:{}", "b".repeat(64)),
                component: "local:skill-1".to_owned(),
            },
            current_revision: 1,
            revisions: vec![revision()],
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            updated_at: "2026-10-20T00:00:00Z".to_owned(),
            version: 1,
        }
    }

    fn grant() -> SkillActivationGrant {
        SkillActivationGrant {
            grant_id: "grant-1".to_owned(),
            capability_ref: "sha256:keydigest".to_owned(),
            manifest_digest: format!("sha256:{}", "b".repeat(64)),
            scope: "TASK_PLANNING".to_owned(),
            status: "ACTIVE".to_owned(),
            allowed_operations: vec!["connector.invoice.read".to_owned()],
            expires_at: Some("2026-10-21T00:00:00Z".to_owned()),
        }
    }

    #[test]
    fn a_pinned_manifest_with_one_verified_file_is_accepted() {
        assert_eq!(validate_skill_manifest(&pinned_manifest()), Ok(()));
    }

    #[test]
    fn a_new_skill_definition_is_always_held_as_a_draft() {
        assert_eq!(
            evaluate_skill_definition_write(None, SkillStatus::Draft),
            Ok(SkillStatus::Draft)
        );
        for status in [
            SkillStatus::Active,
            SkillStatus::Suspended,
            SkillStatus::Revoked,
        ] {
            assert_eq!(
                evaluate_skill_definition_write(None, status),
                Err(SkillGuardError::SkillStatusNotAllowed),
                "a create must never persist {status:?}"
            );
        }
    }

    #[test]
    fn a_draft_definition_cannot_be_promoted_to_active_by_a_write() {
        let mut draft = skill();
        draft.status = SkillStatus::Draft;
        assert_eq!(
            evaluate_skill_definition_write(Some(&draft), SkillStatus::Active),
            Err(SkillGuardError::SkillStatusNotAllowed)
        );
    }

    #[test]
    fn a_write_may_narrow_an_active_definition_but_never_widen_it() {
        let active = skill();
        assert_eq!(
            evaluate_skill_definition_write(Some(&active), SkillStatus::Suspended),
            Ok(SkillStatus::Suspended)
        );
        assert_eq!(
            evaluate_skill_definition_write(Some(&active), SkillStatus::Revoked),
            Ok(SkillStatus::Revoked)
        );
        assert_eq!(
            evaluate_skill_definition_write(Some(&active), SkillStatus::Active),
            Ok(SkillStatus::Active)
        );
    }

    #[test]
    fn a_revoked_definition_can_never_be_revived_by_a_write() {
        let mut revoked = skill();
        revoked.status = SkillStatus::Revoked;
        assert_eq!(
            evaluate_skill_definition_write(Some(&revoked), SkillStatus::Active),
            Err(SkillGuardError::SkillStatusNotAllowed)
        );
    }

    #[test]
    fn a_hostile_manifest_that_lists_a_file_twice_is_rejected() {
        let mut manifest = pinned_manifest();
        manifest.resources.push(manifest.resources[0].clone());
        assert_eq!(
            validate_skill_manifest(&manifest),
            Err(SkillGuardError::DuplicateManifestResource)
        );
    }

    #[test]
    fn a_manifest_with_a_malformed_digest_is_rejected() {
        let mut manifest = pinned_manifest();
        manifest.resources[0].digest = "sha256:not-a-digest".to_owned();
        assert_eq!(
            validate_skill_manifest(&manifest),
            Err(SkillGuardError::InvalidDigest)
        );
    }

    #[test]
    fn a_manifest_that_exceeds_the_total_size_bound_is_rejected() {
        let mut manifest = pinned_manifest();
        manifest.resources[0].size_bytes = SKILL_MANIFEST_MAX_TOTAL_BYTES;
        manifest.resources.push(SkillManifestResource {
            uri: "https://mcp.example/skills/invoice/other.md".to_owned(),
            digest: format!("sha256:{}", "d".repeat(64)),
            size_bytes: 1,
        });
        assert_eq!(
            validate_skill_manifest(&manifest),
            Err(SkillGuardError::ManifestTooLarge)
        );
    }

    #[test]
    fn a_manifest_with_more_than_the_file_bound_is_rejected() {
        let mut manifest = pinned_manifest();
        manifest.resources = (0..=SKILL_MANIFEST_MAX_FILES)
            .map(|index| SkillManifestResource {
                uri: format!("https://mcp.example/skills/invoice/{index}.md"),
                digest: format!("sha256:{}", "a".repeat(64)),
                size_bytes: 1,
            })
            .collect();
        assert_eq!(
            validate_skill_manifest(&manifest),
            Err(SkillGuardError::TooManyManifestResources)
        );
    }

    #[test]
    fn a_dynamic_manifest_is_displayable_but_never_activatable() {
        let manifest = SkillManifest {
            origin_id: "mcp-server-1".to_owned(),
            skill_uri: "https://mcp.example/skills/dynamic/SKILL.md".to_owned(),
            kind: SkillManifestKind::Dynamic,
            resources: Vec::new(),
        };
        assert_eq!(validate_skill_manifest(&manifest), Ok(()));
        assert!(!manifest.is_activatable());
    }

    #[test]
    fn a_skill_without_a_grant_cannot_be_activated() {
        assert_eq!(
            evaluate_skill_activation(&skill(), None, "sha256:keydigest", "2026-10-20T00:00:00Z"),
            Err(SkillGuardError::NoCapabilityGrant)
        );
    }

    #[test]
    fn a_grant_bound_to_another_capability_reference_cannot_activate_a_skill() {
        assert_eq!(
            evaluate_skill_activation(
                &skill(),
                Some(&grant()),
                "sha256:other",
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::GrantScopeMismatch)
        );
    }

    #[test]
    fn a_grant_bound_to_another_manifest_digest_cannot_activate_a_skill() {
        let mut grant = grant();
        grant.manifest_digest = format!("sha256:{}", "e".repeat(64));
        assert_eq!(
            evaluate_skill_activation(
                &skill(),
                Some(&grant),
                "sha256:keydigest",
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::GrantDigestMismatch)
        );
    }

    #[test]
    fn an_expired_grant_cannot_activate_a_skill() {
        assert_eq!(
            evaluate_skill_activation(
                &skill(),
                Some(&grant()),
                "sha256:keydigest",
                "2026-10-22T00:00:00Z"
            ),
            Err(SkillGuardError::GrantExpired)
        );
    }

    #[test]
    fn skill_text_cannot_grant_a_capability_the_grant_does_not_contain() {
        let mut grant = grant();
        grant.allowed_operations.clear();
        assert_eq!(
            evaluate_skill_activation(
                &skill(),
                Some(&grant),
                "sha256:keydigest",
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::CapabilityNotGranted)
        );
    }

    #[test]
    fn a_revoked_skill_cannot_be_activated() {
        let mut skill = skill();
        skill.status = SkillStatus::Revoked;
        assert_eq!(
            evaluate_skill_activation(
                &skill,
                Some(&grant()),
                "sha256:keydigest",
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::SkillRevoked)
        );
    }

    #[test]
    fn a_bound_grant_activates_the_pinned_revision() {
        let activation = evaluate_skill_activation(
            &skill(),
            Some(&grant()),
            "sha256:keydigest",
            "2026-10-20T00:00:00Z",
        )
        .expect("a bound grant activates the pinned revision");
        assert_eq!(activation.revision, 1);
        assert_eq!(activation.grant_id, "grant-1");
    }

    fn proposal() -> SkillProposalRecord {
        SkillProposalRecord {
            proposal_id: "proposal-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            source: SkillProposalSource::Demonstration,
            source_task_id: "task-1".to_owned(),
            source_artifact_id: "artifact-1".to_owned(),
            draft_resource_ref: "resource:draft-1".to_owned(),
            draft_digest: format!("sha256:{}", "f".repeat(64)),
            redaction: SkillRedactionSummary {
                status: SkillRedactionStatus::Passed,
                redacted_spans: 1,
                finding_codes: vec!["REDACTION_PLACEHOLDERS_1".to_owned()],
                redacted_digest: format!("sha256:{}", "f".repeat(64)),
            },
            placeholders: vec![SkillPlaceholder {
                name: "INVOICE_PATH".to_owned(),
                kind: SkillPlaceholderKind::Path,
                sample: None,
            }],
            declared_capabilities: vec!["connector.invoice.read".to_owned()],
            status: SkillProposalStatus::Review,
            proposed_by: "service:demonstration-capture".to_owned(),
            approved_by: None,
            published_capability_ref: None,
            drift: None,
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            updated_at: "2026-10-20T00:00:00Z".to_owned(),
            version: 1,
        }
    }

    fn reviewer() -> SkillProposalReviewer {
        SkillProposalReviewer {
            principal_id: "owner-1".to_owned(),
            principal_kind: "USER".to_owned(),
            assurance_level: "APPROVE_SAFE".to_owned(),
        }
    }

    #[test]
    fn an_owner_approval_never_activates_or_publishes_a_skill() {
        let outcome = evaluate_skill_proposal_review(
            &proposal(),
            "owner-1",
            &reviewer(),
            SkillProposalDecision::Approve,
            "2026-10-20T00:00:00Z",
        )
        .expect("an authenticated owner may approve a reviewed proposal");
        assert_eq!(outcome.status, SkillProposalStatus::Approved);
        assert_eq!(outcome.activated_skill_id, None);
        assert_eq!(
            outcome.publication_blocker.as_deref(),
            Some(PACKAGE_SERVICE_NOT_QUALIFIED)
        );
    }

    #[test]
    fn a_service_may_never_approve_a_learned_skill() {
        let mut reviewer = reviewer();
        reviewer.principal_kind = "SERVICE".to_owned();
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal(),
                "owner-1",
                &reviewer,
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::ReviewerNotHuman)
        );
    }

    #[test]
    fn a_reviewer_who_is_not_the_workspace_owner_may_not_approve() {
        let mut reviewer = reviewer();
        reviewer.principal_id = "someone-else".to_owned();
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal(),
                "owner-1",
                &reviewer,
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::ReviewerNotOwner)
        );
    }

    #[test]
    fn a_draft_proposal_cannot_be_approved_before_review() {
        let mut proposal = proposal();
        proposal.status = SkillProposalStatus::Draft;
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal,
                "owner-1",
                &reviewer(),
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::ProposalNotInReview)
        );
    }

    #[test]
    fn a_proposal_whose_redaction_failed_cannot_be_approved() {
        let mut proposal = proposal();
        proposal.redaction.status = SkillRedactionStatus::Failed;
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal,
                "owner-1",
                &reviewer(),
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::ProposalRedactionNotPassed)
        );
    }

    #[test]
    fn a_proposal_whose_redaction_never_ran_cannot_be_approved() {
        let mut proposal = proposal();
        proposal.redaction.status = SkillRedactionStatus::Pending;
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal,
                "owner-1",
                &reviewer(),
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::ProposalRedactionNotPassed)
        );
    }

    #[test]
    fn a_rejected_proposal_needs_no_publication() {
        let outcome = evaluate_skill_proposal_review(
            &proposal(),
            "owner-1",
            &reviewer(),
            SkillProposalDecision::Reject,
            "2026-10-20T00:00:00Z",
        )
        .expect("an authenticated owner may reject a reviewed proposal");
        assert_eq!(outcome.status, SkillProposalStatus::Rejected);
        assert_eq!(outcome.publication_blocker, None);
        assert_eq!(outcome.activated_skill_id, None);
    }

    #[test]
    fn an_approved_proposal_is_closed_for_further_review() {
        assert!(SkillProposalStatus::Approved.is_review_closed());
        assert!(SkillProposalStatus::Rejected.is_review_closed());
        assert!(!SkillProposalStatus::Review.is_review_closed());
        assert!(!SkillProposalStatus::Draft.is_review_closed());
    }

    #[test]
    fn a_drifted_proposal_cannot_be_approved_without_review() {
        let mut proposal = proposal();
        proposal.drift = detect_skill_drift(
            &format!("sha256:{}", "a".repeat(64)),
            &format!("sha256:{}", "b".repeat(64)),
            vec!["https://mcp.example/skills/invoice/SKILL.md".to_owned()],
            "2026-10-20T00:00:00Z",
        );
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal,
                "owner-1",
                &reviewer(),
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::DriftDetected)
        );
        assert_eq!(
            SkillGuardError::DriftDetected.reason_code(),
            SKILL_DRIFT_BLOCKER_CODE
        );
    }

    #[test]
    fn a_rejected_proposal_is_terminal() {
        let mut proposal = proposal();
        proposal.status = SkillProposalStatus::Rejected;
        assert_eq!(
            evaluate_skill_proposal_review(
                &proposal,
                "owner-1",
                &reviewer(),
                SkillProposalDecision::Approve,
                "2026-10-20T00:00:00Z"
            ),
            Err(SkillGuardError::ProposalAlreadyTerminal)
        );
    }

    fn policy() -> DemonstrationCapturePolicy {
        DemonstrationCapturePolicy {
            max_duration_ms: 60_000,
            max_actions: 10,
            max_trace_bytes: 4_096,
            allowed_environment_class: "BROWSER".to_owned(),
            sensitive_region_policy: SensitiveRegionPolicy::PauseOnDetection,
        }
    }

    fn action(sensitive: bool) -> DemonstrationAction {
        DemonstrationAction {
            sequence: 1,
            kind: "click".to_owned(),
            target: "invoices.download".to_owned(),
            value: None,
            sensitive,
            coordinate_hint: Some("x=12,y=34".to_owned()),
        }
    }

    #[test]
    fn a_capture_appends_observations_while_it_is_recording() {
        let observation = action(false);
        let decision = evaluate_capture_append(
            &policy(),
            DemonstrationStatus::Capturing,
            "env-1",
            "env-1",
            0,
            0,
            0,
            &observation,
        );
        assert_eq!(
            decision,
            CaptureDecision::Recorded {
                action_count: 1,
                trace_bytes: observation.trace_bytes(),
            }
        );
    }

    #[test]
    fn a_capture_accounts_the_real_observation_size() {
        let observation = action(false);
        let decision = evaluate_capture_append(
            &policy(),
            DemonstrationStatus::Capturing,
            "env-1",
            "env-1",
            0,
            0,
            0,
            &observation,
        );
        match decision {
            CaptureDecision::Recorded { trace_bytes, .. } => {
                assert!(trace_bytes > 1, "the trace budget counts real bytes");
            }
            other => panic!("expected a recorded observation, got {other:?}"),
        }
    }

    #[test]
    fn a_capture_pauses_on_a_detected_sensitive_region() {
        assert_eq!(
            evaluate_capture_append(
                &policy(),
                DemonstrationStatus::Capturing,
                "env-1",
                "env-1",
                0,
                0,
                0,
                &action(true)
            ),
            CaptureDecision::Refused {
                reason: CaptureRefusal::SensitiveRegionPaused
            }
        );
    }

    #[test]
    fn a_capture_that_omits_sensitive_fields_keeps_recording_without_the_value() {
        let mut policy = policy();
        policy.sensitive_region_policy = SensitiveRegionPolicy::OmitSensitiveFields;
        let observation = action(true);
        let decision = evaluate_capture_append(
            &policy,
            DemonstrationStatus::Capturing,
            "env-1",
            "env-1",
            0,
            0,
            0,
            &observation,
        );
        match decision {
            CaptureDecision::RecordedWithSensitiveFieldsOmitted {
                action_count,
                trace_bytes,
            } => {
                assert_eq!(action_count, 1);
                assert!(trace_bytes > 0);
            }
            other => panic!("expected an omitted-fields observation, got {other:?}"),
        }
        let omitted = observation.without_sensitive_fields();
        assert_eq!(omitted.value, None);
        assert_eq!(omitted.coordinate_hint, None);
        assert_eq!(omitted.kind, observation.kind);
        assert_eq!(omitted.target, observation.target);
        assert!(omitted.sensitive);
    }

    #[test]
    fn the_action_cap_stops_recording() {
        assert_eq!(
            evaluate_capture_append(
                &policy(),
                DemonstrationStatus::Capturing,
                "env-1",
                "env-1",
                10,
                0,
                0,
                &action(false)
            ),
            CaptureDecision::Refused {
                reason: CaptureRefusal::ActionCapReached
            }
        );
    }

    #[test]
    fn the_trace_byte_cap_stops_recording() {
        assert_eq!(
            evaluate_capture_append(
                &policy(),
                DemonstrationStatus::Capturing,
                "env-1",
                "env-1",
                0,
                4_096,
                0,
                &action(false)
            ),
            CaptureDecision::Refused {
                reason: CaptureRefusal::TraceCapReached
            }
        );
    }

    #[test]
    fn the_duration_cap_stops_recording() {
        assert_eq!(
            evaluate_capture_append(
                &policy(),
                DemonstrationStatus::Capturing,
                "env-1",
                "env-1",
                0,
                0,
                60_000,
                &action(false)
            ),
            CaptureDecision::Refused {
                reason: CaptureRefusal::DurationReached
            }
        );
    }

    #[test]
    fn an_observation_from_another_environment_is_refused() {
        assert_eq!(
            evaluate_capture_append(
                &policy(),
                DemonstrationStatus::Capturing,
                "env-2",
                "env-1",
                0,
                0,
                0,
                &action(false)
            ),
            CaptureDecision::Refused {
                reason: CaptureRefusal::EnvironmentMismatch
            }
        );
    }

    #[test]
    fn a_paused_capture_records_nothing() {
        assert_eq!(
            evaluate_capture_append(
                &policy(),
                DemonstrationStatus::Paused,
                "env-1",
                "env-1",
                0,
                0,
                0,
                &action(false)
            ),
            CaptureDecision::Refused {
                reason: CaptureRefusal::NotCapturing
            }
        );
    }

    #[test]
    fn a_capture_draft_uses_semantic_steps_not_raw_coordinates() {
        let trace = DemonstrationTrace {
            actions: vec![DemonstrationAction {
                sequence: 1,
                kind: "click".to_owned(),
                target: "invoices.download".to_owned(),
                value: Some("/home/owner/invoices/42.pdf".to_owned()),
                sensitive: false,
                coordinate_hint: Some("x=12,y=34".to_owned()),
            }],
            trace_bytes: 10,
            started_at: "2026-10-20T00:00:00Z".to_owned(),
            ended_at: "2026-10-20T00:00:10Z".to_owned(),
        };
        let draft = derive_skill_proposal_draft(&trace, &policy(), "Invoice export");
        assert!(draft.body.contains("invoices.download"));
        assert!(!draft.body.contains("x=12,y=34"));
        assert!(!draft.body.contains("/home/owner/invoices/42.pdf"));
        assert_eq!(draft.placeholders.len(), 1);
        assert_eq!(draft.placeholders[0].name, "PATH_1");
        assert_eq!(draft.placeholders[0].kind, SkillPlaceholderKind::Path);
        assert_eq!(draft.redaction.status, SkillRedactionStatus::Passed);
    }

    #[test]
    fn a_capture_draft_never_carries_the_observed_value() {
        let trace = DemonstrationTrace {
            actions: vec![DemonstrationAction {
                sequence: 1,
                kind: "type".to_owned(),
                target: "invoice.number".to_owned(),
                value: Some("sk-ant-api03-abcdefghijklmnop".to_owned()),
                sensitive: false,
                coordinate_hint: None,
            }],
            trace_bytes: 10,
            started_at: "2026-10-20T00:00:00Z".to_owned(),
            ended_at: "2026-10-20T00:00:10Z".to_owned(),
        };
        let draft = derive_skill_proposal_draft(&trace, &policy(), "Invoice export");
        assert!(!draft.body.contains("sk-ant-api03"));
        assert!(draft.body.contains("[IDENTIFIER]"));
        assert_eq!(draft.placeholders.len(), 1);
        assert_eq!(draft.placeholders[0].kind, SkillPlaceholderKind::Identifier);
        assert_eq!(draft.placeholders[0].sample, None);
    }

    #[test]
    fn a_capture_draft_whose_body_carries_a_secret_fails_redaction() {
        let trace = DemonstrationTrace {
            actions: vec![DemonstrationAction {
                sequence: 1,
                kind: "type".to_owned(),
                target: "sk-ant-api03-abcdefghijklmnop".to_owned(),
                value: None,
                sensitive: false,
                coordinate_hint: None,
            }],
            trace_bytes: 10,
            started_at: "2026-10-20T00:00:00Z".to_owned(),
            ended_at: "2026-10-20T00:00:10Z".to_owned(),
        };
        let draft = derive_skill_proposal_draft(&trace, &policy(), "Invoice export");
        assert_eq!(draft.redaction.status, SkillRedactionStatus::Failed);
        assert!(!draft.body.contains("sk-ant-api03"));
        assert!(
            draft
                .redaction
                .finding_codes
                .iter()
                .any(|code| code == "ACCESS_TOKEN")
        );
    }

    #[test]
    fn a_capture_policy_outside_the_accepted_bounds_is_rejected() {
        let mut policy = policy();
        policy.max_actions = CAPTURE_MAX_POLICY_ACTIONS + 1;
        assert_eq!(
            policy.validate(),
            Err(SkillGuardError::CapturePolicyInvalid)
        );
    }

    fn frame_policy() -> McpAppFramePolicy {
        McpAppFramePolicy {
            frame_id: "frame-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            server_identity: "mcp-server-1".to_owned(),
            resource_uri: "ui://invoice".to_owned(),
            content_digest: format!("sha256:{}", "a".repeat(64)),
            package_version: Some("2026.10.1".to_owned()),
            origin: "https://apps.litecowork.invalid".to_owned(),
            csp: McpAppCsp {
                connect_domains: vec!["https://mcp.example".to_owned()],
                resource_domains: vec!["https://cdn.example".to_owned()],
            },
            task_id: Some("task-1".to_owned()),
            expires_at: "2026-10-21T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn an_app_frame_sandbox_omits_same_origin_forms_and_popups() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        assert_eq!(descriptor.sandbox, vec!["allow-scripts".to_owned()]);
        for forbidden in [
            "allow-same-origin",
            "allow-forms",
            "allow-popups",
            "allow-downloads",
            "allow-top-navigation",
        ] {
            assert!(
                !descriptor.sandbox.iter().any(|value| value == forbidden),
                "{forbidden} must not be granted"
            );
        }
    }

    #[test]
    fn an_app_frame_csp_keeps_only_the_exact_declared_domains() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        assert!(descriptor.csp.starts_with("default-src 'none'"));
        assert!(descriptor.csp.contains("connect-src https://mcp.example"));
        assert!(descriptor.csp.contains("resource-src https://cdn.example"));
        assert!(!descriptor.csp.contains("https://other.example"));
    }

    #[test]
    fn an_app_frame_without_declared_domains_blocks_every_external_source() {
        let mut policy = frame_policy();
        policy.csp = McpAppCsp::default();
        let descriptor = build_app_frame(&policy).expect("a valid frame policy builds");
        assert_eq!(descriptor.csp, "default-src 'none'");
        assert!(descriptor.connect_domains.is_empty());
        assert!(descriptor.resource_domains.is_empty());
    }

    #[test]
    fn an_invalid_declared_domain_is_rejected_and_never_replaced() {
        let mut policy = frame_policy();
        policy.csp.connect_domains = vec!["not a domain".to_owned()];
        assert_eq!(
            build_app_frame(&policy),
            Err(SkillGuardError::AppCspDomainInvalid)
        );
    }

    fn envelope(method: &str, payload: Value) -> McpAppBridgeEnvelope {
        McpAppBridgeEnvelope {
            frame_id: "frame-1".to_owned(),
            origin: "https://apps.litecowork.invalid".to_owned(),
            nonce: "nonce-1".to_owned(),
            method: method.to_owned(),
            payload,
        }
    }

    #[test]
    fn a_forged_app_message_from_another_origin_is_rejected() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        let mut message = envelope("tool.invoke", json!({}));
        message.origin = "https://evil.example".to_owned();
        assert_eq!(
            validate_app_bridge_message(&descriptor, &message, "nonce-1", "2026-10-20T00:00:00Z"),
            Err(SkillGuardError::AppOriginMismatch)
        );
    }

    #[test]
    fn a_forged_app_message_without_the_host_nonce_is_rejected() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        let mut message = envelope("tool.invoke", json!({}));
        message.nonce = "guessed".to_owned();
        assert_eq!(
            validate_app_bridge_message(&descriptor, &message, "nonce-1", "2026-10-20T00:00:00Z"),
            Err(SkillGuardError::AppNonceMismatch)
        );
    }

    #[test]
    fn an_app_message_for_an_unknown_method_is_rejected() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        let message = envelope("host.readSecret", json!({}));
        assert_eq!(
            validate_app_bridge_message(&descriptor, &message, "nonce-1", "2026-10-20T00:00:00Z"),
            Err(SkillGuardError::AppMethodNotAllowed)
        );
    }

    #[test]
    fn an_app_message_that_carries_a_credential_is_rejected() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        let message = envelope("tool.invoke", json!({"authorization": "Bearer abc"}));
        assert_eq!(
            validate_app_bridge_message(&descriptor, &message, "nonce-1", "2026-10-20T00:00:00Z"),
            Err(SkillGuardError::AppCredentialInPayload)
        );
    }

    #[test]
    fn an_expired_app_frame_refuses_every_message() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        let message = envelope("app.ready", json!({}));
        assert_eq!(
            validate_app_bridge_message(&descriptor, &message, "nonce-1", "2026-10-22T00:00:00Z"),
            Err(SkillGuardError::AppFrameExpired)
        );
    }

    #[test]
    fn a_well_formed_app_message_is_accepted() {
        let descriptor = build_app_frame(&frame_policy()).expect("a valid frame policy builds");
        let message = envelope("tool.invoke", json!({"action": "preview"}));
        let request = validate_app_bridge_message(
            &descriptor,
            &message,
            "nonce-1",
            "2026-10-20T00:00:00Z",
        )
        .expect("a bound, non-credential message is accepted");
        assert_eq!(request.method, "tool.invoke");
    }
}
