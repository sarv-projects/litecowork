use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use domain_workspace::{ScanWorkspaceRoot, WorkspaceRootService};
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[cfg(target_os = "linux")]
use storage_core::WorkspaceRootFileObservation;
use storage_core::{
    RootObservationFreshness, RootObservationSearchRequest, WorkspaceRootObservationStore,
};
use storage_core::{
    StateStore, WorkspaceRootRevalidationBindings, WorkspaceRootScanResult, WorkspaceRootScanStatus,
};

#[cfg(target_os = "linux")]
#[derive(Debug)]
enum ScanBlockingError {
    Directory(crate::local_filesystem::LocalDirectoryError),
    DeadlineExceeded,
    IdentityUnavailable,
    RootChanged,
}

#[cfg(target_os = "linux")]
static WORKSPACE_ROOT_SCAN_ADMISSION: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(2)));

#[cfg(target_os = "linux")]
const MAX_SCAN_TIME: Duration = Duration::from_secs(8);
#[cfg(target_os = "linux")]
// Reserve part of the total cooperative limit so TIME_LIMIT partials can still receive
// the mandatory saved-path identity fence before a summary is prepared.
const MAX_FINAL_IDENTITY_FENCE_TIME: Duration = Duration::from_secs(1);

pub(super) fn routes() -> Router<super::ApiState> {
    Router::new()
        .route(
            "/v1/workspace-roots/{workspace_root_id}/scan",
            post(scan_workspace_root),
        )
        .route(
            "/v1/workspace-roots/{workspace_root_id}/observations",
            get(search_workspace_root_observations),
        )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootObservationQuery {
    q: Option<String>,
    freshness: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootObservationCursor {
    version: u32,
    principal_id: String,
    workspace_id: String,
    workspace_root_id: String,
    expected_root_version: u64,
    runtime_id: String,
    runtime_incarnation_id: String,
    query: String,
    freshness: Option<String>,
    limit: usize,
    after_display_name: String,
    after_observation_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedRootObservationCursor {
    cursor: RootObservationCursor,
    integrity_tag: String,
}

#[derive(Serialize)]
struct RootObservationItem {
    observation_id: String,
    display_name: String,
    size_bytes: u64,
    modified_at: Option<String>,
    observed_at: String,
    freshness: &'static str,
    /// Null until canonical Resource aggregates are explicitly produced and linked.
    resource_id: Option<String>,
}

#[derive(Serialize)]
struct RootObservationPage {
    workspace_id: String,
    workspace_root_id: String,
    items: Vec<RootObservationItem>,
    next_cursor: Option<String>,
}

async fn search_workspace_root_observations(
    State(state): State<super::ApiState>,
    headers: HeaderMap,
    Path(workspace_root_id): Path<String>,
    query: Result<Query<RootObservationQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, Response> {
    let workspace_id = super::selected_workspace(&headers)?;
    let expected_root_version = super::parse_if_match(&headers)?;
    let Query(query) = query.map_err(|_| observation_invalid())?;
    let limit = query.limit.unwrap_or(50);
    let search_text = query.q.unwrap_or_default();
    let freshness = query
        .freshness
        .as_deref()
        .map(parse_observation_freshness)
        .transpose()?;
    if workspace_root_id.trim().is_empty()
        || workspace_root_id.len() > 160
        || search_text.len() > 256
        || search_text.contains('\0')
        || !(1..=100).contains(&limit)
    {
        return Err(observation_invalid());
    }
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| {
            decode_root_observation_cursor(
                cursor,
                &state,
                &workspace_id,
                &workspace_root_id,
                expected_root_version,
                &search_text,
                freshness,
                limit,
            )
        })
        .transpose()?;
    let request = RootObservationSearchRequest {
        owner_principal_id: state.principal_id.clone(),
        workspace_id: workspace_id.clone(),
        workspace_root_id: workspace_root_id.clone(),
        expected_root_version,
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
        query: search_text.clone(),
        freshness,
        after_display_name: after
            .as_ref()
            .map(|cursor| cursor.after_display_name.clone()),
        after_observation_id: after
            .as_ref()
            .map(|cursor| cursor.after_observation_id.clone()),
        limit,
    };
    let mut records = state
        .store
        .search_workspace_root_observations(request)
        .map_err(map_observation_store_error)?;
    let has_more = records.len() > limit;
    records.truncate(limit);
    let next_cursor = if has_more {
        records
            .last()
            .map(|record| {
                encode_root_observation_cursor(
                    &state,
                    RootObservationCursor {
                        version: 1,
                        principal_id: state.principal_id.clone(),
                        workspace_id: workspace_id.clone(),
                        workspace_root_id: workspace_root_id.clone(),
                        expected_root_version,
                        runtime_id: state.runtime_id.clone(),
                        runtime_incarnation_id: state.local_incarnation_id.clone(),
                        query: search_text,
                        freshness: freshness.map(|value| value.as_str().to_owned()),
                        limit,
                        after_display_name: record.display_name.clone(),
                        after_observation_id: record.observation_id.clone(),
                    },
                )
            })
            .transpose()?
    } else {
        None
    };
    let items = records
        .into_iter()
        .map(|record| RootObservationItem {
            observation_id: record.observation_id,
            display_name: record.display_name,
            size_bytes: record.size_bytes,
            modified_at: record.modified_at,
            observed_at: record.observed_at,
            freshness: record.freshness.as_str(),
            resource_id: None,
        })
        .collect();
    let mut response = Json(RootObservationPage {
        workspace_id,
        workspace_root_id,
        items,
        next_cursor,
    })
    .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

fn parse_observation_freshness(value: &str) -> Result<RootObservationFreshness, Response> {
    match value {
        "CURRENT" => Ok(RootObservationFreshness::Current),
        "STALE" => Ok(RootObservationFreshness::Stale),
        "UNKNOWN" => Ok(RootObservationFreshness::Unknown),
        _ => Err(observation_invalid()),
    }
}

fn decode_root_observation_cursor(
    encoded: &str,
    state: &super::ApiState,
    workspace_id: &str,
    workspace_root_id: &str,
    expected_root_version: u64,
    query: &str,
    freshness: Option<RootObservationFreshness>,
    limit: usize,
) -> Result<RootObservationCursor, Response> {
    if encoded.is_empty() || encoded.len() > 4096 {
        return Err(observation_invalid());
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| observation_invalid())?;
    let signed: SignedRootObservationCursor =
        serde_json::from_slice(&bytes).map_err(|_| observation_invalid())?;
    let expected_tag = root_observation_cursor_integrity_tag(state, &signed.cursor)?;
    if !cursor_tags_equal(&signed.integrity_tag, &expected_tag) {
        return Err(observation_invalid());
    }
    let cursor = signed.cursor;
    if cursor.version != 1
        || cursor.principal_id != state.principal_id
        || cursor.workspace_id != workspace_id
        || cursor.workspace_root_id != workspace_root_id
        || cursor.expected_root_version != expected_root_version
        || cursor.runtime_id != state.runtime_id
        || cursor.runtime_incarnation_id != state.local_incarnation_id
        || cursor.query != query
        || cursor.freshness != freshness.map(|value| value.as_str().to_owned())
        || cursor.limit != limit
        || cursor.after_display_name.is_empty()
        || cursor.after_display_name.len() > 1024
        || cursor.after_observation_id.len() != 71
        || !cursor.after_observation_id.starts_with("sha256:")
        || cursor.after_observation_id[7..]
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(observation_invalid());
    }
    Ok(cursor)
}

fn cursor_tags_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn encode_root_observation_cursor(
    state: &super::ApiState,
    cursor: RootObservationCursor,
) -> Result<String, Response> {
    let signed = SignedRootObservationCursor {
        integrity_tag: root_observation_cursor_integrity_tag(state, &cursor)?,
        cursor,
    };
    serde_json::to_vec(&signed)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| {
            super::operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Folder search could not be completed",
            )
        })
}

fn root_observation_cursor_integrity_tag(
    state: &super::ApiState,
    cursor: &RootObservationCursor,
) -> Result<String, Response> {
    let payload = serde_json::to_vec(cursor).map_err(|_| {
        super::operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Folder search could not be completed",
        )
    })?;
    Ok(state.runtime_identity.keyed_pseudonym(
        "litecowork.operator.workspace-root-observation-cursor.v1",
        &payload,
    ))
}

