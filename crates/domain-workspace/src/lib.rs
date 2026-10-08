use serde_json::json;
use storage_core::{
    CommittedWorkspace, CommittedWorkspaceInstructionRevision, CommittedWorkspaceRoot, EventDraft,
    IdempotentWorkspaceStore, ReplicationPolicy, StoreError, Workspace, WorkspaceCreateRequest,
    ResourceUploadSessionRecord, ResourceUploadState, ResourceUploadStore,
    CommittedContextDocumentStatus, ContextDocumentOwnerStatus, ContextDocumentStatusCommand,
    ContextDocumentStatusEventContext, ContextDocumentStatusStore,
    is_sha256_digest,
    FolderImportMetadata, LocalResourceLocationBindingRecord, ResourceLocationRecord,
    ResourceRecord, WorkspaceInstructionRevisionRecord, WorkspaceRootCreateCommit,
    WorkspaceRootRecord, WorkspaceRootStatusAction, WorkspaceRootStatusCommit,
    WorkspaceRootListRecord,
    WorkspaceRootResumeCommit,
    WorkspaceRootStore, WorkspaceStore,
    LocalFileIdentityBindingRecord,
};

#[cfg(test)]
const RECORD_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct EventContext {
    pub event_id: String,
    pub origin_runtime_id: String,
    pub hlc_timestamp: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub recorded_at: String,
}

#[derive(Clone, Debug)]
pub struct CreateWorkspace {
    pub workspace_id: String,
    pub name: String,
    pub owner_principal_id: String,
    pub event: EventContext,
}

#[derive(Clone, Debug)]
pub struct ChangeReplicationPolicy {
    pub workspace_id: String,
    pub expected_version: u64,
    pub policy: ReplicationPolicy,
    pub replication_scope_root_ids: Vec<String>,
    pub event: EventContext,
}

#[derive(Clone, Debug)]
pub struct CreateWorkspaceInstructionRevision {
    pub workspace_id: String,
    pub expected_version: u64,
    pub parent_revisions: Vec<u64>,
    pub content_ref: serde_json::Value,
    pub content_digest: String,
    pub authored_by_principal_id: String,
    pub event: EventContext,
}

#[derive(Clone, Debug)]
pub struct SetWorkspaceDefaultAgentBinding {
    pub workspace_id: String,
    pub expected_version: u64,
    pub agent_binding_id: Option<String>,
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: serde_json::Value,
    pub event: EventContext,
}

/// Sets or clears the default Coworker used to prefill future work. The selected
/// Coworker must belong to this Workspace and be active or paused; storage enforces
/// that relationship in the same transaction as the versioned Workspace update.
#[derive(Clone, Debug)]
pub struct SetWorkspacePrimaryCoworker {
    pub workspace_id: String,
    pub expected_version: u64,
    pub coworker_id: Option<String>,
    pub principal_id: String,
    pub request_id: String,
    pub request_payload: serde_json::Value,
    pub event: EventContext,
}

/// One bounded desktop file or one file selected from a folder. The optional folder
/// path is provenance only; the daemon never resolves it against the filesystem.
#[derive(Clone, Debug)]
pub struct CreateResourceUploadSession {
    pub upload_id: String,
    pub workspace_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub display_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub expected_digest: String,
    pub context_document: Option<serde_json::Value>,
    pub folder_import: Option<FolderImportMetadata>,
    pub expires_at: String,
    pub event: EventContext,
}

/// Starts a new immutable ResourceRevision upload. The content is not accepted until
/// storage atomically validates the exact Resource version and complete parent-head set.
#[derive(Clone, Debug)]
pub struct CreateResourceRevisionUploadSession {
    pub upload_id: String,
    pub workspace_id: String,
    pub resource_id: String,
    pub expected_resource_version: u64,
    pub parent_revision_ids: Vec<String>,
    pub principal_id: String,
    pub request_id: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub expected_digest: String,
    pub expires_at: String,
    pub event: EventContext,
}

/// Owner-requested ContextDocument revocation or restoration. Purge transitions are
/// deliberately not represented here; they require the separate purge reconciler.
#[derive(Clone, Debug)]
pub struct SetContextDocumentStatus {
    pub workspace_id: String,
    pub resource_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub expected_version: u64,
    pub target_status: ContextDocumentOwnerStatus,
    pub event: EventContext,
}

/// ResourceService owns ContextDocument owner status transitions. Storage performs the
/// final owner/version/status checks and commits the status, snapshot, event and replay
/// receipt as one transaction.
pub struct ResourceService<S> {
    store: S,
}

impl<S> ResourceService<S>
where
    S: ContextDocumentStatusStore + WorkspaceStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn set_context_document_status(
        &self,
        command: SetContextDocumentStatus,
    ) -> Result<CommittedContextDocumentStatus, StoreError> {
        for (name, value) in [
            ("workspace_id", command.workspace_id.as_str()),
            ("resource_id", command.resource_id.as_str()),
            ("principal_id", command.principal_id.as_str()),
            ("request_id", command.request_id.as_str()),
        ] {
            require_non_empty(name, value)?;
            if value.contains('\\0') {
                return Err(StoreError::Invalid(format!("{name} contains a NUL byte")));
            }
        }
        if command.expected_version == 0 {
            return Err(StoreError::Invalid("expected Resource version must be positive".to_owned()));
        }
        validate_event_context(&command.event)?;

        // This early check avoids disclosing whether another owner's Workspace exists;
        // storage repeats it atomically with the Resource transition.
        let workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != command.principal_id {
            return Err(StoreError::NotFound);
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }

        let status = match command.target_status {
            ContextDocumentOwnerStatus::Active => "ACTIVE",
            ContextDocumentOwnerStatus::Revoked => "REVOKED",
        };
        let request_payload = json!({
            "workspace_id": command.workspace_id,
            "resource_id": command.resource_id,
            "status": status,
            "expected_version": command.expected_version,
        });
        self.store.set_context_document_status(ContextDocumentStatusCommand {
            workspace_id: command.workspace_id,
            resource_id: command.resource_id,
            principal_id: command.principal_id,
            request_id: command.request_id,
            expected_version: command.expected_version,
            target_status: command.target_status,
            request_payload,
            event: ContextDocumentStatusEventContext {
                event_id: command.event.event_id,
                origin_runtime_id: command.event.origin_runtime_id,
                hlc_timestamp: command.event.hlc_timestamp,
                correlation_id: command.event.correlation_id,
                causation_id: command.event.causation_id,
                recorded_at: command.event.recorded_at,
            },
        })
    }
}

/// Domain boundary for starting desktop Resource intake. It performs ownership and
/// Workspace-state checks before invoking the durable resumable-upload store.
pub struct ResourceUploadService<S> {
    store: S,
}

