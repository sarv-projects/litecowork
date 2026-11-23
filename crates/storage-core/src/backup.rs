//! Local desktop backup/restore contract types.
//!
//! This module owns the *contract* for the E13-S01 backup/restore surface: the
//! archive container constants, the filtered-snapshot table policy, the manifest
//! payload shape, the schema-compatibility verdict, and the outcome records the
//! Operator/CLI surfaces return.
//!
//! The contract deliberately separates three things that are easy to conflate:
//!
//! * the **manifest** (control-plane recovery metadata) describes *what* was
//!   captured and how to verify it;
//! * the **blob manifest** lists encrypted blob envelopes and the *encryption
//!   key reference* needed to decrypt them. It never contains key material;
//! * the **snapshot** is a Workspace-filtered serialization of the local
//!   database, never a byte-for-byte copy of the live SQLite file.
//!
//! Restore is all-or-nothing: the destination is replaced only after every
//! manifest, digest, schema, key and foreign-key check has passed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Container magic bytes. A backup artifact always starts with these bytes so a
/// foreign file is rejected before any key material is derived.
pub const BACKUP_CONTAINER_MAGIC: &[u8; 6] = b"LCBK1\0";

/// Container format version. A container with a different version is refused.
pub const BACKUP_CONTAINER_VERSION: u8 = 1;

/// Content encryption algorithm identifier (XChaCha20-Poly1305, per chunk).
pub const BACKUP_CONTENT_ALGORITHM_XCHACHA20POLY1305: u8 = 1;

/// Key derivation function identifier (PBKDF2-HMAC-SHA256).
pub const BACKUP_KDF_PBKDF2_HMAC_SHA256: u8 = 1;

/// Production PBKDF2 iteration count for the container key.
pub const BACKUP_KDF_DEFAULT_ITERATIONS: u32 = 600_000;

/// Lowest iteration count the API accepts. Values below this are refused so a
/// caller cannot silently downgrade the container key.
pub const BACKUP_KDF_MINIMUM_ITERATIONS: u32 = 100_000;

/// Maximum number of payload chunks a container may declare.
pub const BACKUP_MAX_PAYLOAD_CHUNKS: u32 = 1_000_000;

/// Maximum size of a single payload chunk (64 MiB).
pub const BACKUP_MAX_CHUNK_BYTES: u64 = 64 * 1024 * 1024;

/// Maximum size of the encrypted manifest (8 MiB).
pub const BACKUP_MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;

/// Maximum number of blob descriptors a manifest may carry.
pub const BACKUP_MAX_BLOB_DESCRIPTORS: usize = 1_000_000;

/// Relative path of the filtered database snapshot inside the container.
pub const BACKUP_DATABASE_SNAPSHOT_PATH: &str = "database.snapshot.sqlite3";

/// Root prefix for copied blob envelopes inside the container.
pub const BACKUP_BLOB_ROOT: &str = "blobs";

/// Blob purposes that are never carried in a backup.
///
/// `RESOURCE_UPLOAD_CHUNK` holds transient upload chunk bytes. The durable
/// upload session/chunk *metadata* is retained as non-resumable audit state,
/// but the chunk bytes themselves are not restorable content.
pub const BACKUP_EXCLUDED_BLOB_PURPOSES: &[&str] = &["RESOURCE_UPLOAD_CHUNK"];