fn observation_invalid() -> Response {
    super::operator_error(
        StatusCode::BAD_REQUEST,
        "INVALID_ARGUMENT",
        "Folder search request is invalid",
    )
}

fn map_observation_store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::NotFound => super::operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "WorkspaceRoot is unavailable",
        ),
        storage_core::StoreError::Invalid(_) => observation_invalid(),
        storage_core::StoreError::Conflict { .. } => super::operator_error(
            StatusCode::CONFLICT,
            "STALE_WORKSPACE_VERSION",
            "Folder state changed; refresh before searching",
        ),
        storage_core::StoreError::Busy => super::operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "BUSY",
            "Folder search is temporarily unavailable",
        ),
        _ => super::operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Folder search could not be completed",
        ),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyScanRequest {}

async fn scan_workspace_root(
    State(state): State<super::ApiState>,
    headers: HeaderMap,
    Path(workspace_root_id): Path<String>,
    Json(_body): Json<EmptyScanRequest>,
) -> Result<Response, Response> {
    let workspace_id = super::selected_workspace(&headers)?;
    if workspace_root_id.trim().is_empty() || workspace_root_id.len() > 160 {
        return Err(super::operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "WorkspaceRoot selection is invalid",
        ));
    }
    let workspace = map_workspace_preflight_result(state.store.get_workspace(&workspace_id))?;
    if !workspace.is_some_and(|workspace| workspace.owner_principal_id == state.principal_id) {
        return Err(super::operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Workspace is unavailable",
        ));
    }
    let expected_version = super::parse_if_match(&headers)?;
    let request_id = super::idempotency_key(&headers)?;
    let command = ScanWorkspaceRoot {
        workspace_id: workspace_id.clone(),
        workspace_root_id: workspace_root_id.clone(),
        principal_id: state.principal_id.clone(),
        request_id,
        expected_version,
        runtime_id: state.runtime_id.clone(),
        runtime_incarnation_id: state.local_incarnation_id.clone(),
    };
    let service = WorkspaceRootService::new(state.store.clone());
    if let Some(result) = service
        .get_scan_receipt(&command)
        .map_err(map_store_error)?
    {
        return Ok(no_store_json(result));
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (state, service, command);
        Err(super::operator_error(
            StatusCode::NOT_IMPLEMENTED,
            "SCAN_UNSUPPORTED_PLATFORM",
            "Metadata scanning is not available on this platform",
        ))
    }

    #[cfg(target_os = "linux")]
    {
        scan_linux_root(state, service, command).await
    }
}

