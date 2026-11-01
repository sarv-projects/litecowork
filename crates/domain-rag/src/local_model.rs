//! Local model inventory rules.
//!
//! A local model is inventory only. LiteCowork has no global "Providers" feature: a
//! local model becomes selectable only through an Agent, and only when that Agent's
//! AgentModule reports compatibility for the exact model. The presence of an engine or
//! a model on the machine is never proof that it is connected, authorized, or usable
//! for a Task.

use serde::{Deserialize, Serialize};

/// Canonical capability identity of the local model inventory.
pub const LOCAL_MODEL_CAPABILITY_ID: &str = "litecowork.local-models";
/// The only surface that may offer a local model.
pub const LOCAL_MODEL_SURFACE: &str = "AGENT_SCOPED";
/// The policy that decides whether a local model may be offered to an Agent.
pub const LOCAL_MODEL_AGENT_POLICY: &str = "AGENT_MODULE_COMPATIBILITY_REQUIRED";

/// How a local model may be used right now.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalModelState {
    /// The model is known but no AgentModule reports compatibility for it.
    InventoryOnly,
    /// An AgentModule reports compatibility for this exact model.
    AgentCompatible,
    /// An AgentModule reports that it cannot use this exact model.
    AgentIncompatible,
    /// The model cannot be used because no qualified local provider is configured.
    Unavailable,
}

/// One observed local model. The inventory records identity and limits only; it never
/// records an endpoint, a credential, or a claim of connectivity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LocalModelInventoryEntry {
    pub model_id: String,
    pub engine_id: String,
    pub display_name: String,
    pub state: LocalModelState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_module_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

/// The compatibility an AgentModule reports for one exact local model.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AgentModuleCompatibility {
    pub agent_module_id: String,
    pub model_id: String,
    pub compatible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

/// The truthful local model availability for a Workspace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LocalModelAvailability {
    pub capability_id: String,
    pub status: String,
    pub reason_code: String,
    pub reason: String,
    pub inventory_only: bool,
    pub surface: String,
    pub agent_binding_policy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<LocalModelInventoryEntry>,
}

impl LocalModelAvailability {
    /// The fail-closed availability used until a qualified local provider exists.
    pub fn not_configured(reason_code: &str, reason: &str) -> Self {
        Self {
            capability_id: LOCAL_MODEL_CAPABILITY_ID.to_owned(),
            status: "NOT_CONFIGURED".to_owned(),
            reason_code: reason_code.to_owned(),
            reason: reason.to_owned(),
            inventory_only: true,
            surface: LOCAL_MODEL_SURFACE.to_owned(),
            agent_binding_policy: LOCAL_MODEL_AGENT_POLICY.to_owned(),
            models: Vec::new(),
        }
    }
}

/// Resolves the state of one inventory entry against the compatibility an AgentModule
/// reports for it. A model without a matching compatibility record is inventory only.
pub fn resolve_local_model_state(
    entry: &LocalModelInventoryEntry,
    compatibility: Option<&AgentModuleCompatibility>,
) -> LocalModelState {
    let Some(compatibility) = compatibility else {
        return LocalModelState::InventoryOnly;
    };
    if compatibility.model_id != entry.model_id {
        return LocalModelState::InventoryOnly;
    }
    if compatibility.compatible {
        LocalModelState::AgentCompatible
    } else {
        LocalModelState::AgentIncompatible
    }
}

/// Returns true when the exact model may be offered to the Agent that reported the
/// compatibility. The check is exact: agent module, model, and compatibility must all
/// match.
pub fn agent_module_compatible(
    entry: &LocalModelInventoryEntry,
    compatibility: Option<&AgentModuleCompatibility>,
) -> bool {
    matches!(
        resolve_local_model_state(entry, compatibility),
        LocalModelState::AgentCompatible
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AgentModuleCompatibility, LocalModelAvailability, LocalModelInventoryEntry, LocalModelState,
        agent_module_compatible, resolve_local_model_state,
    };

    fn entry() -> LocalModelInventoryEntry {
        LocalModelInventoryEntry {
            model_id: "model-a".to_owned(),
            engine_id: "engine-a".to_owned(),
            display_name: "Model A".to_owned(),
            state: LocalModelState::InventoryOnly,
            context_window_tokens: Some(8_192),
            agent_module_id: None,
            reason_code: None,
        }
    }

    #[test]
    fn a_model_without_agent_module_compatibility_is_inventory_only() {
        assert_eq!(
            resolve_local_model_state(&entry(), None),
            LocalModelState::InventoryOnly
        );
        assert!(!agent_module_compatible(&entry(), None));
    }

    #[test]
    fn only_an_exact_model_compatibility_allows_an_agent_to_use_the_model() {
        let other = AgentModuleCompatibility {
            agent_module_id: "module-a".to_owned(),
            model_id: "model-b".to_owned(),
            compatible: true,
            reason_code: None,
        };
        assert_eq!(
            resolve_local_model_state(&entry(), Some(&other)),
            LocalModelState::InventoryOnly
        );

        let exact = AgentModuleCompatibility {
            agent_module_id: "module-a".to_owned(),
            model_id: "model-a".to_owned(),
            compatible: true,
            reason_code: None,
        };
        assert_eq!(
            resolve_local_model_state(&entry(), Some(&exact)),
            LocalModelState::AgentCompatible
        );
        assert!(agent_module_compatible(&entry(), Some(&exact)));
    }

    #[test]
    fn an_incompatible_agent_module_keeps_the_model_unusable() {
        let exact = AgentModuleCompatibility {
            agent_module_id: "module-a".to_owned(),
            model_id: "model-a".to_owned(),
            compatible: false,
            reason_code: Some("TOOL_CALLS_UNSUPPORTED".to_owned()),
        };
        assert_eq!(
            resolve_local_model_state(&entry(), Some(&exact)),
            LocalModelState::AgentIncompatible
        );
        assert!(!agent_module_compatible(&entry(), Some(&exact)));
    }

    #[test]
    fn the_unconfigured_availability_is_inventory_only_and_agent_scoped() {
        let availability =
            LocalModelAvailability::not_configured("LOCAL_MODEL_PROVIDER_NOT_QUALIFIED", "none");
        assert_eq!(availability.status, "NOT_CONFIGURED");
        assert!(availability.inventory_only);
        assert_eq!(availability.surface, "AGENT_SCOPED");
        assert_eq!(
            availability.agent_binding_policy,
            "AGENT_MODULE_COMPATIBILITY_REQUIRED"
        );
        assert!(availability.models.is_empty());
    }
}