/// Tables whose rows are never carried in a Workspace backup.
///
/// The list follows `docs/STORAGE.md` "Backup and disaster recovery": Runtime
/// local provider/process handles, local-only recovery registries, and live
/// authority (leases, locks, secret leases) are excluded. Every table that
/// remains carries Workspace-scoped durable truth, including ContextDocument
/// purge plans and purge receipts. The complete three-way classification is
/// enforced by `BACKUP_WORKSPACE_SCOPED_TABLES`,
/// `BACKUP_INSTALLATION_TABLES`, and this list; unknown tables fail closed.
pub const BACKUP_EXCLUDED_TABLES: &[&str] = &[
    // Runtime-local provider and process handles.
    "agent_endpoint_bindings",
    "agent_session_host_bindings",
    "agent_host_instances",
    "agent_control_descriptors",
    "agent_auth_observations",
    "agent_installation_observations",
    "agent_lifecycle_operations",
    "agent_registry_cache",
    "capability_activation_host_bindings",
    "capability_host_instances",
    "capability_invocation_provider_bindings",
    "provider_input_bindings",
    "automation_trigger_bindings",
    "channel_ingress_cursor_bindings",
    "channel_reply_targets",
    "channel_host_assignments",
    "channel_host_lease_records",
    "environment_provider_bindings",
    "environment_checkpoint_provider_bindings",
    "resource_location_bindings",
    "file_identity_bindings",
    // Local-only recovery registries (delete fences and upload chunk roots).
    "resource_upload_chunk_blob_registry",
    "resource_index_blob_registry",
    "resource_content_blob_registry",
    "resource_upload_blob_reservations",
    "resource_upload_blob_gc_fences",
    "workspace_root_cleanup_outbox",
    "workspace_root_observation_batch_receipts",
    "workspace_root_observation_bindings",
    "workspace_root_observation_generations",
    "workspace_root_observations",
    // Installation-local diagnostics and previews.
    "runtime_incarnation_local_observations",
    "environment_provision_previews",
    "runtime_offers",
    "provider_circuit_states",
    "pairing_tokens",
    "runtime_workspace_bindings",
    "request_dedup",
    // Live authority and leases are never restored.
    "execution_leases",
    "environment_control_leases",
    "secret_leases",
    "capability_locks",
];

/// How a table's rows are associated with one Workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupWorkspaceFilter {
    /// The row contains a direct Workspace owner column.
    Column(&'static str),
    /// A fixed SQL predicate with exactly one `?1` Workspace parameter.
    ///
    /// Predicates are source-controlled constants, never derived from a
    /// database value or caller input.
    Predicate(&'static str),
}

/// One application table and its Workspace ownership rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupWorkspaceTable {
    pub table: &'static str,
    pub filter: BackupWorkspaceFilter,
}