#[cfg(target_os = "linux")]
async fn scan_linux_root(
    state: super::ApiState,
    service: WorkspaceRootService<storage_sqlite::SqliteWorkspaceStore>,
    command: ScanWorkspaceRoot,
) -> Result<Response, Response> {
    use crate::local_filesystem::{
        LocalDirectoryError, LocalFileScanError, LocalFileScanLimits, LocalFileScanSummary,
        RuntimeIdentityKey, reopen_saved_directory_with_cancel, visit_regular_files_with_progress,
    };
    use std::time::Instant;

    let candidate = service
        .get_scan_candidate(&command)
        .map_err(map_store_error)?
        .ok_or_else(|| {
            super::operator_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "WorkspaceRoot is unavailable",
            )
        })?;
    if candidate.root.workspace_id != command.workspace_id
        || candidate.root.workspace_root_id != command.workspace_root_id
        || candidate.root.version != command.expected_version
        || candidate.root.status != "ACTIVE"
        || candidate.location.availability != "AVAILABLE"
        || candidate.location.runtime_id != command.runtime_id
        || candidate.location.location_id != candidate.root.location_id
        || candidate.resource.resource_id != candidate.root.resource_id
        || candidate.resource.workspace_id != command.workspace_id
        || candidate
            .resource
            .provider_identity
            .get("provider_instance_id")
            .and_then(serde_json::Value::as_str)
            != Some("litecowork.local_filesystem")
    {
        return Err(super::operator_error(
            StatusCode::CONFLICT,
            "STALE_WORKSPACE_VERSION",
            "Folder state changed; refresh before scanning",
        ));
    }
    let (locator_binding, file_identity_binding) = match candidate.bindings {
        WorkspaceRootRevalidationBindings::CurrentIncarnation {
            locator,
            file_identity,
        } if locator.runtime_id == command.runtime_id
            && locator.runtime_incarnation_id == command.runtime_incarnation_id
            && file_identity.runtime_id == command.runtime_id
            && file_identity.runtime_incarnation_id == command.runtime_incarnation_id =>
        {
            (locator, file_identity)
        }
        _ => {
            return Err(super::operator_error(
                StatusCode::CONFLICT,
                "RESOURCE_IDENTITY_UNAVAILABLE",
                "The saved folder could not be verified. Check the folder or remove and add it again.",
            ));
        }
    };
    if locator_binding.location_id != candidate.root.location_id
        || file_identity_binding.location_id != candidate.root.location_id
        || locator_binding.locator_ref_id != candidate.location.locator_ref_id
    {
        return Err(super::operator_error(
            StatusCode::CONFLICT,
            "RESOURCE_IDENTITY_UNAVAILABLE",
            "The saved folder could not be verified. Check the folder or remove and add it again.",
        ));
    }
    const MAX_ENTRIES: usize = storage_core::MAX_WORKSPACE_ROOT_SCAN_ENTRIES as usize;
    const MAX_LOCATOR_BYTES: usize = 1024 * 1024;
    let limits = LocalFileScanLimits::new(32, MAX_ENTRIES, MAX_LOCATOR_BYTES);
    let scan_permit = WORKSPACE_ROOT_SCAN_ADMISSION
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            super::operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "SCAN_UNAVAILABLE",
                "A folder scan is already using the available local scan capacity",
            )
        })?;
    let deadlines = ScanDeadlines::from_start(Instant::now());
    let private_locator = locator_binding.private_locator;
    let display_name = candidate.root.display_name.clone();
    let expected_identity_digest = candidate.resource.identity_digest.clone();
    let expected_keyed_identity = candidate
        .resource
        .provider_identity
        .get("file_identity")
        .cloned();
    let runtime_identity = state.runtime_identity.clone();
    let observed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| {
            super::operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Folder scan response could not be prepared",
            )
        })?;
    let blocking = run_blocking_filesystem_work(move || {
        let _scan_permit = scan_permit;
        ensure_before_scan_deadline(deadlines.final_identity_fence)?;
        let selection = reopen_saved_directory_with_cancel(&private_locator, &display_name, || {
            std::time::Instant::now() >= deadlines.final_identity_fence
        })
        .map_err(|error| match error {
            LocalDirectoryError::ScanDeadlineExceeded => ScanBlockingError::DeadlineExceeded,
            error => ScanBlockingError::Directory(error),
        })?;
        let live_identity = selection.identity();
        let (identity_digest, keyed_identity) = live_identity.keyed_projection(&runtime_identity);
        if !live_identity.matches_binding(&file_identity_binding)
            || expected_identity_digest.as_deref() != Some(identity_digest.as_str())
            || expected_keyed_identity.as_ref() != Some(&keyed_identity)
        {
            return Err(ScanBlockingError::IdentityUnavailable);
        }
        match selection
            .revalidate_with_cancel(|| std::time::Instant::now() >= deadlines.final_identity_fence)
        {
            Ok(()) => {}
            Err(LocalDirectoryError::ScanDeadlineExceeded) => {
                return Err(ScanBlockingError::DeadlineExceeded);
            }
            Err(_) => return Err(ScanBlockingError::IdentityUnavailable),
        }
        let mut progress = LocalFileScanSummary::default();
        let mut observations = Vec::new();
        let result = visit_regular_files_with_progress(
            &selection,
            limits,
            || Instant::now() >= deadlines.traversal,
            |entry| {
                let file_identity = entry.identity();
                let mut file_identity_material = Vec::with_capacity(16);
                file_identity_material.extend_from_slice(&file_identity.device_id().to_be_bytes());
                file_identity_material.extend_from_slice(&file_identity.file_id().to_be_bytes());
                let file_identity_digest = format!(
                    "sha256:{}",
                    runtime_identity.keyed_pseudonym(
                        "litecowork.local-filesystem.workspace-root-file.v1",
                        &file_identity_material,
                    )
                );
                let observation_id = format!(
                    "sha256:{}",
                    runtime_identity.keyed_pseudonym(
                        "litecowork.local-filesystem.workspace-root-path.v1",
                        entry.relative_locator(),
                    )
                );
                observations.push(WorkspaceRootFileObservation {
                    observation_id,
                    file_identity_digest,
                    resource_id: None,
                    display_name: entry.display_basename().to_owned(),
                    size_bytes: entry.size_bytes(),
                    modified_at: None,
                    observed_at: observed_at.clone(),
                    private_relative_locator: entry.relative_locator().to_vec(),
                });
                Ok(())
            },
            |snapshot| progress = snapshot,
        );
        let result = finalize_scan_result(&selection, deadlines, result)?;
        Ok((result, progress, observations))
    })
    .await
    .map_err(|_| {
        super::operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Folder scan could not be completed",
        )
    })?;
    let (scan_result, progress, observations) = blocking.map_err(|error| match error {
        ScanBlockingError::Directory(error) => map_local_directory_error(error),
        ScanBlockingError::DeadlineExceeded => super::operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SCAN_UNAVAILABLE",
            "The scan worker could not verify the selected folder before its time limit; retry with the same idempotency key",
        ),
        ScanBlockingError::IdentityUnavailable => super::operator_error(
            StatusCode::CONFLICT,
            "RESOURCE_IDENTITY_UNAVAILABLE",
            "The saved folder could not be verified. Check the folder or remove and add it again.",
        ),
        ScanBlockingError::RootChanged => super::operator_error(
            StatusCode::CONFLICT,
            "RESOURCE_IDENTITY_UNAVAILABLE",
            "The saved folder changed while it was being scanned",
        ),
    })?;
    let (status, partial_reason) = match scan_result {
        Ok(_) => (WorkspaceRootScanStatus::Complete, None),
        Err(LocalFileScanError::RootChanged) => {
            return Err(super::operator_error(
                StatusCode::CONFLICT,
                "RESOURCE_IDENTITY_UNAVAILABLE",
                "The saved folder changed while it was being scanned",
            ));
        }
        Err(LocalFileScanError::Cancelled) => (
            WorkspaceRootScanStatus::Partial,
            Some("TIME_LIMIT_REACHED".to_owned()),
        ),
        Err(LocalFileScanError::BudgetExceeded) => (
            WorkspaceRootScanStatus::Partial,
            Some("SCAN_BUDGET_REACHED".to_owned()),
        ),
        Err(LocalFileScanError::EntryChanged | LocalFileScanError::ReadFailed) => (
            WorkspaceRootScanStatus::Partial,
            Some("ENTRY_CHANGED_OR_UNREADABLE".to_owned()),
        ),
        Err(LocalFileScanError::MountBoundary) => (
            WorkspaceRootScanStatus::Partial,
            Some("MOUNT_BOUNDARY_REACHED".to_owned()),
        ),
        Err(
            LocalFileScanError::InvalidLimits
            | LocalFileScanError::OpenFailed
            | LocalFileScanError::MountIdentityUnavailable,
        ) => {
            return Err(super::operator_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "SCAN_UNAVAILABLE",
                "Folder metadata could not be verified safely",
            ));
        }
        #[cfg(target_os = "macos")]
        Err(LocalFileScanError::UnsupportedPlatform) => {
            return Err(super::operator_error(
                StatusCode::NOT_IMPLEMENTED,
                "SCAN_UNSUPPORTED_PLATFORM",
                "Metadata scanning is not available on this platform",
            ));
        }
    };
    let completed_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| {
            super::operator_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Folder scan response could not be prepared",
            )
        })?;
    let result = WorkspaceRootScanResult {
        workspace_id: command.workspace_id.clone(),
        workspace_root_id: command.workspace_root_id.clone(),
        request_id: command.request_id.clone(),
        root_version: command.expected_version,
        status,
        entries_examined: u32::try_from(progress.entries_visited).unwrap_or(u32::MAX),
        files_observed: u32::try_from(progress.files_visited).unwrap_or(u32::MAX),
        partial_reason,
        completed_at,
    };
    let committed = service
        .commit_scan(&command, result, observations)
        .map_err(map_store_error)?;
    Ok(no_store_json(committed))
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
struct ScanDeadlines {
    traversal: std::time::Instant,
    final_identity_fence: std::time::Instant,
}

