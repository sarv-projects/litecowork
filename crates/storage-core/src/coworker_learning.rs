//! Deterministic Coworker learning gates.
//!
//! Authority: [`docs/LEARNING.md`](../../../docs/LEARNING.md). This module owns the
//! *deterministic* parts of the learning pipeline only:
//!
//! - source eligibility (LEARNING.md §16, §21),
//! - sensitive-content detection and redaction (§5, §21),
//! - the automatic-commit decision (§6),
//! - contradiction/supersession classification (§15),
//! - the closed candidate state machine (§20),
//! - event payload shape that never carries candidate text (§19, §22.13).
//!
//! It does **not** classify free text, does not run a model, and does not persist
//! anything. A qualified agent-backed classifier and the Resource-backed storage
//! layer own those concerns. Nothing here implies that automatic learning is
//! available in the product: a Coworker still needs a persisted policy revision and
//! an implemented storage path before any candidate can be committed.
//!
//! `docs/LEARNING.md` §23 suggests a dedicated `crates/domain-learning/` crate. The
//! current workspace has no such crate, so the deterministic rules live here beside
//! the other shared typed records until the module is relocated.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Bounded proposed-payload size. A candidate body is Resource-backed; the payload
/// this module inspects is a bounded preview of it.
pub const LEARNING_PAYLOAD_PREVIEW_MAX_BYTES: usize = 16 * 1024;

/// Bounded number of retained reason codes on one evaluation.
pub const LEARNING_REASON_CODE_MAX: usize = 32;

/// What the candidate proposes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningCandidateKind {
    MemoryFact,
    MemoryPreference,
    InstructionPatch,
    SkillProposal,
    RoutineProposal,
    AutomationProposal,
}

impl LearningCandidateKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MemoryFact => "MEMORY_FACT",
            Self::MemoryPreference => "MEMORY_PREFERENCE",
            Self::InstructionPatch => "INSTRUCTION_PATCH",
            Self::SkillProposal => "SKILL_PROPOSAL",
            Self::RoutineProposal => "ROUTINE_PROPOSAL",
            Self::AutomationProposal => "AUTOMATION_PROPOSAL",
        }
    }

    /// Only a narrowly qualified private memory/preference may commit automatically
    /// (LEARNING.md §2). Every other kind keeps its own reviewed/versioned path.
    pub const fn is_memory(self) -> bool {
        matches!(self, Self::MemoryFact | Self::MemoryPreference)
    }
}

/// Where the learning signal came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningSourceKind {
    UserExplicit,
    UserCorrection,
    UserFeedback,
    TaskOutcome,
    RepeatedPattern,
    Demonstration,
}

impl LearningSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserExplicit => "USER_EXPLICIT",
            Self::UserCorrection => "USER_CORRECTION",
            Self::UserFeedback => "USER_FEEDBACK",
            Self::TaskOutcome => "TASK_OUTCOME",
            Self::RepeatedPattern => "REPEATED_PATTERN",
            Self::Demonstration => "DEMONSTRATION",
        }
    }
}

/// The closed candidate lifecycle (LEARNING.md §20).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningCandidateState {
    New,
    Classified,
    NeedsReview,
    AutoCommitEligible,
    Committed,
    Dismissed,
    Rejected,
    Superseded,
}

impl LearningCandidateState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "NEW",
            Self::Classified => "CLASSIFIED",
            Self::NeedsReview => "NEEDS_REVIEW",
            Self::AutoCommitEligible => "AUTO_COMMIT_ELIGIBLE",
            Self::Committed => "COMMITTED",
            Self::Dismissed => "DISMISSED",
            Self::Rejected => "REJECTED",
            Self::Superseded => "SUPERSEDED",
        }
    }

    /// Terminal states never move again for that candidate (LEARNING.md §20).
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Committed | Self::Dismissed | Self::Rejected | Self::Superseded
        )
    }
}

/// The owner-visible memory mode (LEARNING.md §4).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningMemoryMode {
    Off,
    ReviewAll,
    AutoConfirmedPrivate,
    AutoConfirmedAndLowRiskInferredPrivate,
}

impl LearningMemoryMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::ReviewAll => "REVIEW_ALL",
            Self::AutoConfirmedPrivate => "AUTO_CONFIRMED_PRIVATE",
            Self::AutoConfirmedAndLowRiskInferredPrivate => {
                "AUTO_CONFIRMED_AND_LOW_RISK_INFERRED_PRIVATE"
            }
        }
    }
}

/// Instruction, Skill, Routine and Automation proposals always require review.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningReviewMode {
    ReviewRequired,
}

/// Quiet notification preference for a committed memory (LEARNING.md §4, §17).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningNotificationMode {
    Never,
    ImportantOnly,
    Always,
}

/// Conversation class. `TEMPORARY` conversations are never eligible (LEARNING.md §4).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningConversationClass {
    Normal,
    Temporary,
}

/// The source channel a candidate was observed from.
///
/// LEARNING.md §16 fixes the default eligible sources (direct authenticated owner
/// messages, explicit owner feedback, user-controlled demonstration metadata) and the
/// default ineligible sources (web pages, third-party email/chat, imported
/// documents, MCP/tool descriptions, generated model output, worker output,
/// connector event payloads). Only the first four channels may ever produce an
/// automatically committed memory.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningSourceChannel {
    OwnerMessage,
    OwnerFeedback,
    OwnerDemonstration,
    VerifiedTaskOutcome,
    ThirdPartyMessage,
    ImportedDocument,
    WebContent,
    ToolDescription,
    ModelOutput,
    WorkerOutput,
    ConnectorEvent,
}

impl LearningSourceChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerMessage => "OWNER_MESSAGE",
            Self::OwnerFeedback => "OWNER_FEEDBACK",
            Self::OwnerDemonstration => "OWNER_DEMONSTRATION",
            Self::VerifiedTaskOutcome => "VERIFIED_TASK_OUTCOME",
            Self::ThirdPartyMessage => "THIRD_PARTY_MESSAGE",
            Self::ImportedDocument => "IMPORTED_DOCUMENT",
            Self::WebContent => "WEB_CONTENT",
            Self::ToolDescription => "TOOL_DESCRIPTION",
            Self::ModelOutput => "MODEL_OUTPUT",
            Self::WorkerOutput => "WORKER_OUTPUT",
            Self::ConnectorEvent => "CONNECTOR_EVENT",
        }
    }

    /// A channel that may contribute to automatic private memory at all.
    pub const fn is_trusted(self) -> bool {
        matches!(
            self,
            Self::OwnerMessage
                | Self::OwnerFeedback
                | Self::OwnerDemonstration
                | Self::VerifiedTaskOutcome
        )
    }

    /// The source kind this channel may legitimately produce.
    pub const fn permitted_source_kind(self) -> LearningSourceKind {
        match self {
            Self::OwnerMessage => LearningSourceKind::UserExplicit,
            Self::OwnerFeedback => LearningSourceKind::UserFeedback,
            Self::OwnerDemonstration => LearningSourceKind::Demonstration,
            Self::VerifiedTaskOutcome => LearningSourceKind::TaskOutcome,
            // Untrusted channels never map to an eligible source kind.
            Self::ThirdPartyMessage
            | Self::ImportedDocument
            | Self::WebContent
            | Self::ToolDescription
            | Self::ModelOutput
            | Self::WorkerOutput
            | Self::ConnectorEvent => LearningSourceKind::UserExplicit,
        }
    }
}