/// All durable records included in a one-Workspace backup.
///
/// A direct owner column is preferred. Tables without one use a fixed join to
/// their owning Workspace aggregate. The SQLite adapter validates that this
/// map, `BACKUP_INSTALLATION_TABLES`, and `BACKUP_EXCLUDED_TABLES` classify
/// every migrated application table exactly once before creating a snapshot.
pub const BACKUP_WORKSPACE_SCOPED_TABLES: &[BackupWorkspaceTable] = &[
    // Direct Workspace ownership.
    BackupWorkspaceTable {
        table: "agent_bindings",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "agent_sessions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "aggregate_snapshots",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "artifacts",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "audit_records",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "automation_cursors",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "automation_occurrences",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "automation_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "automation_test_runs",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "automations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "budget_reservations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "capability_activations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "capability_invocations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "channel_bindings",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "channel_host_continuity_proofs",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "channel_host_drain_proofs",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "channel_host_lease_releases",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "connections",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "context_document_purge_plans",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "context_document_purge_receipts",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "conversation_control_events",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "conversation_history_state",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "conversation_owner_side_notes",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "conversations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "coworker_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "coworkers",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "delegation_profile_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "delegation_profiles",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "delegation_results",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "demonstration_sessions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "dependency_edges",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "domain_events",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "environments",
        filter: BackupWorkspaceFilter::Column("owner_workspace_id"),
    },
    BackupWorkspaceTable {
        table: "event_archive_segments",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "goal_artifact_links",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "goal_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "goal_routine_links",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "goal_task_links",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "goals",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "notification_deliveries",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "notification_preferences",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "pending_replication_events",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "replication_aggregate_positions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "replication_cursors",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "replication_receipts",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "resource_edges",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "resource_text_index_terms",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "resource_text_indexes",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "resource_upload_sessions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "resources",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "rich_presentations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "routine_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "routines",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "skill_proposals",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "suggestion_preferences",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "suggestions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "task_spec_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "tasks",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "usage_observations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "user_requests",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_backup_manifests",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_delegation_settings_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_instruction_revisions",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_origin_sequences",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_replication_roots",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_root_file_locations",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspace_roots",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    BackupWorkspaceTable {
        table: "workspaces",
        filter: BackupWorkspaceFilter::Column("workspace_id"),
    },
    // Durable child records whose owner is another Workspace-scoped aggregate.
    BackupWorkspaceTable {
        table: "agent_binding_configurations",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM agent_bindings owner WHERE owner.agent_binding_id = agent_binding_configurations.agent_binding_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "agent_endpoints",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM agent_bindings owner WHERE owner.agent_profile_id = agent_endpoints.agent_profile_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "agent_profiles",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM agent_bindings owner WHERE owner.agent_profile_id = agent_profiles.agent_profile_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "approval_uses",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM approvals owner JOIN tasks task_owner ON task_owner.task_id = owner.task_id WHERE owner.approval_id = approval_uses.approval_id AND task_owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "approvals",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = approvals.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "artifact_versions",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM artifacts owner WHERE owner.artifact_id = artifact_versions.artifact_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "attempts",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = attempts.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "capability_grants",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversations owner WHERE owner.conversation_id = capability_grants.conversation_id AND owner.workspace_id IS ?1) OR EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = capability_grants.task_id AND owner.workspace_id IS ?1) OR EXISTS (SELECT 1 FROM attempts attempt_owner JOIN tasks task_owner ON task_owner.task_id = attempt_owner.task_id WHERE attempt_owner.attempt_id = capability_grants.attempt_id AND task_owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "channel_event_receipts",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM channel_bindings owner WHERE owner.channel_binding_id = channel_event_receipts.channel_binding_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "channel_thread_mappings",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM channel_bindings owner WHERE owner.channel_binding_id = channel_thread_mappings.channel_binding_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "conversation_dispatch_intents",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversation_turns turn_owner JOIN conversations owner ON owner.conversation_id = turn_owner.conversation_id WHERE turn_owner.turn_id = conversation_dispatch_intents.turn_id AND turn_owner.conversation_id = conversation_dispatch_intents.conversation_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "conversation_messages",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversations owner WHERE owner.conversation_id = conversation_messages.conversation_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "conversation_response_commits",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversation_turns turn_owner JOIN conversations owner ON owner.conversation_id = turn_owner.conversation_id WHERE turn_owner.turn_id = conversation_response_commits.turn_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "conversation_turn_activity_records",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversation_turns turn_owner JOIN conversations owner ON owner.conversation_id = turn_owner.conversation_id WHERE turn_owner.turn_id = conversation_turn_activity_records.turn_id AND turn_owner.conversation_id = conversation_turn_activity_records.conversation_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "conversation_turn_usage_records",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversation_turns turn_owner JOIN conversations owner ON owner.conversation_id = turn_owner.conversation_id WHERE turn_owner.turn_id = conversation_turn_usage_records.turn_id AND turn_owner.conversation_id = conversation_turn_usage_records.conversation_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "conversation_turns",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM conversations owner WHERE owner.conversation_id = conversation_turns.conversation_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "effects",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = effects.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "environment_checkpoints",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM environments owner WHERE owner.environment_id = environment_checkpoints.environment_id AND owner.owner_workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "evidence",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = evidence.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "handoffs",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = handoffs.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "invalidation_records",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM dependency_edges owner WHERE owner.dependency_edge_id = invalidation_records.dependency_edge_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "plan_revisions",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = plan_revisions.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "resource_locations",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM resources owner WHERE owner.resource_id = resource_locations.resource_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "resource_revision_parents",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM resources owner WHERE owner.resource_id = resource_revision_parents.resource_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "resource_revisions",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM resources owner WHERE owner.resource_id = resource_revisions.resource_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "resource_upload_chunk_requests",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM resource_upload_sessions owner WHERE owner.upload_id = resource_upload_chunk_requests.upload_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "resource_upload_chunks",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM resource_upload_sessions owner WHERE owner.upload_id = resource_upload_chunks.upload_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "steps",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = steps.task_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "user_request_responses",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM user_requests owner WHERE owner.request_id = user_request_responses.request_id AND owner.workspace_id IS ?1)",
        ),
    },
    BackupWorkspaceTable {
        table: "verification_runs",
        filter: BackupWorkspaceFilter::Predicate(
            "EXISTS (SELECT 1 FROM tasks owner WHERE owner.task_id = verification_runs.task_id AND owner.workspace_id IS ?1)",
        ),
    },
];

