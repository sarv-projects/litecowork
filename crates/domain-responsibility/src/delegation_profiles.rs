//! Typed Workspace-owned DelegationProfile commands.
//!
//! This module owns canonical name handling, input validation, immutable revisions,
//! lifecycle transitions, and request fingerprints. Persistence adapters must recheck
//! Workspace ownership and binding eligibility in their committing transaction.
use crate::{PrincipalRef, ContractObject};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DelegationProfileStatus { Enabled, Disabled, Archived }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OptimizationPreference { QualityFirst, Balanced, CostFirst, LatencyFirst }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DelegationLatencyClass { Standard, Interactive, DeadlineSensitive }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NativeDelegationPolicy { Inherit, Allow, DenyIfSupported }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegatedWorkerPolicy {
    pub capability_allowlist: Vec<Value>,
    pub maximum_effect_risk: String,
    pub filesystem_write_scope: String,
    pub external_effects: String,
    pub secret_access: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegatedEnvironmentPolicy {
    pub placement_preference: Value,
    pub isolation: String,
    pub sharing_scope: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationProfileRevisionInput {
    pub name: String,
    pub routing_description: String,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default = "empty_object")]
    pub session_options: Value,
    #[serde(default)]
    pub session_options_descriptor_digest: Option<String>,
    #[serde(default)]
    pub required_features: Vec<String>,
    #[serde(default)]
    pub preferred_features: Vec<String>,
    pub enforced_policy: DelegatedWorkerPolicy,
    pub optimization_preference: OptimizationPreference,
    #[serde(default)]
    pub quality_floor: Option<Vec<Value>>,
    pub max_concurrency: u32,
    pub max_host_delegation_depth: u32,
    #[serde(default)]
    pub budget_ceiling: Option<Value>,
    pub latency_class: DelegationLatencyClass,
    pub environment_policy: DelegatedEnvironmentPolicy,
    pub native_delegation_policy: NativeDelegationPolicy,
    pub warm_policy: Value,
}