#[cfg(target_os = "linux")]
impl ScanDeadlines {
    fn from_start(started_at: std::time::Instant) -> Self {
        let final_identity_fence = started_at + MAX_SCAN_TIME;
        Self {
            traversal: final_identity_fence - MAX_FINAL_IDENTITY_FENCE_TIME,
            final_identity_fence,
        }
    }
}

#[cfg(target_os = "linux")]
fn revalidate_scan_root_before_commit(
    selection: &crate::local_filesystem::OpenedDirectorySelection,
    deadline: std::time::Instant,
) -> Result<(), ScanBlockingError> {
    match selection.revalidate_with_cancel(|| std::time::Instant::now() >= deadline) {
        Ok(()) => Ok(()),
        Err(crate::local_filesystem::LocalDirectoryError::ScanDeadlineExceeded) => {
            Err(ScanBlockingError::DeadlineExceeded)
        }
        Err(_) => Err(ScanBlockingError::RootChanged),
    }
}

#[cfg(target_os = "linux")]
fn finalize_scan_result(
    selection: &crate::local_filesystem::OpenedDirectorySelection,
    deadlines: ScanDeadlines,
    result: Result<
        crate::local_filesystem::LocalFileScanSummary,
        crate::local_filesystem::LocalFileScanError,
    >,
) -> Result<
    Result<
        crate::local_filesystem::LocalFileScanSummary,
        crate::local_filesystem::LocalFileScanError,
    >,
    ScanBlockingError,
