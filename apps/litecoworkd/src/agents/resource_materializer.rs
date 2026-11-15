//! Dispatch-time ResourceRef authorization and text materialization (001-T14, B-08).
//!
//! A Conversation turn may carry `resource_refs` and `RESOURCE` content blocks. The native
//! text transports cannot read LiteCowork Resources, so before dispatch each ref is
//! reauthorized against the selected Workspace and replaced by a delimited, digest-headed
//! TEXT block. Only allowlisted text Resources whose pinned revision is still current are
//! materialized. Binary Resources are refused (the OpenCode text transport has no verified
//! file input), as is anything outside the Workspace, any changed revision and any
//! over-limit or non-UTF-8 content. Nothing here reads a filesystem path, so there is no
//! symlink or traversal surface; WorkspaceRoot files are not Resources until 003-T07.
//! A refusal happens before dispatch, never silently dropping the attachment.

use serde_json::{Value, json};
use storage_core::conversation::ConversationContextPackage;
use storage_core::{ResourceStore, StoreError};

/// Largest single attached text Resource.
pub(crate) const MAX_ATTACHED_RESOURCE_BYTES: u64 = 128 * 1024;
/// Most distinct Resources attached to one message.
pub(crate) const MAX_ATTACHED_RESOURCES_PER_MESSAGE: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaterializeError {
    /// Malformed ref, or a ref pointing outside the selected Workspace.
    NotAuthorized,
    /// The Resource does not exist in this Workspace (or was removed).
    NotFound,
    /// The pinned revision is no longer the current head.
    StaleRevision,
    /// Not allowlisted text; binary attachments are unavailable on this transport.
    BinaryUnavailable,
    /// Larger than the single-Resource bound.
    TooLarge,
    /// Content is not valid UTF-8 text.
    NotText,
    /// RESTRICTED Resources never go to a native harness from a Conversation attachment.
    Restricted,
    /// Too many attachments on one message.
    TooMany,
    /// The Resource store failed; the turn is not dispatched.
    StoreUnavailable,
}

impl MaterializeError {
    pub(crate) fn reason_code(self) -> &'static str {
        match self {
            Self::NotAuthorized => "RESOURCE_NOT_AUTHORIZED",
            Self::NotFound => "RESOURCE_NOT_FOUND",
            Self::StaleRevision => "RESOURCE_REVISION_CHANGED",
            Self::BinaryUnavailable => "RESOURCE_BINARY_UNAVAILABLE",
            Self::TooLarge => "RESOURCE_TOO_LARGE",
            Self::NotText => "RESOURCE_NOT_TEXT",
            Self::Restricted => "RESOURCE_RESTRICTED",
            Self::TooMany => "RESOURCE_TOO_MANY",
            Self::StoreUnavailable => "RESOURCE_STORE_UNAVAILABLE",
        }
    }
}

/// One ref as stored on a message: `{workspace_id, resource_id, revision_id?}`.
struct RefParts {
    resource_id: String,
    revision_id: Option<String>,
}

fn parse_ref(value: &Value, workspace_id: &str) -> Result<RefParts, MaterializeError> {
    let object = value.as_object().ok_or(MaterializeError::NotAuthorized)?;
    let text = |key: &str| object.get(key).and_then(Value::as_str);
    if text("workspace_id") != Some(workspace_id) {
        return Err(MaterializeError::NotAuthorized);
    }
    let resource_id = text("resource_id")
        .filter(|id| !id.trim().is_empty() && id.len() <= 256)
        .ok_or(MaterializeError::NotAuthorized)?;
    let revision_id = match object.get("revision_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(id)) if !id.trim().is_empty() && id.len() <= 256 => Some(id.clone()),
        Some(_) => return Err(MaterializeError::NotAuthorized),
    };
    Ok(RefParts {
        resource_id: resource_id.to_owned(),
        revision_id,
    })
}

fn display_word(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control() && *c != '[' && *c != ']')
        .take(128)
        .collect()
}

