use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeLifecycleView {
    runtime_id: String,
    startup_policy: String,
    incarnation: RuntimeIncarnationView,
    availability: String,
    blockers: Vec<RuntimeLifecycleBlockerView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeIncarnationView {
    runtime_incarnation_id: String,
    runtime_id: String,
    process_started_at: String,
    litecowork_version: String,
    recovered_from_unclean_shutdown: bool,
    recovery_state: String,
    ready_at: Option<String>,
    stopped_at: Option<String>,
    version: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeIncarnationWire {
    runtime_incarnation_id: String,
    runtime_id: String,
    process_started_at: String,
    litecowork_version: String,
    recovered_from_unclean_shutdown: bool,
    recovery_state: String,
    ready_at: Option<String>,
    stopped_at: Option<String>,
    version: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RuntimeLifecycleBlockerView {
    blocker_id: String,
    code: String,
    safe_message: String,
    resolution_hint: String,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeLifecycleWire {
    runtime_id: String,
    startup_policy: String,
    incarnation: Option<RuntimeIncarnationWire>,
    availability: String,
    blockers: Vec<RuntimeLifecycleBlockerWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeLifecycleBlockerWire {
    blocker_id: String,
    code: String,
    safe_message: String,
    resolution_hint: String,
    created_at: String,
}

#[tauri::command]
pub(crate) async fn get_runtime_lifecycle(
    app: AppHandle,
    runtime_id: String,
    incarnation_id: String,
) -> Result<RuntimeLifecycleView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_identity(&runtime_id) || !valid_identity(&incarnation_id) {
            return Err("Runtime lifecycle selection is invalid".to_owned());
        }
        let (client, _) =
            super::validated_operator_connection(&app, Some(&runtime_id), Some(&incarnation_id))?;
        let response = client
            .get("/v1/runtime-lifecycle".to_owned())
            .send()
            .map_err(|_| "Local Runtime lifecycle is unavailable".to_owned())?;
        if !response.is_success() {
            return Err("Local Runtime lifecycle is unavailable".to_owned());
        }
        let body = super::read_bounded_response(response)?;
        decode_runtime_lifecycle(&body, &runtime_id, &incarnation_id)
    })
    .await
    .map_err(|_| "Runtime lifecycle request did not complete".to_owned())?
}

fn decode_runtime_lifecycle(
    body: &[u8],
    expected_runtime_id: &str,
    expected_incarnation_id: &str,
) -> Result<RuntimeLifecycleView, String> {
    let wire: RuntimeLifecycleWire = serde_json::from_slice(body)
        .map_err(|_| "Local Runtime returned an unsupported lifecycle response".to_owned())?;
    let incarnation = wire
        .incarnation
        .ok_or_else(|| "Local Runtime returned no current incarnation".to_owned())?;
    if wire.runtime_id != expected_runtime_id
        || incarnation.runtime_id != expected_runtime_id
        || incarnation.runtime_incarnation_id != expected_incarnation_id
        || !matches!(
            wire.startup_policy.as_str(),
            "MANUAL" | "LOGIN_BACKGROUND" | "ALWAYS_ON_SERVICE"
        )
        || !matches!(
            wire.availability.as_str(),
            "PAIRING"
                | "STARTING"
                | "RECOVERING"
                | "ONLINE"
                | "DEGRADED"
                | "DRAINING"
                | "OFFLINE"
                | "REVOKED"
        )
        || !matches!(
            incarnation.recovery_state.as_str(),
            "STARTING" | "RECOVERING" | "READY" | "DEGRADED" | "DRAINING" | "STOPPING" | "STOPPED"
        )
        || incarnation.version == 0
        || incarnation.litecowork_version.trim().is_empty()
        || !valid_timestamp(&incarnation.process_started_at)
        || incarnation
            .ready_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
        || incarnation
            .stopped_at
            .as_deref()
            .is_some_and(|value| !valid_timestamp(value))
    {
        return Err("Local Runtime returned an invalid lifecycle response".to_owned());
    }

    let blockers = wire
        .blockers
        .into_iter()
        .map(|blocker| {
            if blocker.blocker_id.is_empty()
                || blocker.blocker_id.len() > 256
                || !matches!(
                    blocker.code.as_str(),
                    "DEPENDENCY_UNAVAILABLE" | "RUNTIME_UNAVAILABLE"
                )
                || blocker.safe_message.trim().is_empty()
                || blocker.safe_message.len() > 1024
                || blocker.resolution_hint.trim().is_empty()
                || blocker.resolution_hint.len() > 1024
                || !valid_timestamp(&blocker.created_at)
            {
                return Err("Local Runtime returned an invalid lifecycle blocker".to_owned());
            }
            Ok(RuntimeLifecycleBlockerView {
                blocker_id: blocker.blocker_id,
                code: blocker.code,
                safe_message: blocker.safe_message,
                resolution_hint: blocker.resolution_hint,
                created_at: blocker.created_at,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RuntimeLifecycleView {
        runtime_id: wire.runtime_id,
        startup_policy: wire.startup_policy,
        incarnation: RuntimeIncarnationView {
            runtime_incarnation_id: incarnation.runtime_incarnation_id,
            runtime_id: incarnation.runtime_id,
            process_started_at: incarnation.process_started_at,
            litecowork_version: incarnation.litecowork_version,
            recovered_from_unclean_shutdown: incarnation.recovered_from_unclean_shutdown,
            recovery_state: incarnation.recovery_state,
            ready_at: incarnation.ready_at,
            stopped_at: incarnation.stopped_at,
            version: incarnation.version,
        },
        availability: wire.availability,
        blockers,
    })
}

fn valid_timestamp(value: &str) -> bool {
    value.len() <= 128
        && value.split_once('T').is_some_and(|(_, time)| {
            time.ends_with('Z') || time.contains('+') || time.contains('-')
        })
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

// ---------------------------------------------------------------------------
// Stop preview and update quiescence gate (011-T14). Read-only: this module never
// requests a stop. `requestLocalRuntimeStop` is not mounted by the daemon (011-T13).
// ---------------------------------------------------------------------------

const MAX_STOP_IDS_PER_CATEGORY: usize = 200;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeStopPreviewView {
    runtime_id: String,
    runtime_incarnation_id: String,
    observed_at: String,
    admitted_turn_ids: Vec<String>,
    admitted_attempt_ids: Vec<String>,
    unresolved_effect_ids: Vec<String>,
    execution_lease_ids: Vec<String>,
    environment_control_lease_ids: Vec<String>,
    provider_reference_ids: Vec<String>,
    pending_cleanup_ids: Vec<String>,
    verdict: String,
    blockers: Vec<RuntimeLifecycleBlockerView>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeStopPreviewWire {
    runtime_id: String,
    runtime_incarnation_id: String,
    observed_at: String,
    admitted_turn_ids: Vec<String>,
    admitted_attempt_ids: Vec<String>,
    unresolved_effect_ids: Vec<String>,
    execution_lease_ids: Vec<String>,
    environment_control_lease_ids: Vec<String>,
    provider_reference_ids: Vec<String>,
    pending_cleanup_ids: Vec<String>,
    verdict: String,
    dependent_task_ids: Vec<String>,
    dependent_automation_ids: Vec<String>,
    workspace_root_ids: Vec<String>,
    shared_service_refs: Vec<String>,
    eligible_handoff_task_ids: Vec<String>,
    blockers: Vec<RuntimeLifecycleBlockerWire>,
}

/// Result of the update quiescence gate. `PositiveQuiescence` is the only value that may
/// let an update proceed; every other outcome, including an unreachable Operator, refuses.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum UpdateQuiescenceView {
    PositiveQuiescence,
    NotQuiescent {
        reasons: Vec<UpdateQuiescenceReason>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct UpdateQuiescenceReason {
    label: String,
    status: String,
}

fn fetch_stop_preview(
    app: &AppHandle,
    runtime_id: &str,
    incarnation_id: &str,
) -> Result<RuntimeStopPreviewView, String> {
    if !valid_identity(runtime_id) || !valid_identity(incarnation_id) {
        return Err("Runtime stop preview selection is invalid".to_owned());
    }
    let (client, _) =
        super::validated_operator_connection(app, Some(runtime_id), Some(incarnation_id))?;
    let response = client
        .get("/v1/runtime-lifecycle/stop-preview".to_owned())
        .send()
        .map_err(|_| "Local Runtime stop preview is unavailable".to_owned())?;
    if !response.is_success() {
        return Err("Local Runtime stop preview is unavailable".to_owned());
    }
    let body = super::read_bounded_response(response)?;
    decode_runtime_stop_preview(&body, runtime_id, incarnation_id)
}

#[tauri::command]
pub(crate) async fn get_runtime_stop_preview(
    app: AppHandle,
    runtime_id: String,
    incarnation_id: String,
) -> Result<RuntimeStopPreviewView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        fetch_stop_preview(&app, &runtime_id, &incarnation_id)
    })
    .await
    .map_err(|_| "Runtime stop preview request did not complete".to_owned())?
}

/// Feature 019 `install_update` calls this before any download-to-install step.
#[tauri::command]
pub(crate) async fn check_update_quiescence(
    app: AppHandle,
    runtime_id: String,
    incarnation_id: String,
) -> Result<UpdateQuiescenceView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // An unreachable Operator or an invalid response is an Err the caller must treat as
        // QuiescenceUnavailable, never as permission to update.
        let preview = fetch_stop_preview(&app, &runtime_id, &incarnation_id)?;
        Ok(update_quiescence_from_preview(&preview))
    })
    .await
    .map_err(|_| "Update quiescence check did not complete".to_owned())?
}

fn update_quiescence_from_preview(preview: &RuntimeStopPreviewView) -> UpdateQuiescenceView {
    if preview.verdict == "QUIESCENT" {
        return UpdateQuiescenceView::PositiveQuiescence;
    }
    let status = if preview.verdict == "BLOCKED" {
        "running"
    } else {
        "unknown"
    };
    let reasons = preview
        .blockers
        .iter()
        .map(|blocker| UpdateQuiescenceReason {
            label: blocker.safe_message.clone(),
            status: status.to_owned(),
        })
        .collect();
    UpdateQuiescenceView::NotQuiescent { reasons }
}

fn stop_ids_valid(ids: &[String]) -> bool {
    ids.len() <= MAX_STOP_IDS_PER_CATEGORY && ids.iter().all(|id| valid_identity(id))
}

fn decode_runtime_stop_preview(
    body: &[u8],
    expected_runtime_id: &str,
    expected_incarnation_id: &str,
) -> Result<RuntimeStopPreviewView, String> {
    let wire: RuntimeStopPreviewWire = serde_json::from_slice(body)
        .map_err(|_| "Local Runtime returned an unsupported stop preview".to_owned())?;
    let invalid = || "Local Runtime returned an invalid stop preview".to_owned();
    if wire.runtime_id != expected_runtime_id
        || wire.runtime_incarnation_id != expected_incarnation_id
        || !valid_timestamp(&wire.observed_at)
        || !matches!(wire.verdict.as_str(), "QUIESCENT" | "BLOCKED" | "UNKNOWN")
        || ![
            &wire.admitted_turn_ids,
            &wire.admitted_attempt_ids,
            &wire.unresolved_effect_ids,
            &wire.execution_lease_ids,
            &wire.environment_control_lease_ids,
            &wire.provider_reference_ids,
            &wire.pending_cleanup_ids,
            &wire.dependent_task_ids,
            &wire.dependent_automation_ids,
            &wire.workspace_root_ids,
            &wire.shared_service_refs,
            &wire.eligible_handoff_task_ids,
        ]
        .into_iter()
        .all(|ids| stop_ids_valid(ids))
        || wire.blockers.len() > 32
    {
        return Err(invalid());
    }
    let any_unsettled = !wire.admitted_turn_ids.is_empty()
        || !wire.admitted_attempt_ids.is_empty()
        || !wire.unresolved_effect_ids.is_empty()
        || !wire.execution_lease_ids.is_empty()
        || !wire.environment_control_lease_ids.is_empty()
        || !wire.provider_reference_ids.is_empty()
        || !wire.pending_cleanup_ids.is_empty();
    // Fail closed on a contradictory wire: QUIESCENT with listed work or any blocker.
    if wire.verdict == "QUIESCENT" && (any_unsettled || !wire.blockers.is_empty()) {
        return Err(invalid());
    }
    if any_unsettled && wire.verdict != "BLOCKED" {
        return Err(invalid());
    }
    let blockers = wire
        .blockers
        .into_iter()
        .map(|blocker| {
            if blocker.blocker_id.is_empty()
                || blocker.blocker_id.len() > 256
                || !matches!(
                    blocker.code.as_str(),
                    "RUNTIME_STOP_BLOCKED"
                        | "RUNTIME_STOP_UNSETTLED"
                        | "RUNTIME_QUIESCENCE_UNKNOWN"
                )
                || blocker.safe_message.trim().is_empty()
                || blocker.safe_message.len() > 1024
                || blocker.resolution_hint.trim().is_empty()
                || blocker.resolution_hint.len() > 1024
                || !valid_timestamp(&blocker.created_at)
            {
                return Err("Local Runtime returned an invalid stop blocker".to_owned());
            }
            Ok(RuntimeLifecycleBlockerView {
                blocker_id: blocker.blocker_id,
                code: blocker.code,
                safe_message: blocker.safe_message,
                resolution_hint: blocker.resolution_hint,
                created_at: blocker.created_at,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeStopPreviewView {
        runtime_id: wire.runtime_id,
        runtime_incarnation_id: wire.runtime_incarnation_id,
        observed_at: wire.observed_at,
        admitted_turn_ids: wire.admitted_turn_ids,
        admitted_attempt_ids: wire.admitted_attempt_ids,
        unresolved_effect_ids: wire.unresolved_effect_ids,
        execution_lease_ids: wire.execution_lease_ids,
        environment_control_lease_ids: wire.environment_control_lease_ids,
        provider_reference_ids: wire.provider_reference_ids,
        pending_cleanup_ids: wire.pending_cleanup_ids,
        verdict: wire.verdict,
        blockers,
    })
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::{
        UpdateQuiescenceView, decode_runtime_lifecycle, decode_runtime_stop_preview,
        update_quiescence_from_preview,
    };
    use serde_json::json;

    fn lifecycle() -> serde_json::Value {
        json!({
            "runtime_id": "runtime-1",
            "startup_policy": "MANUAL",
            "incarnation": {
                "runtime_incarnation_id": "incarnation-2",
                "runtime_id": "runtime-1",
                "process_started_at": "2026-10-11T09:00:00Z",
                "litecowork_version": "0.1.0-test",
                "recovered_from_unclean_shutdown": true,
                "recovery_state": "DEGRADED",
                "ready_at": null,
                "stopped_at": null,
                "version": 2
            },
            "availability": "DEGRADED",
            "blockers": [{
                "blocker_id": "local-runtime:runtime-1:blocker:1",
                "code": "DEPENDENCY_UNAVAILABLE",
                "safe_message": "Task execution admission is unavailable.",
                "resolution_hint": "Wait for local execution services.",
                "created_at": "2026-10-11T09:01:00Z"
            }]
        })
    }

    #[test]
    fn decoder_preserves_degraded_lifecycle_and_safe_blockers() {
        let body = serde_json::to_vec(&lifecycle()).expect("serialize lifecycle fixture");
        let view = decode_runtime_lifecycle(&body, "runtime-1", "incarnation-2")
            .expect("current lifecycle decodes");
        let result = serde_json::to_value(view).expect("serialize Tauri lifecycle view");
        assert_eq!(result["availability"], "DEGRADED");
        assert_eq!(result["incarnation"]["recoveryState"], "DEGRADED");
        assert_eq!(
            result["blockers"][0]["safeMessage"],
            "Task execution admission is unavailable."
        );
        assert!(result.get("dependentTasks").is_none());
    }

    #[test]
    fn decoder_rejects_foreign_or_superseded_runtime_identity() {
        let body = serde_json::to_vec(&lifecycle()).expect("serialize lifecycle fixture");
        assert!(decode_runtime_lifecycle(&body, "runtime-other", "incarnation-2").is_err());
        assert!(decode_runtime_lifecycle(&body, "runtime-1", "incarnation-old").is_err());
    }

    #[test]
    fn decoder_rejects_private_unknown_fields_and_unsupported_blockers() {
        let mut value = lifecycle();
        value["os_boot_id"] = json!("private");
        let body = serde_json::to_vec(&value).expect("serialize lifecycle fixture");
        assert!(decode_runtime_lifecycle(&body, "runtime-1", "incarnation-2").is_err());

        let mut value = lifecycle();
        value["blockers"][0]["code"] = json!("UNKNOWN_PRIVATE_BLOCKER");
        let body = serde_json::to_vec(&value).expect("serialize lifecycle fixture");
        assert!(decode_runtime_lifecycle(&body, "runtime-1", "incarnation-2").is_err());
    }

    #[test]
    fn decoder_requires_a_current_incarnation_and_rejects_unknown_nested_fields() {
        let mut value = lifecycle();
        value["incarnation"] = serde_json::Value::Null;
        let body = serde_json::to_vec(&value).expect("serialize lifecycle fixture");
        assert!(decode_runtime_lifecycle(&body, "runtime-1", "incarnation-2").is_err());

        let mut value = lifecycle();
        value["incarnation"]["local_principal_id"] = json!("private");
        let body = serde_json::to_vec(&value).expect("serialize lifecycle fixture");
        assert!(decode_runtime_lifecycle(&body, "runtime-1", "incarnation-2").is_err());
    }

    fn stop_preview() -> serde_json::Value {
        json!({
            "runtime_id": "runtime-1",
            "runtime_incarnation_id": "incarnation-2",
            "observed_at": "2026-11-10T00:00:00Z",
            "admitted_turn_ids": [],
            "admitted_attempt_ids": [],
            "unresolved_effect_ids": [],
            "execution_lease_ids": [],
            "environment_control_lease_ids": [],
            "provider_reference_ids": [],
            "pending_cleanup_ids": [],
            "verdict": "UNKNOWN",
            "dependent_task_ids": [],
            "dependent_automation_ids": [],
            "workspace_root_ids": [],
            "shared_service_refs": [],
            "eligible_handoff_task_ids": [],
            "blockers": [{
                "blocker_id": "local-runtime:runtime-1:stop:1",
                "code": "RUNTIME_QUIESCENCE_UNKNOWN",
                "safe_message": "Live sessions cannot be observed yet.",
                "resolution_hint": "Do not update automatically.",
                "created_at": "2026-11-10T00:00:00Z"
            }]
        })
    }

    #[test]
    fn stop_preview_decodes_and_unknown_refuses_the_update_gate() {
        let body = serde_json::to_vec(&stop_preview()).expect("serialize stop preview");
        let view = decode_runtime_stop_preview(&body, "runtime-1", "incarnation-2")
            .expect("valid preview decodes");
        match update_quiescence_from_preview(&view) {
            UpdateQuiescenceView::NotQuiescent { reasons } => {
                assert_eq!(reasons.len(), 1);
                assert_eq!(reasons[0].status, "unknown");
            }
            UpdateQuiescenceView::PositiveQuiescence => panic!("UNKNOWN must not allow an update"),
        }
    }

    #[test]
    fn only_a_clean_quiescent_preview_opens_the_gate() {
        let mut value = stop_preview();
        value["verdict"] = json!("QUIESCENT");
        value["blockers"] = json!([]);
        let body = serde_json::to_vec(&value).expect("serialize");
        let view = decode_runtime_stop_preview(&body, "runtime-1", "incarnation-2")
            .expect("quiescent preview decodes");
        assert!(matches!(
            update_quiescence_from_preview(&view),
            UpdateQuiescenceView::PositiveQuiescence
        ));
    }

    #[test]
    fn contradictory_foreign_or_private_stop_previews_are_rejected() {
        let mut contradictory = stop_preview();
        contradictory["verdict"] = json!("QUIESCENT");
        contradictory["admitted_turn_ids"] = json!(["turn-1"]);
        contradictory["blockers"] = json!([]);
        let body = serde_json::to_vec(&contradictory).expect("serialize");
        assert!(decode_runtime_stop_preview(&body, "runtime-1", "incarnation-2").is_err());

        let mut listed_but_unknown = stop_preview();
        listed_but_unknown["admitted_turn_ids"] = json!(["turn-1"]);
        let body = serde_json::to_vec(&listed_but_unknown).expect("serialize");
        assert!(decode_runtime_stop_preview(&body, "runtime-1", "incarnation-2").is_err());

        let body = serde_json::to_vec(&stop_preview()).expect("serialize");
        assert!(decode_runtime_stop_preview(&body, "runtime-1", "incarnation-old").is_err());
        assert!(decode_runtime_stop_preview(&body, "runtime-other", "incarnation-2").is_err());

        let mut private = stop_preview();
        private["os_boot_id"] = json!("private");
        let body = serde_json::to_vec(&private).expect("serialize");
        assert!(decode_runtime_stop_preview(&body, "runtime-1", "incarnation-2").is_err());

        let mut bad_code = stop_preview();
        bad_code["blockers"][0]["code"] = json!("SOMETHING_ELSE");
        let body = serde_json::to_vec(&bad_code).expect("serialize");
        assert!(decode_runtime_stop_preview(&body, "runtime-1", "incarnation-2").is_err());
    }
}
