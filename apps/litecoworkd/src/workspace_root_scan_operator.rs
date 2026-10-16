use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use domain_workspace::{ScanWorkspaceRoot, WorkspaceRootService};
use serde::Deserialize;
use std::time::Duration;
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
    Router::new().route(
        "/v1/workspace-roots/{workspace_root_id}/scan",
        post(scan_workspace_root),
    )
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
        reopen_saved_directory_with_cancel, visit_regular_files_with_progress,
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
        let result = visit_regular_files_with_progress(
            &selection,
            limits,
            || Instant::now() >= deadlines.traversal,
            |_| Ok(()),
            |snapshot| progress = snapshot,
        );
        let result = finalize_scan_result(&selection, deadlines, result)?;
        Ok((result, progress))
    })
    .await
    .map_err(|_| {
        super::operator_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Folder scan could not be completed",
        )
    })?;
    let (scan_result, progress) = blocking.map_err(|error| match error {
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
        .commit_scan(&command, result)
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
