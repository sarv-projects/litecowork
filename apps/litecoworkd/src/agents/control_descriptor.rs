//! Closed, bounded AgentControlDescriptor projection and integrity validation.
//!
//! This is the adapter-facing E03-S08 contract model. It validates observations and
//! descriptor metadata; it does not imply that any native provider has implemented or
//! qualified the descriptor.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::time::SystemTime;

const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;
const MAX_ARRAY_ITEMS: usize = 256;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentControlDescriptor {
    pub descriptor_digest: String,
    pub descriptor_api_version: u32,
    pub registry_agent_id: String,
    pub module_id: String,
    pub module_version: String,
    pub harness_version: Option<String>,
    pub protocol_version: Option<String>,
    pub installation: AgentInstallationObservation,
    pub auth: AgentAuthDescriptor,
    pub configuration: AgentConfigurationDescriptor,
    pub session_options: Option<AgentSessionOptionDescriptor>,
    pub input_surface: AgentInputSurfaceDescriptor,
    pub native_surface: AgentNativeSurfaceDescriptor,
    pub capabilities: AgentCapabilities,
    pub observed_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentInstallationObservation {
    pub registry_agent_id: String,
    pub module_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub state: InstallationObservationState,
    pub installed_version: Option<String>,
    pub available_version: Option<String>,
    pub installation_source: Option<String>,
    pub observed_at: String,
    pub expires_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum InstallationObservationState {
    Missing,
    Installing,
    Installed,
    UpdateAvailable,
    Updating,
    VersionUnavailable,
    Broken,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentAuthDescriptor {
    pub state: AgentAuthState,
    pub account_label: Option<String>,
    pub methods: Vec<AgentAuthMethodDescriptor>,
    pub credential_slots: Vec<AgentCredentialSlotDescriptor>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AgentAuthState {
    Unknown,
    NeedsAuth,
    Authenticating,
    Authenticated,
    Expired,
    Degraded,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentAuthMethodDescriptor {
    pub auth_method_id: String,
    pub display_name: String,
    pub kind: AgentAuthMethodKind,
    pub credential_slot_id: Option<String>,
    pub requires_restart: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AgentAuthMethodKind {
    NativeFlow,
    ExternalHandoff,
    DeviceCode,
    SecretSlot,
    NativeConfigOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentCredentialSlotDescriptor {
    pub credential_slot_id: String,
    pub display_name: String,
    pub secret_kind: AgentSecretKind,
    pub storage_owner: CredentialStorageOwner,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AgentSecretKind {
    ApiKey,
    Token,
    Password,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CredentialStorageOwner {
    NativeAgent,
    #[serde(rename = "LITECOWORK_SECRET_STORE")]
    LiteCoworkSecretStore,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentConfigurationDescriptor {
    pub schema_digest: String,
    pub fields: Vec<AgentConfigurationField>,
    pub native_config_targets: Vec<NativeConfigTarget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentConfigurationField {
    pub field_id: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub field_type: AgentConfigurationFieldType,
    pub required: bool,
    pub default_value: Option<Value>,
    pub constraints: Value,
    pub ownership: ConfigurationOwnership,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum AgentConfigurationFieldType {
    Boolean,
    Enum,
    String,
    Integer,
    Path,
    StringList,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ConfigurationOwnership {
    #[serde(rename = "LITECOWORK_NON_SECRET")]
    LiteCoworkNonSecret,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeConfigTarget {
    pub target_id: String,
    pub display_name: String,
    pub kind: NativeConfigTargetKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum NativeConfigTargetKind {
    File,
    Directory,
    NativeSettings,
    ExternalUi,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentSessionOptionDescriptor {
    pub descriptor_digest: String,
    pub options: Vec<AgentSessionOption>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentSessionOption {
    pub option_id: String,
    pub display_name: String,
    pub semantic_hint: Option<SessionOptionSemanticHint>,
    pub value_schema: Value,
    pub mutable_scope: SessionOptionMutableScope,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SessionOptionSemanticHint {
    Model,
    Reasoning,
    Effort,
    Mode,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SessionOptionMutableScope {
    LiveSession,
    NewSession,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentInputSurfaceDescriptor {
    pub text: bool,
    pub images: bool,
    pub files: bool,
    pub resources: bool,
    pub slash_commands: Vec<NativeCommandDescriptor>,
    pub reference_kinds: Vec<NativeReferenceKindDescriptor>,
    pub supports_command_discovery: bool,
    pub supports_reference_discovery: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeCommandDescriptor {
    pub command_id: String,
    pub token: String,
    pub display_name: String,
    pub description: Option<String>,
    pub argument_schema: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeReferenceKindDescriptor {
    pub reference_kind_id: String,
    pub trigger: String,
    pub display_name: String,
    pub description: Option<String>,
    pub selection_mode: NativeReferenceSelectionMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum NativeReferenceSelectionMode {
    NativePicker,
    HostResourcePicker,
    TextCompletion,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentNativeSurfaceDescriptor {
    pub native_config_preserved: bool,
    pub native_extensions_preserved: bool,
    pub native_subagents_preserved: bool,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentCapabilities {
    pub protocol: String,
    pub protocol_version: Option<String>,
    pub session: AgentSessionCapabilities,
    pub input: AgentInputCapabilities,
    pub extension: AgentExtensionCapabilities,
    pub reporting: AgentReportingCapabilities,
    pub environment: AgentEnvironmentCapabilities,
    pub limits: AgentLimits,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_instruction_delivery: Option<HostInstructionDelivery>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ModelSwitchCapability {
    Live,
    NewSession,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentSessionCapabilities {
    pub resume: bool,
    pub steer: bool,
    pub interrupt: bool,
    pub cancel: bool,
    pub fork: bool,
    pub model_switch: ModelSwitchCapability,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentInputCapabilities {
    pub text: bool,
    pub image: bool,
    pub file: bool,
    pub resources: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentExtensionCapabilities {
    pub mcp_stdio: bool,
    pub mcp_http: bool,
    pub skills: bool,
    pub plugins: bool,
    pub dynamic_attach: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentReportingCapabilities {
    pub tool_calls: bool,
    pub plan: bool,
    pub usage: bool,
    pub native_subagents: bool,
    pub approvals: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentEnvironmentCapabilities {
    pub cwd: bool,
    pub extra_directories: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentLimits {
    pub max_context_hint: Option<u64>,
    pub max_concurrent_sessions: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum HostInstructionDelivery {
    NativeSystem,
    NativeDeveloper,
    SessionGuidance,
    ContextAttachment,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorValidationError {
    InvalidEnvelope,
    InvalidDigest,
    InvalidIdentity,
    InvalidTimestamp,
    InvalidAuth,
    InvalidConfiguration,
    InvalidInputSurface,
    InvalidCapabilities,
    TooLarge,
}

impl AgentControlDescriptor {
    /// Parse untrusted descriptor bytes without allocating an unbounded JSON value.
    pub(crate) fn parse_bounded(bytes: &[u8]) -> Result<Self, DescriptorValidationError> {
        if bytes.len() > MAX_DESCRIPTOR_BYTES {
            return Err(DescriptorValidationError::TooLarge);
        }
        let descriptor: Self = serde_json::from_slice(bytes)
            .map_err(|_| DescriptorValidationError::InvalidEnvelope)?;
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Validate descriptor bounds and cross-references, then verify its normalized digest.
    pub(crate) fn validate(&self) -> Result<(), DescriptorValidationError> {
        let encoded =
            serde_json::to_vec(self).map_err(|_| DescriptorValidationError::InvalidEnvelope)?;
        if encoded.len() > MAX_DESCRIPTOR_BYTES {
            return Err(DescriptorValidationError::TooLarge);
        }
        if self.descriptor_api_version == 0
            || !valid_identifier(&self.registry_agent_id, 128)
            || !valid_module_id(&self.module_id)
            || !valid_version(&self.module_version)
            || !optional_text(&self.harness_version, 128)
            || !optional_text(&self.protocol_version, 128)
            || self.installation.registry_agent_id != self.registry_agent_id
            || self.installation.module_id != self.module_id
            || !valid_identifier(&self.installation.runtime_id, 256)
            || !valid_identifier(&self.installation.runtime_incarnation_id, 256)
            || !optional_text(&self.installation.installed_version, 128)
            || !optional_text(&self.installation.available_version, 128)
            || !optional_text(&self.installation.installation_source, 256)
        {
            return Err(DescriptorValidationError::InvalidIdentity);
        }
        if !timestamp_pair(&self.observed_at, &self.expires_at)
            || !timestamp_pair(
                &self.installation.observed_at,
                &self.installation.expires_at,
            )
        {
            return Err(DescriptorValidationError::InvalidTimestamp);
        }
        validate_auth(&self.auth)?;
        validate_configuration(&self.configuration)?;
        if let Some(options) = &self.session_options {
            validate_session_options(options)?;
        }
        validate_input_surface(&self.input_surface)?;
        validate_native_surface(&self.native_surface)?;
        validate_capabilities(&self.capabilities)?;
        if self.descriptor_digest != self.computed_digest()? {
            return Err(DescriptorValidationError::InvalidDigest);
        }
        Ok(())
    }

    pub(crate) fn computed_digest(&self) -> Result<String, DescriptorValidationError> {
        let normalized = serde_json::json!({
            "descriptor_api_version": self.descriptor_api_version,
            "registry_agent_id": self.registry_agent_id,
            "module_id": self.module_id,
            "module_version": self.module_version,
            "harness_version": self.harness_version,
            "protocol_version": self.protocol_version,
            "installation": self.installation,
            "auth": self.auth,
            "configuration": self.configuration,
            "session_options": self.session_options,
            "input_surface": self.input_surface,
            "native_surface": self.native_surface,
            "capabilities": self.capabilities,
            "observed_at": self.observed_at,
            "expires_at": self.expires_at,
        });
        let bytes = serde_json_canonicalizer::to_vec(&normalized)
            .map_err(|_| DescriptorValidationError::InvalidEnvelope)?;
        let digest = Sha256::digest(bytes);
        Ok(format!("sha256:{}", hex::encode(digest)))
    }

    /// Report freshness separately from structural integrity. Callers must fail closed
    /// when either the descriptor or its nested installation observation has expired.
    pub(crate) fn is_fresh_at(&self, now: SystemTime) -> bool {
        let now = time::OffsetDateTime::from(now);
        timestamp_window_contains(&self.observed_at, &self.expires_at, now)
            && timestamp_window_contains(
                &self.installation.observed_at,
                &self.installation.expires_at,
                now,
            )
    }
}

fn validate_auth(value: &AgentAuthDescriptor) -> Result<(), DescriptorValidationError> {
    if value.methods.len() > 16
        || value.credential_slots.len() > 32
        || !optional_text(&value.account_label, 128)
    {
        return Err(DescriptorValidationError::InvalidAuth);
    }
    let mut slot_ids = BTreeSet::new();
    for slot in &value.credential_slots {
        if !valid_identifier(&slot.credential_slot_id, 128)
            || !valid_text(&slot.display_name, 1, 128)
            || !slot_ids.insert(slot.credential_slot_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidAuth);
        }
    }
    let mut method_ids = BTreeSet::new();
    for method in &value.methods {
        if !valid_identifier(&method.auth_method_id, 128)
            || !valid_text(&method.display_name, 1, 128)
            || !method_ids.insert(method.auth_method_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidAuth);
        }
        if method.kind == AgentAuthMethodKind::SecretSlot {
            let Some(id) = method.credential_slot_id.as_deref() else {
                return Err(DescriptorValidationError::InvalidAuth);
            };
            if !value.credential_slots.iter().any(|slot| {
                slot.credential_slot_id == id
                    && slot.storage_owner == CredentialStorageOwner::LiteCoworkSecretStore
            }) {
                return Err(DescriptorValidationError::InvalidAuth);
            }
        } else if method.credential_slot_id.is_some() {
            return Err(DescriptorValidationError::InvalidAuth);
        }
    }
    Ok(())
}

fn validate_configuration(
    value: &AgentConfigurationDescriptor,
) -> Result<(), DescriptorValidationError> {
    if !valid_digest(&value.schema_digest)
        || value.fields.len() > 64
        || value.native_config_targets.len() > 32
    {
        return Err(DescriptorValidationError::InvalidConfiguration);
    }
    let mut field_ids = BTreeSet::new();
    for field in &value.fields {
        if !valid_identifier(&field.field_id, 128)
            || !valid_text(&field.display_name, 1, 128)
            || looks_secret_bearing(&field.field_id)
            || !field.constraints.is_object()
            || serde_json::to_vec(&field.constraints).map_or(true, |bytes| bytes.len() > 16 * 1024)
            || field.default_value.as_ref().is_some_and(|default| {
                serde_json::to_vec(default).map_or(true, |bytes| bytes.len() > 16 * 1024)
                    || !default_matches_field_type(default, field.field_type)
            })
            || !field_ids.insert(field.field_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidConfiguration);
        }
    }
    let mut targets = BTreeSet::new();
    for target in &value.native_config_targets {
        if !valid_identifier(&target.target_id, 128)
            || !valid_text(&target.display_name, 1, 128)
            || !targets.insert(target.target_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidConfiguration);
        }
    }
    Ok(())
}

fn default_matches_field_type(value: &Value, field_type: AgentConfigurationFieldType) -> bool {
    match field_type {
        AgentConfigurationFieldType::Boolean => value.is_boolean(),
        AgentConfigurationFieldType::Enum
        | AgentConfigurationFieldType::String
        | AgentConfigurationFieldType::Path => value.is_string(),
        AgentConfigurationFieldType::Integer => {
            value.as_i64().is_some() || value.as_u64().is_some()
        }
        AgentConfigurationFieldType::StringList => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
    }
}

fn validate_session_options(
    value: &AgentSessionOptionDescriptor,
) -> Result<(), DescriptorValidationError> {
    if !valid_digest(&value.descriptor_digest) || value.options.len() > 64 {
        return Err(DescriptorValidationError::InvalidConfiguration);
    }
    let mut ids = BTreeSet::new();
    for option in &value.options {
        if !valid_identifier(&option.option_id, 128)
            || !valid_text(&option.display_name, 1, 128)
            || !option.value_schema.is_object()
            || serde_json::to_vec(&option.value_schema)
                .map_or(true, |bytes| bytes.len() > 16 * 1024)
            || !has_only_local_schema_references(&option.value_schema)
            || !ids.insert(option.option_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidConfiguration);
        }
    }
    Ok(())
}

/// Descriptor schemas are inline data. Resolving a provider-controlled reference could
/// make validation fetch arbitrary local or remote resources, so only fragment
/// references within the inline schema are accepted and `$id` cannot change their base.
fn has_only_local_schema_references(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().all(|(key, nested)| {
            // `$id` changes the base URI used to resolve even fragment-only
            // references. Inline descriptors do not need identifiers, so reject
            // it rather than allowing an external base to escape the local schema.
            if key == "$id" {
                return false;
            }
            if matches!(key.as_str(), "$ref" | "$dynamicRef" | "$recursiveRef") {
                nested
                    .as_str()
                    .is_some_and(|reference| reference.starts_with('#'))
            } else {
                has_only_local_schema_references(nested)
            }
        }),
        Value::Array(items) => items.iter().all(has_only_local_schema_references),
        _ => true,
    }
}

fn validate_input_surface(
    value: &AgentInputSurfaceDescriptor,
) -> Result<(), DescriptorValidationError> {
    if value.slash_commands.len() > MAX_ARRAY_ITEMS || value.reference_kinds.len() > 64 {
        return Err(DescriptorValidationError::InvalidInputSurface);
    }
    let mut command_ids = BTreeSet::new();
    for command in &value.slash_commands {
        if !valid_identifier(&command.command_id, 128)
            || !command.token.starts_with('/')
            || !valid_text(&command.token, 2, 128)
            || !valid_text(&command.display_name, 1, 128)
            || !optional_text(&command.description, 512)
            || command.argument_schema.as_ref().is_some_and(|schema| {
                !schema.is_object()
                    || serde_json::to_vec(schema).map_or(true, |bytes| bytes.len() > 16 * 1024)
                    || !has_only_local_schema_references(schema)
            })
            || !command_ids.insert(command.command_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidInputSurface);
        }
    }
    let mut reference_ids = BTreeSet::new();
    for reference in &value.reference_kinds {
        if !valid_identifier(&reference.reference_kind_id, 128)
            || !valid_text(&reference.trigger, 1, 8)
            || !valid_text(&reference.display_name, 1, 128)
            || !optional_text(&reference.description, 512)
            || !reference_ids.insert(reference.reference_kind_id.as_str())
        {
            return Err(DescriptorValidationError::InvalidInputSurface);
        }
    }
    Ok(())
}

fn validate_native_surface(
    value: &AgentNativeSurfaceDescriptor,
) -> Result<(), DescriptorValidationError> {
    if !optional_text(&value.summary, 1024) {
        return Err(DescriptorValidationError::InvalidInputSurface);
    }
    Ok(())
}

fn validate_capabilities(value: &AgentCapabilities) -> Result<(), DescriptorValidationError> {
    if !valid_text(&value.protocol, 1, 64)
        || !optional_text(&value.protocol_version, 128)
        || value.limits.max_concurrent_sessions == Some(0)
    {
        return Err(DescriptorValidationError::InvalidCapabilities);
    }
    Ok(())
}

fn timestamp_pair(observed: &str, expires: &str) -> bool {
    let format = &time::format_description::well_known::Rfc3339;
    match (
        time::OffsetDateTime::parse(observed, format),
        time::OffsetDateTime::parse(expires, format),
    ) {
        (Ok(observed), Ok(expires)) => expires > observed,
        _ => false,
    }
}

fn timestamp_window_contains(observed: &str, expires: &str, now: time::OffsetDateTime) -> bool {
    let format = &time::format_description::well_known::Rfc3339;
    match (
        time::OffsetDateTime::parse(observed, format),
        time::OffsetDateTime::parse(expires, format),
    ) {
        (Ok(observed), Ok(expires)) => observed <= now && now < expires,
        _ => false,
    }
}

fn valid_version(value: &str) -> bool {
    value.len() <= 128 && super::module_registry::valid_semver(value)
}

fn valid_module_id(value: &str) -> bool {
    valid_identifier(value, 128)
        && value
            .split('/')
            .all(|part| !part.is_empty() && !matches!(part, "." | ".."))
}

fn valid_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/@-".contains(&byte))
        && !value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_text(value: &str, min: usize, max: usize) -> bool {
    let length = value.chars().count();
    length >= min && length <= max && !value.chars().any(char::is_control)
}

fn optional_text(value: &Option<String>, max: usize) -> bool {
    value.as_ref().is_none_or(|value| valid_text(value, 1, max))
}

fn looks_secret_bearing(value: &str) -> bool {
    let normalized: String = value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    ["secret", "password", "credential", "apikey", "token"]
        .iter()
        .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor_json() -> Value {
        serde_json::json!({
            "descriptor_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "descriptor_api_version": 1,
            "registry_agent_id": "agent.test",
            "module_id": "litecowork/test-agent",
            "module_version": "1.2.3",
            "harness_version": null,
            "protocol_version": "1.0",
            "installation": {
                "registry_agent_id": "agent.test",
                "module_id": "litecowork/test-agent",
                "runtime_id": "runtime-test",
                "runtime_incarnation_id": "runtime-incarnation-test",
                "state": "INSTALLED",
                "installed_version": "1.2.3",
                "available_version": null,
                "installation_source": "test",
                "observed_at": "2026-10-17T10:00:00Z",
                "expires_at": "2026-10-17T10:05:00Z"
            },
            "auth": {
                "state": "NEEDS_AUTH",
                "account_label": null,
                "methods": [{
                    "auth_method_id": "api-key",
                    "display_name": "API key",
                    "kind": "SECRET_SLOT",
                    "credential_slot_id": "api-key-slot",
                    "requires_restart": false
                }],
                "credential_slots": [{
                    "credential_slot_id": "api-key-slot",
                    "display_name": "API key",
                    "secret_kind": "API_KEY",
                    "storage_owner": "LITECOWORK_SECRET_STORE",
                    "required": true
                }]
            },
            "configuration": {
                "schema_digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "fields": [{
                    "field_id": "model_name",
                    "display_name": "Model name",
                    "type": "STRING",
                    "required": false,
                    "constraints": {},
                    "ownership": "LITECOWORK_NON_SECRET"
                }],
                "native_config_targets": []
            },
            "session_options": null,
            "input_surface": {
                "text": true,
                "images": false,
                "files": false,
                "resources": false,
                "slash_commands": [],
                "reference_kinds": [],
                "supports_command_discovery": false,
                "supports_reference_discovery": false
            },
            "native_surface": {
                "native_config_preserved": true,
                "native_extensions_preserved": true,
                "native_subagents_preserved": true,
                "summary": null
            },
            "capabilities": {
                "protocol": "ACP",
                "protocol_version": "1.0",
                "session": {
                    "resume": false,
                    "steer": false,
                    "interrupt": false,
                    "cancel": false,
                    "fork": false,
                    "model_switch": "UNSUPPORTED"
                },
                "input": {"text": true, "image": false, "file": false, "resources": false},
                "extension": {"mcp_stdio": false, "mcp_http": false, "skills": false, "plugins": false, "dynamic_attach": false},
                "reporting": {"tool_calls": false, "plan": false, "usage": false, "native_subagents": false, "approvals": false},
                "environment": {"cwd": false, "extra_directories": false},
                "limits": {"max_context_hint": null, "max_concurrent_sessions": null},
                "host_instruction_delivery": null
            },
            "observed_at": "2026-10-17T10:00:00Z",
            "expires_at": "2026-10-17T10:05:00Z"
        })
    }

    fn valid_descriptor() -> AgentControlDescriptor {
        let mut value: AgentControlDescriptor =
            serde_json::from_value(descriptor_json()).expect("fixture matches descriptor schema");
        value.descriptor_digest = value.computed_digest().expect("descriptor digest computes");
        value
    }

    #[test]
    fn configuration_field_uses_canonical_type_property() {
        let field: AgentConfigurationField = serde_json::from_str(
            r#"{"field_id":"theme","display_name":"Theme","type":"STRING","required":false,"constraints":{},"ownership":"LITECOWORK_NON_SECRET"}"#,
        )
        .expect("canonical AgentConfigurationField JSON should deserialize");

        assert_eq!(field.field_type, AgentConfigurationFieldType::String);
        let serialized = serde_json::to_value(field).expect("field serializes");
        assert_eq!(serialized["type"], "STRING");
        assert!(serialized.get("field_type").is_none());
    }

    #[test]
    fn descriptor_versions_follow_module_semver_contract() {
        for valid in ["0.1.0", "1.2.3", "1.2.3-rc.1", "1.2.3+build.7"] {
            assert!(
                valid_version(valid),
                "expected {valid} to be a valid version"
            );
        }
        for invalid in ["1.2", "v1.2.3", "01.2.3", "1.2.3-", "1.2.3+"] {
            assert!(!valid_version(invalid), "expected {invalid} to be rejected");
        }
    }

    #[test]
    fn valid_bounded_descriptor_passes_digest_and_slot_validation() {
        assert_eq!(valid_descriptor().validate(), Ok(()));
    }

    #[test]
    fn descriptor_digest_tampering_is_rejected() {
        let mut descriptor = valid_descriptor();
        descriptor.module_version = "1.2.4".to_owned();
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidDigest)
        );
    }

    #[test]
    fn secret_slot_method_must_reference_secret_store_owned_slot() {
        let mut descriptor = valid_descriptor();
        descriptor.auth.credential_slots[0].storage_owner = CredentialStorageOwner::NativeAgent;
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidAuth)
        );
    }

    #[test]
    fn descriptor_rejects_unknown_fields_at_the_boundary() {
        let mut value = descriptor_json();
        value["unrecognized_provider_payload"] = serde_json::json!({"token":"private"});
        assert!(serde_json::from_value::<AgentControlDescriptor>(value).is_err());
    }

    #[test]
    fn configuration_rejects_secret_fields_with_separator_variants() {
        for field_id in ["api_key", "API-KEY", "refresh_token", "password"] {
            let mut value = descriptor_json();
            value["configuration"]["fields"][0]["field_id"] =
                serde_json::Value::String(field_id.to_owned());
            let mut descriptor: AgentControlDescriptor =
                serde_json::from_value(value).expect("field shape remains valid");
            descriptor.descriptor_digest = descriptor
                .computed_digest()
                .expect("descriptor digest computes");
            assert_eq!(
                descriptor.validate(),
                Err(DescriptorValidationError::InvalidConfiguration),
                "secret field {field_id} must be rejected"
            );
        }
    }

    #[test]
    fn configuration_defaults_must_match_the_declared_field_type() {
        let cases = [
            ("BOOLEAN", serde_json::json!(true), true),
            ("ENUM", serde_json::json!("balanced"), true),
            ("STRING", serde_json::json!("gpt-5"), true),
            ("INTEGER", serde_json::json!(7), true),
            ("PATH", serde_json::json!("/work/project"), true),
            ("STRING_LIST", serde_json::json!(["one", "two"]), true),
            ("BOOLEAN", serde_json::json!("true"), false),
            ("ENUM", serde_json::json!(1), false),
            ("INTEGER", serde_json::json!(1.5), false),
            ("STRING_LIST", serde_json::json!(["one", 2]), false),
        ];

        for (field_type, default_value, expected_valid) in cases {
            let mut value = descriptor_json();
            value["configuration"]["fields"][0]["type"] =
                serde_json::Value::String(field_type.to_owned());
            value["configuration"]["fields"][0]["default_value"] = default_value;
            let mut descriptor: AgentControlDescriptor =
                serde_json::from_value(value).expect("field shape remains valid");
            descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");

            assert_eq!(
                descriptor.validate().is_ok(),
                expected_valid,
                "default value must be checked against {field_type}"
            );
        }
    }

    #[test]
    fn descriptor_expiry_must_follow_observation_time() {
        let mut descriptor = valid_descriptor();
        descriptor.expires_at = descriptor.observed_at.clone();
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidTimestamp)
        );
    }

    #[test]
    fn freshness_requires_current_descriptor_and_installation_windows() {
        let descriptor = valid_descriptor();
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_792_231_200);
        assert!(descriptor.is_fresh_at(now));
        assert!(!descriptor.is_fresh_at(now + std::time::Duration::from_secs(301)));
        assert!(!descriptor.is_fresh_at(now - std::time::Duration::from_secs(1)));
    }

    #[test]
    fn descriptor_rejects_payloads_over_the_wire_bound() {
        let mut value = descriptor_json();
        value["configuration"]["fields"][0]["constraints"]["description"] =
            serde_json::Value::String("x".repeat(300 * 1024));
        let mut descriptor: AgentControlDescriptor =
            serde_json::from_value(value).expect("large but valid JSON shape deserializes");
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::TooLarge)
        );
    }

    #[test]
    fn bounded_parser_rejects_oversized_input_before_json_decoding() {
        assert_eq!(
            AgentControlDescriptor::parse_bounded(&vec![b' '; MAX_DESCRIPTOR_BYTES + 1]),
            Err(DescriptorValidationError::TooLarge)
        );
    }

    #[test]
    fn bounded_parser_accepts_a_current_closed_descriptor() {
        let expected = valid_descriptor();
        let bytes = serde_json::to_vec(&expected).expect("descriptor serializes");
        assert_eq!(AgentControlDescriptor::parse_bounded(&bytes), Ok(expected));
    }

    #[test]
    fn credential_snapshot_uses_only_fresh_declared_slot_metadata() {
        let descriptor = valid_descriptor();
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_792_231_200);
        let snapshot = crate::agent_credentials_operator::AgentCredentialDescriptorSnapshot::from_control_descriptor(
            "profile-test",
            &descriptor,
            now,
        )
        .expect("fresh module descriptor converts to the narrow credential snapshot");
        assert_eq!(snapshot.agent_profile_id, "profile-test");
        assert_eq!(snapshot.digest, descriptor.descriptor_digest);
        assert_eq!(snapshot.credential_slots.len(), 1);
        assert_eq!(
            snapshot.credential_slots[0].credential_slot_id,
            "api-key-slot"
        );
        assert_eq!(
            snapshot.credential_slots[0].secret_kind,
            storage_core::SecretKind::ApiKey
        );
        assert!(matches!(
            snapshot.credential_slots[0].storage_owner,
            crate::agent_credentials_operator::CredentialStorageOwner::LiteCoworkSecretStore
        ));
        assert_eq!(
            crate::agent_credentials_operator::AgentCredentialDescriptorSnapshot::from_control_descriptor(
                "profile-test",
                &descriptor,
                now + std::time::Duration::from_secs(301),
            )
            .err()
            .expect("expired descriptor is rejected"),
            crate::agent_credentials_operator::CredentialSubmissionError::DescriptorUnavailable
        );
    }

    #[test]
    fn session_option_schemas_reject_external_reference_fetches() {
        let mut descriptor = valid_descriptor();
        descriptor.session_options = Some(AgentSessionOptionDescriptor {
            descriptor_digest:
                "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
            options: vec![AgentSessionOption {
                option_id: "model".to_owned(),
                display_name: "Model".to_owned(),
                semantic_hint: Some(SessionOptionSemanticHint::Model),
                value_schema: serde_json::json!({
                    "type": "string",
                    "$ref": "https://untrusted.invalid/schema.json"
                }),
                mutable_scope: SessionOptionMutableScope::NewSession,
            }],
        });
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidConfiguration)
        );
    }

    #[test]
    fn inline_schema_fragment_refs_are_allowed_but_nested_remote_refs_are_rejected() {
        let mut descriptor = valid_descriptor();
        descriptor.session_options = Some(AgentSessionOptionDescriptor {
            descriptor_digest:
                "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
            options: vec![AgentSessionOption {
                option_id: "model".to_owned(),
                display_name: "Model".to_owned(),
                semantic_hint: Some(SessionOptionSemanticHint::Model),
                value_schema: serde_json::json!({
                    "type": "string",
                    "$ref": "#/$defs/model",
                    "$defs": {"model": {"type": "string"}}
                }),
                mutable_scope: SessionOptionMutableScope::NewSession,
            }],
        });
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(descriptor.validate(), Ok(()));

        descriptor.session_options.as_mut().unwrap().options[0].value_schema["$defs"]["model"]["$dynamicRef"] =
            serde_json::json!("file:///etc/passwd");
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidConfiguration)
        );

        descriptor.session_options.as_mut().unwrap().options[0].value_schema["$defs"]["model"] =
            serde_json::json!({"$id": "https://untrusted.invalid/base", "$ref": "#"});
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidConfiguration)
        );
    }

    #[test]
    fn command_argument_schemas_reject_remote_refs_and_oversized_schemas() {
        let mut descriptor = valid_descriptor();
        descriptor
            .input_surface
            .slash_commands
            .push(NativeCommandDescriptor {
                command_id: "search".to_owned(),
                token: "/search".to_owned(),
                display_name: "Search".to_owned(),
                description: None,
                argument_schema: Some(serde_json::json!({
                    "type": "object",
                    "properties": {"query": {"$ref": "https://untrusted.invalid/query"}}
                })),
            });
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidInputSurface)
        );

        descriptor.input_surface.slash_commands[0].argument_schema =
            Some(serde_json::json!({"description": "x".repeat(16 * 1024)}));
        descriptor.descriptor_digest = descriptor.computed_digest().expect("digest computes");
        assert_eq!(
            descriptor.validate(),
            Err(DescriptorValidationError::InvalidInputSurface)
        );
    }
}