> {
    // A TIME_LIMIT partial is usable only after the required identity fence succeeds.
    // Traversal stops one second before the total deadline to reserve a bounded window
    // for this check; failure returns no scan result and therefore creates no receipt.
    revalidate_scan_root_before_commit(selection, deadlines.final_identity_fence)?;
    if std::time::Instant::now() >= deadlines.traversal && result.is_ok() {
        Ok(Err(crate::local_filesystem::LocalFileScanError::Cancelled))
    } else {
        Ok(result)
    }
}

#[cfg(target_os = "linux")]
fn ensure_before_scan_deadline(deadline: std::time::Instant) -> Result<(), ScanBlockingError> {
    if std::time::Instant::now() >= deadline {
        Err(ScanBlockingError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

async fn run_blocking_filesystem_work<T, F>(work: F) -> Result<T, tokio::task::JoinError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work).await
}

fn no_store_json(result: WorkspaceRootScanResult) -> Response {
    let mut response = Json(result).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

fn map_store_error(error: storage_core::StoreError) -> Response {
    match error {
        storage_core::StoreError::NotFound => super::operator_error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "WorkspaceRoot is unavailable",
        ),
        storage_core::StoreError::IdempotencyConflict => super::operator_error(
            StatusCode::CONFLICT,
            "CONFLICT",
            "This idempotency key was already used for a different folder scan",
        ),
        storage_core::StoreError::Conflict {
            expected: Some(_),
            actual: Some(_),
        } => super::operator_error(
            StatusCode::CONFLICT,
            "STALE_WORKSPACE_VERSION",
            "Folder state changed; refresh before scanning",
        ),
        storage_core::StoreError::Conflict { .. } => super::operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "RUNTIME_UNAVAILABLE",
            "The current Runtime incarnation could not be verified",
        ),
        storage_core::StoreError::Busy => super::operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SCAN_UNAVAILABLE",
            "Local scan storage is busy; retry this request with the same idempotency key",
        ),
        storage_core::StoreError::Invalid(_) => super::operator_error(
            StatusCode::BAD_REQUEST,
            "INVALID_ARGUMENT",
            "Folder scan request is invalid",
        ),
        _ => super::operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Folder scan could not be completed",
        ),
    }
}

