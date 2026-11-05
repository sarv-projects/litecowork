# Digest crates-05

Files: 29 read / 29 listed | Lines read: 21951 | Bytes: 818 KB

Scope: `domain-effects`, `domain-rag`, `domain-responsibility`, `domain-task` (2 files), `linux-process-scope`, `local-environment-staging`, `operator-ipc`, `storage-core` (3 files), `storage-sqlite` (11 files). All files are Rust source (no lock files, no generated artifacts, no icons).

## Per-file notes

### crates/domain-effects/Cargo.toml (13 lines)
- Pure-value crate: only `serde` + `serde_json`. No store, no clock, no I/O — confirms the crate doc claim "domain validation and transitions only".

### crates/domain-effects/src/lib.rs (1216 lines)
- Layer: domain (Effect / Evidence / VerificationRun state machines). No storage, no authorization, no provider calls.
- `EffectState`: `PROPOSED, STARTED, ACKNOWLEDGED, RECONCILING, OBSERVED, VERIFIED, FAILED, AMBIGUOUS`; `allows()` table = Proposed→(Started|Failed), Started→(Acknowledged|Failed|Ambiguous), Acknowledged→(Observed|Failed|Ambiguous), Observed→(Verified|Ambiguous), Ambiguous→Reconciling, Reconciling→(Observed|Failed|Ambiguous|Started).
- `EffectRecord` (`deny_unknown_fields`): effect_id, task_id, attempt_id, capability_ref, operation, target, idempotency_key, state, request_digest, capability_invocation_id, execution_method, dispatch_ordinal: u32, result_ref, observed_state, verification_ref, created_at, updated_at, version: u64.
- `validate()` invariants: digest must be `sha256:` + 64 lowercase hex (71 chars); `version != 0`; `dispatch_ordinal <= 1_000_000`; PROPOSED must have ordinal 0, any state other than PROPOSED/FAILED must be ≥1; VERIFIED requires `verification_ref`.
- `transition(next, at)` refuses Reconciling→Started with `EffectError::RetryNotAuthorized`; retry requires `authorize_retry(at, reconciliation_evidence_ref, confirmed_not_occurred, same_key_idempotent)` and needs a non-empty evidence ref plus either proof-not-occurred or a stable idempotency key.
- `EvidenceLevel`: `REPORTED, OBSERVED, VERIFIED`. `ExecutionMethod`: `STRUCTURED_API, STRUCTURED_BROWSER, ACCESSIBILITY_BROWSER, SCREEN_COMPUTER_USE, DETERMINISTIC_LOCAL, NATIVE_AGENT_TOOL, UNKNOWN`.
- `EvidenceRecord.verification_run_id: Option<String>` carries `#[serde(default)]` — explicitly for legacy payloads (test `old_evidence_payloads_deserialize_without_a_run_binding`).
- `VerificationRunRecord`: statuses `PENDING, RUNNING, PASSED, FAILED, INCONCLUSIVE`; `terminal()`; `start(at)` PENDING→RUNNING; `finish(status, evidence_refs, failure_code, at)`; `VerificationRunError::TerminalImmutable` blocks rewriting a terminal run. Caps `MAX_VERIFICATION_SUBJECTS/INPUTS/EVIDENCE_REFS = 128`; duplicates rejected (`has_duplicates`, `has_duplicate_inputs`).
- `VerificationFailureCode`: `TIMEOUT, INTEGRITY_FAILURE, VERIFIER_UNAVAILABLE, UNSUPPORTED_CRITERION, INPUT_UNAVAILABLE, TASK_CANCELLED`.
- Two completion gates: `assess_completion(...)` is deliberately fail-closed (always pushes `CompletionBlocker::VerifierEvidenceBindingUnavailable`); `assess_completion_with_evidence(...)` is the trusted overload — budgets 256 criteria / 4096 runs / 16384 evidence, `EvaluationInputOverBudget` when exceeded; each PASSED run must bind `evidence_refs` to same-run `VERIFIED` evidence whose `subject_ref` is pinned by `verifier_evidence_subject_is_pinned` → `task:{task_id}` or `resource://{ws}/{res}@{rev}`.
- `valid_producer` accepts exactly `{principal_id, kind}` with kind ∈ `USER|SERVICE|RUNTIME|AGENT|CHANNEL_IDENTITY` or exactly `{service_id}`.
- Clock note: timestamps are shape-checked only (`invalid_bounded` ≤128 chars, no control chars) — **never parsed or compared here**; no ordering is asserted in this crate.
- Tests: ordinal advance, illegal skip to OBSERVED, failure before dispatch, retry authorization, evidence shape, run pinning/duplicates/terminal immutability, both completion gates including oversize rejection.

### crates/domain-rag/Cargo.toml (17 lines)
- Depends on `storage-core` (plus dev-dep). Only `placement.rs` is in this bucket.

### crates/domain-rag/src/placement.rs (223 lines)
- Layer: domain policy (placement, not retrieval).
- `WorkspaceReplicationPolicy {LOCAL_ONLY, METADATA_ONLY, ACTIVE_TASK_INPUTS, SELECTED_FOLDERS}` with manual `parse()`/`as_str()`; unknown strings return `None` (test asserts).
- `ResourcePlacement {LOCAL_ONLY, CLOUD_ELIGIBLE}`, `TaskPlacement {LOCAL, CLOUD}`; `PlacementInputs {policy, has_remote_location, declared_local_only}`; `resolve_resource_placement` (explicit local-only wins; ACTIVE_TASK_INPUTS/SELECTED_FOLDERS need `has_remote_location`).
- `evaluate_task_placement` blocker code `RESOURCE_LOCAL_ONLY`; `PlacementDecision` is camelCase `deny_unknown_fields` with `blocked_code`/`blocked_reason`.