/// Certainty of a proposed memory.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningCertainty {
    Confirmed,
    Inferred,
}

/// Sensitivity class of a proposed memory (LEARNING.md §5).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningSensitivity {
    Low,
    ReviewRequired,
    Forbidden,
}

/// Memory class of a proposed memory (LEARNING.md §5).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningMemoryClass {
    Fact,
    Preference,
    WorkingStyle,
    ProjectContext,
}

/// Automatic memory is Coworker-private only (LEARNING.md §5, §8).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LearningMemoryScope {
    CoworkerPrivate,
}

/// Versioned Coworker learning policy (LEARNING.md §4).
///
/// `inferred_confidence_threshold` carries the threshold LEARNING.md §6 requires
/// ("candidate confidence >= policy threshold when INFERRED") because §4 has no
/// other field that can hold it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoworkerLearningPolicy {
    pub coworker_id: String,
    pub revision: u64,
    pub memory_mode: LearningMemoryMode,
    pub eligible_source_kinds: Vec<LearningSourceKind>,
    pub eligible_conversation_classes: Vec<LearningConversationClass>,
    pub excluded_resource_labels: Vec<String>,
    pub candidate_retention_days: u32,
    pub inferred_confidence_threshold: f64,
    pub instruction_patch_mode: LearningReviewMode,
    pub skill_mode: LearningReviewMode,
    pub routine_mode: LearningReviewMode,
    pub automation_mode: LearningReviewMode,
    pub notify_on_memory_commit: LearningNotificationMode,
    pub created_at: String,
}

impl CoworkerLearningPolicy {
    /// The informed default for eligible, low-sensitivity, user-confirmed sources.
    ///
    /// `TASK_OUTCOME` is intentionally absent: LEARNING.md §16 lists only owner
    /// messages, owner feedback and owner demonstrations as default eligible
    /// sources, so a Task-outcome source needs an explicit owner revision.
    pub fn default_for_coworker(coworker_id: &str, created_at: &str) -> Self {
        Self {
            coworker_id: coworker_id.to_owned(),
            revision: 1,
            memory_mode: LearningMemoryMode::AutoConfirmedPrivate,
            eligible_source_kinds: vec![
                LearningSourceKind::UserExplicit,
                LearningSourceKind::UserCorrection,
                LearningSourceKind::UserFeedback,
                LearningSourceKind::Demonstration,
            ],
            eligible_conversation_classes: vec![LearningConversationClass::Normal],
            excluded_resource_labels: Vec::new(),
            candidate_retention_days: 30,
            inferred_confidence_threshold: 0.8,
            instruction_patch_mode: LearningReviewMode::ReviewRequired,
            skill_mode: LearningReviewMode::ReviewRequired,
            routine_mode: LearningReviewMode::ReviewRequired,
            automation_mode: LearningReviewMode::ReviewRequired,
            notify_on_memory_commit: LearningNotificationMode::ImportantOnly,
            created_at: created_at.to_owned(),
        }
    }

    /// True when the policy may auto-commit a candidate of the given certainty.
    pub fn allows_certainty(&self, certainty: LearningCertainty) -> bool {
        match self.memory_mode {
            LearningMemoryMode::Off | LearningMemoryMode::ReviewAll => false,
            LearningMemoryMode::AutoConfirmedPrivate => certainty == LearningCertainty::Confirmed,
            LearningMemoryMode::AutoConfirmedAndLowRiskInferredPrivate => true,
        }
    }
}

/// The proposed memory body (LEARNING.md §5).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCandidatePayload {
    pub subject_ref: String,
    pub predicate: String,
    pub value: Value,
    pub memory_class: LearningMemoryClass,
    pub certainty: LearningCertainty,
    pub sensitivity: LearningSensitivity,
    pub scope: LearningMemoryScope,
    pub expires_at: Option<String>,
    pub supersedes_memory_id: Option<String>,
}

/// A learning candidate (LEARNING.md §3).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LearningCandidate {
    pub candidate_id: String,
    pub workspace_id: String,
    pub coworker_id: String,
    pub kind: LearningCandidateKind,
    pub source_kind: LearningSourceKind,
    pub source_conversation_id: Option<String>,
    pub source_message_id: Option<String>,
    pub source_task_id: Option<String>,
    pub source_attempt_id: Option<String>,
    pub source_demonstration_id: Option<String>,
    pub source_digest: String,
    pub proposed_payload_ref: String,
    pub redaction_report_ref: String,
    pub classifier_version: String,
    pub confidence: Option<f64>,
    pub state: LearningCandidateState,
    pub reason_codes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

/// Metadata for an automatically learned Coworker-private memory (LEARNING.md §7).
///
/// The stored text lives in a Resource revision; this record never carries it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRecordMetadata {
    pub memory_id: String,
    pub workspace_id: String,
    pub coworker_id: String,
    pub context_document_id: String,
    pub source_candidate_id: String,
    pub memory_class: LearningMemoryClass,
    pub certainty: LearningCertainty,
    pub scope: LearningMemoryScope,
    pub source_digest: String,
    pub committed_at: String,
    pub expires_at: Option<String>,
    pub superseded_by: Option<String>,
    pub status: String,
    pub version: u64,
}

/// One current memory considered during a conflict check.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryComparison {
    pub memory: MemoryRecordMetadata,
    pub subject_ref: String,
    pub predicate: String,
    pub value: Value,
}

/// The kind of secret-shaped content found in a proposed payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SensitiveFindingKind {
    AccessToken,
    PrivateKey,
    AuthorizationHeader,
    SecretAssignment,
    PaymentCard,
    GovernmentIdentifier,
    HighEntropyBlob,
}

impl SensitiveFindingKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AccessToken => "ACCESS_TOKEN",
            Self::PrivateKey => "PRIVATE_KEY",
            Self::AuthorizationHeader => "AUTHORIZATION_HEADER",
            Self::SecretAssignment => "SECRET_ASSIGNMENT",
            Self::PaymentCard => "PAYMENT_CARD",
            Self::GovernmentIdentifier => "GOVERNMENT_IDENTIFIER",
            Self::HighEntropyBlob => "HIGH_ENTROPY_BLOB",
        }
    }
}