/// Reauthorizes one ref and returns its delimited text block.
fn materialize_one(
    store: &dyn ResourceStore,
    workspace_id: &str,
    part: &RefParts,
) -> Result<String, MaterializeError> {
    let record = store
        .get_resource_record(workspace_id, &part.resource_id)
        .map_err(|_| MaterializeError::StoreUnavailable)?
        .ok_or(MaterializeError::NotFound)?;
    if record.workspace_id != workspace_id {
        return Err(MaterializeError::NotAuthorized);
    }
    if record.sensitivity == "RESTRICTED" {
        return Err(MaterializeError::Restricted);
    }
    if part
        .revision_id
        .as_deref()
        .is_some_and(|pinned| record.current_revision_id.as_deref() != Some(pinned))
    {
        return Err(MaterializeError::StaleRevision);
    }
    let stored = store
        .read_resource_content_bounded(
            workspace_id,
            &part.resource_id,
            part.revision_id.as_deref(),
            MAX_ATTACHED_RESOURCE_BYTES,
        )
        .map_err(|error| match error {
            StoreError::Invalid(_) => MaterializeError::TooLarge,
            StoreError::NotFound => MaterializeError::NotFound,
            _ => MaterializeError::StoreUnavailable,
        })?
        .ok_or(MaterializeError::NotFound)?;
    let summary = &stored.summary;
    if summary.workspace_id != workspace_id || summary.resource_id != part.resource_id {
        return Err(MaterializeError::NotAuthorized);
    }
    if !storage_sqlite::is_allowlisted_text(&summary.display_name, &summary.media_type) {
        return Err(MaterializeError::BinaryUnavailable);
    }
    if stored.content.len() as u64 > MAX_ATTACHED_RESOURCE_BYTES {
        return Err(MaterializeError::TooLarge);
    }
    let text = String::from_utf8(stored.content).map_err(|_| MaterializeError::NotText)?;
    if text.contains('\0') {
        return Err(MaterializeError::NotText);
    }
    Ok(format!(
        "[Attached resource BEGIN id={} revision={} name={} media_type={} bytes={} digest={}]\n{}\n[Attached resource END id={}]",
        summary.resource_id,
        summary.resource_revision_id,
        display_word(&summary.display_name),
        display_word(&summary.media_type),
        summary.size_bytes,
        summary.content_digest,
        text,
        summary.resource_id,
    ))
}

/// Replaces every ref and RESOURCE block in the package with delimited TEXT blocks, or
/// fails the whole turn before dispatch. The returned package has no `resource_refs`.
pub(crate) fn materialize_conversation_resources(
    store: &dyn ResourceStore,
    workspace_id: &str,
    mut package: ConversationContextPackage,
) -> Result<ConversationContextPackage, MaterializeError> {
    for message in &mut package.messages {
        let mut parts: Vec<RefParts> = Vec::new();
        let mut push = |part: RefParts| {
            if !parts
                .iter()
                .any(|p| p.resource_id == part.resource_id && p.revision_id == part.revision_id)
            {
                parts.push(part);
            }
        };
        match &message.resource_refs {
            Value::Null => {}
            Value::Array(refs) => {
                for value in refs {
                    push(parse_ref(value, workspace_id)?);
                }
            }
            _ => return Err(MaterializeError::NotAuthorized),
        }
        let mut blocks = Vec::new();
        if let Some(existing) = message.content.as_array() {
            for block in existing {
                if block.get("kind").and_then(Value::as_str) == Some("RESOURCE") {
                    let reference = block
                        .get("resource_ref")
                        .ok_or(MaterializeError::NotAuthorized)?;
                    push(parse_ref(reference, workspace_id)?);
                } else {
                    blocks.push(block.clone());
                }
            }
        } else {
            return Ok(package);
        }
        if parts.is_empty() {
            continue;
        }
        if parts.len() > MAX_ATTACHED_RESOURCES_PER_MESSAGE {
            return Err(MaterializeError::TooMany);
        }
        for part in &parts {
            let text = materialize_one(store, workspace_id, part)?;
            blocks.push(json!({"kind": "TEXT", "text": text}));
        }
        message.content = Value::Array(blocks);
        message.resource_refs = json!([]);
    }
    Ok(package)
}