### crates/domain-responsibility/src/models.rs (435 lines)
- Layer: domain models for Coworker / Automation / Trigger.
- `string_enum!` macro emits `SCREAMING_SNAKE_CASE` enums: `CoworkerStatus {ACTIVE, PAUSED, ARCHIVED}`, `AutomationStatus {ENABLED, PAUSED, DISABLED}`, `PrincipalKind {USER, SERVICE, RUNTIME, AGENT, CHANNEL_IDENTITY}`, `DelegationStrategy {NATIVE_DEFAULT, BALANCED, COST_SAVER, HOST_DELEGATION_ONLY}`, `InteractionDefault {STANDARD_TRUST_POLICY, REQUIRE_OWNER_APPROVAL, HANDOFF_TO_OWNER}`, `ContextDocumentKind {PERSONAL_PROFILE, COWORKER_NOTES, WORKSPACE_NOTES, GOAL_NOTES}`, `BinaryNotification {ALWAYS, SILENT}`, `CompletionNotification`, `AutomationNotification`, `TriggerPlacement {HUB, SPECIFIC_RUNTIME, AUTO}`, `OverlapPolicy {SKIP, QUEUE, CANCEL_OLD, ALLOW}`, `WakePolicy {NEVER, TRY_WAKE, REQUIRE_RUNTIME_AWAKE}`.
- Time/recurrence vocabulary (relevant to clock assumptions, though no clock is read): `RecurrenceFormat` serialized as `CRON_5` / `RFC5545_RRULE`; `AmbiguousLocalTime {EARLIER, LATER}`; `NonexistentLocalTime {SKIP, NEXT_VALID}`; schedule carries `timezone`, `recurrence_semantics_version: u32`, `start_at`, `end_at`.
- `MisfirePolicy` has a hand-written `Deserialize` enforcing exact object shape: `SKIP`, `RUN_ONCE_WHEN_AVAILABLE`, `CATCH_UP_BOUNDED{max_occurrences: u32}` (explicitly because serde's internally-tagged unit variants accept unknown fields).
- `TriggerDefinition` also hand-written: five V1 variants `SCHEDULE, ONE_SHOT, WEBHOOK, CONNECTOR_EVENT, MANUAL`; `MANUAL` rejects any extra field (`object.len() == 1`); wire enum `TriggerDefinitionWire` mirrors it; `MANUAL => unreachable!()` after the early return.
- `WebhookAuthentication {SIGNATURE, BEARER_SECRET, MUTUAL_TLS}`; webhook also declares `replay_window_ms`, `max_payload_bytes`, `delivery_id_field`.
- `ContractObject = BTreeMap<String, serde_json::Value>` with the comment that other domains' schemas must be validated by the owning port before admission — i.e. this crate deliberately does not re-own them.

### crates/domain-responsibility/src/routines.rs (1384 lines)
- Layer: domain service (`RoutineService`) over a `RoutineStore` port; deliberately no scheduling/triggering.
- `RoutineStatus {ACTIVE, ARCHIVED}`; `RoutineError {NotFound, AlreadyExists, Unauthorized, VersionConflict, VersionOverflow, RevisionOverflow, InvalidDefinition, Archived, ArchiveBlocked, IdempotencyConflict, WorkspaceArchived, Storage}`.
- `RoutineCommand` (`tag = "command"`, `SCREAMING_SNAKE_CASE`): `CREATE`, `REVISE{expected_version}`, `ARCHIVE{expected_version}`; `RoutineStore::transaction(scope, fingerprint, op)` must "authorize before replay, bind RequestId to the full command fingerprint, and atomically persist head, immutable revision, event, snapshot, receipt".
- Fingerprint = `sha256:` over `serde_json_canonicalizer::to_vec` (canonical JSON) of the serialized command → stable across replays.
- `validate_routine_revision`: text limits (objective 4000, instructions 32_000, ≤100 constraints/non_goals, preferred_agent_binding_id ≤256); then `validate_routine_input_contract` enforces a closed JSON-Schema subset (allowed keywords `type, properties, required, additionalProperties, maxProperties, minProperties, maxLength, minLength, maxItems, minItems, items, enum`; depth ≤16; ≤64 properties; `additionalProperties` must be `false`; `enum` ≤128; each `input_bindings` entry must be `{source_pointer, template_variable, value_kind ∈ {TEXT, RESOURCE_REF}, required, max_bytes ≤16 KiB}`; `RESOURCE_REF` must be exactly `workspace_id/resource_id/revision_id`; `{{name}}` placeholders must equal the binding variable set exactly).
- `materialize_routine_inputs(revision, inputs, workspace_id)`: 64 KiB input cap, 16 KiB text, ≤64 inputs, 32 KiB objective; rejects control bytes, extra inputs, unbound leaves, duplicate `(resource_id, revision_id)`; template values for resources become the literal `"[attached Resource]"`; instructions are prefixed `"Routine instructions (untrusted task context):\n"` so they cannot masquerade as trusted constraints.
- Events: `routine.created.v1`, `routine.revision.created.v1` (payload includes `definition_digest`), `routine.status.changed.v1` (payload `from`/`to`/`aggregate_version`).
- Clock: `tx.now()` is the only time source (store-owned); author is always `PrincipalKind::User`.
- Tests: round-trip/blank objective, fingerprint stability, empty-schema materialization, unsupported schema shapes rejected, control bytes and extra inputs rejected.

### crates/domain-task/src/delegation_routing_impl/performance.rs (635 lines)
- Layer: domain ranking input (worker performance), pure function.
- `WorkerPerformanceQuery {profile_id, task_category, now: String, source_observations_complete}` — **`now` is supplied by the caller**; missing category normalizes to `OTHER`.
- `PERFORMANCE_WINDOW_SECONDS = 90*24*60*60`; `HIGH_CONFIDENCE_COVERAGE_BPS = 8_000`.
- Timestamps parsed with `time::OffsetDateTime::parse(..., Rfc3339)`; rejects empty/>64 chars/control chars; `terminal_at < admitted_at` is `InvalidSample`; samples with `terminal_at > now` or `< now - 90d` are skipped (**boundary instant is inclusive** — test `rolling_window_includes_the_exact_ninety_day_boundary...`).
- `WorkerPerformanceAttemptState {RUNNING, COMPLETED, FAILED, CANCELLED, ABANDONED}` — only COMPLETED/FAILED become samples, so cancelled/abandoned are excluded from rate denominators.
- Duplicate `attempt_id` → `DuplicateAttemptId`; profile or category mismatch → `ScopeMismatch`; >100_000 attempts → `InvalidQuery`.
- Cost: `WorkerCostObservation{amount, unit, currency, source, confidence}`; median computed only if every sample has a cost with identical `unit` + 3-letter uppercase `currency`, otherwise `None` (never zero-filled); any `ESTIMATED` → median confidence `ESTIMATED`.
- Confidence: LOW if `!source_observations_complete` or complete verifier observations < 3; HIGH if `sample_count >= 10 && coverage >= 8000` bps; else MEDIUM. `apply_to_candidate` strips `verified_pass_rate_basis_points` and `qualified_end_to_end_latency_ms` when LOW.
- Tests: boundary window, duplicate/non-terminal handling, retry + intervention rates, verifier pass rate and cost median semantics, failed-without-verification counting, ranker integration, incompatible cost → unknown, confidence thresholds.

### crates/domain-task/src/lead_handoff_packet.rs (451 lines)
- Layer: domain projection builder for Task lead change (cites `DELEGATION.md`).
- `LeadHandoffPacket` (`deny_unknown_fields`): workspace_id, task_id, task_spec_revision, plan_revision: Option<u64>, objective, constraints, steps (`LeadHandoffStep`), relevant_decisions (`BoundedDecision`), artifact_refs (`ArtifactVersionRef`), evidence_refs, open_effect_refs, blockers (`DelegationBlocker`), remaining_budget, reason. No field can hold a transcript, reasoning, session handle, provider handle, or secret — asserted by test.
- `LeadHandoffSourcePins` carries caller-resolved allow-sets: `authorized_artifacts`, `committed_decision_event_ids`, `authorized_evidence_ids`, `open_effect_ids`.
- `prepare_lead_handoff_packet(packet, source, context_budget_bytes)`: budget must be 1..=1 MiB; identity pins must match exactly; every reference must exist in the allow-set (`Unauthorized*Reference`); content bounds (objective 4000, ≤64 constraints, ≤256 steps/artifacts/evidence/effects, ≤64 blockers, reason ≤1000); canonical JSON digest `sha256:{hex}`; `ContextBudgetExceeded {limit, actual}` instead of truncation.
- Tests: reference-only shape (forbidden key scan), pre-planning handoff carries `plan_revision: null` and `steps: []`, identity mismatch, widened references, oversize rejection, empty/oversized content, invalid budget.

### crates/linux-process-scope/src/lib.rs (2354 lines)
- Layer: OS adapter (Linux local Environment provider). `#![forbid(unsafe_code)]`; whole Linux impl behind `#[cfg(target_os = "linux")]`, non-Linux stub returns `ScopeError::Unsupported("cgroup-v2 transient scopes require Linux")`.
- Doc is explicit that this is **not a sandbox**: without a separate Environment boundary hiding the user systemd socket, a same-user worker can start another transient unit and leave the scope.
- Launch: `systemd-run --user --scope --quiet --expand-environment=no --unit=litecowork-attempt-{32 hex}.scope --slice=...slice --property=MemoryMax/CPUQuota% /TasksMax /KillMode=control-group /Delegate=no /SendSIGKILL=yes -- <env> -i ...<scope_gate> <executable> <args>`.
- Handshake markers: `LITECOWORK_SCOPE_GATE_V1\n` (READY) and `LITECOWORK_SCOPE_GO_CONSUMED_V1\n` (GO consumed); `wait_for_marker` uses a `sync_channel` + `recv_timeout`; GO is written on stdin only after limits verified. Doc states the marker proves bytes consumed, not that target exec succeeded.
- Quiescence: `CgroupPopulatedObserver` reads the **parent slice** `cgroup.events` and accepts only `populated 0`; missing/unreadable file ⇒ `QuiescenceUnobservable`; deadline ⇒ `QuiescenceTimeout`. `verify_membership` requires `/proc/{pid}/cgroup` `0::` path to equal the resolved scope cgroup; `cgroup_is_descendant` requires a strict child.
- `ScopeError {InvalidInput, Unsupported, Io, Control, CleanupPending{cause, cleanup_error, pending}, QuiescenceUnobservable, QuiescenceTimeout}`; `PendingCleanup {AttemptSlice, PreWorker, ManagedScope}` gives `retry_stop` / `retry_kill_and_wait` / `limits_are_enforced`; `#[must_use]` on `ManagedScope` and `PendingCleanup`.
- `limits_are_enforced()` re-reads `memory.max`, `pids.max`, `cpu.max` and compares quota/period arithmetic — a successful systemd request is never treated as proof.
- `safe_environment` emits only `HOME, PATH, XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_CACHE_HOME, TMPDIR, LANG`; requires absolute normalized paths, rejects `=`/NUL/newlines/colons in PATH; `env -i` clears the manager environment.
- Constants: `CONTROL_TIMEOUT = 3s`, `POLL_INTERVAL = 20ms`, `OUTPUT_LIMIT = 4096`, `PREFIX = "litecowork-attempt-"`.
- Ordering assumption: all deadlines use `std::time::Instant` (monotonic) with `checked_add` overflow guards — **no wall clock is consulted**.
- Tests: path/limit validation before slice start, exec-failure with successful cleanup, retained `AttemptSlice`/`PreWorker` retries, ambiguous start, unprovable quiescence, gate marker exactness, readiness timeout, child-reap failure not counted as quiescence, kill failure retaining scope, cgroup disappearance, parent-is-strict-ancestor.

### crates/linux-process-scope/src/provider_probe.rs (300 lines)
- Layer: OS capability probe. `BUBBLEWRAP_CANDIDATES = ["/usr/bin/bwrap", "/bin/bwrap", "/usr/local/bin/bwrap", "/usr/libexec/bwrap"]`.
- `bubblewrap_binary_is_trusted`: canonicalize, must be a file, `uid() == 0`, `mode & 0o022 == 0`; non-Linux returns `false`.
- `LocalProviderProbe {operating_system, operating_system_build, bubblewrap_candidates, trusted_bubblewrap, cgroup_v2, cgroup_path, systemd_available, user_namespaces_available, reasons}`; `host_mechanisms_present()` requires all four mechanisms.
- Probes: `/proc/sys/kernel/osrelease`; `/sys/fs/cgroup/cgroup.controllers`; `/proc/self/cgroup`; `/run/systemd/system`; `unprivileged_userns_clone` and `max_user_namespaces`.
- Doc claim honored in code: "a probe result is not an attestation" — nothing here reports enforcement.
- Tests: missing binary never trusted, trusted path is one of the trusted candidates, a reason per missing requirement, untrusted/symlinked binary refused.

### crates/linux-process-scope/tests/bubblewrap_scope.rs (100 lines)
- Layer: integration test, `#![cfg(target_os = "linux")]`, `#[ignore = "requires live user systemd manager and delegated cgroup-v2 controllers"]` — so it never runs in ordinary CI.
- Exercises `spawn_bubblewrapped` + `wait_for_quiescence`: asserts `/home` absent inside the namespace, pinned input unchanged, `outputs/result.txt` written, stdout exactly `"pinned input\n"`, `limits_are_enforced()` true.

### crates/local-environment-staging/src/output_budget.rs (216 lines)
- Layer: adapter accounting, explicitly "observations, not enforcement".
- `EnforcementStatus {ENFORCED, NOT_ENFORCED, UNKNOWN}` with `is_enforced()` true only for ENFORCED (UNKNOWN never means "probably fine").
- `FileRootMode {READ_ONLY, READ_WRITE, READ_WRITE_NO_DELETE}`; `probe_root_mode_support` returns `NOT_ENFORCED` for `READ_WRITE_NO_DELETE` on unix/windows (deletion is a parent-directory property; no qualified ACL provider exists) and `UNKNOWN` elsewhere.
- `OutputQuota {max_total_bytes, max_file_count}` with `unbounded()`/`is_bounded()`; `OutputUsage`; `OutputScanLimits {max_entries}`.
- `measure_output_root`: `symlink_metadata` on the root, refuses symlink/non-dir (`InvalidParent`); walks iteratively with an entry cap (`OutputScanLimitExceeded`), rejects any symlink (`OutputRootSymlink`) and any non-regular file (`OutputRootUnsupportedFile`), `checked_add` on bytes → `OutputQuotaExceeded` on overflow.
- `check_output_quota` returns the observed usage or `OutputQuotaExceeded{limit_bytes, observed_bytes, limit_files, observed_files}`. No tests in this file.

### crates/operator-ipc/Cargo.toml (26 lines)
- `bytes, http-body, serde, serde_json, tokio(io-util,net,rt,sync,time)`; unix-only `hex, rustix(net,process), sha2`; windows-only `hex, sha2, windows-local-security`.

### crates/operator-ipc/src/unix.rs (937 lines)
- Layer: transport adapter (owner-only Unix socket). Deliberately does **not** decide which UID is trusted: `accept(authorize_peer)` / `connect(authorize_peer)` take a `FnOnce(PeerCredentials) -> bool`.
- Constants: `MAX_CONCURRENT_CONNECTIONS = 8`, `CONNECT_DEADLINE = 3s`, `HEADER_DEADLINE = 3s`, `BODY_DEADLINE = 30s`, `WRITE_DEADLINE = 30s`, `MAX_UNIX_SOCKET_PATH_BYTES = 103`.
- Endpoint derivation: domain separator `litecowork.local-operator-ipc.endpoint.v1\0` + canonical dir → `operator-{hex(sha256[..16])}.sock`; directory must exist, be a real dir, owned by `geteuid()`, mode `0700`; socket forced to `0600`.
- `SocketIdentity {device, inode, uid}` matches before/after bind and before removal, so a replaced or pre-existing socket is refused (`UnsafeEndpoint`, `EndpointAlreadyActive`); `prepare_stale_endpoint` treats any connect error other than `ConnectionRefused`/`NotFound` as "active or uncertain".
- `PeerCredentials {uid, gid, pid: Option<u32>}`; rustix's signed `-1` PID normalizes to `None` (test).
- `ExchangeState {New, RequestWritten, RequestRead, Streaming, Complete, Failed}`; `write_response` requires the response `request_id` to equal the accepted request's; stream handshake must be status 200, empty body, `content-type: application/vnd.litecowork.operator-stream+json`; `read_stream_message_for_session` permits idle gaps; `wait_for_stream_disconnect` treats EOF as cancellation and **any extra byte** as `ProtocolError::InvalidFrame`.
- Tests: peer-pid normalization and a production-deadline test that constrains the socket send buffer and asserts a stalled `write_stream_message` returns `DeadlineExceeded`, poisons the exchange, releases its full body reservation, and lets the peer observe truncated-frame closure.

### crates/operator-ipc/src/windows.rs (642 lines)
- Layer: transport adapter (Windows named pipe). All items `#[cfg(windows)]`; endpoint/name helpers compile on host for tests.
- Peer authentication is the kernel logon-SID DACL installed by `windows_local_security::create_operator_pipe(&pipe_name, first)`; "process IDs are deliberately not used as an authorization signal".
- Pipe name: domain separator `litecowork.local-operator-ipc.windows-endpoint.v1\0` + UTF-16LE canonical path → `operator-{hex(sha256[..16])}` (test pins `operator-ed73cadd8b9dcaf314e31b135d6ab071` for the Alice path and asserts the path is not disclosed).
- `bind()` pre-creates `MAX_CONCURRENT_CONNECTIONS = 8` instances; first-instance `PermissionDenied` maps to `EndpointAlreadyActive` (never replaced or treated as stale); `next_server_slot` keeps pending instances at `permits+1` capped at `INSTANCE_CAPACITY = 8`.
- Deadlines: `CONNECT_DEADLINE = 3s`, `HEADER_DEADLINE = 3s`, `BODY_DEADLINE = 30s`, **`WRITE_DEADLINE = 3s`** — a divergence from the Unix 30s.
- Same `ExchangeState`/handshake/unary-correlation rules as Unix; disconnect kinds `BrokenPipe|ConnectionAborted|ConnectionReset|NotConnected` become `PeerRejected`.

### crates/storage-core/src/artifact.rs (250 lines)
- Layer: storage port/contract types. `ArtifactRecord` (artifact_id, workspace_id, resource_id, task_id, kind, display_name, current_version, library_status, created_at, version).
- `ArtifactLibraryAction {PROMOTE, ARCHIVE}`; `ArtifactLibraryCommand`; `CommittedArtifactLibraryCommand {artifact, event: Option<DomainEvent>, replayed}` — `None` for a fresh archive of an already-ARCHIVED artifact at the same version.
- `ArtifactContentRecord` (`tag = "kind"`): `MANAGED_BLOB {storage_ref: BlobRef, content_digest, media_type, size_bytes}` | `EXTERNAL_RESOURCE {resource_ref, provider_revision, observed_digest, observed_at}`.
- `ArtifactVersionRecord` fields include `input_refs`, `provenance`, `created_by_attempt`, `verification_refs`; `InvalidationRecord`; `DependencyInvalidationPageRequest` keyset on `(created_at, invalidation_record_id)`.
- `ArtifactReadStore`: `get_artifact`, `get_artifact_append_heads`, `list_artifacts_page`, `list_dependency_invalidations_page` (None hides absent/foreign edges), `get_artifact_version`, `read_artifact_content_bounded`.
- `ArtifactVersionAppendCommit` separates three fences: `expected_artifact_version`, `expected_content_version`, `expected_resource_version`, plus `expected_parent_resource_revision_id`; `CommittedArtifactVersionAppend.events` is ordered "[Resource revision observed, then Artifact version created]".
- Doc states the implemented subset: SQLite adapter supports managed `text/plain` ≤1 MiB; other media types and provider-backed artifacts stay read-only.

### crates/storage-core/src/backup.rs (428 lines)
- Layer: storage contract for local backup/restore (E13-S01).
- Container: magic `b"LCBK1\0"`, `BACKUP_CONTAINER_VERSION = 1`, chunk cipher `XChaCha20-Poly1305` id 1, KDF `PBKDF2-HMAC-SHA256` id 1 with `DEFAULT_ITERATIONS = 600_000`, `MINIMUM_ITERATIONS = 100_000`, `MAX_PAYLOAD_CHUNKS = 1_000_000`, `MAX_CHUNK_BYTES = 64 MiB`, `MAX_MANIFEST_BYTES = 8 MiB`, `MAX_BLOB_DESCRIPTORS = 1_000_000`.
- Paths: `BACKUP_DATABASE_SNAPSHOT_PATH = "database.snapshot.sqlite3"`, `BACKUP_BLOB_ROOT = "blobs"`, `BACKUP_EXCLUDED_BLOB_PURPOSES = ["RESOURCE_UPLOAD_CHUNK"]`.
- `BACKUP_EXCLUDED_TABLES` (never carried) includes runtime-local handles (`agent_endpoint_bindings`, `agent_session_host_bindings`, `agent_host_instances`, `capability_*_bindings`, `provider_input_bindings`, `automation_trigger_bindings`, `channel_*`, `environment_*`, `resource_location_bindings`, `file_identity_bindings`), local registries (`resource_upload_chunk_blob_registry`, `resource_index_blob_registry`, `resource_content_blob_registry`), diagnostics (`runtime_incarnation_local_observations`, `environment_provision_previews`, `runtime_offers`, `provider_circuit_states`, `pairing_tokens`) and live authority (`execution_leases`, `environment_control_leases`, `secret_leases`, `capability_locks`).
- `BACKUP_REVOKED_ON_RESTORE_TABLES = ["capability_grants"]` — restored grants are revoked rather than dropped, because `capability_invocations`/`approval_uses` reference them.
- Migration/version: `SchemaCompatibility {Compatible{found,min,max}, RefusedTooNew, RefusedTooOld}` via `check_schema_compatibility(found, minimum_supported, maximum_supported)`; a newer schema is refused (unknown invariants) and older-than-baseline is refused (unprovable migration chain). Test pins max 26.
- Determinism: `backup_integrity_payload` sorts blobs by `relative_path`, key refs by `(workspace_id, purpose, key_version)`, cursors by `origin_runtime_id` — the adapter owns canonical JSON + SHA-256. `BackupRestoreOutcome.new_runtime_identity_required` is always true for a restored installation (no `runtime-state.json`).

### crates/storage-core/src/dependency.rs (54 lines)
- `DependencyEdgeRecord {dependency_edge_id, workspace_id, source_resource_id, source_revision_id, dependent_kind, dependent_ref, created_at}`; `DependencyFreshness {CURRENT, STALE, CONFLICTED, UNKNOWN}`; `ResourceDependencyViewRecord {dependency_edge, dependent_freshness, latest_invalidation}`; `ResourceDependencyPageRequest` keyset on `(created_at, dependency_edge_id)`; `ResourceDependencyReadStore::list_resource_dependents_page(...) -> Result<Option<Vec<_>>, _>` where `None` hides missing/out-of-scope sources.

### crates/storage-sqlite/src/blob.rs (2106 lines)
- Layer: storage adapter (encrypted content-addressed blob store). Cross-platform with a Unix `openat` path and a portable `NamedTempFile` path.
- `WorkspaceBlobKey {version: u32, bytes: Zeroizing<[u8;32]>}`; `WorkspaceBlobKeyProvider {current_key, key_by_version}`; every path rejects `version == 0`.
- Envelope: `"LCB1"` + u32 BE key version + 24-byte nonce + XChaCha20-Poly1305 ciphertext (+16 tag); AAD = `litecowork-blob-v1\0{workspace}\0{PURPOSE}\0{digest}\0{size}\0{media_type}` — binds Workspace, purpose, digest, size and media type.
- Path: `{root}/{hex(sha256(workspace_id))}/{purpose lowercase}/{hex(sha256 digest)}.blob`.
- Unix: `openat`/`mkdirat`(0o700)/`linkat` publish with `NOFOLLOW|DIRECTORY|CLOEXEC`, directory `sync_all` after each mutation, `TemporaryBlobEntry` Drop cleanup, `open_configured_blob_root*` re-syncs when `configured_root_sync_complete` is false.
- Portable: 0700 directory tree validation, `persist_noclobber`, `AlreadyExists` → re-read and compare bytes (`Integrity` on mismatch), exact envelope length check before read.
- `resource_index_tokens`: HKDF-SHA256 with salt `LiteCowork.ResourceIndex.key.v1`, `expand("term-token-hmac-sha256")`, HMAC over `LiteCowork.ResourceIndex.term.v1\0{term}` → lowercase hex (64 chars), version-scoped.
- Tests (all meaningful): round-trip + workspace scoping, key-unavailable fails closed with zero files, token stability/scoping, purpose/wrong-ciphertext rejection, cross-workspace and cross-purpose envelope transplant → `"blob authentication failed"`, idempotent remove, symlink/directory/FIFO-object refusal (FIFO test respawns itself with a 5 s deadline), configured-root and namespace sync retries, symlinked root/workspace/purpose ancestors refused, and three race tests proving get/put/remove stay inside the already-opened directory when the workspace directory is swapped after validation.

### crates/storage-sqlite/src/verification_runs.rs (2067 lines)
- Layer: storage service implementing `VerificationRunStore` for `SqliteWorkspaceStore`.
- `start_verification_run`: canonicalizes `event.recorded_at` (`canonicalize_utc_timestamp`), validates event context, requires status PENDING, `version == 1`, no timestamps/evidence/failure, `task_spec_revision != 0`, and `run.verifier_kind == actor.verifier_service_id`; then applies `run.start(...)` and writes a `PENDING` insert plus a guarded `UPDATE ... SET status='RUNNING',version=2 WHERE status='PENDING' AND version=1`.
- `settle_verification_run`: requires `expected_version != 0`, a terminal `next_status`, and for PASSED `evidence_refs` exactly equal to the `verified_evidence` id list with no `failure_code`; non-PASSED must carry no evidence; FAILED/INCONCLUSIVE require a `failure_code`. Evidence is force-bound to `verification_run_id` and stamped with `event.recorded_at`. Only the starting verifier may settle.
- Idempotency: `request_dedup(principal_id, request_id, request_digest, response_json, ...)` with principal `runtime:{runtime_id}:{runtime_incarnation_id}`; fingerprint over canonical JSON of (workspace, actor, run id, expected_version, status, evidence_refs, verified_evidence, failure_code); identical → `replayed: true`, different → `Invalid("request ID was reused ...")`.
- Ordering/versioning: `workspace_origin_sequences` is bumped per `(workspace_id, origin_runtime_id)` inside the same transaction; event `schema_version` stays `1` while the payload schema is `.v2` (`verification.started.v1`, `verification.completed.v2`); the run UPDATE is fenced on `status='RUNNING' AND version=?`.
- Head freshness: `check_task_criterion_and_inputs` requires Task status `VERIFYING`, `current_spec_revision == run.task_spec_revision`, exactly one matching criterion whose canonical digest equals `criterion_digest`, and each input/subject Resource head to be the unique current head with no children and no competing head; `allow_cancelled` permits closing against the immutable revision when the Task is `CANCEL_REQUESTED`/`CANCELLED`. `validate_input_head` also compares the stored `content_digest` to `consumed_digest` (`Integrity` on mismatch).
- `insert_dependency_edges` writes `dependent_kind='VERIFICATION_RUN'`, id `verification:{run}:{resource}:{revision}`, `dependent_ref='verification://{run}'`, `ON CONFLICT DO NOTHING`.
- Authorization: `authorize_runtime` requires active Workspace, current incarnation, `recovery_state IN ('READY','DEGRADED')`, active non-revoked binding with `EXECUTOR` role, and availability `ONLINE|DEGRADED`; `authorize_owner` = `require_write_access` (ARCHIVED → `WORKSPACE_ARCHIVED`), read path uses `require_read_access`.
- `load_run` refuses unknown status/failure-code tokens with `StoreError::Integrity` and re-runs `run.validate()` on read.
- Tests: v1 payload omits `failure_code`, v2 payload carries only the typed code, verifier-evidence binding (task/verifier/subject/run/payload_ref), atomic PASSED commit creating exactly one evidence row and two events with exact replay, cancelled run closing as INCONCLUSIVE/`TASK_CANCELLED` with no evidence, completion-evaluator rejections (running run, stale digest, REPORTED/OBSERVED, other run, unpinned subject, extra bound evidence), in-memory fixtures plus `SQLITE_V1_DDL` / `SQLITE_V20_DDL` / `SQLITE_V22_DDL` trigger guards asserting SQLite itself rejects caller-supplied PASS and terminal rewrites.

### crates/storage-sqlite/src/context_document_purge.rs (3309 lines)
- Layer: storage service implementing `ContextDocumentPurgeStore` (largest file in the bucket).
- Operations: `begin_context_document_purge`, `get_context_document_deletion_status`, `list_pending_context_document_purge_targets` (limit 1..=100), `list_pending_empty_context_document_purge_plans`, `acknowledge_context_document_purge_target_if_object_physically_removed`, `complete_empty_context_document_purge_plan_if_verified_empty`.
- Statuses `ACTIVE, REVOKED, DELETION_PENDING, DELETED`; replica kinds `BLOB, DERIVED_INDEX, UPLOAD_CHUNK`; tables `context_document_purge_plans`, `context_document_purge_receipts`, plus `resource_locations`, `resource_text_indexes`, `resource_*_blob_registry`, `resource_upload_sessions|chunks|blob_reservations`.
- Sealing: `discover_local_targets` groups targets by `(content/extracted/chunk digest, kind)`, cross-checks revision vs registry size/media-type/state (`RESERVED|TRACKED|DELETING`) and upload chunk size ≤ 4 MiB; manifest digest = `sha256` of canonical targets JSON; `reject_external_locations` blocks any non-`litecowork.encrypted_blob` location; an empty plan is refused while local inventory remains.
- Ordering: the caller's `event.recorded_at` must be **strictly greater** than `resources.updated_at` ("purge timestamp must advance the Resource"), and the completion event must be ≥ the acknowledgement event.
- Locking/ordering of BlobStore: takes `resource_index_write_lock` then per-digest content and upload-chunk locks (poison→`into_inner`), does all `BlobStore` put/get **outside** the SQLite writer transaction, then re-runs the same inspection inside `TransactionBehavior::Immediate` and rejects if the candidate differs (`Conflict`) or returns `Ok(false)` for stale acknowledgement/completion.
- Events: `resource.context_document.status.changed.v1`, `resource.context_document.purge.acknowledged.v2`, `resource.upload.status.changed.v1` (reason `CONTEXT_DOCUMENT_DELETION`); `PURGE_RECONCILER_SERVICE_ID = "context_document_purge_reconciler"`.
- Migration/back-compat: legacy `context_document_json` rows without purge metadata are rejected as `Integrity` when a plan exists ("purge tombstone has no sealed plan"); `verification_runs`-style `request_dedup` receipt guards exact replay, and `verify_request_receipt` re-checks the replay's resource/workspace/entity identity.
- Tests: a `BoundaryBlobStore` wrapper that opens a second `BEGIN IMMEDIATE` on every put/get and counts `violations` (asserted 0), plus version-race tests proving a stale preflight leaves only an unreferenced snapshot and no plan/receipt/event.

### crates/storage-sqlite/src/delegation_failover.rs (974 lines)
- Layer: storage admission for `POLICY_FAILOVER` lead handoff (`POLICY_FAILOVER_CAUSE = "POLICY_FAILOVER"`).
- `StoredFailoverPolicy {mode, triggers, fallback_agent_binding_ids (≤3), max_lead_changes}` read from `task_spec_revisions.lead_failover_policy_json` — never from the caller. `mode` must be `ALLOW_LISTED`; `ASK`/`DISABLED` refuse.
- `StoredFailoverObservation {trigger, affected_agent_binding_id, source.service_id, source_observation_ref, runtime_incarnation_id, observed_at, expires_at}`; triggers `AGENT_UNAVAILABLE | QUOTA_EXHAUSTED | RUNTIME_UNAVAILABLE`; `RUNTIME_UNAVAILABLE` requires an incarnation id.
- Freshness (trusted-clock critical): `validate_observation_freshness(recorded_at)` parses RFC3339 for event time, `observed_at`, `expires_at` and rejects unless `observed <= recorded && expires > recorded && expires > observed`.
- Reuse: one observation authorizes at most one lead change **per Workspace** (`LEAD_FAILOVER_OBSERVATION_ALREADY_USED`), counted across `task.lead_agent.changed.v1` payloads by `trigger_observation.source.service_id` + `source_observation_ref`; `LEAD_FAILOVER_CHANGE_LIMIT_REACHED`; `LEAD_FAILOVER_NO_ELIGIBLE_FALLBACK`; `STALE_SPEC_REVISION` distinct from `StoreError::Conflict`.
- Fallback selection is strictly ordered by the pinned list and requires `enabled = 1 AND lead_eligible = 1` in `agent_bindings`; an out-of-order fallback is rejected.
- Tests: atomic commit + exact replay, stale/expired/future/wrong-lead/wrong-trigger observations leave no mutation, out-of-order and disabled fallbacks, single-use observation across Tasks, running Task with an active Attempt fails over while preserving the Attempt, live `TASK_PLANNING` session blocks, unresolved Effect blocks, change-limit and `ASK` recheck, task/spec version fencing, durability across process restart, `DISABLED` policy refuses crafted observations.

### crates/storage-sqlite/src/notification.rs (783 lines)
- Layer: storage service (owner-scoped reads + preference writes + delivery recording). Cites `docs/DATA-MODEL.md`, `docs/API.md`, `docs/schemas/sqlite-v1.sql`; adds no schema.
- `list_notification_deliveries(owner, workspace, status, after_created_at, after_delivery_id, limit)`: `MAX_PAGE = 201` (OpenAPI max 200 + 1 for cursor detection), statuses `PENDING|SENDING|SENT|FAILED|AMBIGUOUS|SUPPRESSED`, keyset **ascending** on `(created_at, delivery_id)` because `created_at` alone is not unique; cursor pair is all-or-nothing.
- `list_notification_preferences`: returns only persisted rows, no synthetic defaults.
- `set_notification_preference(...)`: `expected_version` (0 = first write), validates `NOTIFICATION_POLICIES = [ALWAYS, ON_SUCCESS, ON_FAILURE, ON_CONDITION, SILENT]` and `QuietHours` (`timezone` ≤64, weekdays subset of `MONDAY..SUNDAY` unique, `HH:MM` with hour<24 minute<60); writes the aggregate-state blob first (verify round-trip), then one `TransactionBehavior::Immediate` doing upsert + `notification.preference.changed.v1` event + `request_dedup` receipt; `preference_receipt` validates response digest and that the receipt belongs to the same `(workspace, event_class)`.
- `record_automation_outcome_delivery(...)`: delegates the decision to `domain_responsibility::{decide_notification, event_class_for, NotificationFacts}` using the Automation's own `notification_policy`, the Workspace class preference, quiet hours, `already_delivered`, `has_local_channel`, and `now_epoch_ms`; a suppressed decision inserts nothing; `delivery_id = "ndl-" + sha256(workspace, dedupe_key)`. **An unparsable quiet window suppresses (fail closed)** rather than delivering outside it.
- Note: `now_epoch_ms` is caller-supplied; `attempt_runtime_id` and `attempt_host_epoch` must be provided together.

### crates/storage-sqlite/src/rag_retrieval_tests.rs (476 lines)
- Layer: focused tests for the Core-owned Resource index projection (`resource_index_state`).
- Asserts a ready projection binds `indexed_source_digest == current_content_digest` with `parser_id == "litecowork.plain-text.v1"`; a moved head to an unindexable ZIP reports `indexed_revision_id: None`, `superseded_index_count: 1`; `application/zip` maps to `ResourceTextIndexSkipReason::UnsupportedType`; unknown and other-Workspace resources return `None`.
- Helpers build real revisions through `create_revision` + `put_chunk` + `commit` (upload protocol), i.e. these are end-to-end storage tests, not mocks.

### crates/storage-sqlite/src/resource_edge_tests.rs (319 lines)
- Tests `DerivedResourceCreateCommit` / `ResourceEdgeRecord` (`relation = "DERIVED_FROM"`, provenance `{provider: "archive-extractor", source: "zip-entry"}`).
- Asserts child Resource + edge commit once and replay as one receipt; `edge_event.origin_sequence == resource.event.origin_sequence + 1`; rows `(resources, edges, events, receipts) == (1,1,2,1)`; SQLite triggers refuse `UPDATE`/`DELETE` of `resource_edges`; cross-Workspace source and missing revision fail atomically leaving `(0,0,0)`.

### crates/storage-sqlite/src/routine_tests.rs (189 lines)
- End-to-end Routine create/revise/archive through `RoutineService` + `SqliteRoutineStore`: idempotent create replay returns the same aggregate; revise → `(current_revision, version) == (2,2)`; archive → `(ARCHIVED, 3)`; historical revision 1 still readable.
- Asserts SQLite rejects `UPDATE`/`DELETE` on `routine_revisions` (immutability triggers) and that a Routine is invisible from another Workspace.

### crates/storage-sqlite/src/verification_runs.rs — see above (tests embedded).

### crates/storage-sqlite/src/writer_shutdown_tests.rs (171 lines)
- Layer: writer-thread lifecycle test. Saturates `writer_queue_capacity: 3`, blocks the writer in a gated `Command::Operation`, abandons two reply receivers (one read, one `CommitWorkspace`), sends `Command::Shutdown`, and asserts `try_send` returns `TrySendError::Full`.
- After joining the writer and reopening the store, the abandoned write **did** commit (`version == 2`, `replication_policy == METADATA_ONLY`), `read_workspace_events` returns 2, and `rebuild_workspace_projection` reproduces the current workspace — i.e. an admitted write is durable even when its response waiter is gone, and replay is deterministic.

### crates/storage-sqlite/tests/agent_registry_cache.rs (142 lines)
- Integration test for `AgentRegistryCacheStore`: `replace_agent_registry_snapshot` is atomic; reads intentionally do **not** filter `expires_at` (the Operator decides staleness — the expired snapshot reads back identically); an empty snapshot clears prior rows (`None`); a malformed snapshot with a duplicate `registry_agent_id` returns `StoreError::Invalid` and leaves the last good cache readable. Fixture metadata uses a fake `safe-package` npx distribution.

## Cross-cutting observations

- **Implemented architecture.** Every storage entry point is a `SqliteWorkspaceStore` method driven through the single `Command::Operation` writer channel (`execute_command` + `mpsc` reply), with `TransactionBehavior::Immediate` for mutations. Domain crates are pure (no I/O, no clock) and expose traits (`RoutineStore`, `ArtifactReadStore`, `ArtifactVersionWriteStore`, `VerificationRunStore`, `ContextDocumentPurgeStore`, `ResourceDependencyReadStore`, `WorkspaceBlobKeyProvider`). OS-specific code is isolated behind `#[cfg(target_os = "linux")]` / `#[cfg(windows)]` with fail-closed stubs elsewhere.
- **Trusted-clock / ordering assumptions.** (a) No crate in this bucket reads a wall clock during a domain decision: `now` is a parameter (`WorkerPerformanceQuery.now`), a store callback (`RoutineTransaction::now()`), or a caller-supplied event field (`event.recorded_at`, `now_epoch_ms`). (b) Storage canonicalizes and *strictly advances* timestamps (context-document purge requires `recorded_at > resources.updated_at`; completion ≥ acknowledgement). (c) Failover freshness is the only explicit temporal window in this bucket: `observed <= recorded < expires`. (d) Cursors never rewind on a backwards clock (`advance_cursor` keeps the max; test writes `1970-01-01T00:00:02.000000000Z`). (e) The 90-day performance window is inclusive at the boundary and excludes future samples. (f) `linux-process-scope` uses `Instant` (monotonic) only; `hlc_timestamp` is carried through events but never generated or compared in these files — ordering authority lives elsewhere.
- **Idempotency is uniform.** `request_dedup(principal_id, request_id)` + SHA-256 fingerprint of canonical JSON + stored response; identical replay sets `replayed: true`, different fingerprint is `StoreError::Invalid`/`IdempotencyConflict`/`Conflict`. Replay resolution consistently happens **before** mutable-state checks (see `a_repeated_settlement_replays_before_mutable_automation_checks`, `resolve_artifact_version_append_replay`).
- **Migration and version handling.** No migration runner appears in this bucket; what is visible is (i) `check_schema_compatibility(found, min, max)` refusing both newer-than-max and older-than-baseline schemas, (ii) event `schema_version: 1` with payload `.v2` suffixes (`verification.completed.v2`, `resource.context_document.purge.acknowledged.v2`), (iii) test DDL constants `SQLITE_V1_DDL`, `SQLITE_V20_DDL`, `SQLITE_V22_DDL` used to prove SQLite triggers enforce the newer invariants, (iv) legacy-tolerant deserialization (`EvidenceRecord.verification_run_id` `#[serde(default)]`), (v) versioned encrypted blobs (`version` field in the `LCB1` header, `key_by_version`, `resource_index_token(key_version)`), and (vi) versioned backup containers with min/max supported schema. Compatibility is always fail-closed on unknown.
- **Divergences / risks worth naming.** `operator-ipc` `WRITE_DEADLINE` is 30 s on Unix but 3 s on Windows. `domain-effects` validates timestamps by shape only, so Effect/Evidence ordering is unenforced at that layer. `linux-process-scope` documents its own insufficiency as a sandbox (same-user systemd socket escape). `local-environment-staging::output_budget` is observation-only, with `READ_WRITE_NO_DELETE` reported `NOT_ENFORCED` rather than simulated. `context_document_purge` contains `unwrap_or_default()`/`unreachable!("the grouped replica kind is locally constructed")` and a `as u64` cast path in size comparison — bounded but worth review. `expect`/`unwrap` appear in test helpers and fixtures only; no `unwrap` on a load-bearing production path was observed in these files.
- **Evidence tier.** Everything here is **TARGET + CODE**: the modules compile-shaped contracts with unit/integration tests, but no SERVIED/INTEGRATED/QUALIFIED evidence exists in this bucket. The one live-environment test (`bubblewrap_scope.rs`) is `#[ignore]`d, so Linux containment is not exercised by default CI.

## Coverage check

- All 29 listed files read to the end; large files paged (`domain-effects/src/lib.rs`, `domain-responsibility/src/routines.rs`, `linux-process-scope/src/lib.rs`, `blob.rs`, `verification_runs.rs`, `context_document_purge.rs`, `delegation_failover.rs`, `automation_host_tests.rs`). Line total reconciles with the plan header (21,951).
- No generated files, lock files, icons or binaries in this bucket; nothing skipped.
- Out of scope by design: `automation_host.rs` (test target only, not listed), `storage-sqlite/src/lib.rs` migration runner, and the React/Operator side are not in this bucket and were not read.
