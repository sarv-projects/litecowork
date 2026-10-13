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

fn valid_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::decode_runtime_lifecycle;
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
}