/// Installation-scoped records retained only to satisfy Workspace record
/// references after restore. They do not authorize use in the restored Runtime.
pub const BACKUP_INSTALLATION_TABLES: &[&str] =
    &["runtime_incarnations", "runtimes", "schema_migrations"];

/// Authority tables whose restored rows are revoked instead of restored.
///
/// `capability_grants` cannot be dropped wholesale: `capability_invocations`
/// and `approval_uses` reference it, and dropping the parent would destroy
/// durable Effect/approval history. Restore therefore revokes every restored
/// grant so no pre-backup authority is live again.
pub const BACKUP_REVOKED_ON_RESTORE_TABLES: &[&str] = &["capability_grants"];

/// A non-secret reference to the encryption key a blob envelope needs.
///
/// The reference records *which* key version decrypts a blob. It never records
/// key material; losing the referenced key makes the backup unrestorable.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupKeyReference {
    pub workspace_id: String,
    pub workspace_scope_digest: String,
    pub purpose: String,
    pub key_version: u32,
}

/// Descriptor of one encrypted blob envelope carried by a backup.
///
/// `size_bytes` is the size of the stored envelope. `plaintext_size_bytes` and
/// `media_type` are the `BlobRef` fields that the blob envelope's associated
/// data is bound to; without them the envelope cannot be authenticated after a
/// restore.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupBlobDescriptor {
    pub workspace_scope_digest: String,
    pub purpose: String,
    pub blob_digest: String,
    pub size_bytes: u64,
    pub plaintext_size_bytes: u64,
    pub media_type: String,
    pub envelope_digest: String,
    pub key_version: u32,
    pub relative_path: String,
}

/// Per-origin event replication cursor recorded at backup time.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupEventCursor {
    pub origin_runtime_id: String,
    pub last_sequence: i64,
}

/// Descriptor of a payload stored inside the container.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupPayloadDescriptor {
    pub relative_path: String,
    pub size_bytes: u64,
    pub digest: String,
}

/// The verified manifest of one backup container.
///
/// The manifest is stored inside the container as an authenticated ciphertext
/// chunk. `integrity_digest` binds the snapshot and blob digests together and
/// `manifest_authentication` binds the manifest to the exact container bytes
/// that carry it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupArchiveManifest {
    pub format_version: u32,
    pub backup_id: String,
    pub workspace_id: String,
    pub created_at: String,
    pub verified_at: String,
    pub schema_version: i64,
    pub source_runtime_id: Option<String>,
    pub event_cursors: Vec<BackupEventCursor>,
    pub database_snapshot: BackupPayloadDescriptor,
    pub blobs: Vec<BackupBlobDescriptor>,
    pub key_references: Vec<BackupKeyReference>,
    pub excluded_tables: Vec<String>,
    pub integrity_digest: String,
    pub manifest_authentication: String,
    /// Number of rows that were dropped from each excluded table.
    pub dropped_local_row_counts: BTreeMap<String, u64>,
}