/// A native folder-selection result handed to the domain only after a trusted desktop
/// provider has opened and inspected the selected directory. The raw path is carried to
/// the storage adapter as a private locator and is never copied into an event or public
/// Resource projection.
pub struct AddWorkspaceRoot {
    pub workspace_id: String,
    pub expected_workspace_version: u64,
    pub principal_id: String,
    pub request_id: String,
    pub resource_id: String,
    pub identity_digest: String,
    pub file_identity: serde_json::Value,
    pub location_id: String,
    pub locator_ref_id: String,
    pub workspace_root_id: String,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub private_locator: String,
    pub file_identity_binding: LocalFileIdentityBindingRecord,
    pub display_name: String,
    pub watch_policy: String,
    pub replication_policy: String,
    pub event: EventContext,
}

pub struct RevokeWorkspaceRoot {
    pub workspace_id: String,
    pub workspace_root_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub expected_version: u64,
    pub event: EventContext,
}

#[derive(Clone, Debug)]
pub struct ChangeWorkspaceRootStatus {
    pub workspace_id: String,
    pub workspace_root_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub expected_version: u64,
    pub action: WorkspaceRootStatusAction,
    pub runtime_id: Option<String>,
    pub runtime_incarnation_id: Option<String>,
    pub event: EventContext,
}

pub struct ResumeWorkspaceRoot {
    pub workspace_id: String,
    pub workspace_root_id: String,
    pub principal_id: String,
    pub request_id: String,
    pub expected_version: u64,
    pub runtime_id: String,
    pub runtime_incarnation_id: String,
    pub previous_locator_binding: LocalResourceLocationBindingRecord,
    pub previous_file_identity_binding: LocalFileIdentityBindingRecord,
    pub resource: ResourceRecord,
    pub location: ResourceLocationRecord,
    pub locator_binding: LocalResourceLocationBindingRecord,
    pub file_identity_binding: LocalFileIdentityBindingRecord,
    pub event: EventContext,
}

/// Owns authorization and canonical event construction for persistent local roots.
/// The platform adapter must supply an identity digest derived from the Runtime's
/// protected identity key and a directory handle it actually verified.
pub struct WorkspaceRootService<S> {
    store: S,
}

