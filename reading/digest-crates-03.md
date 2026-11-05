# Digest crates-03

Files: 18 read (14 full, 4 partial) / 26 listed | Lines read: ~7,344 / 21,769 | Bytes: ~259 KB / 818 KB

## Per-file notes

### crates/domain-environment/src/browser_pointer.rs (250 lines)
- **Layer:** domain (pure helper). Fenced mapping of a *displayed* browser frame to Chrome CSS viewport coordinates. Explicitly never dispatches input; caller must still authorize + persist via control lease and Effect path.
- `BrowserPointerError`: `InvalidGeometry, InvalidIdentity, StaleSession, StaleTab, StaleTabGeneration, StaleControlEpoch, StaleFrame, ExpiredFrame, FrameFromFuture, StaleGeometry, OutsideFrame`.
- `BrowserViewport::new(css_width, css_height, device_scale_factor)` — rejects 0 dims, non-finite DPR, DPR outside `0.5..=8.0`.
- `PaneBounds::new(left, top, width, height)` — rejects negative `left`/`top`, non-positive `width`/`height`, non-finite `left+width`/`top+height`.
- `BrowserFrameGeometry::new(...)` (11 args): `session_id, tab_id, tab_generation, control_epoch, frame_sequence, geometry_revision, captured_at_ms, scroll_x_css, scroll_y_css, viewport, pane`. Rejects blank ids, zero `tab_generation/control_epoch/frame_sequence/geometry_revision`, non-finite scroll.
- `transform_browser_pointer(geometry, position, expected_session_id, expected_tab_id, expected_tab_generation, expected_control_epoch, expected_frame_sequence, expected_geometry_revision, now_ms, max_frame_age_ms) -> Result<BrowserViewportPosition, _>` — fixed check order: session → tab → tab_generation → control_epoch → frame → geometry → `now<captured` (`FrameFromFuture`) → `age>max` (`ExpiredFrame`) → finite → pane containment (`x_in_pane >= width` is outside) → linear scale to `css_width`/`css_height`.
- Deliberate: DPR is **not** multiplied (UI CSS px and CDP dispatch are both CSS px); scroll offset retained for provenance only, never added to the mapped point.
- No tests in-file; behaviour is exercised by `tests/browser_control.rs`.

