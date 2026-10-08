//! Read-only ZIP-intake capability status for the authenticated local Operator.
//!
//! This route intentionally does not invoke the Python parser. The parser has no
//! supervised isolation boundary with hard resource limits, so ZIP content remains an
//! opaque Resource until that boundary is qualified.

use super::*;

pub(super) fn routes() -> Router<ApiState> {
    Router::new().route("/v1/capabilities/zip-intake", get(zip_intake_status))
}

#[derive(Serialize)]
struct ZipIntakeStatus {
    capability_id: &'static str,
    status: &'static str,
    resource_behavior: &'static str,
    provider_integrated: bool,
    extraction_enabled: bool,
    reason_code: &'static str,
    reason: &'static str,
}

async fn zip_intake_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    let workspace_id = selected_workspace(&headers)?;
    ensure_workspace_owner(&state, &workspace_id)?;
    let binding = state
        .store
        .get_current_local_binding(LocalRuntimeWorkspaceBindingLookup {
            owner_principal_id: state.principal_id.clone(),
            workspace_id,
            runtime_id: state.runtime_id.clone(),
            runtime_incarnation_id: state.local_incarnation_id.clone(),
        })
        .map_err(|_| unavailable())?;
    if !binding.is_some_and(|binding| binding.status == "ACTIVE") {
        return Err(operator_error(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Local Runtime Workspace access is unavailable",
        ));
    }

    let mut response = Json(ZipIntakeStatus {
        capability_id: "litecowork.zip-intake",
        status: "UNAVAILABLE",
        resource_behavior: "OPAQUE_RESOURCE_ONLY",
        // The repository contains parser source, but the Runtime has no usable provider
        // until process isolation, transport and resource limits are qualified.
        provider_integrated: false,
        extraction_enabled: false,
        reason_code: "ISOLATED_WORKER_NOT_QUALIFIED",
        reason: "ZIP extraction is disabled until a supervised isolated worker enforces hard resource limits and passes platform qualification.",
    })
    .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "ZIP-intake status is unavailable",
    )
}