impl<S> WorkspaceRootService<S>
where
    S: WorkspaceRootStore + WorkspaceStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn add_root(&self, command: AddWorkspaceRoot) -> Result<CommittedWorkspaceRoot, StoreError> {
        for (name, value) in [
            ("workspace_id", command.workspace_id.as_str()),
            ("principal_id", command.principal_id.as_str()),
            ("request_id", command.request_id.as_str()),
            ("resource_id", command.resource_id.as_str()),
            ("location_id", command.location_id.as_str()),
            ("locator_ref_id", command.locator_ref_id.as_str()),
            ("workspace_root_id", command.workspace_root_id.as_str()),
            ("runtime_id", command.runtime_id.as_str()),
            ("runtime_incarnation_id", command.runtime_incarnation_id.as_str()),
            ("private_locator", command.private_locator.as_str()),
            ("display_name", command.display_name.as_str()),
        ] {
            require_non_empty(name, value)?;
            if value.contains('\0') {
                return Err(StoreError::Invalid(format!("{name} contains a NUL byte")));
            }
        }
        if !storage_core::is_sha256_digest(&command.identity_digest) {
            return Err(StoreError::Invalid("folder identity digest is invalid".to_owned()));
        }
        if command.display_name.chars().any(char::is_control)
            || command.display_name.len() > 255
            || !matches!(command.watch_policy.as_str(), "METADATA" | "CONTENT_DIGESTS" | "SELECTED_TEXT_EXTRACTION")
            || !matches!(command.replication_policy.as_str(), "NONE" | "ACTIVE_TASKS" | "SELECTED_WORKSPACE_POLICY")
        {
            return Err(StoreError::Invalid("WorkspaceRoot display name or policy is invalid".to_owned()));
        }
        validate_event_context(&command.event)?;

        let workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != command.principal_id {
            return Err(StoreError::NotFound);
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }
        if workspace.version != command.expected_workspace_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_workspace_version),
                actual: Some(workspace.version),
            });
        }

        let provenance = json!({"source_inputs": [], "transformations": [], "tool_reports": []});
        let resource = ResourceRecord {
            resource_id: command.resource_id.clone(),
            workspace_id: command.workspace_id.clone(),
            kind: "FOLDER".to_owned(),
            provider_identity: json!({
                "provider_instance_id": "litecowork.local_filesystem",
                "stable_object_id": command.identity_digest.clone(),
                "identity_confidence": "PROVIDER_SCOPED",
                "file_identity": command.file_identity.clone(),
            }),
            identity_digest: Some(command.identity_digest.clone()),
            display_name: command.display_name.clone(),
            current_revision_id: None,
            sensitivity: "PERSONAL".to_owned(),
            provenance: provenance.clone(),
            created_at: command.event.recorded_at.clone(),
            updated_at: command.event.recorded_at.clone(),
            version: 1,
        };
        let location = ResourceLocationRecord {
            location_id: command.location_id.clone(),
            resource_id: command.resource_id.clone(),
            runtime_id: command.runtime_id.clone(),
            locator_ref_id: command.locator_ref_id.clone(),
            provider_ref: "litecowork.local_filesystem".to_owned(),
            availability: "AVAILABLE".to_owned(),
            writable: false,
            observed_revision_id: None,
            observed_digest: None,
            observed_at: command.event.recorded_at.clone(),
        };
        let added_by = json!({"kind": "USER", "principal_id": command.principal_id.clone()});
        let root = WorkspaceRootRecord {
            workspace_root_id: command.workspace_root_id.clone(),
            workspace_id: command.workspace_id.clone(),
            resource_id: command.resource_id.clone(),
            location_id: command.location_id.clone(),
            display_name: command.display_name.clone(),
            watch_policy: command.watch_policy.clone(),
            replication_policy: command.replication_policy.clone(),
            status: "ACTIVE".to_owned(),
            added_by: added_by.clone(),
            created_at: command.event.recorded_at.clone(),
            updated_at: command.event.recorded_at.clone(),
            version: 1,
        };
        let request = WorkspaceCreateRequest {
            principal_id: command.principal_id.clone(),
            request_id: command.request_id.clone(),
            // Never include the absolute filesystem path or native provider handle.
            request_payload: json!({
                "operation": "workspace.root.create.v1",
                "workspace_id": command.workspace_id.clone(),
                "expected_workspace_version": command.expected_workspace_version,
                "resource_id": command.resource_id.clone(),
                "identity_digest": command.identity_digest.clone(),
                "location_id": command.location_id.clone(),
                "locator_ref_id": command.locator_ref_id.clone(),
                "workspace_root_id": command.workspace_root_id.clone(),
                "runtime_id": command.runtime_id.clone(),
                "runtime_incarnation_id": command.runtime_incarnation_id.clone(),
                "display_name": command.display_name.clone(),
                "watch_policy": command.watch_policy.clone(),
                "replication_policy": command.replication_policy.clone(),
            }),
        };
        let resource_event = resource_event(
            &command.event,
            &format!("{}:resource", command.event.event_id),
            &resource,
            "resource.created.v1",
            "Resource",
            json!({
                "resource_id": resource.resource_id.clone(),
                "workspace_id": resource.workspace_id.clone(),
                "kind": resource.kind.clone(),
                "identity_digest": resource.identity_digest.clone(),
                "provenance": provenance.clone(),
                "aggregate_version": resource.version,
            }),
        );
        let location_event = resource_event(
            &command.event,
            &format!("{}:location", command.event.event_id),
            &resource,
            "resource.location.changed.v1",
            "Resource",
            json!({
                "location_id": location.location_id.clone(),
                "resource_id": location.resource_id.clone(),
                "availability": location.availability.clone(),
                "observed_at": location.observed_at.clone(),
            }),
        );
        let root_event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: root.workspace_id.clone(),
            entity_type: "WorkspaceRoot".to_owned(),
            entity_id: root.workspace_root_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: root.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.root.created.v1".to_owned(),
            payload: json!({
                "workspace_root_id": root.workspace_root_id.clone(),
                "workspace_id": root.workspace_id.clone(),
                "resource_id": root.resource_id.clone(),
                "location_id": root.location_id.clone(),
                "added_by": added_by.clone(),
                "aggregate_version": root.version,
            }),
            recorded_at: command.event.recorded_at,
        };
        let private_binding = LocalResourceLocationBindingRecord {
            location_id: location.location_id.clone(),
            locator_ref_id: location.locator_ref_id.clone(),
            runtime_id: location.runtime_id.clone(),
            runtime_incarnation_id: command.runtime_incarnation_id,
            private_locator: command.private_locator,
            observed_at: location.observed_at.clone(),
        };
        self.store.create_workspace_root(WorkspaceRootCreateCommit {
            request,
            resource,
            location,
            private_binding,
            file_identity_binding: command.file_identity_binding,
            root,
            resource_created_event: resource_event,
            location_observed_event: location_event,
            root_created_event: root_event,
        })
    }

    pub fn list_roots(
        &self,
        workspace_id: &str,
        principal_id: &str,
        status: Option<&str>,
        after_created_at: Option<&str>,
        after_workspace_root_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WorkspaceRootListRecord>, StoreError> {
        require_non_empty("workspace_id", workspace_id)?;
        require_non_empty("principal_id", principal_id)?;
        let workspace = self.store.get_workspace(workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != principal_id {
            return Err(StoreError::NotFound);
        }
        self.store.list_workspace_roots(
            workspace_id,
            status,
            after_created_at,
            after_workspace_root_id,
            limit,
        )
    }

    pub fn revoke_root(
        &self,
        command: RevokeWorkspaceRoot,
    ) -> Result<storage_core::CommittedWorkspaceRootStatus, StoreError> {
        self.change_root_status(ChangeWorkspaceRootStatus {
            workspace_id: command.workspace_id,
            workspace_root_id: command.workspace_root_id,
            principal_id: command.principal_id,
            request_id: command.request_id,
            expected_version: command.expected_version,
            action: WorkspaceRootStatusAction::Revoke,
            runtime_id: None,
            runtime_incarnation_id: None,
            event: command.event,
        })
    }

    /// Resumes a paused root only from a platform provider's fresh, verified directory
    /// observation. The store commits that observation and the owner transition together.
    pub fn resume_root(
        &self,
        command: ResumeWorkspaceRoot,
    ) -> Result<storage_core::CommittedWorkspaceRootStatus, StoreError> {
        for (name, value) in [
            ("workspace_id", command.workspace_id.as_str()),
            ("workspace_root_id", command.workspace_root_id.as_str()),
            ("principal_id", command.principal_id.as_str()),
            ("request_id", command.request_id.as_str()),
            ("runtime_id", command.runtime_id.as_str()),
            ("runtime_incarnation_id", command.runtime_incarnation_id.as_str()),
        ] {
            require_non_empty(name, value)?;
            if value.contains('\0') {
                return Err(StoreError::Invalid(format!("{name} contains a NUL byte")));
            }
        }
        validate_event_context(&command.event)?;
        let workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != command.principal_id {
            return Err(StoreError::NotFound);
        }
        let request = workspace_root_status_request(
            &command.workspace_id,
            &command.workspace_root_id,
            &command.principal_id,
            &command.request_id,
            command.expected_version,
            WorkspaceRootStatusAction::Resume,
        );
        if let Some(receipt) = self.store.get_workspace_root_status_receipt(&request)? {
            return Ok(receipt);
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }
        let mut root = self.store.get_workspace_root(&command.workspace_id, &command.workspace_root_id)?
            .ok_or(StoreError::NotFound)?;
        if root.version != command.expected_version || root.status != "PAUSED" {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(root.version),
            });
        }
        if command.resource.workspace_id != command.workspace_id
            || command.resource.resource_id != root.resource_id
            || command.location.resource_id != root.resource_id
            || command.location.location_id != root.location_id
            || command.location.runtime_id != command.runtime_id
            || command.location.locator_ref_id != command.locator_binding.locator_ref_id
            || command.location.availability == "REVOKED"
            || command.location.writable
            || command.location.observed_revision_id.is_some()
            || command.location.observed_digest.is_some()
            || command.previous_locator_binding.location_id != root.location_id
            || command.previous_file_identity_binding.location_id != root.location_id
            || command.locator_binding.location_id != root.location_id
            || command.file_identity_binding.location_id != root.location_id
            || command.previous_locator_binding.private_locator != command.locator_binding.private_locator
            || command.previous_locator_binding.locator_ref_id != command.locator_binding.locator_ref_id
            || command.previous_file_identity_binding.raw_filesystem_instance_id != command.file_identity_binding.raw_filesystem_instance_id
            || command.previous_file_identity_binding.raw_volume_id != command.file_identity_binding.raw_volume_id
            || command.previous_file_identity_binding.raw_file_id != command.file_identity_binding.raw_file_id
            || command.previous_file_identity_binding.raw_generation != command.file_identity_binding.raw_generation
            || command.previous_file_identity_binding.platform_kind != command.file_identity_binding.platform_kind
            || command.locator_binding.runtime_id != command.runtime_id
            || command.locator_binding.runtime_incarnation_id != command.runtime_incarnation_id
            || command.file_identity_binding.runtime_id != command.runtime_id
            || command.file_identity_binding.runtime_incarnation_id != command.runtime_incarnation_id
        {
            return Err(StoreError::Invalid("fresh WorkspaceRoot identity proof is inconsistent".to_owned()));
        }
        let next_version = root.version.checked_add(1)
            .ok_or_else(|| StoreError::Invalid("WorkspaceRoot version overflow".to_owned()))?;
        root.status = "ACTIVE".to_owned();
        root.updated_at = command.event.recorded_at.clone();
        root.version = next_version;
        let mut location = command.location;
        location.availability = "AVAILABLE".to_owned();
        location.observed_at = command.event.recorded_at.clone();
        let mut locator_binding = command.locator_binding;
        locator_binding.observed_at = command.event.recorded_at.clone();
        let mut file_identity_binding = command.file_identity_binding;
        file_identity_binding.observed_at = command.event.recorded_at.clone();
        let root_event = EventDraft {
            event_id: command.event.event_id.clone(),
            workspace_id: root.workspace_id.clone(),
            entity_type: "WorkspaceRoot".to_owned(),
            entity_id: root.workspace_root_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id.clone(),
            entity_revision: root.version,
            hlc_timestamp: command.event.hlc_timestamp.clone(),
            correlation_id: command.event.correlation_id.clone(),
            causation_id: command.event.causation_id.clone(),
            schema_version: 1,
            event_type: "workspace.root.status.changed.v1".to_owned(),
            payload: json!({
                "workspace_root_id": root.workspace_root_id,
                "from": "PAUSED",
                "to": "ACTIVE",
                "reason_code": "USER_RESUMED",
                "aggregate_version": root.version,
            }),
            recorded_at: command.event.recorded_at.clone(),
        };
        let location_event = EventDraft {
            event_id: format!("{}:location", command.event.event_id),
            workspace_id: root.workspace_id.clone(),
            entity_type: "Resource".to_owned(),
            entity_id: command.resource.resource_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: command.resource.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "resource.location.changed.v1".to_owned(),
            payload: json!({
                "location_id": location.location_id,
                "resource_id": location.resource_id,
                "availability": "AVAILABLE",
                "observed_at": location.observed_at,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.resume_workspace_root(WorkspaceRootResumeCommit {
            request,
            expected_version: command.expected_version,
            runtime_id: command.runtime_id,
            runtime_incarnation_id: command.runtime_incarnation_id,
            previous_locator_binding: command.previous_locator_binding,
            previous_file_identity_binding: command.previous_file_identity_binding,
            resource: command.resource,
            location,
            locator_binding,
            file_identity_binding,
            root,
            root_event,
            location_event,
        })
    }

    /// Looks up the owner-intent receipt before a Resume performs a fresh filesystem
    /// observation. This preserves idempotent replay without returning a stale receipt
    /// only after changing ResourceLocation availability again.
    pub fn get_root_status_receipt(
        &self,
        workspace_id: &str,
        workspace_root_id: &str,
        principal_id: &str,
        request_id: &str,
        expected_version: u64,
        action: WorkspaceRootStatusAction,
    ) -> Result<Option<storage_core::CommittedWorkspaceRootStatus>, StoreError> {
        let workspace = self.store.get_workspace(workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != principal_id {
            return Err(StoreError::NotFound);
        }
        self.store.get_workspace_root_status_receipt(&workspace_root_status_request(
            workspace_id,
            workspace_root_id,
            principal_id,
            request_id,
            expected_version,
            action,
        ))
    }

    pub fn change_root_status(
        &self,
        command: ChangeWorkspaceRootStatus,
    ) -> Result<storage_core::CommittedWorkspaceRootStatus, StoreError> {
        for (name, value) in [
            ("workspace_id", command.workspace_id.as_str()),
            ("workspace_root_id", command.workspace_root_id.as_str()),
            ("principal_id", command.principal_id.as_str()),
            ("request_id", command.request_id.as_str()),
        ] {
            require_non_empty(name, value)?;
            if value.contains('\0') {
                return Err(StoreError::Invalid(format!("{name} contains a NUL byte")));
            }
        }
        validate_event_context(&command.event)?;
        let workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != command.principal_id {
            return Err(StoreError::NotFound);
        }
        let resume_runtime = match command.action {
            WorkspaceRootStatusAction::Resume => {
                let runtime_id = command.runtime_id.as_deref().filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| StoreError::Invalid("root resume requires a Runtime".to_owned()))?;
                let incarnation_id = command.runtime_incarnation_id.as_deref().filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| StoreError::Invalid("root resume requires a Runtime incarnation".to_owned()))?;
                Some((runtime_id.to_owned(), incarnation_id.to_owned()))
            }
            _ if command.runtime_id.is_none() && command.runtime_incarnation_id.is_none() => None,
            _ => return Err(StoreError::Invalid("only root resume accepts a Runtime incarnation".to_owned())),
        };
        // The request digest contains only stable owner intent. Runtime identity
        // is a resume admission precondition, not part of the idempotency key:
        // retries after daemon restart must replay the committed response.
        let request = workspace_root_status_request(
            &command.workspace_id,
            &command.workspace_root_id,
            &command.principal_id,
            &command.request_id,
            command.expected_version,
            command.action,
        );
        if let Some(receipt) = self.store.get_workspace_root_status_receipt(&request)? {
            return Ok(receipt);
        }
        if command.action == WorkspaceRootStatusAction::Resume {
            return Err(StoreError::Invalid("root Resume requires a fresh filesystem identity proof".to_owned()));
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }
        let current = self.store.get_workspace_root(&command.workspace_id, &command.workspace_root_id)?
            .ok_or(StoreError::NotFound)?;
        if current.version != command.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(current.version),
            });
        }
        let transition_allowed = match command.action {
            WorkspaceRootStatusAction::Pause => current.status == "ACTIVE",
            WorkspaceRootStatusAction::Resume => current.status == "PAUSED",
            WorkspaceRootStatusAction::Revoke => matches!(current.status.as_str(), "ACTIVE" | "PAUSED" | "UNAVAILABLE"),
        };
        if !transition_allowed {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(current.version),
            });
        }
        let next_version = current.version.checked_add(1)
            .ok_or_else(|| StoreError::Invalid("WorkspaceRoot version overflow".to_owned()))?;
        let mut root = current;
        let from_status = root.status.clone();
        root.status = command.action.target_status().to_owned();
        root.updated_at = command.event.recorded_at.clone();
        root.version = next_version;
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: root.workspace_id.clone(),
            entity_type: "WorkspaceRoot".to_owned(),
            entity_id: root.workspace_root_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: root.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.root.status.changed.v1".to_owned(),
            payload: json!({
                "workspace_root_id": root.workspace_root_id,
                "from": from_status,
                "to": command.action.target_status(),
                "reason_code": command.action.reason_code(),
                "aggregate_version": root.version,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.update_workspace_root_status(WorkspaceRootStatusCommit {
            request,
            expected_version: command.expected_version,
            action: command.action,
            runtime_id: resume_runtime.as_ref().map(|value| value.0.clone()),
            runtime_incarnation_id: resume_runtime.map(|value| value.1),
            root,
            event,
        })
    }
}