### crates/domain-environment/tests/browser_control.rs (973 lines)
- **Layer:** integration test. ~22 tests over `BrowserControlLease`, `BrowserControlActor`, `BrowserObservation`.
- Constructors: `BrowserControlLease::new(env, task, attempt, runtime, incarnation, actor, epoch, owner_since_ms, expires_ms)`; `::new_with_agent_owner(..., current_actor, original_agent_owner: Option<..>, epoch, since, expires)` for rehydration.
- Methods: `map_pointer_for_current_owner`, `take_control(actor, expected_epoch, now_ms, in_flight)`, `authorize_action(actor, epoch, now_ms)`, `return_to_agent(current_user, agent, expected_epoch, now_ms, in_flight, last_observed_revision, max_age_ms, &observation)`.
- Error codes asserted: `Expired`, `StaleEpoch{expected,actual}`, `WrongOwner`, `ActionsInFlight`, `TimeBeforeOwnerSince`, `EpochExhausted`, `StaleObservation`, `ObservationIdentityMismatch`, `Pointer(ExpiredFrame)`, `Pointer(StaleControlEpoch)`.
- Invariants proven: takeover advances epoch 7→8, return advances 8→9; agent actor can never `take_control`; a different user cannot return control; observation must match env/task/attempt/runtime/**incarnation**; observation at exactly `max_age_ms` accepted, +1 ms rejected; same-millisecond-as-takeover observation rejected (`StaleObservation`); rehydrated user lease without original agent owner is refused (`WrongOwner`); `u64::MAX` epoch fails closed in both directions (`EpochExhausted`).

### crates/domain-rag/src/limits.rs (164 lines)
- **Layer:** domain. Hard, fail-closed budgets: `MAX_RESULTS=20`, `MAX_EXCERPT_BYTES=2_048`, `MAX_TOTAL_EXCERPT_BYTES=16_384`, `MAX_QUERY_BYTES=2_048`.
- `RetrievalBudgetError::reason_code()`: `RETRIEVAL_RESULT_COUNT_OUT_OF_RANGE`, `RETRIEVAL_EXCERPT_TOO_LARGE`, `RETRIEVAL_TOTAL_EXCERPT_TOO_LARGE`, `RETRIEVAL_QUERY_TOO_LARGE`.
- `RetrievalBudget{max_results, max_excerpt_bytes, max_total_excerpt_bytes, max_query_bytes}` — `serde(deny_unknown_fields, rename_all="camelCase")`; default `max_results=10`; `local_defaults()`; `validate()`; `validate_query(&str)`. `validate()` also enforces `max_total >= max_excerpt`.
- 5 unit tests.

### crates/domain-rag/src/provider.rs (152 lines)
- **Layer:** domain port. `LOCAL_LEXICAL_PROVIDER_ID="litecowork.local-lexical"`, `LOCAL_LEXICAL_PARSER_ID="litecowork.plain-text.v1"` (doc: matches `resource_text_indexes.parser_id` constraint).
- Enums: `ProviderAvailability{Qualified,NotConfigured,Unavailable,Incompatible}`, `ProviderPlacement{Local,Cloud}`, `RetrievalMode{LexicalOnly,Hybrid}` (SCREAMING_SNAKE_CASE).
- `ProviderStatus{provider_id,provider_version,availability,retrieval_mode,placement,reason_code?,reason?}`; `RetrievalRequest{workspace_id,query,scope,budget}`; `RetrievalPage{citations,scanned_candidates,truncated}`.
- `RetrievalError{NotConfigured|Unavailable|Incompatible|InvalidRequest|BudgetExceeded|Integrity}{reason_code,reason}` + `reason_code()`/`reason()`.
- `trait RetrievalProvider: Send + Sync { fn status(); fn search(&RetrievalRequest) -> Result<RetrievalPage, RetrievalError>; }` — Core never assumes wire protocol.

### crates/domain-rag/src/local_lexical.rs (395 lines)
- **Layer:** adapter (provider over the Core-owned encrypted text index; "a provider, not a second index").
- `LOCAL_LEXICAL_PROVIDER_VERSION="litecowork.local-lexical/1"`; `CANDIDATE_MULTIPLIER=4`, `MAX_CANDIDATES=100`.
- `LocalLexicalRetrievalProvider{store: Arc<dyn ResourceStore>, provider_version}`; `supported_parser_id()`; free fn `parser_generation_is_compatible(&str)`.
- `search()` order: blank workspace → `RETRIEVAL_WORKSPACE_INVALID`; empty trimmed query → `RETRIEVAL_QUERY_EMPTY`; `budget.validate()` → `BudgetExceeded`; `validate_query`; `scope.validate()` → `RETRIEVAL_SCOPE_INVALID`; `Resources([])` → `RETRIEVAL_SCOPE_EMPTY`; `candidate_limit=4*max_results` clamped `1..=100`; `store.search_indexed_resource_text(ws, query, None×5, limit)`.
- Per candidate: parser mismatch ⇒ **whole request fails** `Incompatible/PROVIDER_INDEX_GENERATION_INCOMPATIBLE` (not skipped); `scope.allows(resource_id, revision_id)`; `is_still_authorized()`; `build_citation()`; running `total_excerpt_bytes` overflow ⇒ `truncated=true` + break.
- `is_still_authorized()`: `get_resource_detail` → `context_document_is_active` → `current_revision_id == record.resource_revision_id` → `summary.content_digest == source_content_digest` else `Integrity/RETRIEVAL_SOURCE_DIGEST_MISMATCH`.
- `context_document_is_active()`: no ContextDocument ⇒ readable; `status == "ACTIVE"` only; missing status ⇒ false.
- `citation_id = "{resource_id}:{revision_id}:{primary.start_utf8_byte}"`.
- `truncate_excerpt()` walks back to a char boundary; `RetrievalWarning::ExcerptTruncated` and `EmbeddedInstructionDetected` (via `detect_embedded_instructions`).
- `map_store_error()` → `RETRIEVAL_REQUEST_INVALID`, `RETRIEVAL_INDEX_UNAVAILABLE`, `RETRIEVAL_INDEX_BUSY`, `RETRIEVAL_SOURCE_INTEGRITY_FAILURE`.
- 5 tests (ACTIVE/REVOKED/DELETED/DELETION_PENDING, no-ContextDocument, missing status, UTF-8 truncation, parser generation).

### crates/domain-responsibility/src/tests.rs (383 lines)
- **Layer:** domain unit tests for `super::*` (Coworker/Automation/Trigger lifecycle, occurrence identity).
- Pinned SHA-256 occurrence keys: `Schedule{trigger-weekly,1791000000000}` = `3761b46716c5a9a3c4088c0dcb18cfc42dd647fea86ccf2d3149140c1d502460`; `Manual{...}` = `1a29dba53c8865650745074940a438f97e44eca49c011dcfb6883cd212d7f294`; `ConnectorEvent{...}` = `b0b42fa53ec4eb84d1d5487f6b72f3156550746414df4c239e562d47e8e63b45`.
- `encode_occurrence_fields` prefix `LiteCowork/AutomationOccurrence/v2\0`, UTF-8 byte-length encoding `[0,0,0,2,0xc3,0xa9]` for `"é"`, field-boundary non-ambiguity.
- `check_origin_status`: `Paused+Owner=Ok`, `Paused+Proactive=CoworkerInactive`, `Archived+Owner=CoworkerArchived`.
- `check_coworker_transition`: archive blocked by `is_primary`/`has_active_automation`/`has_nonterminal_tasks` (`ArchiveBlocked`); `Archived→Active` = `CoworkerArchived`.
- `check_automation_transition`: resume needs `trigger_dependencies_reconciled` (`ReconciliationRequired`), `RoutineArchived`, `AutomationDisabled` terminal.
- `next_version`: stale ⇒ `VersionConflict`, `u64::MAX` ⇒ `VersionOverflow`.
- `TestRunIdentity` has its own key domain (keyed by `automation_revision`), distinct from manual occurrence keys.
- `validate_triggers`: non-empty, unique `trigger_id`, `SpecificRuntime⇒runtime_id Some`, `Auto⇒runtime_id None`.
- `check_trigger_identity`: connector source swap ⇒ `TriggerSourceChanged`; new `trigger_id` ⇒ Ok (instruction edits allowed).
- Serde codes: `CRON_5`, `RFC5545_RRULE`, `{"kind":"SKIP"}`, `{"kind":"CATCH_UP_BOUNDED","max_occurrences":2}`; unknown fields rejected; `CoworkerRevision` is flattened (`value["name"]`, no `definition` key).
- `revision` (content) and `version` (status) are independent counters.

### crates/domain-task/src/execution.rs (990 lines)
- **Layer:** domain decision + coordinator. `MAX_LEASE_DURATION_MS=300_000`.
- `ExecutionDenial` (16): `InvalidCommand, UnsupportedGuarantee, UnconfirmedGuarantee, AuthorizationDenied, StaleVersion, StalePlanOrSpec, TaskNotRunnable, StepNotReady, DependencyNotComplete, LeaseConflict, StaleFence, RuntimeIdentityMismatch, PriorAttemptUnsettled, InvalidLeaseState, LeaseExpired, UnsafeRelease`. `ExecutionDecision<T>{Eligible(T), Denied(..)}`.
- `decide_attempt_admission` → proposed `epoch` only (never an allocation). Checks: id shape → workspace/task/step/plan agreement → task+step `version` → `task.status in (READY,RUNNING)` → plan revision == `current_plan_revision` and `task_spec_revision` match and step `logical_key` member of plan → `step.status=="READY"` → dependency count/identity/`COMPLETED`/no duplicates → conflicting `Active|Releasing` lease ⇒ `LeaseConflict` (wall-clock expiry does not substitute) → `latest_step_epoch == expected_previous_epoch` with `checked_add` → environment/runtime/incarnation match → `selected_agent_binding_id` + `dependency_plan_digest` → budget → `(task READY) == events.task_status.is_some()` → prior-attempt settlement + explicit recovery → 13 `ExecutionCheck`s.
- `ExecutionCheck{Confirmed,Denied,Unknown,Unsupported}` → `AuthorizationDenied`/`UnconfirmedGuarantee`/`UnsupportedGuarantee`. Unknown never authorizes.
- `decide_lease_renewal`: requires `Active`, `now < expires_at`, `now < renew_by`, non-terminal attempt, and strictly extending expiry.
- `decide_lease_release`: `Begin: Active→Releasing` (only if not expired); `Complete: Releasing→Released` requires `writer_quiescence`, `invocations_settled`, `effects_reconciled` all Confirmed (`UnsafeRelease`).
- `decide_lease_expiry`: recovery by current authority; needs all 9 ids, 4 version pins, 4 distinct events sharing correlation/hlc/recorded_at; expires when `now >= expires_at` **or** incarnation fenced (`runtime_revoked` or current incarnation ≠ lease incarnation).
- `valid_id` (≤256, non-blank, no control chars), `valid_digest` (`sha256:` + 64 lowercase hex), `valid_duration` (1..=300_000), `failover_class ∈ {SAFE_PORTABLE, REPLAYABLE, HANDOFF_REQUIRED, LOCAL_BOUND}`, ≤64 unique `capability_grant_ids`, ≤32 `expected_budget_reservation_ids`, delegated identity all-or-none.
- `StepAttemptCoordinator<S: StepAttemptStore>`: `preview_admission`, `preview_delegated_admission` (→`preview_child_attempt_admission`), `admit`, `renew`, `release`, `expire`. `StepAttemptStore` has default methods, so an unimplemented store returns `Unsupported` (test `stores_are_unsupported_unless_the_atomic_contract_is_implemented`).
- 8 tests; fixture builds a full `StepAttemptAdmissionSnapshot` (task/plan/step/checks/environment/budget).

### crates/domain-task/src/planning.rs (633 lines)
- **Layer:** domain. `MAX_PLANNING_PACKET_BYTES=128*1024`, `MAX_PLAN_OUTPUT_BYTES=256*1024`.
- `TaskPlanningPacket::from_assignment(&PlanningAssignment)` — whitelists only intent + planning rules; explicitly excludes binding config/auth, locators, account details, native private state. Emits `kind:"INITIAL_TASK_PLANNING_INPUT"`, `planning_rules{mode:"PROPOSE_ONLY", input_references_grant_access:false, execution_authority:false, content_is_untrusted:true}`. Canonical JSON via `serde_json_canonicalizer`, `digest = "sha256:{hex}"`. Accessors `task_id/task_version/task_spec_revision/lead_agent_binding_id/digest/text` (`text()` = runtime-private, never logged).
- `parse_initial_plan_output(&[u8]) -> Vec<ProposedStep>` — strict `object_fields` (no extra keys: rejects `task_id`, `agent_session_id`, `expected_task_version`, `reason_for_revision`, `transcript`, `step_id`); ≤100 steps; `bounded_text` limits (`logical_key` 128, `title` 512, `objective` 16 KiB); then `validate_plan_proposal` (cycles, missing deps, duplicate keys). Fenced Markdown rejected.
- `validate_plan_step_content` shared with the service so typed callers cannot bypass parser bounds: `required_capabilities` ≤32 with `{semantic_requirement, operation_ids, resource_scope?}`, `acceptance_criteria` ≤32 with `{criterion_id, description, required_evidence, mandatory, verifier_hint?, subject_refs?}`, `required_evidence ∈ {REPORTED, OBSERVED, VERIFIED}`.
- `validate_resource_scope` / `validate_constraint_json`: depth ≤8, ≤1024 nodes, ≤64 object keys, ≤32 array items, ≤2048-byte strings, ≤16 KiB serialized; unknown locator fields (`root`, `path`) and `null` scope refused.
- `initial_plan_output_schema()` — JSON Schema mirror of the same bounds; `plan_step_serialized_size()` adds +1 for commas.
- `invalid_output()` deliberately does not echo provider text (test asserts error Debug lacks the token).
- 7 tests incl. `typed_submission_cannot_bypass_parser_bounds`.

### crates/domain-task/src/task_packet_preparation.rs (541 lines)
- **Layer:** domain. `MAX_PACKET_BYTES=1 MiB`.
- `TaskPacketSourcePins` (BTreeSets): `authorized_inputs`, `authorized_artifacts`, `committed_decision_event_ids`, `child_scoped_grant_ids` (parent grants must not appear), `authorized_resource_refs`, `authorized_output_destinations` + identity pins.
- `PreparedTaskPacket{packet, canonical_json, canonical_digest}`; `AcceptedTaskPacketContext{task, task_spec, plan, step, delegation_profile_id, delegation_profile_revision}`.
- `TaskPacketPreparationError`: `InvalidContextBudget, IdentityMismatch, UnauthorizedInputReference, UnauthorizedArtifactReference, UnauthorizedDecisionReference, UnauthorizedGrantReference, UnauthorizedResourceReference, UnauthorizedOutputDestination, AcceptedStepMismatch, ObjectiveDoesNotMatchAcceptedStep, ContextBudgetExceeded{limit,actual}, InvalidEnvelope(DelegationEnvelopeError)`.
- `prepare_task_packet_for_accepted_step` — verifies task/spec/plan/step pins, step membership in plan by `(logical_key, objective, title)`, and **exact** `packet.objective == step.objective` (documented as conservative V1 narrowing because prose cannot be proven narrower without another model call).
- `prepare_task_packet` — identity pins, `validate_task_packet(packet, workspace_id)`, then allowset membership for every ref class, then canonicalize; over budget ⇒ `ContextBudgetExceeded` (never truncates/rewrites).
- 5 tests.

### crates/domain-task/src/worker_selection_decision.rs (330 lines)
- **Layer:** domain composition of the ranker + released B83 benefit gate.
- `StayReason{NoEligibleCandidate, BenefitGateDidNotPermit}`; `WorkerSelectionDecision{StayWithCurrentOption{reason}, DelegateToProfile{profile_id, ranking_policy_version, measured_improvement_basis_points, benefit_policy_version}, PinnedWorkerUnavailable{profile_id}}`.
- `WorkerSelectionInputs{profiles, request, benefit_policy, benchmark_report, benefit_arm}`; `decide_worker_selection()`.
- Ordering guarantee: ranker always runs first, so `Require(_)` either stays pinned or returns `PinnedWorkerUnavailable`; the automatic benefit gate never replaces an explicit owner pin. Pinned path reports `measured_improvement_basis_points: 0`.
- `WorkerSelectionDiagnostics{ranking_policy_version, ranking_rule, eligible_profile_ids, unknown_cost_profile_ids, matched_requested_profile, decision}`; `decision` string ∈ `STAY_WITH_CURRENT_OPTION | DELEGATE | PINNED_WORKER_UNAVAILABLE`; `to_diagnostic_json()` — projections only, no prompts/transcripts/provider handles (asserted).
- Panic path: `_ => unreachable!()` inside the `WorkerSelectionError` re-match.

### crates/local-environment-staging/src/lib.rs (1,613 lines)
- **Layer:** local adapter (below Resource authorization). Bounded materialization of already-authorized exact-pinned bytes into `inputs/` (read-only) + `outputs/` (writable).
- Re-exports submodules `isolation_digest` (`digest_of_canonical_json, digest_of_labels, digest_pinned_inputs, digest_root_set`) and `output_budget` (`EnforcementStatus, FileRootMode, OutputQuota, OutputScanLimits, OutputUsage, RootModeSupport, check_output_quota, measure_output_root, probe_root_mode_support`).
- `PreparationStatus::PreparedOnly` is the **only** status: no isolation or dispatch proof is claimed.
- `PinnedInput{workspace_id, resource_id, revision_id, content_digest, relative_path, bytes}`; `StagingLimits{max_files, max_file_bytes, max_total_input_bytes, max_path_bytes}`.
- `StagingError`: `InvalidParent, InvalidWorkspace, UnsupportedPlatform, InvalidPin, InvalidDigest, DigestMismatch, InvalidRelativePath(String), DuplicatePin, DuplicatePath, InputLimitExceeded, OutputRootSymlink(String), OutputRootUnsupportedFile(String), OutputScanLimitExceeded, OutputQuotaExceeded{limit_bytes,observed_bytes,limit_files,observed_files}, Io(String)`.
- `ExactResourceReader` port + blanket impl for `R: ResourceStore` via `read_resource_content_bounded(ws, res, Some(revision), max)`; `TaskInputPreparationError{StaleTaskRevision, InvalidTaskInput, DuplicatePin, ResourceUnavailable, ResourceMetadataMismatch, InputLimitExceeded, Staging(_)}`.
- `stage_task_spec_inputs(reader, task, expected_workspace_id, expected_task_id, expected_task_version, expected_spec_revision, staging_parent, limits)` — stale pins rejected **before** any store read (tested with a counting reader ⇒ 0 calls).
- `PreparedEnvironment{root, input_root, output_root, status, input_file_count, input_bytes, input_set_digest, readable_root_set_digest, writable_root_set_digest}` + `cleanup()` (removes only this root; does not fence processes), `output_usage()`, `assert_output_quota()`, `readable_root_names()=["inputs"]`, `writable_root_names()=["outputs"]`.
- Path validation: rejects absolute/backslash/`..`/`.`/empty component/control chars/`<>:"|?*`/trailing dot-or-space/Windows reserved (`CON,PRN,AUX,NUL,COM1-9,LPT1-9`); dedup key = NFC + Unicode casefold (catches `σ.txt` vs `ς.txt`); rejects file-that-is-parent-of-another-input.
- All input digests verified **before** any filesystem state is created (`DigestMismatch` leaves parent dir count 0); `RootCleanupGuard` (Drop) removes a partially built root.
- Platform split: Windows uses `windows_local_security::{open_directory_path_without_reparse, VerifiedDirectory, secure_current_user_directory}`; Unix uses `0o700` dirs + read-only files; other targets `create_dir_private` returns `Unsupported` ⇒ `StagingError::UnsupportedPlatform`.
- Cleanup restores only owner write: Unix test asserts final mode `0o644` and `mode & 0o022 == 0`.
- Explicit non-claims repeated in doc comments: not OS confinement, not a write quota, not an isolation attestation, not grounds for admitting an Agent/Attempt; Windows DACL "remains unqualified as an isolation provider".
- `FileRootMode::ReadWriteNoDelete` → `RootModeSupport::NotEnforced` (deletion is a parent-directory permission on POSIX and Windows); `ReadOnly`/`ReadWrite` → `Enforced`.
- ~30 tests incl. `prepared_environment_publishes_digests_without_host_paths`.

### crates/storage-core/src/delegation_result.rs (60 lines)
- **Layer:** storage port. `DelegationResultPublishCommit{workspace_id, runtime_id, runtime_incarnation_id, agent_session_id, child_attempt_id, envelope_json: Value, envelope_digest, event: EventDraft}`; `DelegationResultReadQuery{owner_principal_id, workspace_id, task_id, parent_attempt_id, child_attempt_id, task_spec_revision}`; `CommittedDelegationResult{... event: DomainEvent, replayed: bool}`.
- `trait DelegationResultStore { publish_delegation_result; get_delegation_result; }` — envelopes are untrusted reports; must fail closed without exact child-session provenance.

### crates/storage-core/src/resource_edge.rs (44 lines)
- `ResourceEdgeRecord{edge_id, workspace_id, from_resource_id, from_revision_id: Option, relation: String, to_resource_id, to_revision_id: Option, observed_at, provenance: Value}` — immutable same-Workspace structural/provenance relation.
- `DerivedResourceCreateCommit{request: WorkspaceCreateRequest, resource, revision, resource_event, edge, edge_event, content}` — one receipt, both events; target must be the newly created Resource.
- `CommittedDerivedResource{resource: CommittedResource, edge, edge_event}`.

### crates/storage-core/src/workspace_root_observations.rs (111 lines)
- `RootObservationScanStatus{Complete,Partial}`; `RootObservationFreshness{Current,Stale,Unknown}` with `as_str()` → `CURRENT|STALE|UNKNOWN`.
- `WorkspaceRootFileObservation` — `observation_id` (membership pseudonym), `file_identity_digest` (never the raw OS identity tuple), `resource_id: Option` (null until linked), `display_name`, `size_bytes`, `modified_at`, `observed_at`, `private_relative_locator: Vec<u8>` with **custom `Debug` printing `<private>`**.
- `WorkspaceRootObservationBatch{workspace_id, workspace_root_id, expected_root_version, runtime_id, runtime_incarnation_id, request_id, request_digest, status, observations, completed_at}`; `RootObservationCommitSummary{complete_generation, observations_upserted, observations_marked_stale}`; `RootObservationSearchRequest{... freshness: Option, after_display_name, after_observation_id, limit}`; `RootObservationSearchRecord`.
- `trait WorkspaceRootObservationStore` returns **at most `limit + 1`** so the caller can emit an opaque keyset cursor; never returns a private locator.

### crates/storage-sqlite/src/dependencies.rs (239 lines)
- **Layer:** storage (SQLite read store). `impl ResourceDependencyReadStore for SqliteWorkspaceStore` → `list_resource_dependents_page(ResourceDependencyPageRequest)`.
- Validation: non-empty owner/workspace/resource; `limit in 1..=201`; cursor pair both-or-neither; cursor text ≤64 / ids ≤256, no control chars. Dispatches `Command::ListResourceDependents{request, reply}` over an mpsc channel.
- `list_resource_dependents()` SQL touches `dependency_edges`, `workspaces` (owner check), `resources`, `resource_revisions`, `resource_revision_parents` (head computation: revisions with no child parent), `invalidation_records` (latest by `created_at DESC, invalidation_record_id DESC`), plus `artifacts`/`artifact_versions` and `verification_runs`/`tasks` verified through `json_each` over `input_refs_json` / `inputs_json`.
- Freshness CASE → `CURRENT` (sole head == edge source revision), `STALE`, `CONFLICTED` (`head_count > 1`), `UNKNOWN` (missing source/revision, unverifiable dependent, or head_count 0). Keyset order `created_at COLLATE BINARY, dependency_edge_id COLLATE BINARY`.
- Note: freshness/ownership logic lives in SQL, not Rust — untestable in isolation and duplicates state names from domain constants.

### crates/storage-sqlite/src/invocation_dispatch_migration_tests.rs (120 lines)
- Migration **v26** opens Invocation creation to the combined Trust/ApprovalUse/Invocation/Effect admission writer and closes the direct-dispatch path with the consumed-approval guard.
- `migrate_with_failpoint(&mut conn, Some(Failpoint::DuringV26Migration))` ⇒ `user_version` stays **25**, `capability_invocation_dispatch_requires_consumed_approval` absent, partial migration must not leave the guard installed.
- After `migrate()`: `verify_migration_record(conn, 26, V26_MIGRATION_NAME, SQLITE_V26_DDL, false)` and `(27, V27_MIGRATION_NAME, SQLITE_V27_DDL, true)`; `user_version == SCHEMA_VERSION`.
- Removed: `capability_invocation_creation_admission_closed`. Installed: `capability_invocation_dispatch_requires_consumed_approval`. Preserved: `capability_invocation_status_transition_guard` (adds `CREATED→DISPATCHED`; asserts `OLD.status='CREATED' AND NEW.status IN ('DISPATCHED','CANCELLED')` and `NEW.version <> OLD.version + 1`), `capability_invocation_admission_matches_scope`, `capability_invocation_first_dispatch_revalidates_owner`, `capability_invocation_identity_immutable`.

### crates/storage-sqlite/src/lead_handoff_boundary.rs (226 lines)
- **Layer:** storage (in-transaction boundary validator). `LeadHandoffCause{OwnerRequest, PolicyFailover}`.
- `validate_lead_handoff_boundary(conn, workspace_id, task_id, task_status, current_plan_revision, completed_at, current_lead_agent_binding_id, target_agent_binding_id, cause)`.
- Blocks both causes: `completed_at.is_some()`, no-op binding swap, live `TASK_PLANNING` session (`status IN ('STARTING','ACTIVE','INTERRUPTING','CLOSING')`), unresolved Effect (`effects.state NOT IN ('OBSERVED','VERIFIED','FAILED')`).
- `OwnerRequest`: `task_status == "READY"` **and** `current_plan_revision.is_none()` **and** no non-terminal Attempt.
- `PolicyFailover`: `task_status ∈ {READY,RUNNING}`; active Attempts are **preserved** under their original `agent_binding_id`/lease (documented against `TASK-RUNTIME.md`).
- 3 tests with an in-memory `agent_sessions/attempts/effects` fixture.

### crates/storage-sqlite/src/trust.rs (5,001 lines) — PARTIAL, first 120 lines only
- **Layer:** storage (Trust). The module doc scope is broader than the filename: durable approval records **plus** the local write-only `SecretStore` adapter.
- Doc invariant: "Approval resolution records an owner's one-time decision. It does not consume that decision, issue a CapabilityGrant or SecretLease, or admit an Effect."
- Imports reveal the implemented ports: `ApprovalStore`, `CapabilityGrantStore`, `SecretLeaseStore`, `SecretStore` (+ `ApprovalDecision/ApprovalPage/ApprovalCursor`, `CapabilityGrantScope/Status`, `IssueCapabilityGrantCommit`, `IssueSecretLeaseCommit`, `RevokeSecretLeaseCommit`, `SecretKind`, `SecretPlacement::LocalOnly`).
- Bounds: `MAX_APPROVAL_ID_BYTES=512`, `MAX_PRINCIPAL_ID_BYTES=512`, `MAX_REQUEST_ID_BYTES=200`, `MAX_APPROVAL_TEXT_BYTES=16_384`, `MAX_APPROVAL_TARGET_DEPTH=8`, `MAX_APPROVAL_TARGET_COLLECTION_ITEMS=64`, `MAX_SECRET_BYTES=48*1024`, `SECRET_KEYRING_SERVICE="com.litecow.secret-store"`.
- `trait SecretCredentialBackend{set_secret,remove_secret}`, `OsSecretCredentialBackend` over `keyring::Entry` (treats `NoEntry` as success on delete), `pub struct OsSecretStore` implementing **only** `SecretStore::write` — no read path exists by design.
- `write_secret_with()`: copies caller bytes into `Zeroizing`, zeroizes the caller buffer on *every* return path, `secret_ref_id = "secret_{hex(32 random)}"`, `account = "v1:{account_hash}"`, `provider_ref = "os-keyring:v1:{account_hash}"`, `placement = LocalOnly`, `URL_SAFE_NO_PAD` base64 payload, best-effort `remove_secret` rollback if the keychain write fails.
- Unread remainder (~4,882 lines) presumed to hold the approval CRUD SQL, grant issuance/revocation transactions and their tests — **not verified**.

## Cross-cutting observations
- **Layer discipline holds.** Every crate states its own non-authority: `browser_pointer` "never dispatches", `local-environment-staging` "PreparedOnly… not grounds for admitting an Attempt", `worker_selection_decision` "grants no authority", `planning` "no durable IDs", `delegation_result` "untrusted reports". This matches ARCHITECTURE.md's reported/observed/committed/verified ladder and the "installed ≠ connected ≠ … ≠ invoked" chain.
- **Duplicated primitives — the main fidelity risk.** There is no shared id/digest module: `valid_id`/`valid_digest` in `domain-task/execution.rs`, `validate_digest` in `local-environment-staging`, `sha256:{hex}` construction in `planning.rs`, `task_packet_preparation.rs`, `local_lexical.rs`, `trust.rs`, `isolation_digest`. Four independent lowercase-hex-64 checks can drift from `docs/schemas/`.
- **Two authorities for one plan contract.** `initial_plan_output_schema()` (JSON Schema) and `validate_plan_step_content()`/`object_fields()` (imperative) encode the same bounds twice inside one file; the schema is not generated from the validator, so a bound change in one silently desynchronises the other.
- **State names as string literals in SQL.** `lead_handoff_boundary.rs` and `dependencies.rs` hardcode `'STARTING'`, `'OBSERVED'`, `'COMPLETED'`, `'ARTIFACT_VERSION'`, `'VERIFICATION_RUN'`; no compile-time link to the Rust enums that elsewhere own those states.
- **Cost/safety tradeoff in RAG:** `local_lexical` does a per-candidate `get_resource_detail` authorization recheck (N+1) and fails the *entire* page on the first incompatible parser generation rather than skipping that segment. Fail-closed and defensible, but O(results) extra reads per query.
- **Idempotency patterns are consistent:** admission binds an authenticated receipt before mutable checks; delegation publish reports `replayed`; derived-resource creation commits resource+edge+2 events under one receipt; staging is all-or-nothing with digest pre-verification.
- **Panic risk:** `worker_selection_decision.rs` uses `unreachable!()` in a decision path; a future `WorkerSelectionError` variant turns a typed denial into a panic.
- **No `unwrap`/`expect` found in non-test code paths** of the files read (only in `#[cfg(test)]` blocks and tests).
- Nothing read contradicts ARCHITECTURE.md; the crates are consistently more conservative than the prose promises (e.g. exact objective equality instead of semantic narrowing).

## Coverage check
- **Fully read (14):** `domain-environment/src/browser_pointer.rs`, `domain-environment/tests/browser_control.rs`, `domain-rag/{limits,provider,local_lexical}.rs`, `domain-responsibility/src/tests.rs`, `domain-task/{execution,planning,task_packet_preparation,worker_selection_decision}.rs`, `local-environment-staging/src/lib.rs`, `storage-core/{delegation_result,resource_edge,workspace_root_observations}.rs`.
- **Fully read (4):** `storage-sqlite/{dependencies,lead_handoff_boundary,invocation_dispatch_migration_tests}.rs`.
- **Partial (1):** `storage-sqlite/src/trust.rs` — first 120 / 5,001 lines (2.4%). Scope and SecretStore contract verified; approval/grant SQL and tests unverified. A 1,250-line read was cancelled at the run deadline.
- **Not read (8), deadline:** `storage-sqlite/src/artifact_tests.rs` (1,394), `backup.rs` (2,430), `environment_tests.rs` (2,111), `needs_you_tests.rs` (708), `runtime_binding_tests.rs` (1,240), `runtime_offer_tests.rs` (244), `user_request_details.rs` (1,039), `workspace_root_cleanup.rs` (378) — together ~9,544 lines / ~35% of the bucket. `backup.rs` and `trust.rs` are the two load-bearing omissions and should be re-queued first.
- Also unaudited: `local-environment-staging`'s `isolation_digest.rs` and `output_budget.rs` submodules (outside this bucket).