/// One secret-shaped span in a proposed payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SensitiveFinding {
    pub kind: SensitiveFindingKind,
    pub sensitivity: LearningSensitivity,
    pub start: usize,
    pub end: usize,
}

/// The result of scanning and redacting a proposed payload body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactionReport {
    pub redacted_text: String,
    pub findings: Vec<SensitiveFinding>,
}

impl RedactionReport {
    /// A report with no findings means the body carried no recognized secret shape.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// A report still carries forbidden material after redaction.
    pub fn has_forbidden_content(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.sensitivity == LearningSensitivity::Forbidden)
    }

    /// A report carries content that a human must review before it is stored.
    pub fn needs_review(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.sensitivity == LearningSensitivity::ReviewRequired)
    }
}

/// What the gates decided about one candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LearningDisposition {
    /// Eligible for the narrow automatic private-memory commit.
    AutoCommit,
    /// Persisted and visible; the owner must review it.
    NeedsReview,
    /// Only metadata/digest may be retained; the body must not be stored.
    Rejected,
    /// An identical memory already exists; nothing new is committed.
    Deduplicated,
}

/// How a candidate relates to the current authoritative memory (LEARNING.md §15).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LearningConflict {
    None,
    /// The same normalized claim already exists.
    Duplicate {
        memory_id: String,
    },
    /// A confirmed claim replaces a prior inferred claim.
    Supersedes {
        memory_id: String,
    },
    /// The new claim restates or narrows the current claim.
    Supports {
        memory_id: String,
    },
    /// The new claim contradicts the current claim and needs a human.
    Conflicting {
        memory_id: String,
    },
}

/// The full gate result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LearningEvaluation {
    pub disposition: LearningDisposition,
    pub state: LearningCandidateState,
    pub reason_codes: Vec<String>,
    pub conflict: LearningConflict,
}

/// Everything the deterministic gates need besides the candidate record.
#[derive(Clone, Debug)]
pub struct LearningEvaluationContext {
    pub policy: CoworkerLearningPolicy,
    /// The channel the candidate was observed from.
    pub source_channel: LearningSourceChannel,
    /// The conversation class of the source, when the source is a conversation.
    pub conversation_class: Option<LearningConversationClass>,
    /// The bounded proposed body and its classification.
    pub payload: MemoryCandidatePayload,
    /// The redaction report for the proposed body.
    pub redaction: RedactionReport,
    /// Workspace/Trust level exclusions already known to the caller.
    pub trust_excluded: bool,
    /// Resource labels attached to the source, checked against the policy.
    pub source_resource_labels: Vec<String>,
    /// Current Coworker-private memory considered for conflicts.
    pub current_memories: Vec<MemoryComparison>,
}

/// Normalized comparison text: trimmed, lowercased, whitespace-collapsed.
pub fn normalize_learning_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        for lowered in character.to_lowercase() {
            output.push(lowered);
        }
    }
    output
}

/// The normalized semantic key used for dedupe (LEARNING.md §21).
pub fn normalized_memory_key(subject_ref: &str, predicate: &str, value: &Value) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        normalize_learning_text(subject_ref),
        normalize_learning_text(predicate),
        normalized_learning_value(value)
    )
}

