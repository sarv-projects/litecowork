use super::{Command, SqliteWorkspaceStore, canonicalize_utc_timestamp, map_database_error};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::OnceLock;
use std::sync::mpsc;
use storage_core::{
    NeedsYouCursor, NeedsYouItem, NeedsYouItemKind, NeedsYouPage, NeedsYouPageRequest,
    NeedsYouReadStore, NeedsYouStatus, StoreError,
};

const MAX_NEEDS_YOU_PAGE_SIZE: usize = 200;
const MAX_NEEDS_YOU_ID_BYTES: usize = 512;
const MAX_SAFE_SUMMARY_BYTES: usize = 4_096;
const MAX_SAFE_HINT_BYTES: usize = 4_096;
const ERROR_CODES_SCHEMA: &str = include_str!("../../../docs/schemas/error-codes.schema.json");
static ERROR_CODES: OnceLock<HashSet<String>> = OnceLock::new();

impl NeedsYouReadStore for SqliteWorkspaceStore {
    fn list_needs_you_page(
        &self,
        request: NeedsYouPageRequest,
    ) -> Result<NeedsYouPage, StoreError> {
        validate_request(&request)?;
        let (reply, receiver) = mpsc::channel();
        self.execute_command(Command::ListNeedsYouPage { request, reply }, receiver)
    }
}

pub(super) fn list_needs_you_page(
    connection: &mut Connection,
    request: NeedsYouPageRequest,
) -> Result<NeedsYouPage, StoreError> {
    validate_request(&request)?;
    let transaction = connection.transaction().map_err(map_database_error)?;
    let mut candidates = Vec::with_capacity(request.limit.saturating_add(1));
    let page_capacity = request.limit.saturating_add(1);

    if request
        .filter
        .kind
        .is_none_or(|kind| kind == NeedsYouItemKind::Approval)
    {
        collect_approvals(&transaction, &request, &mut candidates, page_capacity)?;
    }
    if request
        .filter
        .kind
        .is_none_or(|kind| kind == NeedsYouItemKind::UserRequest)
    {
        collect_user_requests(&transaction, &request, &mut candidates, page_capacity)?;
    }
    if request
        .filter
        .status
        .is_none_or(|status| status == NeedsYouStatus::Open)
        && request
            .filter
            .kind
            .is_none_or(|kind| kind == NeedsYouItemKind::TaskBlocker)
    {
        collect_task_blockers(&transaction, &request, &mut candidates, page_capacity)?;
    }

    transaction.commit().map_err(map_database_error)?;
    candidates.sort_by(compare_items);
    let has_more = candidates.len() > request.limit;
    candidates.truncate(request.limit);
    let next_cursor = if has_more {
        candidates.last().map(|item| NeedsYouCursor {
            workspace_id: request.workspace_id.clone(),
            filter: request.filter.clone(),
            limit: request.limit,
            after_created_at: item.created_at.clone(),
            after_item_id: item.item_id.clone(),
        })
    } else {
        None
    };
    Ok(NeedsYouPage {
        items: candidates,
        next_cursor,
    })
}