fn workspace_root_status_request(
    workspace_id: &str,
    workspace_root_id: &str,
    principal_id: &str,
    request_id: &str,
    expected_version: u64,
    action: WorkspaceRootStatusAction,
) -> WorkspaceCreateRequest {
    WorkspaceCreateRequest {
        principal_id: principal_id.to_owned(),
        request_id: request_id.to_owned(),
        request_payload: json!({
            "operation": action.operation(),
            "workspace_id": workspace_id,
            "workspace_root_id": workspace_root_id,
            "expected_version": expected_version,
        }),
    }
}

fn resource_event(
    context: &EventContext,
    event_id: &str,
    resource: &ResourceRecord,
    event_type: &str,
    entity_type: &str,
    payload: serde_json::Value,
) -> EventDraft {
    EventDraft {
        event_id: event_id.to_owned(),
        workspace_id: resource.workspace_id.clone(),
        entity_type: entity_type.to_owned(),
        entity_id: resource.resource_id.clone(),
        origin_runtime_id: context.origin_runtime_id.clone(),
        entity_revision: resource.version,
        hlc_timestamp: context.hlc_timestamp.clone(),
        correlation_id: context.correlation_id.clone(),
        causation_id: context.causation_id.clone(),
        schema_version: 1,
        event_type: event_type.to_owned(),
        payload,
        recorded_at: context.recorded_at.clone(),
    }
}