/// Schema compatibility verdict for an upgrade or restore attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaCompatibility {
    /// The found schema version can be opened by this Runtime.
    Compatible {
        found: i64,
        minimum_supported: i64,
        maximum_supported: i64,
    },
    /// The backup was taken by a newer Runtime. Upgrading is refused.
    RefusedTooNew { found: i64, maximum_supported: i64 },
    /// The backup predates the oldest schema this Runtime can open.
    RefusedTooOld { found: i64, minimum_supported: i64 },
}

impl SchemaCompatibility {
    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible { .. })
    }

    pub fn reason(&self) -> String {
        match self {
            Self::Compatible {
                found,
                minimum_supported,
                maximum_supported,
            } => format!(
                "schema version {found} is within the supported range {minimum_supported}..={maximum_supported}"
            ),
            Self::RefusedTooNew {
                found,
                maximum_supported,
            } => format!(
                "schema version {found} is newer than the maximum supported version {maximum_supported}"
            ),
            Self::RefusedTooOld {
                found,
                minimum_supported,
            } => format!(
                "schema version {found} is older than the minimum supported version {minimum_supported}"
            ),
        }
    }
}

/// Decides whether a found schema version may be opened by this Runtime.
///
/// A safe upgrade refuses to run on an unknown schema version. The Runtime
/// refuses both a newer schema (it cannot know the newer invariants) and a
/// schema older than the oldest supported baseline (the migration chain it
/// ships cannot be proven to apply).
pub fn check_schema_compatibility(
    found: i64,
    minimum_supported: i64,
    maximum_supported: i64,
) -> SchemaCompatibility {
    if found > maximum_supported {
        return SchemaCompatibility::RefusedTooNew {
            found,
            maximum_supported,
        };
    }
    if found < minimum_supported {
        return SchemaCompatibility::RefusedTooOld {
            found,
            minimum_supported,
        };
    }
    SchemaCompatibility::Compatible {
        found,
        minimum_supported,
        maximum_supported,
    }
}

/// Result of verifying a backup container without mutating anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupVerification {
    pub backup_id: String,
    pub workspace_id: String,
    pub schema_version: i64,
    pub database_digest: String,
    pub blob_count: usize,
    pub key_reference_count: usize,
    pub excluded_table_count: usize,
    pub verified_at: String,
    pub compatibility: SchemaCompatibility,
    pub integrity_digest: String,
}

/// Result of a completed restore.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupRestoreOutcome {
    pub backup_id: String,
    pub workspace_id: String,
    pub schema_version: i64,
    pub restored_blob_count: usize,
    pub revoked_grant_count: usize,
    pub dropped_lease_count: usize,
    pub restored_at: String,
    /// A restored installation has no `runtime-state.json`, so the daemon
    /// registers a fresh Runtime identity and incarnation on first start.
    pub new_runtime_identity_required: bool,
}

/// Result of creating a backup container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupCreateOutcome {
    pub backup_id: String,
    pub workspace_id: String,
    pub schema_version: i64,
    pub archive_path: String,
    pub archive_size_bytes: u64,
    pub blob_count: usize,
    pub database_digest: String,
    pub integrity_digest: String,
    pub created_at: String,
    /// Whether the verified manifest was persisted in
    /// `workspace_backup_manifests` on the source installation.
    pub manifest_row_recorded: bool,
}

/// The canonical payload a backup integrity digest is computed over.
///
/// The payload is deterministic: the blob descriptors, key references and event
/// cursors are sorted by their stable identity so two captures of the same
/// content produce the same digest. The digest itself is computed by the
/// storage adapter, which owns the canonical JSON and SHA-256 primitives.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BackupIntegrityPayload {
    pub schema_version: i64,
    pub database_snapshot: BackupPayloadDescriptor,
    pub blobs: Vec<BackupBlobDescriptor>,
    pub key_references: Vec<BackupKeyReference>,
    pub event_cursors: Vec<BackupEventCursor>,
}

