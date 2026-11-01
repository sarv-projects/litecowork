use super::{parse_workspace_root_scan_response, workspace_root_scan_operator_error};
use serde_json::{json, Value};

fn valid_response() -> Value {
    json!({
        "workspace_id": "workspace-1",
        "workspace_root_id": "root-1",
        "request_id": "request-1",
        "root_version": 7,
        "status": "COMPLETE",
        "entries_examined": 12,
        "files_observed": 8,
        "partial_reason": null,
        "completed_at": "2026-10-16T12:34:56Z"
    })
}

fn parse(value: Value) -> Result<super::WorkspaceRootScanView, String> {
    parse_workspace_root_scan_response(
        &serde_json::to_vec(&value).expect("test response serializes"),
        "workspace-1",
        "root-1",
        "request-1",
        7,
    )
}

#[test]
fn accepts_valid_complete_scan_projection_and_serializes_camel_case() {
    let view = parse(valid_response()).expect("valid scan summary is accepted");
    let projection = serde_json::to_value(view).expect("Tauri view serializes");
    assert_eq!(projection["workspaceId"], "workspace-1");
    assert_eq!(projection["workspaceRootId"], "root-1");
    assert_eq!(projection["requestId"], "request-1");
    assert_eq!(projection["rootVersion"], 7);
    assert_eq!(projection["status"], "COMPLETE");
}

#[test]
fn rejects_malformed_counts_timestamp_and_partial_reason() {
    let mut too_many_files = valid_response();
    too_many_files["files_observed"] = json!(13);
    assert!(parse(too_many_files).is_err());

    let mut invalid_timestamp = valid_response();
    invalid_timestamp["completed_at"] = json!("yesterday");
    assert!(parse(invalid_timestamp).is_err());

    let mut missing_partial_reason = valid_response();
    missing_partial_reason["status"] = json!("PARTIAL");
    assert!(parse(missing_partial_reason).is_err());

    let mut unknown_metadata = valid_response();
    unknown_metadata["private_locator"] = json!("/private/path");
    assert!(parse(unknown_metadata).is_err());

    let mut too_many_entries = valid_response();
    too_many_entries["entries_examined"] = json!(1_001);
    too_many_entries["files_observed"] = json!(1_000);
    assert!(parse(too_many_entries).is_err());
}

#[test]
fn accepts_partial_scan_with_a_reason_and_rfc3339_offset() {
    let mut partial = valid_response();
    partial["status"] = json!("PARTIAL");
    partial["partial_reason"] = json!("SCAN_BUDGET_REACHED");
    partial["completed_at"] = json!("2026-10-16T12:34:56.25+02:30");
    let view = parse(partial).expect("valid partial scan summary is accepted");
    let projection = serde_json::to_value(view).expect("Tauri view serializes");
    assert_eq!(projection["status"], "PARTIAL");
    assert_eq!(projection["partialReason"], "SCAN_BUDGET_REACHED");
}

#[test]
fn accepts_only_the_partial_reasons_emitted_by_the_runtime() {
    for reason in [
        "TIME_LIMIT_REACHED",
        "SCAN_BUDGET_REACHED",
        "ENTRY_CHANGED_OR_UNREADABLE",
        "MOUNT_BOUNDARY_REACHED",
    ] {
        let mut partial = valid_response();
        partial["status"] = json!("PARTIAL");
        partial["partial_reason"] = json!(reason);
        assert!(parse(partial).is_ok(), "rejected emitted reason {reason:?}");
    }
}

#[test]
fn rejects_unrecognized_partial_reason_codes_that_could_contain_private_data() {
    for reason in ["ENTRY_LIMIT_REACHED", "/private/folder/name", "custom reason"] {
        let mut partial = valid_response();
        partial["status"] = json!("PARTIAL");
        partial["partial_reason"] = json!(reason);
        assert!(parse(partial).is_err(), "accepted partial reason {reason:?}");
    }
}

#[test]
fn maps_known_scan_operator_errors_to_stable_markers() {
    assert_eq!(
        workspace_root_scan_operator_error(
            "Folder state changed; refresh before scanning [STALE_WORKSPACE_VERSION; retryable=false; reference=cor_123]".to_owned()
        ),
        "STALE_WORKSPACE_VERSION"
    );
    assert_eq!(
        workspace_root_scan_operator_error(
            "Metadata scanning is not available on this platform [SCAN_UNSUPPORTED_PLATFORM; retryable=false; reference=cor_123]".to_owned()
        ),
        "SCAN_UNSUPPORTED_PLATFORM"
    );
    assert_eq!(
        workspace_root_scan_operator_error(
            "Folder scan could not be completed [INTERNAL; retryable=true; reference=cor_123]".to_owned()
        ),
        "INTERNAL"
    );
}

#[test]
fn rejects_projection_for_a_different_root_or_workspace() {
    let mut different_root = valid_response();
    different_root["workspace_root_id"] = json!("root-2");
    assert!(parse(different_root).is_err());

    let mut different_workspace = valid_response();
    different_workspace["workspace_id"] = json!("workspace-2");
    assert!(parse(different_workspace).is_err());
}

#[test]
fn rejects_projection_for_a_different_request_id_or_root_version() {
    let mut different_request = valid_response();
    different_request["request_id"] = json!("request-2");
    assert!(parse(different_request).is_err());

    let mut different_version = valid_response();
    different_version["root_version"] = json!(8);
    assert!(parse(different_version).is_err());
}

#[test]
fn rejects_unknown_scan_status_and_inconsistent_complete_reason() {
    let mut unknown_status = valid_response();
    unknown_status["status"] = json!("FAILED");
    assert!(parse(unknown_status).is_err());

    let mut complete_with_reason = valid_response();
    complete_with_reason["partial_reason"] = json!("LIMIT_REACHED");
    assert!(parse(complete_with_reason).is_err());
}