impl<S> ResourceUploadService<S>
where
    S: ResourceUploadStore + WorkspaceStore,
{
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn create_session(
        &self,
        command: CreateResourceUploadSession,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        require_non_empty("upload_id", &command.upload_id)?;
        require_non_empty("principal_id", &command.principal_id)?;
        require_non_empty("request_id", &command.request_id)?;
        require_non_empty("display_name", &command.display_name)?;
        require_non_empty("media_type", &command.media_type)?;
        require_non_empty("expires_at", &command.expires_at)?;
        validate_event_context(&command.event)?;

        let workspace = self
            .store
            .get_workspace(&command.workspace_id)?
            .ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != command.principal_id {
            return Err(StoreError::NotFound);
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }

        let session = ResourceUploadSessionRecord {
            upload_id: command.upload_id,
            workspace_id: workspace.workspace_id.clone(),
            display_name: command.display_name,
            media_type: command.media_type,
            expected_size_bytes: command.size_bytes,
            expected_digest: Some(command.expected_digest),
            context_document: command.context_document.clone(),
            folder_import: command.folder_import,
            resource_id: None,
            committed_resource_id: None,
            expected_resource_version: None,
            parent_revision_ids: Vec::new(),
            chunk_size_bytes: 4_194_304,
            received_ranges: Vec::new(),
            next_missing_offset: 0,
            state: ResourceUploadState::Open,
            expires_at: command.expires_at,
            created_at: command.event.recorded_at.clone(),
            version: 1,
            progress_version: 1,
        };
        session.validate_initial_metadata()?;

        if command.context_document.as_ref().is_some_and(|metadata| {
            !valid_context_document_metadata(
                metadata,
                &workspace.workspace_id,
                &command.principal_id,
            )
        }) {
            return Err(StoreError::Invalid(
                "Context Document metadata does not match the authenticated owner and Workspace".to_owned(),
            ));
        }

        let mut payload = json!({
            "operation": "resource.upload.create.v1",
            "workspace_id": session.workspace_id,
            "display_name": session.display_name,
            "media_type": session.media_type,
            "size_bytes": session.expected_size_bytes,
            "expected_digest": session.expected_digest,
            "context_document": session.context_document,
        });
        if let Some(folder_import) = &session.folder_import {
            payload["folder_import"] = json!(folder_import);
        }
        let request = WorkspaceCreateRequest {
            principal_id: command.principal_id,
            request_id: command.request_id,
            request_payload: payload,
        };
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: session.workspace_id.clone(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: session.upload_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: session.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "resource.upload.created.v1".to_owned(),
            payload: json!({
                "upload_id": session.upload_id,
                "workspace_id": session.workspace_id,
                "expected_size_bytes": session.expected_size_bytes,
                "expected_digest": session.expected_digest,
                "chunk_size_bytes": session.chunk_size_bytes,
                "expires_at": session.expires_at,
                "aggregate_version": session.version,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.create(request, session, event)
    }

    pub fn create_revision_session(
        &self,
        command: CreateResourceRevisionUploadSession,
    ) -> Result<ResourceUploadSessionRecord, StoreError> {
        require_non_empty("upload_id", &command.upload_id)?;
        require_non_empty("resource_id", &command.resource_id)?;
        require_non_empty("principal_id", &command.principal_id)?;
        require_non_empty("request_id", &command.request_id)?;
        require_non_empty("media_type", &command.media_type)?;
        require_non_empty("expires_at", &command.expires_at)?;
        validate_event_context(&command.event)?;
        if command.expected_resource_version == 0
            || command.media_type.len() > 160
            || command.size_bytes > 104_857_600
            || !is_sha256_digest(&command.expected_digest)
            || command.parent_revision_ids.is_empty()
            || command.parent_revision_ids.len() > 16
            || command.parent_revision_ids.iter().any(|value| value.trim().is_empty())
        {
            return Err(StoreError::Invalid("Resource revision upload metadata is invalid".to_owned()));
        }
        let workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.owner_principal_id != command.principal_id {
            return Err(StoreError::NotFound);
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }

        let session = ResourceUploadSessionRecord {
            upload_id: command.upload_id,
            workspace_id: command.workspace_id.clone(),
            // ResourceUploadStore derives the display name from the authorized Resource
            // row before persisting its snapshot and response.
            display_name: String::new(),
            media_type: command.media_type,
            expected_size_bytes: command.size_bytes,
            expected_digest: Some(command.expected_digest),
            context_document: None,
            folder_import: None,
            resource_id: Some(command.resource_id.clone()),
            committed_resource_id: None,
            expected_resource_version: Some(command.expected_resource_version),
            parent_revision_ids: command.parent_revision_ids.clone(),
            chunk_size_bytes: 4_194_304,
            received_ranges: Vec::new(),
            next_missing_offset: 0,
            state: ResourceUploadState::Open,
            expires_at: command.expires_at,
            created_at: command.event.recorded_at.clone(),
            version: 1,
            progress_version: 1,
        };
        let payload = json!({
            "operation": "resource.revision.upload.create.v1",
            "workspace_id": command.workspace_id,
            "resource_id": command.resource_id,
            "expected_resource_version": command.expected_resource_version,
            "parent_revision_ids": command.parent_revision_ids,
            "media_type": session.media_type,
            "size_bytes": session.expected_size_bytes,
            "expected_digest": session.expected_digest,
        });
        let request = WorkspaceCreateRequest {
            principal_id: command.principal_id,
            request_id: command.request_id,
            request_payload: payload,
        };
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: session.workspace_id.clone(),
            entity_type: "ResourceUpload".to_owned(),
            entity_id: session.upload_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: session.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "resource.upload.created.v1".to_owned(),
            payload: json!({
                "upload_id": session.upload_id,
                "workspace_id": session.workspace_id,
                "expected_size_bytes": session.expected_size_bytes,
                "expected_digest": session.expected_digest,
                "chunk_size_bytes": session.chunk_size_bytes,
                "expires_at": session.expires_at,
                "resource_id": command.resource_id,
                "expected_resource_version": command.expected_resource_version,
                "parent_revision_ids": command.parent_revision_ids,
                "aggregate_version": session.version,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.create_revision(request, session, event)
    }
}

fn valid_context_document_metadata(
    metadata: &serde_json::Value,
    workspace_id: &str,
    principal_id: &str,
) -> bool {
    let Some(object) = metadata.as_object() else { return false; };
    if object.len() != 2 || !object.contains_key("kind") || !object.contains_key("owner_ref") {
        return false;
    }
    let owner = metadata.get("owner_ref");
    match metadata.get("kind").and_then(serde_json::Value::as_str) {
        Some("PERSONAL_PROFILE") => owner.is_some_and(|owner| {
            owner.as_object().is_some_and(|object| {
                object.len() == 2 && object.contains_key("kind") && object.contains_key("principal_id")
            })
                && owner.get("kind").and_then(serde_json::Value::as_str) == Some("USER")
                && owner.get("principal_id").and_then(serde_json::Value::as_str) == Some(principal_id)
        }),
        Some("WORKSPACE_NOTES") => owner.is_some_and(|owner| {
            owner.as_object().is_some_and(|object| {
                object.len() == 2 && object.contains_key("kind") && object.contains_key("workspace_id")
            })
                && owner.get("kind").and_then(serde_json::Value::as_str) == Some("WORKSPACE")
                && owner.get("workspace_id").and_then(serde_json::Value::as_str) == Some(workspace_id)
        }),
        // Coworker/Goal ownership requires their authoritative aggregate services.
        _ => false,
    }
}

pub struct WorkspaceService<S> {
    store: S,
}

impl<S: WorkspaceStore> WorkspaceService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn create(&self, command: CreateWorkspace) -> Result<CommittedWorkspace, StoreError> {
        let (workspace, event) = build_workspace_commit(command, ReplicationPolicy::LocalOnly)?;
        self.store.commit_workspace(None, workspace, event)
    }

    pub fn create_with_policy(
        &self,
        command: CreateWorkspace,
        policy: ReplicationPolicy,
    ) -> Result<CommittedWorkspace, StoreError> {
        validate_initial_replication_policy(&policy)?;
        let (workspace, event) = build_workspace_commit(command, policy)?;
        self.store.commit_workspace(None, workspace, event)
    }

    pub fn create_idempotent(
        &self,
        command: CreateWorkspace,
        request_id: String,
        request_payload: serde_json::Value,
    ) -> Result<CommittedWorkspace, StoreError>
    where
        S: IdempotentWorkspaceStore,
    {
        let request = WorkspaceCreateRequest {
            principal_id: command.owner_principal_id.clone(),
            request_id,
            request_payload,
        };
        let (workspace, event) = build_workspace_commit(command, ReplicationPolicy::LocalOnly)?;
        self.store
            .create_workspace_idempotent(request, workspace, event)
    }

    pub fn create_idempotent_with_policy(
        &self,
        command: CreateWorkspace,
        policy: ReplicationPolicy,
        request_id: String,
        request_payload: serde_json::Value,
    ) -> Result<CommittedWorkspace, StoreError>
    where
        S: IdempotentWorkspaceStore,
    {
        validate_initial_replication_policy(&policy)?;
        let request = WorkspaceCreateRequest {
            principal_id: command.owner_principal_id.clone(),
            request_id,
            request_payload,
        };
        let (workspace, event) = build_workspace_commit(command, policy)?;
        self.store
            .create_workspace_idempotent(request, workspace, event)
    }

    pub fn change_replication_policy(
        &self,
        command: ChangeReplicationPolicy,
    ) -> Result<CommittedWorkspace, StoreError> {
        let (expected_version, workspace, event) =
            self.build_replication_policy_change(command, true)?;
        self.store
            .commit_workspace(Some(expected_version), workspace, event)
    }

    pub fn change_replication_policy_idempotent(
        &self,
        command: ChangeReplicationPolicy,
        principal_id: String,
        request_id: String,
        request_payload: serde_json::Value,
    ) -> Result<CommittedWorkspace, StoreError>
    where
        S: IdempotentWorkspaceStore,
    {
        let (expected_version, workspace, event) =
            self.build_replication_policy_change(command, false)?;
        self.store.commit_workspace_idempotent(
            WorkspaceCreateRequest {
                principal_id,
                request_id,
                request_payload,
            },
            expected_version,
            workspace,
            event,
        )
    }

    /// Sets or clears the Workspace lead used for future admissions. The storage
    /// adapter atomically checks that a selected binding is enabled, lead-eligible,
    /// and belongs to this Workspace while committing the versioned Workspace event.
    pub fn set_default_agent_binding_idempotent(
        &self,
        command: SetWorkspaceDefaultAgentBinding,
    ) -> Result<storage_core::CommittedWorkspace, StoreError>
    where
        S: IdempotentWorkspaceStore,
    {
        require_non_empty("workspace_id", &command.workspace_id)?;
        require_non_empty("principal_id", &command.principal_id)?;
        require_non_empty("request_id", &command.request_id)?;
        if command.expected_version == 0 {
            return Err(StoreError::Invalid("expected Workspace version must be positive".to_owned()));
        }
        if command.agent_binding_id.as_ref().is_some_and(|value| value.trim().is_empty()) {
            return Err(StoreError::Invalid("AgentBinding ID must not be empty".to_owned()));
        }
        validate_event_context(&command.event)?;

        let mut workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }

        let previous = workspace.default_agent_binding_id.clone();
        workspace.default_agent_binding_id = command.agent_binding_id.clone();
        workspace.version = workspace.version.checked_add(1)
            .ok_or_else(|| StoreError::Invalid("Workspace version overflow".to_owned()))?;
        workspace.updated_at = command.event.recorded_at.clone();
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: workspace.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.default_agent_binding.changed.v1".to_owned(),
            payload: serde_json::json!({
                "workspace_id": workspace.workspace_id,
                "from_agent_binding_id": previous,
                "to_agent_binding_id": workspace.default_agent_binding_id,
                "changed_by": {"kind": "USER", "principal_id": command.principal_id},
                "aggregate_version": workspace.version,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.commit_workspace_idempotent(
            WorkspaceCreateRequest {
                principal_id: command.principal_id,
                request_id: command.request_id,
                request_payload: command.request_payload,
            },
            command.expected_version,
            workspace,
            event,
        )
    }

    /// Sets or clears the primary Coworker for future work. Existing Task origins
    /// are immutable and are not changed by this Workspace preference.
    pub fn set_primary_coworker_idempotent(
        &self,
        command: SetWorkspacePrimaryCoworker,
    ) -> Result<storage_core::CommittedWorkspace, StoreError>
    where
        S: IdempotentWorkspaceStore,
    {
        require_non_empty("workspace_id", &command.workspace_id)?;
        require_non_empty("principal_id", &command.principal_id)?;
        require_non_empty("request_id", &command.request_id)?;
        if command.expected_version == 0 {
            return Err(StoreError::Invalid("expected Workspace version must be positive".to_owned()));
        }
        if command.coworker_id.as_ref().is_some_and(|value| value.trim().is_empty()) {
            return Err(StoreError::Invalid("Coworker ID must not be empty".to_owned()));
        }
        validate_event_context(&command.event)?;

        let mut workspace = self.store.get_workspace(&command.workspace_id)?.ok_or(StoreError::NotFound)?;
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }

        let previous = workspace.primary_coworker_id.clone();
        workspace.primary_coworker_id = command.coworker_id.clone();
        workspace.version = workspace.version.checked_add(1)
            .ok_or_else(|| StoreError::Invalid("Workspace version overflow".to_owned()))?;
        workspace.updated_at = command.event.recorded_at.clone();
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: workspace.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.primary_coworker.changed.v1".to_owned(),
            payload: serde_json::json!({
                "workspace_id": workspace.workspace_id,
                "from_coworker_id": previous,
                "to_coworker_id": workspace.primary_coworker_id,
                "changed_by": {"kind": "USER", "principal_id": command.principal_id},
                "aggregate_version": workspace.version,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.commit_workspace_idempotent(
            WorkspaceCreateRequest {
                principal_id: command.principal_id,
                request_id: command.request_id,
                request_payload: command.request_payload,
            },
            command.expected_version,
            workspace,
            event,
        )
    }

    pub fn create_instruction_revision_idempotent(
        &self,
        command: CreateWorkspaceInstructionRevision,
        request_id: String,
        request_payload: serde_json::Value,
    ) -> Result<CommittedWorkspaceInstructionRevision, StoreError>
    where
        S: IdempotentWorkspaceStore,
    {
        require_non_empty("workspace_id", &command.workspace_id)?;
        require_non_empty("principal_id", &command.authored_by_principal_id)?;
        validate_event_context(&command.event)?;
        let mut workspace = self
            .store
            .get_workspace(&command.workspace_id)?
            .ok_or(StoreError::NotFound)?;
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid("an archived Workspace is read-only".to_owned()));
        }
        let expected_revision = workspace.current_instruction_revision.unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| StoreError::Invalid("instruction revision overflow".to_owned()))?;
        workspace.current_instruction_revision = Some(expected_revision);
        workspace.version = workspace.version.checked_add(1)
            .ok_or_else(|| StoreError::Invalid("Workspace version overflow".to_owned()))?;
        workspace.updated_at = command.event.recorded_at.clone();
        let instruction_revision = WorkspaceInstructionRevisionRecord {
            workspace_id: workspace.workspace_id.clone(),
            revision: expected_revision,
            parent_revisions: command.parent_revisions,
            content_ref: command.content_ref,
            content_digest: command.content_digest,
            authored_by: serde_json::json!({
                "kind": "USER",
                "principal_id": command.authored_by_principal_id,
            }),
            created_at: command.event.recorded_at.clone(),
        };
        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: workspace.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.instructions.revision.created.v1".to_owned(),
            payload: serde_json::json!({
                "workspace_id": workspace.workspace_id,
                "revision": instruction_revision.revision,
                "parent_revisions": instruction_revision.parent_revisions,
                "content_ref": instruction_revision.content_ref,
                "content_digest": instruction_revision.content_digest,
                "authored_by": instruction_revision.authored_by,
            }),
            recorded_at: command.event.recorded_at,
        };
        self.store.create_workspace_instruction_revision_idempotent(
            WorkspaceCreateRequest {
                principal_id: command.authored_by_principal_id,
                request_id,
                request_payload,
            },
            command.expected_version,
            workspace,
            instruction_revision,
            event,
        )
    }

    fn build_replication_policy_change(
        &self,
        command: ChangeReplicationPolicy,
        enforce_expected_version: bool,
    ) -> Result<(u64, storage_core::Workspace, EventDraft), StoreError> {
        require_non_empty("workspace_id", &command.workspace_id)?;
        validate_event_context(&command.event)?;

        let mut workspace = self
            .store
            .get_workspace(&command.workspace_id)?
            .ok_or(StoreError::NotFound)?;

        if enforce_expected_version && workspace.version != command.expected_version {
            return Err(StoreError::Conflict {
                expected: Some(command.expected_version),
                actual: Some(workspace.version),
            });
        }
        if workspace.status != "ACTIVE" {
            return Err(StoreError::Invalid(
                "an archived Workspace is read-only".to_owned(),
            ));
        }
        if command.policy == ReplicationPolicy::SelectedFolders {
            return Err(StoreError::Invalid(
                "SELECTED_FOLDERS requires the WorkspaceRoot service and is unavailable here"
                    .to_owned(),
            ));
        }
        if command.policy != ReplicationPolicy::SelectedFolders
            && !command.replication_scope_root_ids.is_empty()
        {
            return Err(StoreError::Invalid(
                "replication roots are valid only with SELECTED_FOLDERS".to_owned(),
            ));
        }

        let previous_policy = workspace.replication_policy.clone();
        workspace.replication_policy = command.policy;
        workspace.replication_scope_root_ids = command.replication_scope_root_ids;
        workspace.version += 1;
        workspace.updated_at = command.event.recorded_at.clone();

        let event = EventDraft {
            event_id: command.event.event_id,
            workspace_id: workspace.workspace_id.clone(),
            entity_type: "Workspace".to_owned(),
            entity_id: workspace.workspace_id.clone(),
            origin_runtime_id: command.event.origin_runtime_id,
            entity_revision: workspace.version,
            hlc_timestamp: command.event.hlc_timestamp,
            correlation_id: command.event.correlation_id,
            causation_id: command.event.causation_id,
            schema_version: 1,
            event_type: "workspace.replication_policy.changed.v1".to_owned(),
            payload: json!({
                "workspace_id": workspace.workspace_id,
                "from": previous_policy.as_str(),
                "to": workspace.replication_policy.as_str(),
                "replication_scope_root_ids": workspace.replication_scope_root_ids,
                "aggregate_version": workspace.version,
            }),
            recorded_at: command.event.recorded_at,
        };

        Ok((command.expected_version, workspace, event))
    }
}

fn validate_initial_replication_policy(policy: &ReplicationPolicy) -> Result<(), StoreError> {
    if *policy == ReplicationPolicy::SelectedFolders {
        return Err(StoreError::Invalid(
            "SELECTED_FOLDERS requires WorkspaceRoots created after the Workspace".to_owned(),
        ));
    }
    Ok(())
}

fn build_workspace_commit(
    command: CreateWorkspace,
    replication_policy: ReplicationPolicy,
) -> Result<(Workspace, EventDraft), StoreError> {
    require_non_empty("workspace_id", &command.workspace_id)?;
    require_non_empty("workspace name", command.name.trim())?;
    require_non_empty("owner_principal_id", &command.owner_principal_id)?;
    validate_event_context(&command.event)?;

    let workspace = Workspace {
        workspace_id: command.workspace_id.clone(),
        name: command.name.trim().to_owned(),
        owner_principal_id: command.owner_principal_id.clone(),
        replication_policy,
        replication_scope_root_ids: Vec::new(),
        current_instruction_revision: None,
        default_agent_binding_id: None,
        primary_coworker_id: None,
        hub_runtime_id: None,
        status: "ACTIVE".to_owned(),
        created_at: command.event.recorded_at.clone(),
        updated_at: command.event.recorded_at.clone(),
        version: 1,
    };

    let event = EventDraft {
        event_id: command.event.event_id,
        workspace_id: workspace.workspace_id.clone(),
        entity_type: "Workspace".to_owned(),
        entity_id: workspace.workspace_id.clone(),
        origin_runtime_id: command.event.origin_runtime_id,
        entity_revision: workspace.version,
        hlc_timestamp: command.event.hlc_timestamp,
        correlation_id: command.event.correlation_id,
        causation_id: command.event.causation_id,
        schema_version: 1,
        event_type: "workspace.created.v1".to_owned(),
        payload: json!({
            "workspace_id": workspace.workspace_id,
            "owner_principal_id": workspace.owner_principal_id,
            "replication_policy": workspace.replication_policy.as_str(),
            "replication_scope_root_ids": workspace.replication_scope_root_ids,
        }),
        recorded_at: command.event.recorded_at,
    };

    Ok((workspace, event))
}

fn require_non_empty(field: &str, value: &str) -> Result<(), StoreError> {
    if value.trim().is_empty() {
        return Err(StoreError::Invalid(format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_event_context(context: &EventContext) -> Result<(), StoreError> {
    require_non_empty("event_id", &context.event_id)?;
    require_non_empty("origin_runtime_id", &context.origin_runtime_id)?;
    require_non_empty("hlc_timestamp", &context.hlc_timestamp)?;
    require_non_empty("correlation_id", &context.correlation_id)?;
    require_non_empty("recorded_at", &context.recorded_at)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use storage_core::{BlobRef, DomainEvent, EventStore, StateStore};

    #[derive(Clone, Default)]
    struct MemoryStore(Arc<Mutex<Vec<CommittedWorkspace>>>);

    impl StateStore for MemoryStore {
        fn get_workspace(&self, workspace_id: &str) -> Result<Option<Workspace>, StoreError> {
            Ok(self
                .0
                .lock()
                .expect("lock")
                .last()
                .filter(|row| row.workspace.workspace_id == workspace_id)
                .map(|row| row.workspace.clone()))
        }

        fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
            let rows = self.0.lock().expect("lock");
            let mut latest = BTreeMap::new();
            for row in rows.iter() {
                latest.insert(row.workspace.workspace_id.clone(), row.workspace.clone());
            }
            let mut workspaces = latest.into_values().collect::<Vec<_>>();
            workspaces.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.workspace_id.cmp(&right.workspace_id))
            });
            Ok(workspaces)
        }

        fn commit_workspace(
            &self,
            expected_version: Option<u64>,
            workspace: Workspace,
            draft: EventDraft,
        ) -> Result<CommittedWorkspace, StoreError> {
            let mut rows = self.0.lock().expect("lock");
            let current = rows.last().map(|row| row.workspace.version);
            if current != expected_version {
                return Err(StoreError::Conflict {
                    expected: expected_version,
                    actual: current,
                });
            }
            let event = DomainEvent {
                event_id: draft.event_id,
                workspace_id: draft.workspace_id,
                entity_type: draft.entity_type,
                entity_id: draft.entity_id,
                origin_runtime_id: draft.origin_runtime_id,
                origin_sequence: current.unwrap_or(0) + 1,
                entity_revision: draft.entity_revision,
                hlc_timestamp: draft.hlc_timestamp,
                correlation_id: draft.correlation_id,
                causation_id: draft.causation_id,
                schema_version: draft.schema_version,
                event_type: draft.event_type,
                payload: draft.payload,
                aggregate_state_ref: storage_core::AggregateStateRef {
                    blob: BlobRef {
                        digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                        size_bytes: 1,
                        media_type: "application/json".to_owned(),
                    },
                    entity_revision: workspace.version,
                    record_schema_version: RECORD_SCHEMA_VERSION,
                },
                recorded_at: draft.recorded_at,
                payload_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            };
            let result = CommittedWorkspace { workspace, event };
            rows.push(result.clone());
            Ok(result)
        }
    }

    impl EventStore for MemoryStore {
        fn read_workspace_events(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<DomainEvent>, StoreError> {
            Ok(self
                .0
                .lock()
                .expect("lock")
                .iter()
                .map(|row| row.event.clone())
                .collect())
        }
    }

    fn event_context(id: &str, time: &str) -> EventContext {
        EventContext {
            event_id: id.to_owned(),
            origin_runtime_id: "runtime-1".to_owned(),
            hlc_timestamp: time.to_owned(),
            correlation_id: "correlation-1".to_owned(),
            causation_id: None,
            recorded_at: time.to_owned(),
        }
    }

    #[test]
    fn creates_local_workspace_with_empty_scope_and_created_event() {
        let service = WorkspaceService::new(MemoryStore::default());
        let created = service
            .create(CreateWorkspace {
                workspace_id: "workspace-1".to_owned(),
                name: "  My Workspace  ".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                event: event_context("event-1", "2026-10-06T10:00:00Z"),
            })
            .expect("workspace created");

        assert_eq!(created.workspace.name, "My Workspace");
        assert_eq!(
            created.workspace.replication_policy,
            ReplicationPolicy::LocalOnly
        );
        assert!(created.workspace.replication_scope_root_ids.is_empty());
        assert_eq!(created.workspace.version, 1);
        assert_eq!(created.event.event_type, "workspace.created.v1");
    }

    #[test]
    fn refuses_selected_folders_without_an_active_root() {
        let service = WorkspaceService::new(MemoryStore::default());
        service
            .create(CreateWorkspace {
                workspace_id: "workspace-1".to_owned(),
                name: "Workspace".to_owned(),
                owner_principal_id: "owner-1".to_owned(),
                event: event_context("event-1", "2026-10-06T10:00:00Z"),
            })
            .expect("workspace created");

        let result = service.change_replication_policy(ChangeReplicationPolicy {
            workspace_id: "workspace-1".to_owned(),
            expected_version: 1,
            policy: ReplicationPolicy::SelectedFolders,
            replication_scope_root_ids: Vec::new(),
            event: event_context("event-2", "2026-10-06T10:01:00Z"),
        });

        assert!(matches!(result, Err(StoreError::Invalid(_))));
    }
}