/// Builds the deterministic integrity payload for a captured backup set.
pub fn backup_integrity_payload(
    schema_version: i64,
    database_snapshot: &BackupPayloadDescriptor,
    blobs: &[BackupBlobDescriptor],
    key_references: &[BackupKeyReference],
    event_cursors: &[BackupEventCursor],
) -> BackupIntegrityPayload {
    let mut sorted_blobs = blobs.to_vec();
    sorted_blobs.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut sorted_keys = key_references.to_vec();
    sorted_keys.sort_by(|left, right| {
        (&left.workspace_id, &left.purpose, left.key_version).cmp(&(
            &right.workspace_id,
            &right.purpose,
            right.key_version,
        ))
    });
    let mut sorted_cursors = event_cursors.to_vec();
    sorted_cursors.sort_by(|left, right| left.origin_runtime_id.cmp(&right.origin_runtime_id));
    BackupIntegrityPayload {
        schema_version,
        database_snapshot: database_snapshot.clone(),
        blobs: sorted_blobs,
        key_references: sorted_keys,
        event_cursors: sorted_cursors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_compatibility_refuses_unknown_versions() {
        assert_eq!(
            check_schema_compatibility(26, 1, 26),
            SchemaCompatibility::Compatible {
                found: 26,
                minimum_supported: 1,
                maximum_supported: 26
            }
        );
        assert_eq!(
            check_schema_compatibility(27, 1, 26),
            SchemaCompatibility::RefusedTooNew {
                found: 27,
                maximum_supported: 26
            }
        );
        assert_eq!(
            check_schema_compatibility(0, 1, 26),
            SchemaCompatibility::RefusedTooOld {
                found: 0,
                minimum_supported: 1
            }
        );
        assert!(!check_schema_compatibility(27, 1, 26).is_compatible());
    }

    #[test]
    fn integrity_payload_is_order_independent() {
        let snapshot = BackupPayloadDescriptor {
            relative_path: BACKUP_DATABASE_SNAPSHOT_PATH.to_owned(),
            size_bytes: 10,
            digest: "sha256:aa".to_owned(),
        };
        let blob = BackupBlobDescriptor {
            workspace_scope_digest: "scope".to_owned(),
            purpose: "RESOURCE".to_owned(),
            blob_digest: "sha256:bb".to_owned(),
            size_bytes: 4,
            plaintext_size_bytes: 2,
            media_type: "text/plain".to_owned(),
            envelope_digest: "sha256:cc".to_owned(),
            key_version: 1,
            relative_path: "blobs/scope/resource/bb.blob".to_owned(),
        };
        let other = BackupBlobDescriptor {
            relative_path: "blobs/scope/resource/aa.blob".to_owned(),
            ..blob.clone()
        };
        let first =
            backup_integrity_payload(26, &snapshot, &[blob.clone(), other.clone()], &[], &[]);
        let second = backup_integrity_payload(26, &snapshot, &[other, blob], &[], &[]);
        assert_eq!(first, second);
        assert_eq!(first.blobs[0].relative_path, "blobs/scope/resource/aa.blob");
    }

    #[test]
    fn excluded_tables_never_include_purge_receipt_tables() {
        assert!(!BACKUP_EXCLUDED_TABLES.contains(&"context_document_purge_plans"));
        assert!(!BACKUP_EXCLUDED_TABLES.contains(&"context_document_purge_receipts"));
        assert!(BACKUP_EXCLUDED_TABLES.contains(&"execution_leases"));
        assert!(BACKUP_EXCLUDED_TABLES.contains(&"secret_leases"));
        assert!(BACKUP_EXCLUDED_TABLES.contains(&"agent_auth_observations"));
    }
}