fn map_workspace_preflight_result<T>(
    result: Result<T, storage_core::StoreError>,
) -> Result<T, Response> {
    result.map_err(map_store_error)
}

#[cfg(all(test, target_os = "linux"))]
mod blocking_worker_tests {
    use super::{
        ScanBlockingError, ScanDeadlines, ensure_before_scan_deadline, finalize_scan_result,
        run_blocking_filesystem_work,
    };
    use std::{
        cell::Cell,
        fs,
        time::{Duration, Instant},
    };
    use tokio::sync::oneshot;

    #[test]
    fn final_scan_fence_rejects_a_root_recheck_that_exceeds_the_deadline() {
        let directory = tempfile::tempdir().expect("temporary selected root");
        let selected_path = directory.path().join("selected");
        fs::create_dir(&selected_path).expect("create selected root");
        let selection = crate::local_filesystem::open_selected_directory(&selected_path)
            .expect("open selected root");
        let deadlines = ScanDeadlines {
            traversal: Instant::now() - Duration::from_millis(2),
            final_identity_fence: Instant::now() - Duration::from_millis(1),
        };

        assert!(matches!(
            finalize_scan_result(
                &selection,
                deadlines,
                Err(crate::local_filesystem::LocalFileScanError::Cancelled),
            ),
            Err(ScanBlockingError::DeadlineExceeded)
        ));
    }

