//! `beginAgentAuthentication` (specs/002 T07).
//!
//! Serves only a method declared by the profile's fresh module descriptor. The
//! request carries no body, path, shell string or secret; the route launches the
//! module's own native flow through the launcher boundary and never sees a
//! credential. A second begin while one is in flight returns the in-flight
//! challenge instead of launching again.
#![allow(clippy::result_large_err)]

use super::agent_control_operator::{
    AgentControlApiState, fresh_descriptor, no_store_middleware, validate_profile_id,
};
use super::{ApiState, operator_error};
use crate::agents::auth_service::{AgentAuthService, BeginError, BeginOutcome};
use axum::{
    Json, Router,
    extract::{FromRef, Path, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::post,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
struct AgentAuthApiState {
    control: AgentControlApiState,
    auth: Arc<AgentAuthService>,
}

impl FromRef<ApiState> for AgentAuthApiState {
    fn from_ref(state: &ApiState) -> Self {
        Self {
            control: AgentControlApiState::from_ref(state),
            auth: AgentAuthService::shared(),
        }
    }
}

pub(super) fn routes() -> Router<ApiState> {
    routes_with_state::<ApiState>()
        .route_layer(middleware::from_fn(super::authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
}

fn routes_with_state<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    AgentAuthApiState: FromRef<S>,
{
    Router::new().route(
        "/v1/agent-profiles/{agent_profile_id}/auth/{auth_method_id}/begin",
        post(begin_agent_authentication),
    )
}

#[cfg(test)]
pub(super) fn test_router(control: AgentControlApiState) -> Router {
    routes_with_state::<AgentAuthApiState>()
        .route_layer(middleware::from_fn(super::authenticate))
        .route_layer(middleware::from_fn(no_store_middleware))
        .with_state(AgentAuthApiState {
            control,
            auth: Arc::new(AgentAuthService::new(Arc::new(
                crate::agents::auth_service::UnqualifiedLauncher,
            ))),
        })
}

async fn begin_agent_authentication(
    State(state): State<AgentAuthApiState>,
    headers: HeaderMap,
    Path((agent_profile_id, auth_method_id)): Path<(String, String)>,
) -> Response {
    if let Err(response) = validate_profile_id(&agent_profile_id) {
        return response;
    }
    if auth_method_id.trim().is_empty()
        || auth_method_id.len() > 128
        || auth_method_id.chars().any(char::is_control)
        || auth_method_id.contains(['/', '\\'])
    {
        return operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Authentication method ID is invalid",
        );
    }
    let descriptor = match fresh_descriptor(&state.control, &headers, &agent_profile_id).await {
        Ok(descriptor) => descriptor,
        Err(response) => return response,
    };
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let auth = state.auth.clone();
    let store = state.control.store.clone();
    let profile_id = agent_profile_id.clone();
    let method = auth_method_id.clone();
    // The launch can block on process spawn, so it never runs on the async executor.
    let outcome = tokio::task::spawn_blocking(move || {
        auth.begin(&store, &profile_id, &descriptor, &method, now)
    })
    .await;
    match outcome {
        Ok(Ok(
            BeginOutcome::Started { expires_at, .. } | BeginOutcome::InFlight { expires_at, .. },
        )) => Json(json!({
            "kind": "NATIVE_FLOW_STARTED",
            "auth_method_id": auth_method_id,
            "expires_at": rfc3339(expires_at),
        }))
        .into_response(),
        Ok(Ok(BeginOutcome::SecretRequired)) => Json(json!({
            "kind": "SECRET_REQUIRED",
            "auth_method_id": auth_method_id,
        }))
        .into_response(),
        Ok(Ok(BeginOutcome::NativeConfigRequired)) => Json(json!({
            "kind": "NATIVE_CONFIG_REQUIRED",
            "auth_method_id": auth_method_id,
        }))
        .into_response(),
        Ok(Err(BeginError::MethodNotDeclared)) => operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Authentication method is not declared by this agent module",
        ),
        Ok(Err(BeginError::Unsupported)) => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "No qualified native sign-in launch exists for this agent module",
        ),
        Ok(Err(BeginError::Storage)) => operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Native sign-in observation could not be recorded",
        ),
        Ok(Err(BeginError::LaunchFailed)) | Err(_) => operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "DEPENDENCY_UNAVAILABLE",
            "Native sign-in could not be started",
        ),
    }
}

fn rfc3339(epoch: i64) -> Option<String> {
    time::OffsetDateTime::from_unix_timestamp(epoch)
        .ok()?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}