fn empty_object() -> Value { Value::Object(Default::default()) }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelegationProfileRevision {
    pub delegation_profile_id: String,
    pub revision: u64,
    pub workspace_id: String,
    pub name: String,
    pub name_key: String,
    pub routing_description: String,
    pub instructions: Option<String>,
    pub session_options: Value,
    pub session_options_descriptor_digest: Option<String>,
    pub required_features: Vec<String>,
    pub preferred_features: Vec<String>,
    pub enforced_policy: DelegatedWorkerPolicy,
    pub optimization_preference: OptimizationPreference,
    pub quality_floor: Option<Vec<Value>>,
    pub max_concurrency: u32,
    pub max_host_delegation_depth: u32,
    pub budget_ceiling: Option<Value>,
    pub latency_class: DelegationLatencyClass,
    pub environment_policy: DelegatedEnvironmentPolicy,
    pub native_delegation_policy: NativeDelegationPolicy,
    pub warm_policy: Value,
    pub authored_by: PrincipalRef,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegationProfile {
    pub delegation_profile_id: String,
    pub workspace_id: String,
    pub agent_binding_id: String,
    pub name: String,
    pub name_key: String,
    pub current_revision: u64,
    pub status: DelegationProfileStatus,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommittedDelegationProfile {
    pub profile: DelegationProfile,
    pub revision: DelegationProfileRevision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DelegationProfileError {
    NotFound, Unauthorized, AlreadyExists, VersionConflict, VersionOverflow,
    IdempotencyConflict, InvalidDefinition, OptionsInvalid, SessionOptionsUnsupported,
    Archived, WorkspaceArchived, EnablementUnavailable, Storage,
}
impl std::fmt::Display for DelegationProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for DelegationProfileError {}

#[derive(Clone, Debug)]
pub struct DelegationProfileCommandScope {
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum DelegationProfileCommand {
    Create { profile_id: String, agent_binding_id: String, revision: DelegationProfileRevisionInput },
    Revise { profile_id: String, expected_version: u64, revision: DelegationProfileRevisionInput },
    Duplicate { source_profile_id: String, expected_version: u64, profile_id: String, name: String },
    SetStatus { profile_id: String, expected_version: u64, status: DelegationProfileStatus },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelegationProfileEvent {
    pub kind: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DelegationProfileAppend { Revision(DelegationProfileRevision) }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelegationProfileMutation {
    pub committed: CommittedDelegationProfile,
    pub expected_version: Option<u64>,
    pub append: Option<DelegationProfileAppend>,
    pub event: DelegationProfileEvent,
}

pub trait DelegationProfileTransaction {
    fn now(&self) -> String;
    fn profile(&mut self, id: &str) -> Result<Option<CommittedDelegationProfile>, DelegationProfileError>;
    fn binding_is_enabled_in_workspace(&mut self, id: &str) -> Result<bool, DelegationProfileError>;
    fn name_is_available(&mut self, binding_id: &str, name_key: &str, excluding_profile_id: Option<&str>) -> Result<bool, DelegationProfileError>;
    fn commit(&mut self, mutation: DelegationProfileMutation) -> Result<CommittedDelegationProfile, DelegationProfileError>;
}

/// Persistence transaction contract. Implementations authenticate/recheck current
/// Workspace ownership before replay and commit; command receipts, aggregate snapshots,
/// immutable revision, event envelope and aggregate head commit atomically.
pub trait DelegationProfileStore {
    fn transaction<F>(&mut self, scope: &DelegationProfileCommandScope, fingerprint: &str, operation: F) -> Result<CommittedDelegationProfile, DelegationProfileError>
    where F: Fn(&mut dyn DelegationProfileTransaction) -> Result<CommittedDelegationProfile, DelegationProfileError> + Send + 'static;
}

pub struct DelegationProfileService<S> { store: S }
impl<S: DelegationProfileStore> DelegationProfileService<S> {
    pub fn new(store: S) -> Self { Self { store } }
    pub fn into_store(self) -> S { self.store }

    pub fn execute(&mut self, scope: &DelegationProfileCommandScope, command: DelegationProfileCommand) -> Result<CommittedDelegationProfile, DelegationProfileError> {
        if scope.principal_id.trim().is_empty() || scope.workspace_id.trim().is_empty() || scope.request_id.trim().is_empty() { return Err(DelegationProfileError::Unauthorized); }
        validate_command(&command)?;
        let fingerprint = digest(&serde_json_canonicalizer::to_vec(&serde_json::json!({"workspace_id":scope.workspace_id,"command":command})).map_err(|_| DelegationProfileError::InvalidDefinition)?);
        let owned_scope = scope.clone();
        self.store.transaction(scope, &fingerprint, move |tx| decide(tx, &owned_scope, command.clone()))
    }
}

fn decide(tx: &mut dyn DelegationProfileTransaction, scope: &DelegationProfileCommandScope, command: DelegationProfileCommand) -> Result<CommittedDelegationProfile, DelegationProfileError> {
    let now = tx.now();
    let author = PrincipalRef { principal_id: scope.principal_id.clone(), kind: crate::PrincipalKind::User };
    match command {
        DelegationProfileCommand::Create { profile_id, agent_binding_id, revision } => {
            if tx.profile(&profile_id)?.is_some() { return Err(DelegationProfileError::AlreadyExists); }
            if !tx.binding_is_enabled_in_workspace(&agent_binding_id)? { return Err(DelegationProfileError::InvalidDefinition); }
            let revision = build_revision(&profile_id, &scope.workspace_id, 1, revision, &author, &now)?;
            if !tx.name_is_available(&agent_binding_id, &revision.name_key, None)? { return Err(DelegationProfileError::AlreadyExists); }
            let profile = DelegationProfile { delegation_profile_id: profile_id.clone(), workspace_id: scope.workspace_id.clone(), agent_binding_id, name: revision.name.clone(), name_key: revision.name_key.clone(), current_revision: 1, status: DelegationProfileStatus::Disabled, created_at: now.clone(), updated_at: now, version: 1 };
            let event = DelegationProfileEvent { kind: "delegation_profile.created.v1".into(), payload: serde_json::json!({"delegation_profile_id":profile_id,"workspace_id":scope.workspace_id,"agent_binding_id":profile.agent_binding_id,"current_revision":1,"status":"DISABLED","aggregate_version":1}) };
            tx.commit(DelegationProfileMutation { committed: CommittedDelegationProfile { profile, revision: revision.clone() }, expected_version: None, append: Some(DelegationProfileAppend::Revision(revision)), event })
        }
        DelegationProfileCommand::Revise { profile_id, expected_version, revision: input } => {
            let current = tx.profile(&profile_id)?.ok_or(DelegationProfileError::NotFound)?;
            ensure_mutable(&current.profile, expected_version)?;
            let next_revision = current.profile.current_revision.checked_add(1).ok_or(DelegationProfileError::VersionOverflow)?;
            let revision = build_revision(&profile_id, &scope.workspace_id, next_revision, input, &author, &now)?;
            if !tx.name_is_available(&current.profile.agent_binding_id, &revision.name_key, Some(&profile_id))? { return Err(DelegationProfileError::AlreadyExists); }
            let mut profile = current.profile;
            profile.version = next_version(profile.version, expected_version)?;
            profile.current_revision = next_revision;
            profile.name = revision.name.clone(); profile.name_key = revision.name_key.clone(); profile.updated_at = now;
            let event = DelegationProfileEvent { kind: "delegation_profile.revised.v1".into(), payload: serde_json::json!({"delegation_profile_id":profile_id,"revision":next_revision,"revision_digest":format!("sha256:{}", digest(&serde_json_canonicalizer::to_vec(&revision).map_err(|_|DelegationProfileError::InvalidDefinition)?)),"authored_by":author,"aggregate_version":profile.version}) };
            tx.commit(DelegationProfileMutation { committed: CommittedDelegationProfile { profile, revision: revision.clone() }, expected_version: Some(expected_version), append: Some(DelegationProfileAppend::Revision(revision)), event })
        }
        DelegationProfileCommand::Duplicate { source_profile_id, expected_version, profile_id, name } => {
            let source = tx.profile(&source_profile_id)?.ok_or(DelegationProfileError::NotFound)?;
            ensure_mutable(&source.profile, expected_version)?;
            let mut input = input_from_revision(&source.revision);
            input.name = name;
            let revision = build_revision(&profile_id, &scope.workspace_id, 1, input, &author, &now)?;
            if !tx.name_is_available(&source.profile.agent_binding_id, &revision.name_key, None)? { return Err(DelegationProfileError::AlreadyExists); }
            let profile = DelegationProfile { delegation_profile_id: profile_id.clone(), workspace_id: scope.workspace_id.clone(), agent_binding_id: source.profile.agent_binding_id, name: revision.name.clone(), name_key: revision.name_key.clone(), current_revision: 1, status: DelegationProfileStatus::Disabled, created_at: now.clone(), updated_at: now, version: 1 };
            // The current created-event schema is deliberately closed and does not
            // carry the copy source. Idempotency fingerprints still bind the source
            // and reviewed version; the new immutable revision records the copied data.
            let event = DelegationProfileEvent { kind: "delegation_profile.created.v1".into(), payload: serde_json::json!({"delegation_profile_id":profile_id,"workspace_id":scope.workspace_id,"agent_binding_id":profile.agent_binding_id,"current_revision":1,"status":"DISABLED","aggregate_version":1}) };
            tx.commit(DelegationProfileMutation { committed: CommittedDelegationProfile { profile, revision: revision.clone() }, expected_version: None, append: Some(DelegationProfileAppend::Revision(revision)), event })
        }
        DelegationProfileCommand::SetStatus { profile_id, expected_version, status } => {
            let current = tx.profile(&profile_id)?.ok_or(DelegationProfileError::NotFound)?;
            ensure_mutable(&current.profile, expected_version)?;
            if current.profile.status == status { return Err(DelegationProfileError::InvalidDefinition); }
            if status == DelegationProfileStatus::Enabled { return Err(DelegationProfileError::EnablementUnavailable); }
            let mut profile = current.profile;
            let from = profile.status;
            profile.status = status;
            profile.version = next_version(profile.version, expected_version)?;
            profile.updated_at = now;
            let event = DelegationProfileEvent { kind: "delegation_profile.status.changed.v1".into(), payload: serde_json::json!({"delegation_profile_id":profile_id,"from":from,"to":status,"aggregate_version":profile.version}) };
            tx.commit(DelegationProfileMutation { committed: CommittedDelegationProfile { profile, revision: current.revision }, expected_version: Some(expected_version), append: None, event })
        }
    }
}

fn ensure_mutable(profile: &DelegationProfile, expected: u64) -> Result<(), DelegationProfileError> {
    if profile.status == DelegationProfileStatus::Archived { return Err(DelegationProfileError::Archived); }
    if profile.version != expected { return Err(DelegationProfileError::VersionConflict); }
    Ok(())
}
fn next_version(actual: u64, expected: u64) -> Result<u64, DelegationProfileError> {
    if actual != expected { return Err(DelegationProfileError::VersionConflict); }
    actual.checked_add(1).ok_or(DelegationProfileError::VersionOverflow)
}

pub fn normalize_profile_name(value: &str) -> Result<(String, String), DelegationProfileError> {
    let trimmed = value.trim();
    let normalized: String = trimmed.nfc().collect();
    if normalized.is_empty() || normalized.chars().count() > 120 || normalized.chars().any(char::is_control) { return Err(DelegationProfileError::InvalidDefinition); }
    let folded: String = normalized.case_fold().collect();
    let key: String = folded.nfc().collect();
    Ok((normalized, key))
}

fn build_revision(profile_id: &str, workspace_id: &str, revision: u64, mut input: DelegationProfileRevisionInput, author: &PrincipalRef, now: &str) -> Result<DelegationProfileRevision, DelegationProfileError> {
    let (name, name_key) = normalize_profile_name(&input.name)?; input.name = name.clone();
    if profile_id.trim().is_empty() || workspace_id.trim().is_empty() || revision == 0
        || input.routing_description.trim().is_empty() || input.routing_description.chars().count() > 240
        || input.instructions.as_ref().is_some_and(|value| value.chars().count() > 12000)
        || !(1..=8).contains(&input.max_concurrency) || input.max_host_delegation_depth > 2
        || !unique(&input.required_features) || !unique(&input.preferred_features)
        || input.required_features.iter().any(|feature| !valid_agent_feature(feature))
        || input.preferred_features.iter().any(|feature| !valid_agent_feature(feature))
        || !valid_policy(&input.enforced_policy) || !valid_environment_policy(&input.environment_policy)
        || !valid_warm_policy(&input.warm_policy)
        || input.quality_floor.as_ref().is_some_and(|items| items.iter().any(|item| !valid_acceptance_criterion(item, workspace_id)))
        || input.budget_ceiling.as_ref().is_some_and(|budget| !valid_budget(budget))
    { return Err(DelegationProfileError::InvalidDefinition); }
    if !valid_digest_option(&input.session_options, input.session_options_descriptor_digest.as_deref()) { return Err(DelegationProfileError::OptionsInvalid); }
    let option_bytes = serde_json_canonicalizer::to_vec(&input.session_options).map_err(|_| DelegationProfileError::InvalidDefinition)?;
    if option_bytes.len() > 65_536 || input.session_options.as_object().is_none_or(|object| object.len() > 64) { return Err(DelegationProfileError::OptionsInvalid); }
    // Until adapter descriptors and Trust/Environment profile validation are connected,
    // persisted session overrides cannot be trusted as negotiated values.
    if input.session_options.as_object().is_some_and(|object| !object.is_empty()) { return Err(DelegationProfileError::SessionOptionsUnsupported); }
    Ok(DelegationProfileRevision {
        delegation_profile_id: profile_id.into(), revision, workspace_id: workspace_id.into(), name, name_key,
        routing_description: input.routing_description.trim().into(), instructions: input.instructions,
        session_options: input.session_options, session_options_descriptor_digest: input.session_options_descriptor_digest,
        required_features: input.required_features, preferred_features: input.preferred_features,
        enforced_policy: input.enforced_policy, optimization_preference: input.optimization_preference,
        quality_floor: input.quality_floor, max_concurrency: input.max_concurrency,
        max_host_delegation_depth: input.max_host_delegation_depth, budget_ceiling: input.budget_ceiling,
        latency_class: input.latency_class, environment_policy: input.environment_policy,
        native_delegation_policy: input.native_delegation_policy, warm_policy: input.warm_policy,
        authored_by: author.clone(), created_at: now.into(),
    })
}

fn input_from_revision(revision: &DelegationProfileRevision) -> DelegationProfileRevisionInput {
    DelegationProfileRevisionInput {
        name: revision.name.clone(), routing_description: revision.routing_description.clone(), instructions: revision.instructions.clone(),
        session_options: revision.session_options.clone(), session_options_descriptor_digest: revision.session_options_descriptor_digest.clone(),
        required_features: revision.required_features.clone(), preferred_features: revision.preferred_features.clone(),
        enforced_policy: revision.enforced_policy.clone(), optimization_preference: revision.optimization_preference,
        quality_floor: revision.quality_floor.clone(), max_concurrency: revision.max_concurrency,
        max_host_delegation_depth: revision.max_host_delegation_depth, budget_ceiling: revision.budget_ceiling.clone(),
        latency_class: revision.latency_class, environment_policy: revision.environment_policy.clone(),
        native_delegation_policy: revision.native_delegation_policy, warm_policy: revision.warm_policy.clone(),
    }
}
fn valid_digest_option(options: &Value, digest: Option<&str>) -> bool {
    let nonempty = options.as_object().is_some_and(|object| !object.is_empty());
    match (nonempty, digest) {
        (false, None) => true,
        (true, Some(value)) => value.strip_prefix("sha256:").is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())),
        _ => false,
    }
}
fn valid_policy(value: &DelegatedWorkerPolicy) -> bool {
    matches!(value.maximum_effect_risk.as_str(), "SAFE" | "SENSITIVE" | "HIGH_IMPACT")
        && matches!(value.filesystem_write_scope.as_str(), "WORKTREE_ONLY" | "ATTEMPT_PRIVATE" | "EXPLICIT_SHARED")
        && matches!(value.external_effects.as_str(), "DENY" | "REQUIRE_EXISTING_POLICY")
        && matches!(value.secret_access.as_str(), "NONE" | "TASK_SCOPED_GRANTS_ONLY")
        && value.capability_allowlist.iter().all(valid_capability_ref)
}
fn valid_agent_feature(feature: &str) -> bool {
    ["session.resume","session.steer","session.interrupt","session.cancel","session.fork","input.text","input.image","input.file","input.resources","extension.mcp_stdio","extension.mcp_http","extension.skills","extension.plugins","extension.dynamic_attach","reporting.tool_calls","reporting.plan","reporting.usage","reporting.native_subagents","reporting.approvals","environment.cwd","environment.extra_directories"].contains(&feature)
}
fn valid_capability_ref(value: &Value) -> bool {
    let Some(object) = value.as_object() else { return false };
    let identity = object.get("identity_kind").and_then(Value::as_str);
    let common = ["capability_id","identity_kind","source","digest"];
    if object.keys().any(|key| !["capability_id","identity_kind","source","digest","package_version","component"].contains(&key.as_str()))
        || common.iter().any(|key| !object.contains_key(*key))
        || ["capability_id","source"].iter().any(|key| object.get(*key).and_then(Value::as_str).is_none_or(str::is_empty))
        || !object.get("digest").and_then(Value::as_str).is_some_and(|value| value.strip_prefix("sha256:").is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))))
    { return false; }
    match identity {
        Some("PACKAGE_COMPONENT") => object.get("package_version").and_then(Value::as_str).is_some_and(|value| !value.is_empty()),
        Some("MCP_SKILL") => object.get("component").and_then(Value::as_str).is_some_and(|value| !value.is_empty()),
        _ => false,
    }
}
fn valid_acceptance_criterion(value: &Value, workspace_id: &str) -> bool {
    let Some(object) = value.as_object() else { return false };
    let allowed = ["criterion_id", "description", "subject_refs", "required_evidence", "verifier_hint", "mandatory"];
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || ["criterion_id", "description"].iter().any(|key| object.get(*key).and_then(Value::as_str).is_none_or(str::is_empty))
        || !matches!(object.get("required_evidence").and_then(Value::as_str), Some("REPORTED" | "OBSERVED" | "VERIFIED"))
        || object.get("mandatory").and_then(Value::as_bool).is_none()
        || object.get("verifier_hint").is_some_and(|value| !value.is_null() && value.as_str().is_none())
    { return false; }
    object.get("subject_refs").is_none_or(|refs| refs.as_array().is_some_and(|refs| refs.iter().all(|reference| {
        let Some(reference) = reference.as_object() else { return false };
        reference.keys().all(|key| ["workspace_id", "resource_id", "revision_id"].contains(&key.as_str()))
            && reference.get("workspace_id").and_then(Value::as_str) == Some(workspace_id)
            && reference.get("resource_id").and_then(Value::as_str).is_some_and(|id| !id.is_empty())
            && reference.get("revision_id").is_none_or(|id| id.is_null() || id.as_str().is_some_and(|id| !id.is_empty()))
    })))
}
fn valid_budget(value: &Value) -> bool {
    let Some(object) = value.as_object() else { return false };
    let allowed = ["max_wall_time_ms", "max_cost_minor_units", "currency", "max_tokens", "max_child_attempts", "max_concurrency"];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) { return false; }
    for key in ["max_wall_time_ms", "max_cost_minor_units", "max_tokens", "max_child_attempts", "max_concurrency"] {
        if object.get(key).is_some_and(|value| !value.is_null() && value.as_u64().is_none()) { return false; }
    }
    if object.get("max_concurrency").and_then(Value::as_u64).is_some_and(|value| value == 0) { return false; }
    object.get("currency").is_none_or(|value| value.is_null() || value.as_str().is_some_and(|currency| currency.len() == 3 && currency.bytes().all(|byte| byte.is_ascii_uppercase())))
}
fn valid_environment_policy(value: &DelegatedEnvironmentPolicy) -> bool {
    matches!(value.isolation.as_str(), "REQUIRED" | "PREFERRED")
        && matches!(value.sharing_scope.as_str(), "ATTEMPT_PRIVATE" | "TASK_SHARED" | "COWORKER_PRIVATE" | "WORKSPACE_SHARED")
        && (matches!(value.placement_preference.as_str(), Some("AUTO" | "LOCAL_ONLY" | "CLOUD_PREFERRED" | "CLOUD_ONLY"))
            || value.placement_preference.as_object().is_some_and(|object| object.len() == 1 && object.get("runtime_id").and_then(Value::as_str).is_some_and(|id| !id.trim().is_empty())))
}
fn valid_warm_policy(value: &Value) -> bool {
    let Some(object) = value.as_object() else { return false };
    let fields = ["host", "native_session", "capability_hosts", "browser_environment", "local_model", "ttl_ms", "max_memory_bytes", "max_idle_cost", "triggers"];
    if object.keys().any(|key| !fields.contains(&key.as_str())) { return false; }
    let string = |key: &str| object.get(key).and_then(Value::as_str);
    matches!(string("host"), Some("COLD" | "TTL" | "PIN_WHILE_ACTIVE"))
        && matches!(string("native_session"), Some("CLOSE_ON_SETTLE" | "REUSE_IF_SAFE"))
        && matches!(string("capability_hosts"), Some("COLD" | "TTL"))
        && matches!(string("browser_environment"), Some("COLD" | "TASK" | "WORKSPACE"))
        && matches!(string("local_model"), Some("PROVIDER_DEFAULT" | "KEEP_RECENT_HINT"))
        && object.get("triggers").and_then(Value::as_array).is_some_and(|items| items.iter().all(|item| item.as_str().is_some_and(|value| ["ACTIVE_TASK","RECENT_USE","USER_SELECTED","QUOTA_LOW","PREDICTED_FAILOVER","DEADLINE_APPROACHING"].contains(&value))))
        && ["ttl_ms","max_memory_bytes"].iter().all(|key| object.get(*key).is_none_or(Value::is_null) || object.get(*key).and_then(Value::as_u64).is_some())
}
fn unique(values: &[String]) -> bool { values.iter().collect::<BTreeSet<_>>().len() == values.len() }
fn validate_command(command: &DelegationProfileCommand) -> Result<(), DelegationProfileError> {
    let id_ok = |value: &str| !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control);
    match command {
        DelegationProfileCommand::Create { profile_id, agent_binding_id, revision } => { if !id_ok(profile_id) || !id_ok(agent_binding_id) { return Err(DelegationProfileError::InvalidDefinition); } validate_input(revision) }
        DelegationProfileCommand::Revise { profile_id, expected_version, revision } => { if !id_ok(profile_id) || *expected_version == 0 { return Err(DelegationProfileError::InvalidDefinition); } validate_input(revision) }
        DelegationProfileCommand::Duplicate { source_profile_id, expected_version, profile_id, name } => { if !id_ok(source_profile_id) || !id_ok(profile_id) || *expected_version == 0 { return Err(DelegationProfileError::InvalidDefinition); } normalize_profile_name(name).map(|_| ()) }
        DelegationProfileCommand::SetStatus { profile_id, expected_version, status } => {
            if !id_ok(profile_id) || *expected_version == 0 { return Err(DelegationProfileError::InvalidDefinition); }
            if *status == DelegationProfileStatus::Enabled { return Err(DelegationProfileError::EnablementUnavailable); }
            Ok(())
        }
    }
}
fn validate_input(input: &DelegationProfileRevisionInput) -> Result<(), DelegationProfileError> {
    let author = PrincipalRef { principal_id: "validation".into(), kind: crate::PrincipalKind::User };
    build_revision("validation", "validation", 1, input.clone(), &author, "1970-01-01T00:00:00Z").map(|_| ())
}
fn digest(bytes: &[u8]) -> String { use sha2::{Digest, Sha256}; hex::encode(Sha256::digest(bytes)) }