fn validate_request(request: &NeedsYouPageRequest) -> Result<(), StoreError> {
    validate_opaque_id(&request.workspace_id, "Workspace ID")?;
    if !(1..=MAX_NEEDS_YOU_PAGE_SIZE).contains(&request.limit) {
        return Err(StoreError::Invalid(
            "Needs You page size must be between 1 and 200".to_owned(),
        ));
    }
    if let Some(task_id) = request.filter.task_id.as_deref() {
        validate_opaque_id(task_id, "Task ID")?;
    }
    if let Some(cursor) = request.cursor.as_ref() {
        if cursor.workspace_id != request.workspace_id
            || cursor.filter != request.filter
            || cursor.limit != request.limit
        {
            return Err(StoreError::Invalid(
                "Needs You cursor does not match its Workspace, filters, or page size".to_owned(),
            ));
        }
        let normalized_created_at = stored_timestamp(&cursor.after_created_at, "cursor timestamp")?;
        if normalized_created_at != cursor.after_created_at
            || !is_sha256_digest(&cursor.after_item_id)
        {
            return Err(StoreError::Invalid(
                "Needs You cursor position is malformed".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_opaque_id(value: &str, name: &str) -> Result<(), StoreError> {
    if value.is_empty()
        || value.trim().is_empty()
        || value.len() > MAX_NEEDS_YOU_ID_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid(format!("{name} is malformed")));
    }
    Ok(())
}

fn collect_approvals(
    transaction: &Transaction<'_>,
    request: &NeedsYouPageRequest,
    candidates: &mut Vec<NeedsYouItem>,
    capacity: usize,
) -> Result<(), StoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT a.approval_id, a.task_id, a.status, a.requested_at, a.resolved_at
             FROM approvals a
             JOIN tasks t ON t.task_id = a.task_id
             WHERE t.workspace_id = ?1 AND (?2 IS NULL OR a.task_id = ?2)",
        )
        .map_err(map_database_error)?;
    let mut rows = statement
        .query(params![
            request.workspace_id,
            request.filter.task_id.as_deref()
        ])
        .map_err(map_database_error)?;
    while let Some(row) = rows.next().map_err(map_database_error)? {
        let approval_id: String = row.get(0).map_err(map_database_error)?;
        let task_id: String = row.get(1).map_err(map_database_error)?;
        let raw_status: String = row.get(2).map_err(map_database_error)?;
        let requested_at: String = row.get(3).map_err(map_database_error)?;
        let resolved_at: Option<String> = row.get(4).map_err(map_database_error)?;
        validate_opaque_id(&approval_id, "Approval ID").map_err(as_integrity_error)?;
        validate_opaque_id(&task_id, "Task ID").map_err(as_integrity_error)?;
        let (status, title, safe_summary) = approval_projection(&raw_status)?;
        let created_at = stored_timestamp(&requested_at, "Approval requested_at")?;
        let resolved_at = resolved_at
            .as_deref()
            .map(|value| stored_timestamp(value, "Approval resolved_at"))
            .transpose()?;
        let item = NeedsYouItem {
            item_id: item_id(NeedsYouItemKind::Approval, &[&approval_id])?,
            kind: NeedsYouItemKind::Approval,
            status,
            approval_id: Some(approval_id),
            user_request_id: None,
            task_id: Some(task_id),
            blocker_id: None,
            title: title.to_owned(),
            safe_summary: safe_summary.to_owned(),
            created_at,
            resolved_at,
        };
        push_candidate(candidates, item, request, capacity);
    }
    Ok(())
}

fn collect_user_requests(
    transaction: &Transaction<'_>,
    request: &NeedsYouPageRequest,
    candidates: &mut Vec<NeedsYouItem>,
    capacity: usize,
) -> Result<(), StoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT ur.request_id, ur.task_id, ur.status, ur.created_at, ur.resolved_at,
                    t.workspace_id
             FROM user_requests ur
             LEFT JOIN tasks t ON t.task_id = ur.task_id
             WHERE ur.workspace_id = ?1 AND (?2 IS NULL OR ur.task_id = ?2)",
        )
        .map_err(map_database_error)?;
    let mut rows = statement
        .query(params![
            request.workspace_id,
            request.filter.task_id.as_deref()
        ])
        .map_err(map_database_error)?;
    while let Some(row) = rows.next().map_err(map_database_error)? {
        let user_request_id: String = row.get(0).map_err(map_database_error)?;
        let task_id: Option<String> = row.get(1).map_err(map_database_error)?;
        let raw_status: String = row.get(2).map_err(map_database_error)?;
        let created_at: String = row.get(3).map_err(map_database_error)?;
        let resolved_at: Option<String> = row.get(4).map_err(map_database_error)?;
        let task_workspace_id: Option<String> = row.get(5).map_err(map_database_error)?;
        validate_opaque_id(&user_request_id, "UserRequest ID").map_err(as_integrity_error)?;
        if let Some(task_id) = task_id.as_deref() {
            validate_opaque_id(task_id, "Task ID").map_err(as_integrity_error)?;
            if task_workspace_id.as_deref() != Some(request.workspace_id.as_str()) {
                return Err(StoreError::Integrity(
                    "UserRequest Task is missing or belongs to a different Workspace".to_owned(),
                ));
            }
        }
        let (status, title, safe_summary) = user_request_projection(&raw_status)?;
        let created_at = stored_timestamp(&created_at, "UserRequest created_at")?;
        let resolved_at = resolved_at
            .as_deref()
            .map(|value| stored_timestamp(value, "UserRequest resolved_at"))
            .transpose()?;
        let item = NeedsYouItem {
            item_id: item_id(NeedsYouItemKind::UserRequest, &[&user_request_id])?,
            kind: NeedsYouItemKind::UserRequest,
            status,
            approval_id: None,
            user_request_id: Some(user_request_id),
            task_id,
            blocker_id: None,
            title: title.to_owned(),
            safe_summary: safe_summary.to_owned(),
            created_at,
            resolved_at,
        };
        push_candidate(candidates, item, request, capacity);
    }
    Ok(())
}