    #[test]
    fn final_scan_fence_allows_time_limit_partial_within_reserved_allowance() {
        let directory = tempfile::tempdir().expect("temporary selected root");
        let selected_path = directory.path().join("selected");
        fs::create_dir(&selected_path).expect("create selected root");
        let selection = crate::local_filesystem::open_selected_directory(&selected_path)
            .expect("open selected root");
        let started_at = Instant::now() - Duration::from_secs(7) - Duration::from_millis(100);
        let deadlines = super::ScanDeadlines::from_start(started_at);

        assert_eq!(
            deadlines.final_identity_fence - started_at,
            Duration::from_secs(8)
        );
        assert_eq!(
            deadlines.final_identity_fence - deadlines.traversal,
            Duration::from_secs(1)
        );
        assert!(Instant::now() >= deadlines.traversal);
        assert!(Instant::now() < deadlines.final_identity_fence);
        let result = finalize_scan_result(
            &selection,
            deadlines,
            Ok(crate::local_filesystem::LocalFileScanSummary::default()),
        )
        .expect("root identity fence completes within the reserved allowance");
        assert!(matches!(
            result,
            Err(crate::local_filesystem::LocalFileScanError::Cancelled)
        ));
    }

    #[test]
    fn final_scan_fence_keeps_root_replacement_as_a_conflict() {
        let directory = tempfile::tempdir().expect("temporary selected root");
        let selected_path = directory.path().join("selected");
        let replacement_path = directory.path().join("replacement");
        fs::create_dir(&selected_path).expect("create selected root");
        let selection = crate::local_filesystem::open_selected_directory(&selected_path)
            .expect("open selected root");
        fs::rename(&selected_path, &replacement_path).expect("move selected root");
        fs::create_dir(&selected_path).expect("replace selected root");
        let deadlines = ScanDeadlines::from_start(Instant::now());

        assert!(matches!(
            finalize_scan_result(
                &selection,
                deadlines,
                Err(crate::local_filesystem::LocalFileScanError::Cancelled),
            ),
            Err(ScanBlockingError::RootChanged)
        ));
    }