/// Submit-time check: the same authorization and revision rules, so a bad attachment is
/// refused while the draft and chips are still on screen.
pub(crate) fn check_resource_refs(
    store: &dyn ResourceStore,
    workspace_id: &str,
    refs: &[Value],
) -> Result<(), MaterializeError> {
    let mut seen: Vec<RefParts> = Vec::new();
    for value in refs {
        let part = parse_ref(value, workspace_id)?;
        if !seen
            .iter()
            .any(|p| p.resource_id == part.resource_id && p.revision_id == part.revision_id)
        {
            seen.push(part);
        }
    }
    if seen.len() > MAX_ATTACHED_RESOURCES_PER_MESSAGE {
        return Err(MaterializeError::TooMany);
    }
    for part in &seen {
        materialize_one(store, workspace_id, part)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use storage_core::conversation::ConversationContextMessage;
    use storage_core::{
        BlobPurpose, EventDraft, ResourceRecord, ResourceRevisionRecord, WorkspaceCreateRequest,
    };
    use storage_sqlite::{
        FileBlobStore, SqliteConfig, SqliteWorkspaceStore, WorkspaceBlobKey,
        WorkspaceBlobKeyProvider,
    };

    const WORKSPACE: &str = "materialize-workspace";
    const OTHER: &str = "materialize-other-workspace";
    const OWNER: &str = "materialize-owner";
    const NOW: &str = "2026-10-20T12:00:00Z";

    struct Keys;
    impl WorkspaceBlobKeyProvider for Keys {
        fn current_key(&self, _w: &str, _p: BlobPurpose) -> Result<WorkspaceBlobKey, StoreError> {
            Ok(WorkspaceBlobKey {
                version: 1,
                bytes: zeroize::Zeroizing::new([73_u8; 32]),
            })
        }
        fn key_by_version(
            &self,
            w: &str,
            p: BlobPurpose,
            v: u32,
        ) -> Result<WorkspaceBlobKey, StoreError> {
            if v != 1 {
                return Err(StoreError::Blob("unknown test key version".to_owned()));
            }
            self.current_key(w, p)
        }
    }

    fn open(directory: &tempfile::TempDir) -> SqliteWorkspaceStore {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .expect("private dir");
        }
        SqliteWorkspaceStore::open(
            directory.path().join("state.sqlite3"),
            Arc::new(FileBlobStore::new(directory.path().join("blobs"), Keys)),
            SqliteConfig {
                writer_queue_capacity: 8,
                busy_timeout: std::time::Duration::from_secs(2),
            },
        )
        .expect("store opens")
    }

    fn workspace(store: &SqliteWorkspaceStore, id: &str) {
        use domain_workspace::{CreateWorkspace, EventContext, WorkspaceService};
        WorkspaceService::new(store.clone())
            .create(CreateWorkspace {
                workspace_id: id.to_owned(),
                name: "Materializer fixture".to_owned(),
                owner_principal_id: OWNER.to_owned(),
                event: EventContext {
                    event_id: format!("event-{id}"),
                    origin_runtime_id: "materialize-runtime".to_owned(),
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: format!("event-{id}"),
                    causation_id: None,
                    recorded_at: NOW.to_owned(),
                },
            })
            .expect("workspace commits");
    }

    fn resource(
        store: &SqliteWorkspaceStore,
        workspace_id: &str,
        resource_id: &str,
        name: &str,
        media: &str,
        sensitivity: &str,
        content: &[u8],
    ) -> String {
        use sha2::{Digest, Sha256};
        let revision_id = format!("revision-{resource_id}");
        let digest = format!("sha256:{:x}", Sha256::digest(content));
        store
            .create_resource(
                WorkspaceCreateRequest {
                    principal_id: OWNER.to_owned(),
                    request_id: format!("request-{resource_id}"),
                    request_payload: json!({"operation":"resource.quick_import.v1","workspace_id":workspace_id,"display_name":name,"media_type":media,"content_digest":digest,"size_bytes":content.len()}),
                },
                ResourceRecord {
                    resource_id: resource_id.to_owned(),
                    workspace_id: workspace_id.to_owned(),
                    kind: "FILE".to_owned(),
                    provider_identity: json!({}),
                    identity_digest: None,
                    display_name: name.to_owned(),
                    current_revision_id: Some(revision_id.clone()),
                    sensitivity: sensitivity.to_owned(),
                    provenance: json!({"source_inputs": [], "transformations": [], "tool_reports": []}),
                    created_at: NOW.to_owned(),
                    updated_at: NOW.to_owned(),
                    version: 1,
                },
                ResourceRevisionRecord {
                    resource_revision_id: revision_id.clone(),
                    resource_id: resource_id.to_owned(),
                    parent_revision_ids: Vec::new(),
                    provider_revision: None,
                    content_digest: Some(digest.clone()),
                    size_bytes: Some(content.len() as u64),
                    media_type: Some(media.to_owned()),
                    observed_at: NOW.to_owned(),
                    created_by: json!({"kind": "USER", "principal_id": OWNER}),
                },
                EventDraft {
                    event_id: format!("event-{resource_id}"),
                    workspace_id: workspace_id.to_owned(),
                    entity_type: "Resource".to_owned(),
                    entity_id: resource_id.to_owned(),
                    origin_runtime_id: "materialize-runtime".to_owned(),
                    entity_revision: 1,
                    hlc_timestamp: NOW.to_owned(),
                    correlation_id: format!("event-{resource_id}"),
                    causation_id: None,
                    schema_version: 1,
                    event_type: "resource.created.v1".to_owned(),
                    payload: json!({"resource_id":resource_id,"workspace_id":workspace_id,"kind":"FILE","aggregate_version":1}),
                    recorded_at: NOW.to_owned(),
                },
                content.to_vec(),
            )
            .expect("resource commits");
        revision_id
    }

    fn package(
        refs: serde_json::Value,
        extra_block: Option<serde_json::Value>,
    ) -> ConversationContextPackage {
        let mut content = vec![json!({"kind":"TEXT","text":"summarize the attachment"})];
        content.extend(extra_block);
        ConversationContextPackage {
            conversation_id: "c".to_owned(),
            target_agent_binding_id: "b".to_owned(),
            through_message_id: "m".to_owned(),
            omitted_message_count: 0,
            messages: vec![ConversationContextMessage {
                message_id: "m".to_owned(),
                role: "USER".to_owned(),
                turn_id: Some("t".to_owned()),
                source_agent_binding_id: None,
                content: serde_json::Value::Array(content),
                resource_refs: refs,
                created_at: NOW.to_owned(),
            }],
        }
    }

    fn reference(
        workspace_id: &str,
        resource_id: &str,
        revision_id: Option<&str>,
    ) -> serde_json::Value {
        match revision_id {
            Some(revision) => {
                json!({"workspace_id":workspace_id,"resource_id":resource_id,"revision_id":revision})
            }
            None => json!({"workspace_id":workspace_id,"resource_id":resource_id}),
        }
    }

    #[test]
    fn conversation_resource_materialization_inlines_text_with_header_and_digest() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = open(&directory);
        workspace(&store, WORKSPACE);
        let revision = resource(
            &store,
            WORKSPACE,
            "notes",
            "notes.txt",
            "text/plain",
            "PERSONAL",
            b"alpha beta",
        );
        let materialized = materialize_conversation_resources(
            &store,
            WORKSPACE,
            package(
                json!([reference(WORKSPACE, "notes", Some(&revision))]),
                None,
            ),
        )
        .expect("text resource materializes");
        let message = &materialized.messages[0];
        assert_eq!(message.resource_refs, json!([]));
        let blocks = message.content.as_array().expect("blocks");
        assert_eq!(blocks.len(), 2);
        let text = blocks[1]["text"].as_str().expect("text block");
        assert!(text.starts_with("[Attached resource BEGIN id=notes revision=revision-notes name=notes.txt media_type=text/plain bytes=10 digest=sha256:"));
        assert!(text.contains("\nalpha beta\n[Attached resource END id=notes]"));
        // The materialized package encodes; the encoder still refuses unmaterialized refs.
        assert!(
            crate::agents::conversation_prompt::encode_conversation_context_prompt(&materialized)
                .is_ok()
        );
    }

    #[test]
    fn conversation_resource_materialization_resource_block_and_ref_dedupe_to_one_attachment() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = open(&directory);
        workspace(&store, WORKSPACE);
        resource(
            &store,
            WORKSPACE,
            "notes",
            "notes.md",
            "text/markdown",
            "PUBLIC",
            b"hello",
        );
        let block = json!({"kind":"RESOURCE","resource_ref":reference(WORKSPACE, "notes", None)});
        let out = materialize_conversation_resources(
            &store,
            WORKSPACE,
            package(json!([reference(WORKSPACE, "notes", None)]), Some(block)),
        )
        .expect("materializes");
        assert_eq!(
            out.messages[0].content.as_array().expect("blocks").len(),
            2,
            "text block plus one attachment"
        );
    }

    #[test]
    fn conversation_resource_materialization_refuses_foreign_stale_binary_restricted_and_oversized()
    {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = open(&directory);
        workspace(&store, WORKSPACE);
        workspace(&store, OTHER);
        resource(
            &store,
            WORKSPACE,
            "notes",
            "notes.txt",
            "text/plain",
            "PERSONAL",
            b"x",
        );
        resource(
            &store,
            OTHER,
            "foreign",
            "foreign.txt",
            "text/plain",
            "PERSONAL",
            b"secret",
        );
        resource(
            &store,
            WORKSPACE,
            "image",
            "photo.png",
            "image/png",
            "PERSONAL",
            &[0x89, b'P', b'N', b'G'],
        );
        resource(
            &store,
            WORKSPACE,
            "locked",
            "locked.txt",
            "text/plain",
            "RESTRICTED",
            b"x",
        );
        resource(
            &store,
            WORKSPACE,
            "bad",
            "bad.txt",
            "text/plain",
            "PERSONAL",
            &[0xff, 0xfe, 0xfd],
        );
        resource(
            &store,
            WORKSPACE,
            "huge",
            "huge.txt",
            "text/plain",
            "PERSONAL",
            &vec![b'a'; (MAX_ATTACHED_RESOURCE_BYTES + 1) as usize],
        );
        let run = |refs: serde_json::Value| {
            materialize_conversation_resources(&store, WORKSPACE, package(refs, None)).err()
        };

        // A ref naming another Workspace is refused by identity, and a foreign Resource id
        // under this Workspace does not resolve.
        assert_eq!(
            run(json!([reference(OTHER, "foreign", None)])),
            Some(MaterializeError::NotAuthorized)
        );
        assert_eq!(
            run(json!([reference(WORKSPACE, "foreign", None)])),
            Some(MaterializeError::NotFound)
        );
        assert_eq!(
            run(json!([reference(WORKSPACE, "missing", None)])),
            Some(MaterializeError::NotFound)
        );
        // A pinned revision that is not the current head is a changed revision.
        assert_eq!(
            run(json!([reference(WORKSPACE, "notes", Some("revision-old"))])),
            Some(MaterializeError::StaleRevision)
        );
        assert_eq!(
            run(json!([reference(WORKSPACE, "image", None)])),
            Some(MaterializeError::BinaryUnavailable)
        );
        assert_eq!(
            run(json!([reference(WORKSPACE, "locked", None)])),
            Some(MaterializeError::Restricted)
        );
        assert_eq!(
            run(json!([reference(WORKSPACE, "bad", None)])),
            Some(MaterializeError::NotText)
        );
        assert_eq!(
            run(json!([reference(WORKSPACE, "huge", None)])),
            Some(MaterializeError::TooLarge)
        );
        assert_eq!(
            run(json!("not-an-array")),
            Some(MaterializeError::NotAuthorized)
        );
        // Submit-time check applies the same rules without building a package.
        assert_eq!(
            check_resource_refs(&store, WORKSPACE, &[reference(WORKSPACE, "image", None)]),
            Err(MaterializeError::BinaryUnavailable)
        );
        assert_eq!(
            check_resource_refs(&store, WORKSPACE, &[reference(WORKSPACE, "notes", None)]),
            Ok(())
        );
    }

    #[test]
    fn conversation_resource_materialization_refuses_pinned_revision_without_unique_current_head() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = open(&directory);
        workspace(&store, WORKSPACE);
        let revision = resource(
            &store,
            WORKSPACE,
            "branching-notes",
            "notes.txt",
            "text/plain",
            "PERSONAL",
            b"branch content",
        );

        // Simulate a legacy conflicted Resource row. The current schema's write trigger
        // prevents creating this state through a normal mutation, but a valid older DB can
        // already contain a null head when opened by a newer binary.
        let connection = rusqlite::Connection::open(directory.path().join("state.sqlite3"))
            .expect("open fixture database");
        connection
            .busy_timeout(std::time::Duration::from_secs(2))
            .expect("set fixture database timeout");
        connection
            .execute_batch("DROP TRIGGER resource_content_blob_registry_head_update_guard")
            .expect("allow legacy conflict fixture");
        let (digest, size_bytes, media_type): (String, i64, String) = connection
            .query_row(
                "SELECT content_digest, size_bytes, media_type FROM resource_revisions WHERE resource_id = ?1 AND resource_revision_id = ?2",
                rusqlite::params!["branching-notes", revision],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read first revision metadata");
        connection
            .execute(
                "INSERT INTO resource_revisions(resource_revision_id, resource_id, provider_revision, content_digest, size_bytes, media_type, observed_at, created_by_json) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, '{}')",
                rusqlite::params![
                    "revision-branching-notes-sibling",
                    "branching-notes",
                    digest,
                    size_bytes,
                    media_type,
                    NOW
                ],
            )
            .expect("add an incomparable sibling revision");
        let changed = connection
            .execute(
                "UPDATE resources SET current_revision_id = NULL WHERE workspace_id = ?1 AND resource_id = ?2",
                rusqlite::params![WORKSPACE, "branching-notes"],
            )
            .expect("remove unique current head");
        assert_eq!(changed, 1);

        let pinned = reference(WORKSPACE, "branching-notes", Some(&revision));
        assert_eq!(
            materialize_conversation_resources(
                &store,
                WORKSPACE,
                package(json!([pinned.clone()]), None),
            )
            .err(),
            Some(MaterializeError::StaleRevision),
            "dispatch-time materialization must refuse a branch when no unique head exists"
        );
        assert_eq!(
            check_resource_refs(&store, WORKSPACE, &[pinned]),
            Err(MaterializeError::StaleRevision),
            "submit-time validation must apply the same unique-head rule"
        );
    }
}
