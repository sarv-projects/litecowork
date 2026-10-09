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

    Ok(unavailable_zip_intake_response())
}

fn unavailable_zip_intake_response() -> Response {
    let mut response = Json(ZipIntakeStatus {
        capability_id: "litecowork.zip-intake",
        status: "UNAVAILABLE",
        resource_behavior: "OPAQUE_RESOURCE_ONLY",
        // A standalone worker is not a Runtime integration: exact Resource revision
        // resolution, owner authorization, cancellation, and platform qualification
        // are not part of this status route.
        provider_integrated: false,
        extraction_enabled: false,
        reason_code: "ISOLATED_WORKER_NOT_QUALIFIED",
        reason: "ZIP preview remains disabled until the isolated worker is integrated with authenticated exact-Resource resolution, bounded transport, and supported-platform qualification.",
    })
    .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

fn unavailable() -> Response {
    operator_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "DEPENDENCY_UNAVAILABLE",
        "ZIP-intake status is unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::unavailable_zip_intake_response;
    use axum::{
        body::to_bytes,
        http::{StatusCode, header},
    };
    use serde_json::Value;

    #[tokio::test]
    async fn readiness_remains_opaque_and_disabled_until_runtime_integration_is_qualified() {
        let response = unavailable_zip_intake_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("readiness body remains bounded");
        let body: Value = serde_json::from_slice(&body).expect("valid readiness JSON");
        assert_eq!(body["capability_id"], "litecowork.zip-intake");
        assert_eq!(body["status"], "UNAVAILABLE");
        assert_eq!(body["resource_behavior"], "OPAQUE_RESOURCE_ONLY");
        assert_eq!(body["provider_integrated"], false);
        assert_eq!(body["extraction_enabled"], false);
        assert_eq!(body["reason_code"], "ISOLATED_WORKER_NOT_QUALIFIED");
    }
}