    #[test]
    fn worker_delayed_past_deadline_never_starts_saved_path_reopen() {
        let deadlines = ScanDeadlines::from_start(Instant::now() - Duration::from_secs(9));
        let path_reopened = Cell::new(false);

        let result = (|| {
            ensure_before_scan_deadline(deadlines.final_identity_fence)?;
            path_reopened.set(true);
            Ok::<(), ScanBlockingError>(())
        })();

        assert!(matches!(result, Err(ScanBlockingError::DeadlineExceeded)));
        assert!(
            !path_reopened.get(),
            "expired worker must do no filesystem I/O"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn slow_filesystem_work_does_not_block_the_operator_executor() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let worker = tokio::spawn(run_blocking_filesystem_work(move || {
            let _ = started_tx.send(());
            let _ = release_rx.blocking_recv();
        }));

        started_rx.await.expect("blocking filesystem task started");
        let (tick_tx, tick_rx) = oneshot::channel();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            let _ = tick_tx.send(());
        });
        tokio::time::timeout(Duration::from_millis(100), tick_rx)
            .await
            .expect("Operator executor remains schedulable")
            .expect("executor tick sent");

        let _ = release_tx.send(());
        worker
            .await
            .expect("blocking task join")
            .expect("worker completes");
    }
}

#[cfg(test)]
mod error_mapping_tests {
    use super::{map_store_error, map_workspace_preflight_result};
    use axum::{body::to_bytes, http::StatusCode};
    use serde_json::Value;
    use storage_core::StoreError;

    async fn error_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), 4096)
            .await
            .expect("bounded Operator error response");
        serde_json::from_slice(&bytes).expect("Operator error response is JSON")
    }

    #[tokio::test]
    async fn runtime_incarnation_fence_conflict_is_not_reported_as_a_stale_root_version() {
        let response = map_store_error(StoreError::Conflict {
            expected: None,
            actual: None,
        });

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error_body(response).await["code"], "RUNTIME_UNAVAILABLE");
    }

    #[tokio::test]
    async fn versioned_conflict_remains_a_stale_workspace_version() {
        let response = map_store_error(StoreError::Conflict {
            expected: Some(4),
            actual: Some(5),
        });

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_body(response).await["code"],
            "STALE_WORKSPACE_VERSION"
        );
    }

    #[tokio::test]
    async fn transient_storage_busy_is_reported_as_retryable_scan_unavailable() {
        let response = map_store_error(StoreError::Busy);

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = error_body(response).await;
        assert_eq!(body["code"], "SCAN_UNAVAILABLE");
        assert_eq!(body["retryable"], true);
    }

    #[tokio::test]
    async fn workspace_preflight_busy_is_reported_as_retryable_scan_unavailable() {
        let Err(response) = map_workspace_preflight_result::<Option<storage_core::Workspace>>(Err(
            StoreError::Busy,
        )) else {
            panic!("busy Workspace preflight must return an Operator error");
        };

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = error_body(response).await;
        assert_eq!(body["code"], "SCAN_UNAVAILABLE");
        assert_eq!(body["retryable"], true);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn map_local_directory_error(error: crate::local_filesystem::LocalDirectoryError) -> Response {
    use crate::local_filesystem::LocalDirectoryError;
    match error {
        LocalDirectoryError::UnsupportedPlatform => super::operator_error(
            StatusCode::NOT_IMPLEMENTED,
            "SCAN_UNSUPPORTED_PLATFORM",
            "Metadata scanning is not available on this platform",
        ),
        LocalDirectoryError::ScanDeadlineExceeded => super::operator_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SCAN_UNAVAILABLE",
            "The scan worker could not verify the selected folder before its time limit; retry with the same idempotency key",
        ),
        _ => super::operator_error(
            StatusCode::CONFLICT,
            "RESOURCE_IDENTITY_UNAVAILABLE",
            "The saved folder could not be verified. Check the folder or remove and add it again.",
        ),
    }
}