/// The normalized comparison form of a proposed value.
pub fn normalized_learning_value(value: &Value) -> String {
    match value {
        Value::String(text) => normalize_learning_text(text),
        Value::Null => "null".to_owned(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn push_reason(reasons: &mut Vec<String>, reason: &str) {
    if reasons.len() < LEARNING_REASON_CODE_MAX && !reasons.iter().any(|value| value == reason) {
        reasons.push(reason.to_owned());
    }
}

fn rejected(reasons: Vec<String>) -> LearningEvaluation {
    LearningEvaluation {
        disposition: LearningDisposition::Rejected,
        state: LearningCandidateState::Rejected,
        reason_codes: reasons,
        conflict: LearningConflict::None,
    }
}

/// The `USER_EXPLICIT`/`USER_CORRECTION` source kinds accept a direct owner message,
/// a Task outcome, or an owner demonstration. `USER_FEEDBACK` is feedback only, and
/// `TASK_OUTCOME`/`REPEATED_PATTERN` come from a Core-observed Task outcome.
fn source_kind_matches_channel(
    source_kind: LearningSourceKind,
    channel: LearningSourceChannel,
) -> bool {
    match source_kind {
        LearningSourceKind::UserExplicit => matches!(
            channel,
            LearningSourceChannel::OwnerMessage
                | LearningSourceChannel::OwnerFeedback
                | LearningSourceChannel::OwnerDemonstration
                | LearningSourceChannel::VerifiedTaskOutcome
        ),
        LearningSourceKind::UserCorrection => matches!(
            channel,
            LearningSourceChannel::OwnerMessage | LearningSourceChannel::OwnerFeedback
        ),
        LearningSourceKind::UserFeedback => {
            matches!(channel, LearningSourceChannel::OwnerFeedback)
        }
        LearningSourceKind::TaskOutcome | LearningSourceKind::RepeatedPattern => {
            matches!(channel, LearningSourceChannel::VerifiedTaskOutcome)
        }
        LearningSourceKind::Demonstration => {
            matches!(channel, LearningSourceChannel::OwnerDemonstration)
        }
    }
}

/// Classify a candidate against the current authoritative memory (LEARNING.md §15).
pub fn classify_learning_conflict(
    payload: &MemoryCandidatePayload,
    current: &[MemoryComparison],
) -> LearningConflict {
    let subject = normalize_learning_text(&payload.subject_ref);
    let predicate = normalize_learning_text(&payload.predicate);
    let new_value = normalized_learning_value(&payload.value);
    let mut outcome = LearningConflict::None;
    for comparison in current {
        if comparison.memory.status != "ACTIVE" {
            continue;
        }
        let same_subject = normalize_learning_text(&comparison.subject_ref) == subject;
        let same_predicate = normalize_learning_text(&comparison.predicate) == predicate;
        if !same_subject || !same_predicate {
            continue;
        }
        let existing_value = normalized_learning_value(&comparison.value);
        if existing_value == new_value {
            return LearningConflict::Duplicate {
                memory_id: comparison.memory.memory_id.clone(),
            };
        }
        let supports = supports_value(&payload.value, &comparison.value);
        let conflict = match (payload.certainty, comparison.memory.certainty) {
            (LearningCertainty::Confirmed, LearningCertainty::Inferred) => {
                LearningConflict::Supersedes {
                    memory_id: comparison.memory.memory_id.clone(),
                }
            }
            _ if supports => LearningConflict::Supports {
                memory_id: comparison.memory.memory_id.clone(),
            },
            _ => LearningConflict::Conflicting {
                memory_id: comparison.memory.memory_id.clone(),
            },
        };
        // A confirmed conflict outranks a softer observation.
        let conflict_is_conflicting = matches!(conflict, LearningConflict::Conflicting { .. });
        let outcome_is_conflicting = matches!(outcome, LearningConflict::Conflicting { .. });
        if (conflict_is_conflicting && !outcome_is_conflicting)
            || matches!(outcome, LearningConflict::None)
        {
            outcome = conflict;
        }
    }
    outcome
}

fn supports_value(new_value: &Value, existing_value: &Value) -> bool {
    match (new_value, existing_value) {
        (Value::String(new_text), Value::String(existing_text)) => {
            let new_normalized = normalize_learning_text(new_text);
            let existing_normalized = normalize_learning_text(existing_text);
            !new_normalized.is_empty()
                && !existing_normalized.is_empty()
                && (new_normalized.contains(&existing_normalized)
                    || existing_normalized.contains(&new_normalized))
        }
        _ => false,
    }
}

/// Run every deterministic gate for one candidate.
///
/// The returned disposition is the maximum authority this module may claim. A
/// caller must still re-read the candidate, policy, source and current memory inside
/// its own transaction before committing anything (LEARNING.md §11).
pub fn evaluate_learning_candidate(
    candidate: &LearningCandidate,
    context: &LearningEvaluationContext,
) -> LearningEvaluation {
    let mut reasons: Vec<String> = Vec::new();

    if candidate.coworker_id != context.policy.coworker_id {
        push_reason(&mut reasons, "POLICY_COWORKER_MISMATCH");
        return rejected(reasons);
    }
    if context.policy.memory_mode == LearningMemoryMode::Off {
        push_reason(&mut reasons, "MEMORY_MODE_OFF");
        return rejected(reasons);
    }

    // LEARNING.md §16: only owner-controlled or Core-verified channels may
    // contribute; third-party content can never alter the Coworker.
    if !context.source_channel.is_trusted() {
        push_reason(&mut reasons, "UNTRUSTED_SOURCE_CHANNEL");
        return rejected(reasons);
    }
    if !source_kind_matches_channel(candidate.source_kind, context.source_channel) {
        push_reason(&mut reasons, "SOURCE_KIND_CHANNEL_MISMATCH");
        return rejected(reasons);
    }
    if !context
        .policy
        .eligible_source_kinds
        .contains(&candidate.source_kind)
    {
        push_reason(&mut reasons, "SOURCE_KIND_NOT_ELIGIBLE");
        return rejected(reasons);
    }

    // LEARNING.md §4/§21: a temporary conversation never learns.
    if let Some(class) = context.conversation_class {
        if class == LearningConversationClass::Temporary {
            push_reason(&mut reasons, "TEMPORARY_CONVERSATION");
            return rejected(reasons);
        }
        if !context
            .policy
            .eligible_conversation_classes
            .contains(&class)
        {
            push_reason(&mut reasons, "CONVERSATION_CLASS_NOT_ELIGIBLE");
            return rejected(reasons);
        }
    }

    if context.trust_excluded {
        push_reason(&mut reasons, "TRUST_EXCLUDED_SOURCE");
        return rejected(reasons);
    }
    for label in &context.source_resource_labels {
        if context
            .policy
            .excluded_resource_labels
            .iter()
            .any(|excluded| excluded == label)
        {
            push_reason(&mut reasons, "EXCLUDED_RESOURCE_LABEL");
            return rejected(reasons);
        }
    }

    // LEARNING.md §5: only Coworker-private scope exists for automatic memory.
    if context.payload.scope != LearningMemoryScope::CoworkerPrivate {
        push_reason(&mut reasons, "NON_PRIVATE_SCOPE");
        return rejected(reasons);
    }

    // LEARNING.md §5/§21: forbidden material is never stored; uncertain material
    // never auto-commits.
    if context.payload.sensitivity == LearningSensitivity::Forbidden
        || context.redaction.has_forbidden_content()
    {
        push_reason(&mut reasons, "FORBIDDEN_SENSITIVE_CONTENT");
        return rejected(reasons);
    }
    if context.payload.sensitivity == LearningSensitivity::ReviewRequired
        || context.redaction.needs_review()
    {
        push_reason(&mut reasons, "SENSITIVE_CONTENT_REQUIRES_REVIEW");
        return LearningEvaluation {
            disposition: LearningDisposition::NeedsReview,
            state: LearningCandidateState::NeedsReview,
            reason_codes: reasons,
            conflict: LearningConflict::None,
        };
    }

    // LEARNING.md §2/§6: only private memory/preference may commit automatically.
    if !candidate.kind.is_memory() {
        push_reason(&mut reasons, "KIND_REQUIRES_REVIEW");
        return LearningEvaluation {
            disposition: LearningDisposition::NeedsReview,
            state: LearningCandidateState::NeedsReview,
            reason_codes: reasons,
            conflict: LearningConflict::None,
        };
    }

    // LEARNING.md §6: the policy mode decides which certainty may commit.
    if !context.policy.allows_certainty(context.payload.certainty) {
        push_reason(
            &mut reasons,
            match context.policy.memory_mode {
                LearningMemoryMode::ReviewAll => "REVIEW_ALL_POLICY",
                LearningMemoryMode::AutoConfirmedPrivate => "INFERRED_REQUIRES_REVIEW",
                _ => "INFERRED_REQUIRES_REVIEW",
            },
        );
        return LearningEvaluation {
            disposition: LearningDisposition::NeedsReview,
            state: LearningCandidateState::NeedsReview,
            reason_codes: reasons,
            conflict: LearningConflict::None,
        };
    }
    if context.payload.certainty == LearningCertainty::Inferred {
        let confidence = candidate.confidence.unwrap_or_default();
        if !confidence.is_finite() || confidence < context.policy.inferred_confidence_threshold {
            push_reason(&mut reasons, "INFERRED_CONFIDENCE_BELOW_THRESHOLD");
            return LearningEvaluation {
                disposition: LearningDisposition::NeedsReview,
                state: LearningCandidateState::NeedsReview,
                reason_codes: reasons,
                conflict: LearningConflict::None,
            };
        }
    }

    // LEARNING.md §15: contradiction and supersession.
    let conflict = classify_learning_conflict(&context.payload, &context.current_memories);
    match &conflict {
        LearningConflict::Duplicate { .. } => {
            push_reason(&mut reasons, "DUPLICATE_MEMORY");
            LearningEvaluation {
                disposition: LearningDisposition::Deduplicated,
                state: LearningCandidateState::Rejected,
                reason_codes: reasons,
                conflict,
            }
        }
        LearningConflict::Conflicting { .. } => {
            push_reason(&mut reasons, "CONFLICTING_MEMORY_REQUIRES_REVIEW");
            LearningEvaluation {
                disposition: LearningDisposition::NeedsReview,
                state: LearningCandidateState::NeedsReview,
                reason_codes: reasons,
                conflict,
            }
        }
        LearningConflict::Supports { .. } => {
            push_reason(&mut reasons, "SUPPORTS_EXISTING_MEMORY");
            LearningEvaluation {
                disposition: LearningDisposition::Deduplicated,
                state: LearningCandidateState::Rejected,
                reason_codes: reasons,
                conflict,
            }
        }
        LearningConflict::Supersedes { .. } => {
            push_reason(&mut reasons, "SUPERSEDES_INFERRED_MEMORY");
            LearningEvaluation {
                disposition: LearningDisposition::AutoCommit,
                state: LearningCandidateState::AutoCommitEligible,
                reason_codes: reasons,
                conflict,
            }
        }
        LearningConflict::None => {
            push_reason(&mut reasons, "ELIGIBLE_FOR_AUTO_COMMIT");
            LearningEvaluation {
                disposition: LearningDisposition::AutoCommit,
                state: LearningCandidateState::AutoCommitEligible,
                reason_codes: reasons,
                conflict,
            }
        }
    }
}

/// Scan a proposed body for secret-shaped content and produce a redacted copy.
///
/// The detector is deliberately conservative and deterministic: it recognises a
/// closed set of secret shapes, never guesses, and never invents content. An
/// unrecognised body is reported as clean, so a qualified classifier remains
/// responsible for the semantic sensitivity decision.
pub fn redact_sensitive_content(body: &str) -> RedactionReport {
    let mut findings = Vec::new();
    let bytes = body.as_bytes();
    let lower = body.to_lowercase();

    // Private key blocks.
    let mut search_from = 0usize;
    while let Some(found) = lower[search_from..].find("-----begin ") {
        let start = search_from + found;
        let remainder = &lower[start..];
        if remainder.contains("private key-----") {
            findings.push(SensitiveFinding {
                kind: SensitiveFindingKind::PrivateKey,
                sensitivity: LearningSensitivity::Forbidden,
                start,
                end: start
                    + remainder
                        .find("-----end ")
                        .map(|value| value + 11)
                        .unwrap_or(remainder.len()),
            });
            search_from = start + 11;
        } else {
            search_from = start + 1;
        }
    }

    // Provider access tokens with a known prefix.
    const TOKEN_PREFIXES: [&str; 17] = [
        "sk-ant-",
        "sk-proj-",
        "sk-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        "xoxs-",
        "glpat-",
        "ya29.",
        "npm_",
    ];
    for prefix in TOKEN_PREFIXES {
        let mut from = 0usize;
        while let Some(found) = lower[from..].find(prefix) {
            let start = from + found;
            let tail = &body[start + prefix.len()..];
            let token_len = tail
                .char_indices()
                .take_while(|(_, character)| {
                    character.is_ascii_alphanumeric()
                        || *character == '-'
                        || *character == '_'
                        || *character == '.'
                })
                .map(|(index, character)| index + character.len_utf8())
                .last()
                .unwrap_or(0);
            if token_len >= 8 {
                findings.push(SensitiveFinding {
                    kind: SensitiveFindingKind::AccessToken,
                    sensitivity: LearningSensitivity::Forbidden,
                    start,
                    end: start + prefix.len() + token_len,
                });
            }
            from = start + prefix.len();
        }
    }

    // Cloud access key identifiers.
    for marker in ["AKIA", "ASIA"] {
        let mut from = 0usize;
        while let Some(found) = lower[from..].find(marker) {
            let start = from + found;
            let tail = &body[start + marker.len()..];
            if tail.len() >= 16
                && tail[..16]
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                findings.push(SensitiveFinding {
                    kind: SensitiveFindingKind::AccessToken,
                    sensitivity: LearningSensitivity::Forbidden,
                    start,
                    end: start + marker.len() + 16,
                });
            }
            from = start + marker.len();
        }
    }

    // JSON web tokens.
    let mut from = 0usize;
    while let Some(found) = lower[from..].find("eyj") {
        let start = from + found;
        let tail = &body[start..];
        let segments: Vec<&str> = tail.split('.').take(3).collect();
        if segments.len() == 3
            && !segments[0].is_empty()
            && segments[1].len() >= 8
            && segments[2].len() >= 8
            && segments.iter().all(|segment| {
                segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            })
        {
            let end = start + segments[0].len() + segments[1].len() + segments[2].len() + 2;
            findings.push(SensitiveFinding {
                kind: SensitiveFindingKind::AccessToken,
                sensitivity: LearningSensitivity::Forbidden,
                start,
                end: end.min(body.len()),
            });
            from = end.min(body.len());
        } else {
            from = start + 3;
        }
    }

    // Authorization headers.
    let mut from = 0usize;
    while let Some(found) = lower[from..].find("bearer ") {
        let start = from + found;
        let tail = &body[start + 7..];
        let token_len = tail
            .char_indices()
            .take_while(|(_, character)| {
                !character.is_whitespace() && *character != '"' && *character != '\''
            })
            .map(|(index, character)| index + character.len_utf8())
            .last()
            .unwrap_or(0);
        if token_len >= 16 {
            findings.push(SensitiveFinding {
                kind: SensitiveFindingKind::AuthorizationHeader,
                sensitivity: LearningSensitivity::Forbidden,
                start,
                end: start + 7 + token_len,
            });
        }
        from = start + 7;
    }

    // Explicit secret assignments.
    const SECRET_KEYS: [&str; 10] = [
        "password",
        "passwd",
        "secret",
        "api_key",
        "apikey",
        "api-key",
        "access_token",
        "access-token",
        "client_secret",
        "private_key",
    ];
    for key in SECRET_KEYS {
        let mut from = 0usize;
        while let Some(found) = lower[from..].find(key) {
            let start = from + found;
            let tail = &body[start + key.len()..];
            let trimmed = tail.trim_start();
            let offset = tail.len() - trimmed.len();
            if matches!(trimmed.chars().next(), Some('=') | Some(':')) {
                let value_start = start + key.len() + offset + 1;
                let value_tail = &body[value_start..];
                let value_trimmed = value_tail.trim_start();
                let value_offset = value_tail.len() - value_trimmed.len();
                let value = value_trimmed.trim_matches(|c| c == '"' || c == '\'');
                if value.len() >= 6 {
                    findings.push(SensitiveFinding {
                        kind: SensitiveFindingKind::SecretAssignment,
                        sensitivity: LearningSensitivity::Forbidden,
                        start,
                        end: value_start + value_offset + value.len().min(value_trimmed.len()),
                    });
                }
            }
            from = start + key.len();
        }
    }

    // Payment card numbers with a valid Luhn checksum.
    let digits: Vec<(usize, char)> = body
        .char_indices()
        .filter(|(_, character)| character.is_ascii_digit())
        .collect();
    let mut index = 0usize;
    while index + 12 < digits.len() {
        let mut length = 12usize;
        while index + length <= digits.len() {
            let separator_free: String = digits[index..index + length]
                .iter()
                .map(|(_, character)| *character)
                .collect();
            if luhn_valid(&separator_free) {
                let start = digits[index].0;
                let end = digits[index + length - 1].0 + 1;
                findings.push(SensitiveFinding {
                    kind: SensitiveFindingKind::PaymentCard,
                    sensitivity: LearningSensitivity::Forbidden,
                    start,
                    end,
                });
                index += length;
                break;
            }
            length += 1;
        }
        index += 1;
    }

    // Government identifiers and long opaque blobs are uncertain, not forbidden.
    let body_bytes = body.as_bytes();
    let mut from = 0usize;
    while from + 11 <= body_bytes.len() {
        let digit_run = |offset: usize, length: usize| {
            body_bytes[from + offset..from + offset + length]
                .iter()
                .all(|byte| byte.is_ascii_digit())
        };
        if body_bytes[from + 3] == b'-'
            && body_bytes[from + 6] == b'-'
            && digit_run(0, 3)
            && digit_run(4, 2)
            && digit_run(7, 4)
        {
            findings.push(SensitiveFinding {
                kind: SensitiveFindingKind::GovernmentIdentifier,
                sensitivity: LearningSensitivity::ReviewRequired,
                start: from,
                end: from + 11,
            });
            from += 11;
        } else {
            from += 1;
        }
    }

    findings.sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
    let redacted_text = apply_redactions(body, &findings);
    let _ = bytes;
    RedactionReport {
        redacted_text,
        findings,
    }
}

fn apply_redactions(body: &str, findings: &[SensitiveFinding]) -> String {
    let mut output = String::with_capacity(body.len());
    let mut cursor = 0usize;
    for finding in findings {
        if finding.start < cursor || finding.end > body.len() {
            continue;
        }
        output.push_str(&body[cursor..finding.start]);
        output.push_str("[redacted]");
        cursor = finding.end;
    }
    output.push_str(&body[cursor..]);
    output
}

fn luhn_valid(digits: &str) -> bool {
    let mut sum = 0usize;
    let mut double = false;
    for character in digits.chars().rev() {
        let Some(digit) = character.to_digit(10) else {
            return false;
        };
        let mut value = digit as usize;
        if double {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
        double = !double;
    }
    sum.is_multiple_of(10)
}

/// The event payload for a learning candidate (LEARNING.md §19).
///
/// Only identifiers, digests, reason codes and bounded metadata are included. The
/// candidate body is never present: `proposed_payload_ref` names a Resource, and
/// the resource content is the owner's to review, not the journal's.
pub fn learning_candidate_event_payload(
    candidate: &LearningCandidate,
    evaluation: &LearningEvaluation,
) -> Value {
    json!({
        "candidate_id": candidate.candidate_id,
        "coworker_id": candidate.coworker_id,
        "kind": candidate.kind.as_str(),
        "source_kind": candidate.source_kind.as_str(),
        "source_digest": candidate.source_digest,
        "proposed_payload_ref": candidate.proposed_payload_ref,
        "redaction_report_ref": candidate.redaction_report_ref,
        "classifier_version": candidate.classifier_version,
        "confidence": candidate.confidence,
        "state": evaluation.state.as_str(),
        "disposition": format!("{:?}", evaluation.disposition),
        "reason_codes": evaluation.reason_codes,
        "conflict": match &evaluation.conflict {
            LearningConflict::None => Value::Null,
            LearningConflict::Duplicate { memory_id } => json!({ "kind": "DUPLICATE", "memory_id": memory_id }),
            LearningConflict::Supersedes { memory_id } => json!({ "kind": "SUPERSEDES", "memory_id": memory_id }),
            LearningConflict::Supports { memory_id } => json!({ "kind": "SUPPORTS", "memory_id": memory_id }),
            LearningConflict::Conflicting { memory_id } => json!({ "kind": "CONFLICTING", "memory_id": memory_id }),
        },
        "policy_revision": candidate.version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> CoworkerLearningPolicy {
        CoworkerLearningPolicy::default_for_coworker("coworker-1", "2026-10-20T00:00:00Z")
    }

    fn base_candidate() -> LearningCandidate {
        LearningCandidate {
            candidate_id: "candidate-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            coworker_id: "coworker-1".to_owned(),
            kind: LearningCandidateKind::MemoryPreference,
            source_kind: LearningSourceKind::UserExplicit,
            source_conversation_id: Some("conversation-1".to_owned()),
            source_message_id: Some("message-1".to_owned()),
            source_task_id: None,
            source_attempt_id: None,
            source_demonstration_id: None,
            source_digest: "sha256:abc".to_owned(),
            proposed_payload_ref: "resource:proposal-1".to_owned(),
            redaction_report_ref: "resource:redaction-1".to_owned(),
            classifier_version: "classifier-1".to_owned(),
            confidence: Some(0.95),
            state: LearningCandidateState::Classified,
            reason_codes: Vec::new(),
            created_at: "2026-10-20T00:00:00Z".to_owned(),
            updated_at: "2026-10-20T00:00:00Z".to_owned(),
            version: 1,
        }
    }

    fn payload() -> MemoryCandidatePayload {
        MemoryCandidatePayload {
            subject_ref: "user.reporting.preference".to_owned(),
            predicate: "prefers".to_owned(),
            value: json!("weekly one-page summary"),
            memory_class: LearningMemoryClass::Preference,
            certainty: LearningCertainty::Confirmed,
            sensitivity: LearningSensitivity::Low,
            scope: LearningMemoryScope::CoworkerPrivate,
            expires_at: None,
            supersedes_memory_id: None,
        }
    }

    fn base_context() -> LearningEvaluationContext {
        LearningEvaluationContext {
            policy: policy(),
            source_channel: LearningSourceChannel::OwnerMessage,
            conversation_class: Some(LearningConversationClass::Normal),
            payload: payload(),
            redaction: RedactionReport {
                redacted_text: "weekly one-page summary".to_owned(),
                findings: Vec::new(),
            },
            trust_excluded: false,
            source_resource_labels: Vec::new(),
            current_memories: Vec::new(),
        }
    }

    #[test]
    fn confirmed_owner_preference_auto_commits_under_the_default_private_policy() {
        let evaluation = evaluate_learning_candidate(&base_candidate(), &base_context());
        assert_eq!(evaluation.disposition, LearningDisposition::AutoCommit);
        assert_eq!(evaluation.state, LearningCandidateState::AutoCommitEligible);
        assert!(evaluation
            .reason_codes
            .contains(&"ELIGIBLE_FOR_AUTO_COMMIT".to_owned()));
    }

    #[test]
    fn review_all_policy_keeps_a_confirmed_candidate_for_the_owner() {
        let mut context = base_context();
        context.policy.memory_mode = LearningMemoryMode::ReviewAll;
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
        assert_eq!(evaluation.state, LearningCandidateState::NeedsReview);
        assert!(evaluation
            .reason_codes
            .contains(&"REVIEW_ALL_POLICY".to_owned()));
    }

    #[test]
    fn private_policy_does_not_auto_commit_an_inferred_claim() {
        let mut context = base_context();
        context.payload.certainty = LearningCertainty::Inferred;
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
        assert!(evaluation
            .reason_codes
            .contains(&"INFERRED_REQUIRES_REVIEW".to_owned()));
    }

    #[test]
    fn low_risk_inferred_policy_commits_only_above_the_threshold() {
        let mut context = base_context();
        context.policy.memory_mode = LearningMemoryMode::AutoConfirmedAndLowRiskInferredPrivate;
        context.payload.certainty = LearningCertainty::Inferred;

        let mut candidate = base_candidate();
        candidate.confidence = Some(0.5);
        let low = evaluate_learning_candidate(&candidate, &context);
        assert_eq!(low.disposition, LearningDisposition::NeedsReview);
        assert!(low
            .reason_codes
            .contains(&"INFERRED_CONFIDENCE_BELOW_THRESHOLD".to_owned()));

        candidate.confidence = Some(0.95);
        let high = evaluate_learning_candidate(&candidate, &context);
        assert_eq!(high.disposition, LearningDisposition::AutoCommit);
    }

    #[test]
    fn a_temporary_conversation_never_learns() {
        let mut context = base_context();
        context.conversation_class = Some(LearningConversationClass::Temporary);
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"TEMPORARY_CONVERSATION".to_owned()));
    }

    #[test]
    fn a_temporary_conversation_class_is_never_eligible_even_if_the_policy_lists_it() {
        let mut context = base_context();
        context.policy.eligible_conversation_classes = vec![
            LearningConversationClass::Normal,
            LearningConversationClass::Temporary,
        ];
        context.conversation_class = Some(LearningConversationClass::Temporary);
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"TEMPORARY_CONVERSATION".to_owned()));
    }

    #[test]
    fn third_party_content_cannot_create_memory() {
        for channel in [
            LearningSourceChannel::ThirdPartyMessage,
            LearningSourceChannel::ImportedDocument,
            LearningSourceChannel::WebContent,
            LearningSourceChannel::ToolDescription,
            LearningSourceChannel::ModelOutput,
            LearningSourceChannel::WorkerOutput,
            LearningSourceChannel::ConnectorEvent,
        ] {
            let mut context = base_context();
            context.source_channel = channel;
            let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
            assert_eq!(
                evaluation.disposition,
                LearningDisposition::Rejected,
                "channel {channel:?} must not create memory"
            );
            assert!(evaluation
                .reason_codes
                .contains(&"UNTRUSTED_SOURCE_CHANNEL".to_owned()));
        }
    }

    #[test]
    fn a_source_kind_outside_the_policy_is_rejected() {
        let mut context = base_context();
        let mut candidate = base_candidate();
        candidate.source_kind = LearningSourceKind::TaskOutcome;
        context.source_channel = LearningSourceChannel::VerifiedTaskOutcome;
        let evaluation = evaluate_learning_candidate(&candidate, &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"SOURCE_KIND_NOT_ELIGIBLE".to_owned()));
    }

    #[test]
    fn a_feedback_source_kind_requires_the_feedback_channel() {
        let context = base_context();
        let mut candidate = base_candidate();
        candidate.source_kind = LearningSourceKind::UserFeedback;
        let evaluation = evaluate_learning_candidate(&candidate, &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"SOURCE_KIND_CHANNEL_MISMATCH".to_owned()));
    }

    #[test]
    fn instruction_and_skill_proposals_always_require_review() {
        for kind in [
            LearningCandidateKind::InstructionPatch,
            LearningCandidateKind::SkillProposal,
            LearningCandidateKind::RoutineProposal,
            LearningCandidateKind::AutomationProposal,
        ] {
            let mut candidate = base_candidate();
            candidate.kind = kind;
            let evaluation = evaluate_learning_candidate(&candidate, &base_context());
            assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
            assert!(evaluation
                .reason_codes
                .contains(&"KIND_REQUIRES_REVIEW".to_owned()));
        }
    }

    #[test]
    fn forbidden_sensitivity_is_rejected_and_reviewable_sensitivity_is_not_auto_committed() {
        let mut forbidden = base_context();
        forbidden.payload.sensitivity = LearningSensitivity::Forbidden;
        let evaluation = evaluate_learning_candidate(&base_candidate(), &forbidden);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"FORBIDDEN_SENSITIVE_CONTENT".to_owned()));

        let mut review = base_context();
        review.payload.sensitivity = LearningSensitivity::ReviewRequired;
        let evaluation = evaluate_learning_candidate(&base_candidate(), &review);
        assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
        assert!(evaluation
            .reason_codes
            .contains(&"SENSITIVE_CONTENT_REQUIRES_REVIEW".to_owned()));
    }

    #[test]
    fn a_trust_exclusion_and_an_excluded_resource_label_fail_closed() {
        let mut trusted = base_context();
        trusted.trust_excluded = true;
        let evaluation = evaluate_learning_candidate(&base_candidate(), &trusted);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"TRUST_EXCLUDED_SOURCE".to_owned()));

        let mut labelled = base_context();
        labelled.policy.excluded_resource_labels = vec!["private-imports".to_owned()];
        labelled.source_resource_labels = vec!["private-imports".to_owned()];
        let evaluation = evaluate_learning_candidate(&base_candidate(), &labelled);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"EXCLUDED_RESOURCE_LABEL".to_owned()));
    }

    #[test]
    fn an_off_policy_never_creates_a_candidate() {
        let mut context = base_context();
        context.policy.memory_mode = LearningMemoryMode::Off;
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"MEMORY_MODE_OFF".to_owned()));
    }

    #[test]
    fn a_candidate_for_another_coworker_is_rejected() {
        let mut context = base_context();
        context.policy.coworker_id = "coworker-2".to_owned();
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"POLICY_COWORKER_MISMATCH".to_owned()));
    }

    fn memory(certainty: LearningCertainty, value: &str) -> MemoryComparison {
        MemoryComparison {
            memory: MemoryRecordMetadata {
                memory_id: "memory-1".to_owned(),
                workspace_id: "workspace-1".to_owned(),
                coworker_id: "coworker-1".to_owned(),
                context_document_id: "document-1".to_owned(),
                source_candidate_id: "candidate-0".to_owned(),
                memory_class: LearningMemoryClass::Preference,
                certainty,
                scope: LearningMemoryScope::CoworkerPrivate,
                source_digest: "sha256:def".to_owned(),
                committed_at: "2026-10-19T00:00:00Z".to_owned(),
                expires_at: None,
                superseded_by: None,
                status: "ACTIVE".to_owned(),
                version: 1,
            },
            subject_ref: "user.reporting.preference".to_owned(),
            predicate: "prefers".to_owned(),
            value: json!(value),
        }
    }

    #[test]
    fn an_identical_claim_is_deduplicated_instead_of_committed_twice() {
        let mut context = base_context();
        context.current_memories = vec![memory(
            LearningCertainty::Confirmed,
            "weekly one-page summary",
        )];
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Deduplicated);
        assert!(evaluation
            .reason_codes
            .contains(&"DUPLICATE_MEMORY".to_owned()));
        assert!(matches!(
            evaluation.conflict,
            LearningConflict::Duplicate { .. }
        ));
    }

    #[test]
    fn a_confirmed_claim_conflicting_with_a_confirmed_claim_requires_review() {
        let mut context = base_context();
        context.current_memories = vec![memory(LearningCertainty::Confirmed, "daily long report")];
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
        assert!(matches!(
            evaluation.conflict,
            LearningConflict::Conflicting { .. }
        ));
    }

    #[test]
    fn a_confirmed_claim_supersedes_an_inferred_claim() {
        let mut context = base_context();
        context.current_memories = vec![memory(LearningCertainty::Inferred, "daily long report")];
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::AutoCommit);
        assert!(matches!(
            evaluation.conflict,
            LearningConflict::Supersedes { .. }
        ));
        assert!(evaluation
            .reason_codes
            .contains(&"SUPERSEDES_INFERRED_MEMORY".to_owned()));
    }

    #[test]
    fn an_inferred_claim_conflicting_with_a_confirmed_claim_requires_review() {
        let mut context = base_context();
        context.payload.certainty = LearningCertainty::Inferred;
        context.policy.memory_mode = LearningMemoryMode::AutoConfirmedAndLowRiskInferredPrivate;
        context.current_memories = vec![memory(LearningCertainty::Confirmed, "daily long report")];
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
        assert!(matches!(
            evaluation.conflict,
            LearningConflict::Conflicting { .. }
        ));
    }

    #[test]
    fn a_revoked_memory_does_not_create_a_conflict() {
        let mut context = base_context();
        let mut existing = memory(LearningCertainty::Confirmed, "daily long report");
        existing.memory.status = "REVOKED".to_owned();
        context.current_memories = vec![existing];
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::AutoCommit);
    }

    #[test]
    fn normalized_keys_ignore_case_and_whitespace() {
        let left = normalized_memory_key("User.Reporting ", " Prefers ", &json!("Weekly  Summary"));
        let right = normalized_memory_key("user.reporting", "prefers", &json!("weekly summary"));
        assert_eq!(left, right);
    }

    #[test]
    fn the_detector_finds_a_provider_access_token_and_redacts_it() {
        let report = redact_sensitive_content("my key is sk-ant-api03-abcdefghijklmnop");
        assert!(!report.is_clean());
        assert!(report.has_forbidden_content());
        assert!(report.redacted_text.contains("[redacted]"));
        assert!(!report.redacted_text.contains("abcdefghijklmnop"));
    }

    #[test]
    fn the_detector_finds_a_private_key_block() {
        let report = redact_sensitive_content(
            "-----BEGIN RSA PRIVATE KEY-----\nabcdef\n-----END RSA PRIVATE KEY-----",
        );
        assert!(report.has_forbidden_content());
        assert!(!report.redacted_text.contains("abcdef"));
    }

    #[test]
    fn the_detector_finds_a_secret_assignment() {
        let report = redact_sensitive_content("password = \"hunter2secret\"");
        assert!(report.has_forbidden_content());
        assert!(!report.redacted_text.contains("hunter2secret"));
    }

    #[test]
    fn the_detector_finds_a_payment_card_number() {
        let report = redact_sensitive_content("card 4111 1111 1111 1111");
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.kind == SensitiveFindingKind::PaymentCard));
    }

    #[test]
    fn the_detector_leaves_an_ordinary_preference_alone() {
        let report = redact_sensitive_content("I prefer weekly one-page summaries");
        assert!(report.is_clean());
        assert_eq!(report.redacted_text, "I prefer weekly one-page summaries");
    }

    #[test]
    fn the_detector_flags_a_government_identifier_for_review_rather_than_forbidden() {
        let report = redact_sensitive_content("id 123-45-6789 on file");
        assert!(report.needs_review());
        assert!(!report.has_forbidden_content());
    }

    #[test]
    fn a_redaction_report_with_forbidden_content_blocks_auto_commit() {
        let mut context = base_context();
        context.redaction = redact_sensitive_content("token sk-ant-api03-abcdefghijklmnop");
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::Rejected);
        assert!(evaluation
            .reason_codes
            .contains(&"FORBIDDEN_SENSITIVE_CONTENT".to_owned()));
    }

    #[test]
    fn a_redaction_report_with_reviewable_content_blocks_auto_commit() {
        let mut context = base_context();
        context.redaction = redact_sensitive_content("id 123-45-6789 on file");
        let evaluation = evaluate_learning_candidate(&base_candidate(), &context);
        assert_eq!(evaluation.disposition, LearningDisposition::NeedsReview);
        assert!(evaluation
            .reason_codes
            .contains(&"SENSITIVE_CONTENT_REQUIRES_REVIEW".to_owned()));
    }

    #[test]
    fn the_event_payload_never_carries_the_candidate_body() {
        let candidate = base_candidate();
        let evaluation = evaluate_learning_candidate(&candidate, &base_context());
        let payload = learning_candidate_event_payload(&candidate, &evaluation);
        let rendered = payload.to_string();
        assert!(!rendered.contains("weekly one-page summary"));
        assert!(!rendered.contains("user.reporting.preference"));
        assert!(rendered.contains("candidate-1"));
        assert!(rendered.contains("sha256:abc"));
    }

    #[test]
    fn terminal_candidate_states_do_not_move_again() {
        assert!(LearningCandidateState::Committed.is_terminal());
        assert!(LearningCandidateState::Dismissed.is_terminal());
        assert!(LearningCandidateState::Rejected.is_terminal());
        assert!(LearningCandidateState::Superseded.is_terminal());
        assert!(!LearningCandidateState::NeedsReview.is_terminal());
        assert!(!LearningCandidateState::AutoCommitEligible.is_terminal());
    }
}
