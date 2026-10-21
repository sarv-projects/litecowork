//! Local model inventory status for the authenticated local Operator.
//!
//! Local models are inventory only. LiteCowork has no global Providers feature: a
//! local model becomes selectable only through an Agent, and only when that Agent's
//! AgentModule reports compatibility for the exact model. This route therefore reports
//! inventory and policy only. It never probes an endpoint, never accepts an
//! Agent- or document-supplied URL, and never claims that an engine on the machine is
//! connected, authorized, or usable for a Task.

#![allow(clippy::result_large_err)] // Handlers return full Axum Responses to preserve status, body, and headers.

use super::*;
use domain_rag::LocalModelAvailability;
use serde::Serialize;

pub(super) fn routes() -> Router<ApiState> {
    Router::new().route("/v1/capabilities/local-models", get(local_model_status))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalModelStatusResponse {
    capability_id: String,
    status: String,
    reason_code: String,
    reason: String,
    inventory_only: bool,
    surface: String,
    agent_binding_policy: String,
    models: Vec<domain_rag::LocalModelInventoryEntry>,
}

async fn local_model_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner_for(&state.store, &state.principal_id, &workspace_id)?;
    let availability = LocalModelAvailability::not_configured(
        "LOCAL_MODEL_PROVIDER_NOT_QUALIFIED",
        "No qualified local inference provider is registered. Local models stay inventory only and are offered through an Agent when its AgentModule reports compatibility.",
    );
    let mut response = Json(LocalModelStatusResponse {
        capability_id: availability.capability_id,
        status: availability.status,
        reason_code: availability.reason_code,
        reason: availability.reason,
        inventory_only: availability.inventory_only,
        surface: availability.surface,
        agent_binding_policy: availability.agent_binding_policy,
        models: availability.models,
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::LocalModelStatusResponse;
    use domain_rag::{LOCAL_MODEL_AGENT_POLICY, LOCAL_MODEL_CAPABILITY_ID, LOCAL_MODEL_SURFACE};

    #[test]
    fn the_local_model_status_is_inventory_only_and_agent_scoped() {
        let availability = domain_rag::LocalModelAvailability::not_configured(
            "LOCAL_MODEL_PROVIDER_NOT_QUALIFIED",
            "none",
        );
        let response = LocalModelStatusResponse {
            capability_id: availability.capability_id,
            status: availability.status,
            reason_code: availability.reason_code,
            reason: availability.reason,
            inventory_only: availability.inventory_only,
            surface: availability.surface,
            agent_binding_policy: availability.agent_binding_policy,
            models: availability.models,
        };
        assert_eq!(response.capability_id, LOCAL_MODEL_CAPABILITY_ID);
        assert_eq!(response.status, "NOT_CONFIGURED");
        assert_eq!(response.reason_code, "LOCAL_MODEL_PROVIDER_NOT_QUALIFIED");
        assert!(response.inventory_only);
        assert_eq!(response.surface, LOCAL_MODEL_SURFACE);
        assert_eq!(response.agent_binding_policy, LOCAL_MODEL_AGENT_POLICY);
        assert!(
            response.models.is_empty(),
            "an unconfigured local provider must not claim an inventory"
        );
    }
}