fn collect_task_blockers(
    transaction: &Transaction<'_>,
    request: &NeedsYouPageRequest,
    candidates: &mut Vec<NeedsYouItem>,
    capacity: usize,
) -> Result<(), StoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT task_id, blocking_conditions_json
             FROM tasks
             WHERE workspace_id = ?1 AND status IN ('BLOCKED', 'NEEDS_USER')
               AND (?2 IS NULL OR task_id = ?2)
             ORDER BY task_id ASC",
        )
        .map_err(map_database_error)?;
    let mut rows = statement
        .query(params![
            request.workspace_id,
            request.filter.task_id.as_deref()
        ])
        .map_err(map_database_error)?;
    while let Some(row) = rows.next().map_err(map_database_error)? {
        let task_id: String = row.get(0).map_err(map_database_error)?;
        let raw_blockers: String = row.get(1).map_err(map_database_error)?;
        validate_opaque_id(&task_id, "Task ID").map_err(as_integrity_error)?;
        let blockers: Value = serde_json::from_str(&raw_blockers).map_err(|_| {
            StoreError::Integrity("Task blocker projection is not valid JSON".to_owned())
        })?;
        let blockers = blockers.as_array().ok_or_else(|| {
            StoreError::Integrity("Task blocker projection is not an array".to_owned())
        })?;
        let mut blocker_ids = HashSet::with_capacity(blockers.len());
        for value in blockers {
            let blocker = parse_blocker(value, &request.workspace_id)?;
            if !blocker_ids.insert(blocker.blocker_id.clone()) {
                return Err(StoreError::Integrity(
                    "Task blocker projection contains duplicate blocker IDs".to_owned(),
                ));
            }
            if let Some(approval_id) = blocker.approval_id.as_deref() {
                validate_approval_link(transaction, &request.workspace_id, &task_id, approval_id)?;
                continue;
            }
            if let Some(user_request_id) = blocker.user_request_id.as_deref() {
                validate_user_request_link(
                    transaction,
                    &request.workspace_id,
                    &task_id,
                    user_request_id,
                )?;
                continue;
            }
            let item = NeedsYouItem {
                item_id: item_id(
                    NeedsYouItemKind::TaskBlocker,
                    &[&task_id, &blocker.blocker_id],
                )?,
                kind: NeedsYouItemKind::TaskBlocker,
                status: NeedsYouStatus::Open,
                approval_id: None,
                user_request_id: None,
                task_id: Some(task_id.clone()),
                blocker_id: Some(blocker.blocker_id),
                title: "Task needs attention".to_owned(),
                safe_summary: blocker.safe_message,
                created_at: blocker.created_at,
                resolved_at: None,
            };
            push_candidate(candidates, item, request, capacity);
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredBlocker {
    blocker_id: String,
    code: String,
    safe_message: String,
    resolution_hint: String,
    #[serde(default)]
    subject_ref: Option<StoredSubjectRef>,
    #[serde(default)]
    user_request_id: Option<String>,
    #[serde(default)]
    approval_id: Option<String>,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredSubjectRef {
    Resource(StoredResourceRef),
    Service(StoredServiceRef),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredResourceRef {
    workspace_id: String,
    resource_id: String,
    #[serde(default)]
    revision_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredServiceRef {
    service_id: String,
}

struct ParsedBlocker {
    blocker_id: String,
    safe_message: String,
    created_at: String,
    user_request_id: Option<String>,
    approval_id: Option<String>,
}

fn parse_blocker(value: &Value, workspace_id: &str) -> Result<ParsedBlocker, StoreError> {
    let stored: StoredBlocker = serde_json::from_value(value.clone()).map_err(|_| {
        StoreError::Integrity(
            "Task blocker projection does not match the Blocker schema".to_owned(),
        )
    })?;
    validate_opaque_id(&stored.blocker_id, "blocker_id").map_err(as_integrity_error)?;
    validate_opaque_id(&stored.code, "blocker error code").map_err(as_integrity_error)?;
    if !known_error_code(&stored.code) {
        return Err(StoreError::Integrity(
            "Task blocker projection contains an unknown error code".to_owned(),
        ));
    }
    validate_safe_text(&stored.safe_message, MAX_SAFE_SUMMARY_BYTES, "safe_message")?;
    validate_safe_text(
        &stored.resolution_hint,
        MAX_SAFE_HINT_BYTES,
        "resolution_hint",
    )?;
    if stored.approval_id.is_some() && stored.user_request_id.is_some() {
        return Err(StoreError::Integrity(
            "Task blocker projection links both an Approval and UserRequest".to_owned(),
        ));
    }
    if let Some(approval_id) = stored.approval_id.as_deref() {
        validate_opaque_id(approval_id, "Approval link").map_err(as_integrity_error)?;
    }
    if let Some(user_request_id) = stored.user_request_id.as_deref() {
        validate_opaque_id(user_request_id, "UserRequest link").map_err(as_integrity_error)?;
    }
    if let Some(subject_ref) = stored.subject_ref.as_ref() {
        match subject_ref {
            StoredSubjectRef::Resource(resource) => {
                validate_opaque_id(&resource.workspace_id, "ResourceRef Workspace ID")
                    .map_err(as_integrity_error)?;
                validate_opaque_id(&resource.resource_id, "ResourceRef Resource ID")
                    .map_err(as_integrity_error)?;
                if resource.workspace_id != workspace_id {
                    return Err(StoreError::Integrity(
                        "Task blocker subject references a different Workspace".to_owned(),
                    ));
                }
                if let Some(revision_id) = resource.revision_id.as_deref() {
                    validate_opaque_id(revision_id, "ResourceRef revision ID")
                        .map_err(as_integrity_error)?;
                }
            }
            StoredSubjectRef::Service(service) => {
                validate_opaque_id(&service.service_id, "ServiceRef ID")
                    .map_err(as_integrity_error)?;
            }
        }
    }
    let created_at = stored_timestamp(&stored.created_at, "Blocker created_at")?;
    Ok(ParsedBlocker {
        blocker_id: stored.blocker_id,
        safe_message: stored.safe_message,
        created_at,
        user_request_id: stored.user_request_id,
        approval_id: stored.approval_id,
    })
}

fn validate_approval_link(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    task_id: &str,
    approval_id: &str,
) -> Result<(), StoreError> {
    let source = transaction
        .query_row(
            "SELECT a.task_id, t.workspace_id
             FROM approvals a LEFT JOIN tasks t ON t.task_id = a.task_id
             WHERE a.approval_id = ?1",
            [approval_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match source {
        Some((source_task_id, Some(source_workspace_id)))
            if source_workspace_id == workspace_id && source_task_id == task_id =>
        {
            Ok(())
        }
        _ => Err(StoreError::Integrity(
            "Task blocker Approval link is dangling or outside its Task and Workspace".to_owned(),
        )),
    }
}

fn validate_user_request_link(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    task_id: &str,
    user_request_id: &str,
) -> Result<(), StoreError> {
    let source = transaction
        .query_row(
            "SELECT workspace_id, task_id FROM user_requests WHERE request_id = ?1",
            [user_request_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(map_database_error)?;
    match source {
        Some((source_workspace_id, Some(source_task_id)))
            if source_workspace_id == workspace_id && source_task_id == task_id =>
        {
            Ok(())
        }
        _ => Err(StoreError::Integrity(
            "Task blocker UserRequest link is dangling or outside its Task and Workspace"
                .to_owned(),
        )),
    }
}

fn approval_projection(
    status: &str,
) -> Result<(NeedsYouStatus, &'static str, &'static str), StoreError> {
    match status {
        "PENDING" => Ok((
            NeedsYouStatus::Open,
            "Approval needed",
            "An action is awaiting approval.",
        )),
        "APPROVED" => Ok((
            NeedsYouStatus::Resolved,
            "Approval approved",
            "The approval was approved.",
        )),
        "DENIED" => Ok((
            NeedsYouStatus::Resolved,
            "Approval denied",
            "The approval was denied.",
        )),
        "EXPIRED" => Ok((
            NeedsYouStatus::Resolved,
            "Approval expired",
            "The approval expired.",
        )),
        "CANCELLED" => Ok((
            NeedsYouStatus::Resolved,
            "Approval cancelled",
            "The approval was cancelled.",
        )),
        _ => Err(StoreError::Integrity(
            "Approval projection contains an unknown status".to_owned(),
        )),
    }
}

fn user_request_projection(
    status: &str,
) -> Result<(NeedsYouStatus, &'static str, &'static str), StoreError> {
    match status {
        "PENDING" => Ok((
            NeedsYouStatus::Open,
            "Response needed",
            "A response is waiting.",
        )),
        "ANSWERED" => Ok((
            NeedsYouStatus::Resolved,
            "Response received",
            "The response was submitted.",
        )),
        "DISMISSED" => Ok((
            NeedsYouStatus::Resolved,
            "Request dismissed",
            "The request was dismissed.",
        )),
        "EXPIRED" => Ok((
            NeedsYouStatus::Resolved,
            "Request expired",
            "The request expired.",
        )),
        "CANCELLED" => Ok((
            NeedsYouStatus::Resolved,
            "Request cancelled",
            "The request was cancelled.",
        )),
        _ => Err(StoreError::Integrity(
            "UserRequest projection contains an unknown status".to_owned(),
        )),
    }
}

fn push_candidate(
    candidates: &mut Vec<NeedsYouItem>,
    item: NeedsYouItem,
    request: &NeedsYouPageRequest,
    capacity: usize,
) {
    if request
        .filter
        .status
        .is_some_and(|status| item.status != status)
        || request.filter.kind.is_some_and(|kind| item.kind != kind)
        || request
            .filter
            .task_id
            .as_deref()
            .is_some_and(|task_id| item.task_id.as_deref() != Some(task_id))
        || request.cursor.as_ref().is_some_and(|cursor| {
            item.created_at > cursor.after_created_at
                || (item.created_at == cursor.after_created_at
                    && item.item_id <= cursor.after_item_id)
        })
    {
        return;
    }
    candidates.push(item);
    candidates.sort_by(compare_items);
    candidates.truncate(capacity);
}

fn compare_items(left: &NeedsYouItem, right: &NeedsYouItem) -> std::cmp::Ordering {
    right
        .created_at
        .cmp(&left.created_at)
        .then_with(|| left.item_id.cmp(&right.item_id))
}

fn item_id(kind: NeedsYouItemKind, identity: &[&str]) -> Result<String, StoreError> {
    let mut bytes =
        Vec::with_capacity(40 + identity.iter().map(|field| field.len() + 4).sum::<usize>());
    bytes.extend_from_slice(b"litecowork.needs-you.item.v1\0");
    for field in std::iter::once(kind.as_str()).chain(identity.iter().copied()) {
        let length = u32::try_from(field.len()).map_err(|_| {
            StoreError::Integrity("Needs You source identity field is too long".to_owned())
        })?;
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn stored_timestamp(value: &str, field: &str) -> Result<String, StoreError> {
    canonicalize_utc_timestamp(value).map_err(|_| {
        StoreError::Integrity(format!(
            "Needs You {field} is not a valid RFC 3339 UTC instant"
        ))
    })
}

fn validate_safe_text(value: &str, max_bytes: usize, name: &str) -> Result<(), StoreError> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(StoreError::Integrity(format!(
            "Task blocker {name} is malformed"
        )));
    }
    Ok(())
}

fn known_error_code(value: &str) -> bool {
    ERROR_CODES
        .get_or_init(|| {
            serde_json::from_str::<Value>(ERROR_CODES_SCHEMA)
                .ok()
                .and_then(|schema| schema.get("enum").and_then(Value::as_array).cloned())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|code| code.as_str().map(str::to_owned))
                .collect()
        })
        .contains(value)
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn as_integrity_error(error: StoreError) -> StoreError {
    match error {
        StoreError::Invalid(message) => StoreError::Integrity(message),
        other => other,
    }
}
