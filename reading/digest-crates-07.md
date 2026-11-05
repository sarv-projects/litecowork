# Digest crates-07

Files: 29 read / 29 listed | Lines read: 22,184 | Bytes: 818 KB

Scope: domain crates (environment, invocations, rag, responsibility, task), platform crates
(linux-process-scope, local-environment-staging), and `storage-sqlite` adapters. Every file was
read end-to-end (large files paged). Test counts are from source inspection: **130 `#[test]`
functions across 16 files**, of which 4 are `#[ignore]` platform qualifications.

## Per-file notes

### crates/domain-environment/src/lib.rs (1725)
Pure Environment identity validation + lifecycle decisions. Layer: domain. No provider calls,
no persistence, no leases.
- Exports: `EnvironmentLifecycle` trait (`request_provision`, `apply_provision_result`,
  `request_suspend`, `apply_suspend_result`, `request_resume`, `request_destroy`,
  `apply_destroy_result`); `LifecycleDecision::{Applied, ReconciliationRequired}`;
  `ProviderProvisionResult::{Ready(ProviderReadinessProof), Failed, Ambiguous}`;
  `ProviderSuspendResult`, `ProviderDestroyResult::{ConfirmedAbsent, Failed, Ambiguous}`;
  `EnvironmentError` (24 variants incl. `StaleState`, `LifecycleHeld`, `RetentionDisallowsDestroy`,
  `RequiredOutputsUncommitted`, `ImmutableConfigurationChanged`, `VersionExhausted`);
  `new_environment`, `restore_environment`, `validate_configuration`,
  `validate_transition_proposal`, `change_sharing_scope`.
- Submodules `browser_control` (`BrowserControlActor/Error/Lease`, `BrowserObservation`) and
  `browser_pointer` (`transform_browser_pointer`, `PaneBounds`, `BrowserViewport`) are declared
  but **not in this bucket** (separate files).
- Status graph: `New→Provisioning→{Ready,Failed}`, `Ready→{Busy,Checkpointing,Destroying}`,
  `Busy→{Ready,Checkpointing}`, `Checkpointing→{Ready,Suspended,Failed}`,
  `Suspended→{Provisioning,Destroying}`, `Failed→Destroying`, `Destroying→{Destroyed,Failed}`.
- Fail-closed semantics that matter: a `Ready` proof that mismatches environment/runtime/
  incarnation/provider/source/isolation/health/budget returns `ReconciliationRequired`, **not**
  `Failed` and not `Ready`; `Ambiguous` never settles; `UserShared` scope is rejected outright
  (`UserSharedUnavailable`); `deny_private_networks=false` rejected; `WorkspacePersistent`
  requires a `sha256:` `provision_preview_digest`.
- Tests (24): `configuration_enforces_scope_owner_lifetime_and_workspace`,
  `coworker_and_user_scope_rules_are_explicit`,
  `sharing_scope_change_requires_suspended_persistent_environment_without_holds`,
  `sharing_scope_change_rejects_active_status_invalid_target_and_each_hold`,
  `cost_budget_requires_valid_currency_and_enforcement`,
  `source_digest_and_private_network_denial_are_validated`,
  `restore_revalidates_configuration_and_positive_version`,
  `environment_creation_request_rejects_persistent_without_preview_transaction`,
  `canonical_record_retains_all_added_v11_environment_metadata`,
  `environment_list_request_requires_explicit_positive_limit`,
  `ready_lifecycle_request_requires_matching_runtime_local_binding`,
  `provision_success_requires_exact_provider_source_isolation_and_budget_proof`,
  `definite_provision_failure_is_terminal_for_readiness`,
  `ambiguous_provision_does_not_claim_ready_or_failed`,
  `stale_status_and_version_cas_rejects_commands`,
  `storage_transition_validator_rejects_config_rewrite_and_illegal_edges`,
  `every_active_or_unsettled_hold_denies_suspend_and_destroy`,
  `resume_and_all_provider_result_paths_obey_state_machine`,
  `suspend_has_checkpointing_intermediate_and_ambiguous_result_stays_there`,
  `resume_revalidates_as_provisioning_and_carries_no_execution_authority`,
  `destroy_denies_retention_and_uncommitted_output_holds`,
  `ambiguous_destroy_keeps_destroying_and_requires_reconciliation`,
  `only_positive_absence_proof_marks_destroyed_and_terminal`,
  `definite_cleanup_failure_is_visible_and_not_destroyed`.

### crates/domain-invocations/src/lib.rs (498)
Invocation state machine + capability route classifier (`classify_agent_capability_route`,
`AgentCapabilityBridgeInput`, `CapabilityBridgeDecision` from `mod capability_bridge`, submodule
not in bucket).
- `InvocationStatus` = CREATED, DISPATCHED, WAITING, INPUT_REQUIRED, CANCEL_REQUESTED, SUCCEEDED,
  FAILED, CANCELLED, AMBIGUOUS. `CapabilityInvocationRecord` carries action-batch tuple
  (`action_batch_id/ordinal/operation_count/digest`), provider task TTL/poll fields.
- **Hard gate:** `transition(..)` returns `DispatchAdmissionUnavailable` for *any* target
  `Dispatched`; there is no path to DISPATCHED in this crate. `Ambiguous` requires the separate
  `reconcile_ambiguous_terminal` transform (`ReconciliationProofRequired` otherwise).
- Scope invariants: `CONVERSATION` (conversation_id, no task/attempt/effect), `TASK_PLANNING`
  (task only), `ATTEMPT_EXECUTION` (task+attempt; effect_id allowed).
- Tests (8): `permits_pre_dispatch_local_cancellation`,
  `rejects_any_dispatch_transition_without_combined_admission`,
  `rejects_partial_or_oversized_action_batch_metadata`,
  `validates_batch_member_position_digest_and_idempotency_identity`,
  `rejects_malformed_idempotency_key_even_without_action_batch`,
  `terminal_invocation_cannot_be_rewritten`,
  `ambiguous_invocations_cannot_become_terminal_through_generic_transition`,
  `reconciliation_state_transform_sets_provider_terminal_projection`.

### crates/domain-rag/src/grounding.rs (417)
Retrieval grounding. `TextSpan`, `SegmentLocator::{TextSpan,Page}`, `RetrievalCitation`,
`RetrievalWarning::{EmbeddedInstructionDetected, ExcerptTruncated, IncompatibleIndexGeneration}`,
`InsufficientEvidenceReason`, `GroundedStatus`, `GroundedRetrieval`, `GroundedClaim`,
`validate_claims`, `detect_embedded_instructions`.
- Reason codes: `GROUNDING_UNKNOWN_CITATION`, `GROUNDING_MISSING_CITATION`,
  `GROUNDING_POISONED_SUPPORT`. Marker list is explicit (13 strings, e.g. `ignore previous
  instructions`, `<|im_start|>`, `### instruction`).
- Tests (8): `a_claim_with_a_real_citation_is_grounded`, `a_claim_without_a_citation_is_rejected`,
  `a_claim_naming_an_unknown_citation_is_rejected`,
  `a_claim_supported_only_by_poisoned_content_is_rejected`,
  `a_claim_with_one_clean_and_one_poisoned_citation_is_grounded`,
  `embedded_instruction_text_is_detected`,
  `a_citation_with_an_invalid_digest_or_span_is_rejected`,
  `grounded_status_and_reason_serialize_as_upper_case`.

### crates/domain-responsibility/Cargo.toml (20) / README.md (141)
Deps: serde, serde_json, serde_json_canonicalizer, sha2, hex, unicode-casefold,
unicode-normalization, chrono-tz, chrono. No storage dependency (port-only).
README is unusually honest: Automation definition persistence exists; TriggerCoordinator,
recurring hosting, occurrence settlement and truthful Coworker presence are **absent**; resume
returns `ReconciliationRequired` while retained Automations/occurrences need reconciliation. Claims
"all 21 domain-responsibility tests passing" (2026-10-09) — see divergence note.

### crates/domain-responsibility/src/delegation_profiles.rs (981)
Typed `DelegationProfile` commands (`Create`, `Revise`, `Duplicate`, `SetStatus`);
`DelegationProfileStore::transaction(scope, fingerprint, F)` two-phase decision pattern.
- `normalize_profile_name`: NFC then case-fold then NFC → `name_key`; uniqueness per binding.
- `build_revision` bounds: routing_description ≤240 chars, instructions ≤12000, max_concurrency
  1..=8, max_host_delegation_depth ≤2, session_options ≤65 536 bytes / 64 keys,
  feature allowlist is a fixed 21-item list (`session.resume`, `extension.mcp_http`,
  `reporting.native_subagents`, `environment.extra_directories`, …).
- Two deliberate hard refusals: `EnablementUnavailable` for `SetStatus{Enabled}` (create is
  DISABLED; profiles can never be enabled here) and `SessionOptionsUnsupported` for any non-empty
  `session_options`.
- **No `#[cfg(test)]` module.** All validation is covered only indirectly.

### crates/domain-responsibility/src/identity.rs (143)
`OccurrenceIdentity` (Schedule, OneShot, Manual, ConnectorEvent, Webhook) keyed by
`LiteCowork/AutomationOccurrence/v2\0` + u32-be length framing; **no revision field**, so a
definition edit cannot replay an accepted delivery. `TestRunIdentity` uses a separate private
prefix `LiteCowork/AutomationTestCommand/v1\0`. `definition_digest` = canonical JSON sha256.
No tests.

### crates/domain-responsibility/src/schedule.rs (1245)
Pure recurrence + misfire planner (`ScheduleTrigger::parse`, `slots_between`, `next_slot_after`,
`plan_settlement`; `Cron5`, `Rrule`, `Freq`, `MisfireReason`, `OccurrenceIntent`,
`ScheduleSettlement`, `SettlementWindow`, `format_instant`, `parse_instant`).
- Bounds: `MAX_SCANNED_PERIODS = 20_000`, `MAX_HORIZON_DAYS = 366*5`, cron field ≤128 values,
  rrule INTERVAL ≤1000, per-day times ≤256. Unsupported cron symbols `? L W #` rejected; Vixie
  DOM∪DOW union when both restricted; Sunday normalizes 7→0. RRULE subset: FREQ
  MINUTELY..YEARLY, INTERVAL, COUNT xor UNTIL (UNTIL must be UTC), BYMONTH/BYMONTHDAY/BYDAY/
  BYHOUR/BYMINUTE/BYSECOND, WKST=MO only; BYDAY ordinal (`2MO`) rejected.
- Misfire: `SKIP` emits one `Skip{MissedSlot}` per missed slot; `RUN_ONCE_WHEN_AVAILABLE`
  coalesces to one run with `covered_from/covered_to/covered_count`; `CATCH_UP_BOUNDED{n}` runs n
  then one `Skip{ExcessCatchUp}`. Backwards clock returns unchanged cursor.
- **Load-bearing `expect`:** `local_from_instant` does
  `chrono::DateTime::from_timestamp_millis(instant).expect("schedule instants are bounded")` — an
  out-of-range persisted cursor/`now` panics instead of erroring.

### crates/domain-responsibility/src/schedule_tests.rs (568)
24 tests: `cron_5_rejects_unsupported_shapes`, `cron_5_named_month_and_day_are_normalized`,
`cron_5_sunday_is_accepted_as_zero_and_seven`,
`cron_5_daily_slot_is_resolved_in_the_declared_timezone`,
`cron_5_slot_uses_the_declared_timezone_offset`,
`rrule_rejects_unsupported_parts_and_ambiguous_bounds`,
`rrule_daily_every_second_day_produces_the_expected_grid`,
`rrule_count_bounds_the_rule_from_its_anchor`, `rrule_until_bounds_the_rule`,
`rrule_weekly_byday_selects_the_requested_weekdays`,
`rrule_monthly_bymonthday_clamps_to_the_month_length`,
`dst_gap_slot_is_skipped_and_next_valid_shifts_forward`,
`dst_fold_slot_honors_the_ambiguity_policy`, `on_time_slot_produces_exactly_one_run_intent`,
`skip_misfire_records_every_missed_slot_and_creates_no_run`,
`run_once_misfire_coalesces_the_missed_interval_into_one_logical_occurrence`,
`catch_up_bounded_misfire_creates_a_bounded_run_set_and_one_summary`,
`a_backwards_clock_never_fires_or_rewinds_the_persisted_deadline`,
`a_first_observation_creates_no_occurrence_and_finds_the_next_slot`,
`settlement_is_deterministic_for_the_same_cursor_and_window`,
`an_exhausted_rule_reports_no_next_slot`, `schedule_parsing_rejects_an_unknown_timezone`,
`rrule_without_a_start_anchor_is_rejected`,
`instants_round_trip_through_the_storage_timestamp_format`.

### crates/domain-responsibility/src/suggestions.rs (376)
`Suggestion`, `SuggestionPage`, `SuggestionPreference`, `SuggestionKind/Action/Status`,
`SuggestionService{settle_expired, owner_action, kind_preferences, set_kind_preference}` over
ports `SuggestionExpiryStore`, `SuggestionOwnerActionStore`, `SuggestionPreferenceStore`,
`SuggestionClock`. `SUGGESTION_EXPIRY_BATCH_LIMIT = 100`;
`SUGGESTION_SERVICE_PRINCIPAL_ID = "service:suggestion-service"`. `owner_action` first drains the
due set and returns `ExpiryPending` if `more_due` — an actionable page is never served stale.
Errors: `InvalidRequest, ClockUnavailable, Unauthorized, NotFound, VersionConflict,
IdempotencyConflict, InvalidTransition, Expired, ExpiryPending, Storage`.
1 test: `suggestion_owner_actions_accept_the_shared_200_byte_limit`.

### crates/domain-task/Cargo.toml (21) / src/delegation_channel.rs (317)
`DelegationChannelParentScope` (owner, workspace, task, parent_attempt, task_spec_revision>0) and
`DelegationChannelResultReader::result/recovery_decision`; `decide_child_recovery` blocks with
`EscalationBlocker::TaskSpecChanged` if result revision ≠ request revision. Decision is advisory
(`EscalationDecision::RecommendNewAttempt`), never admits an Attempt.
6 tests: `result_reads_only_the_child_under_its_bound_parent_context`,
`malformed_child_ids_are_rejected_before_storage_access`,
`malformed_parent_context_is_rejected_at_adapter_construction`,
`recovery_decision_uses_the_durable_result_and_recommends_the_next_attempt`,
`stale_child_result_revision_never_escalates`,
`missing_durable_result_is_not_found_and_channel_recovery_stays_bounded`.

### crates/domain-task/src/lead_failover.rs (782)
`FailoverPolicyMode::{Disabled,Ask,AllowListed}`, `FailoverTrigger::{AgentUnavailable,
QuotaExhausted,RuntimeUnavailable}`, `decide_lead_failover(...) -> FailoverDecision::{Disabled,
AskOwner,Recommend{expected_task_version,task_spec_revision,next_change_number},Blocked(..)}`,
`validate_failover_policy_value`, `resolve_task_failover_policy` (explicit > Coworker default >
DISABLED). `MAX_FALLBACKS=3`, `MAX_LEAD_CHANGES=3`; ALLOW_LISTED requires
`max_lead_changes <= len(fallbacks)`; observation must be `Confirmed`, unexpired,
`affected_agent_binding_id == current lead`, and for RUNTIME_UNAVAILABLE must carry the matching
`runtime_incarnation_id`. Blockers: `InvalidPolicy, InvalidContext, TriggerNotConfirmed,
TriggerNotAllowed, ObservationStale, ObservationDoesNotMatchCurrentLead, ChangeLimitReached,
NoEligibleFallback, UnsafeBoundary`.
13 tests: `task_spec_failover_policy_json_is_strict_and_structurally_validated`,
`task_policy_resolution_pins_explicit_over_coworker_and_defaults_disabled`,
`disabled_policy_never_changes_the_lead`,
`ask_policy_requests_owner_decision_without_recommending_a_fallback`,
`ask_policy_presents_the_ordered_eligible_fallback_choices`,
`allow_listed_uses_policy_order_and_returns_version_fenced_recommendation`,
`unconfirmed_or_unconfigured_trigger_never_fails_over`,
`stale_wrong_lead_or_expired_observations_are_rejected`,
`exhausted_change_budget_unsafe_boundary_and_stale_candidate_block`,
`a_failed_first_fallback_never_promotes_a_later_one`,
`the_current_lead_is_never_offered_as_its_own_fallback`,
`invalid_policy_and_duplicate_fallbacks_fail_closed`,
`runtime_failover_requires_incarnation_and_candidate_observations_are_unambiguous`.

### crates/linux-process-scope/Cargo.toml (12) / tests/systemd_scope.rs (199)
`#![cfg(target_os = "linux")]`; exercises `spawn(&SystemdCommands{systemd_run, systemctl,
scope_gate: CARGO_BIN_EXE_litecowork-scope-gate, env}, LaunchSpec{attempt_id:[u8;16], limits:
ResourceLimits})`. Positive-quiescence slice name `litecowork-attempt-<32 hex>.slice`; asserts
slice `ActiveState=inactive`. All 3 tests `#[ignore = "requires a live Linux systemd user manager
and delegated cgroup v2"]`: `systemd_scope_checks_limits_before_exec_and_preserves_native_stdio`,
`systemd_scope_kills_descendants_before_confirming_quiescence`,
`parent_exit_does_not_prove_quiescence_while_descendant_continues`. Docstring scores honestly:
qualifies the process/cgroup primitive, **not** filesystem or network isolation.

### crates/local-environment-staging/src/isolation_digest.rs (111)
Deterministic digests binding an admission request to observed evidence: `digest_of_labels`
(sorted, deduped, `\n`-framed), `digest_root_set`, `digest_pinned_inputs` (unit-separator
`\u{1f}` over workspace/resource/revision/content_digest/relative_path), `digest_of_canonical_json`
(type-tagged `n/s/[ ]/{ }`, sorted keys). Never emits paths or secrets. No tests.

### crates/storage-core/src/agent_registry_cache.rs (163)
`AgentRegistryCacheStore` port, `AgentRegistryCacheSnapshot`, bounds
`AGENT_REGISTRY_CACHE_MAX_ENTRIES=512`, `..._MAX_SNAPSHOT_BYTES=2 MiB`, per-entry
`registry_agent_id` ≤128 ASCII unique, `metadata` must be JSON object. `BoundedCounter` implements
`Write` to abort encoding at the byte cap (not post-hoc length check). Expired snapshots stay
readable (rendered stale). 2 tests:
`registry_cache_requires_unique_bounded_ids_and_object_metadata`,
`registry_cache_has_a_hard_byte_and_entry_bound`.

### crates/storage-sqlite/src/agent_binding_tests.rs (3278)
Integration-style tests driving the real `SqliteWorkspaceStore` (temp dir, `FileBlobStore`,
`TestKeys`). Fixtures build Workspace → Runtime incarnation (RECOVERING→DEGRADED) →
RuntimeWorkspaceBinding (role `EXECUTOR`) → AgentProfile/Endpoint → endpoint binding → RuntimeOffer
→ binding (disabled) → enabled. `PausingAgentBindingBlobStore` gates `put` to force the
blob-published/SQL-not-yet race.
16 tests: `enable_transition_records_an_append_only_owner_audit`,
`disabling_current_default_returns_stable_conflict_without_writes`,
`disable_receipt_replays_exactly_rejects_payload_reuse_and_checks_stale_version`,
`failed_event_insert_rolls_back_the_binding_change_audit_and_receipt`,
`duplicate_agent_binding_id_rejection_does_not_write_aggregate_state`,
`agent_binding_create_rejects_auth_refs_without_writing_any_state`,
`invalid_agent_binding_event_schema_does_not_write_aggregate_state`,
`admitted_planner_can_commit_after_binding_is_disabled_and_lead_eligibility_changes`,
`durable_starting_planner_can_activate_after_its_binding_is_disabled`,
`nonusable_compatible_offer_cannot_create_an_agent_binding`,
`nonusable_compatible_offer_cannot_enable_an_agent_binding`,
`enable_rechecks_offer_after_blob_publication_before_sql_commit`,
`conversation_starting_session_recovery_uses_exclusive_keyset_pages`,
`conversation_activation_commits_session_turn_and_local_host_binding_atomically`,
`conversation_agent_response_commit_settles_turn_and_session_atomically`,
`conversation_host_resolution_uses_turn_pinned_binding_after_selection_changes`.

### crates/storage-sqlite/src/effect_evidence.rs (2050)
`SqliteEffectEvidenceStore` implements `EffectEvidenceStore`: `propose_effect`,
`transition_effect`, `append_evidence`, `get_effect`, `list_effects`, `list_evidence`.
- `transition_effect` to `Started` returns `DispatchAdmissionUnavailable
  { blockers: DispatchAdmissionBlocker::current() }` **before** blob write, queueing, DB access,
  event or provider boundary.
- Replay is resolved in a probe transaction *before* applying the transition, so a retry after a
  successful transition does not fail as invalid-state first.
- `validate_live_fence` is one SQL predicate joining tasks/steps/attempts/execution_leases
  (epoch + fencing_token_digest + ACTIVE + unexpired)/capability_invocations (CREATED, effect_id
  NULL)/capability_grants (ACTIVE, matching `capability_ref_json`, operation in allowed set).
- Evidence admission: only `REPORTED`; producer must equal
  `{"principal_id":"runtime:<id>:<inc>","kind":"RUNTIME"}`; cannot attach `verification_run_id`.
  `OBSERVED` needs same-Task OBSERVED/VERIFIED evidence with `subject_ref = effect:<id>`;
  `VERIFIED` needs a PASSED `verification_runs` row whose `verifier_kind` equals evidence
  `producer.service_id`, run `task_spec_revision = tasks.current_spec_revision`, evidence id in
  `evidence_refs_json`, and all pinned subject/input revisions still head-of-lineage.
- Event types: `effect.proposed|started|retry.authorized|failed|acknowledged|observed|verified|
  ambiguous|reconciliation.started.v1`, `evidence.created.v1`. Tables: `effects`, `evidence`,
  `verification_runs`, `resource_revisions`, `resource_revision_parents`, `request_dedup`,
  `domain_events`, `workspace_origin_sequences`.
- Tests (14): `proposed_event_payload_matches_zero_dispatch_ordinal_schema`,
  `invocation_match_rejects_missing_effect_capability_identity`,
  `live_fence_rejects_attempt_grant_for_another_capability_identity`,
  `observed_transition_requires_evidence_reference`,
  `failure_and_ambiguity_events_require_safe_provenance`,
  `runtime_cannot_submit_observed_or_verified_evidence_or_claim_another_producer`,
  `replay_response_is_marked_as_replayed`, `evidence_row_preserves_verification_run_binding`,
  `stale_task_spec_verification_cannot_verify_effect`,
  `changed_input_head_invalidates_verification_for_effect`,
  `current_task_spec_and_input_head_allow_exact_verification_evidence`,
  `evidence_bound_to_another_verification_run_cannot_verify_effect`,
  `stale_verification_subject_cannot_verify_effect`,
  `conflicted_resource_heads_invalidate_verification_for_effect`.

### crates/storage-sqlite/src/environments.rs (2655)
`SqliteEnvironmentStore` (`EnvironmentStore` + `EnvironmentControlLeaseStore`). Tables:
`environments` (28 columns), `environment_provider_bindings` (opaque locator only, excluded from
snapshots/events/receipts), `environment_control_leases`, `environment_checkpoints`,
`request_dedup`, `domain_events`.
- Auth before replay: `validate_workspace_mutation_scope` (owner principal + `ACTIVE`) runs before
  `read_receipt`; `validate_create_owner_scope` then `validate_create_scope` (runtime must be
  current incarnation, `ONLINE/READY` or `DEGRADED/DEGRADED`, workspace binding with `EXECUTOR`).
- Error codes: `ENVIRONMENT_REQUEST_DIGEST_MISMATCH`, `ENVIRONMENT_EVENT_PAYLOAD_INVALID`,
  `ENVIRONMENT_IMMUTABLE_IDENTITY_CHANGED`, `ENVIRONMENT_PROVIDER_BINDING_REQUIRED`,
  `ENVIRONMENT_PROVIDER_BINDING_STALE_INCARNATION`, `ENVIRONMENT_LIFECYCLE_HELD`,
  `ENVIRONMENT_DESTROY_ADMISSION_UNAVAILABLE`, `ENVIRONMENT_RUNTIME_NOT_READY`,
  `CONTROL_LEASE_ATTEMPT_NOT_CURRENT`, `CONTROL_LEASE_ACTIONS_NOT_DRAINED`,
  `CONTROL_LEASE_TAKEOVER_UNAVAILABLE`, `CONTROL_LEASE_FENCING_CREDENTIAL_INVALID`.
- `validate_transition_holds`: entering `Checkpointing` counts active attempts, unsettled
  invocations, ACTIVE/RELEASING control leases, unresolved effects; entering `Destroying` is
  **always refused** (no durable retention/output proof available in the store).
- Control lease: epoch = `MAX(epoch)+1`; takeover requires version+epoch CAS plus
  `ensure_no_unsettled_control_invocations`; end requires state ACTIVE. Fencing token stored only
  as `sha256:` digest.
- Tests (2): `accepts_only_running_attempt_and_task_with_current_live_provider_binding`,
  `acquire_takeover_release_commit_events_and_exact_receipts_atomically` (asserts 3 events +
  3 receipts, stale takeover → `Conflict{expected:1,actual:2}`, raw fence credential absent from
  both events and receipts).

### crates/storage-sqlite/src/invocations.rs (1001)
`SqliteCapabilityInvocationStore`: `transition_invocation`, `reconcile_terminal_invocation`,
`get_invocation_for_owner`, `list_task_invocations_for_owner`.
- Hard gate: `next_status == Dispatched` → `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`.
- `verify_invocation_runtime` binds the caller runtime/incarnation to `capability_activations`;
  `verify_provider_identity` additionally joins `capability_activation_host_bindings` →
  `capability_host_instances.capability_identity_digest` and requires
  `AMBIGUOUS` + matching `request_digest` → `INVOCATION_PROVIDER_OBSERVATION_MISMATCH`.
- Fingerprint covers workspace/invocation/expected_version/status-or-observation/event; mismatch →
  `StoreError::IdempotencyConflict`.
- Keyset pagination preserves the exact `created_at` string (`COLLATE BINARY`) to avoid skipping
  rows at `Z`/fractional-second edges. **No `#[cfg(test)]` module in this file.**

### crates/storage-sqlite/src/needs_you.rs (645)
`NeedsYouReadStore` projection over `approvals`, `user_requests`, and `tasks` with
`status IN ('BLOCKED','NEEDS_USER')`. Source-specific safe copy only (never raw provider text);
`blocking_conditions_json` validated against a `deny_unknown_fields` `StoredBlocker` and against
`docs/schemas/error-codes.schema.json` via `include_str!` + `OnceLock<HashSet<String>>`. Item id =
`sha256:` over `litecowork.needs-you.item.v1\0` + kind + length-framed identities.
`MAX_NEEDS_YOU_PAGE_SIZE=200`, summary/hint ≤4096 bytes, code → `StoreError::Integrity` if
unknown. Cursor must match workspace+filter+limit. **No tests in this file.**

### crates/storage-sqlite/src/os_principal_binding.rs (587)
`OsRuntimePrincipalBindingProvider::load_or_create(data_dir, runtime_id, allow_create)`;
`RuntimeOsPlatform::{Linux,MacOs,Windows}`; `RuntimeOsPrincipal::{UnixUid,WindowsLogonSid}`.
- Unix: rejects symlink/non-dir/wrong-uid/mode≠0700 directories, both pre- and post-canonicalize.
- `allow_create=false` + missing credential → `Integrity("OS principal binding is missing;
  explicit local recovery is required")`; changed principal → `Integrity("OS principal changed
  for this Runtime installation")`.
- Keyring service `com.litecowork.runtime-os-principal-binding`; fingerprint = local RFC 2104
  HMAC-SHA-256 (`litecowork-runtime-os-principal-v1\0`), `hmac-sha256:<hex>`.
- `RuntimeOsPrincipalIdentity::test_support` exists only under
  `#[cfg(all(feature = "test-support", debug_assertions, ...))]` with fixed fake fingerprint
  `test-only:fake-runtime-principal-binding-v1` and a hard-coded test pseudonym key.
- 2 platform-gated tests: `test_support_identity_uses_fake_binding_and_stable_pseudonym` (unix),
  `test_support_identity_uses_a_non_current_synthetic_logon_sid` (windows).

### crates/storage-sqlite/src/resource_edge_migration_tests.rs (48)
1 test: `resource_edge_migration_is_atomic_and_adds_immutable_guards` — injects
`Failpoint::DuringV25Migration`, asserts `user_version` rolls back to 24 and trigger
`resource_edges_immutable_update` is absent; then asserts receipts for V26 (`SQLITE_V26_DDL`,
non-latest) and V27 (`SQLITE_V27_DDL`, latest), `user_version == SCHEMA_VERSION`, and both
`resource_edges_immutable_update` / `resource_edges_immutable_delete` exist.

### crates/storage-sqlite/src/resource_edges.rs (363)
`create_derived_resource` (DERIVED_FROM only) and `insert`. Relations:
`CONTAINS | DERIVED_FROM | REFERENCES | SAME_PROVIDER_OBJECT`; rejects self-edge; event
`resource.edge.created.v1` with exact 5-key payload. Request payload gets a
`derived_resource_edge` block with `commit_fingerprint`; receipt replay path re-reads the edge
event. Source resource `context_document_json.status` must be ACTIVE. No tests in this file.

### crates/storage-sqlite/src/runtime_identity.rs (152)
`OsRuntimeDeviceIdentityProvider::load_or_create` → `DeviceIdentityRecord{device_id:
"ed25519-sha256:<sha256(pubkey)>", public_key:"ed25519:<hex>", key_version, issued_at}`. Keyring
service `com.litecowork.runtime-device-signing-key`; account = sha256 of canonical data dir;
`StoredSigningIdentity` zeroizes its seed on drop, `SigningKey` scoped to pubkey derivation only.
Mismatched `runtime_id`/`key_version≠1` → `Integrity`. `MUTATION_LOCK` serializes create. No tests.

### crates/storage-sqlite/src/skill.rs (1943)
`SqliteSkillStore` over the shared `aggregate_snapshots` projection for four entity types:
`skill`, `skill_proposal`, `demonstration_session`, `mcp_app_frame`. Methods: `put_skill`,
`get_skill`, `list_skills`, `list_skills_matching`, `create_skill_proposal`,
`get_skill_proposal`, `list_skill_proposals`, `review_skill_proposal`,
`create_demonstration_session`, `get_demonstration_session`, `list_demonstration_sessions`,
`transition_demonstration_session`, `append_demonstration_action`,
`complete_demonstration_session`, `put_app_frame`, `get_app_frame`.
- One capability identity → one Skill per Workspace (duplicate → `Conflict{None,None}`).
- `review_skill_proposal` delegates authority to `evaluate_skill_proposal_review` and records
  `publication_blocker`/`activated_skill_id`; `write_audit` always writes
  `decision='ALLOW'`, `reason_code='OWNER_AUTHORIZED'`.
- Capture: `evaluate_capture_append` enforces consent/Environment/status/caps; sensitive region
  under `PAUSE_ON_DETECTION` durably pauses in the same transaction
  (`SENSITIVE_REGION_DETECTED`); `elapsed_millis` returns `u64::MAX` on unparsable instants
  (fail closed).
- Events: `skill.revised.v1`, `skill.proposal.created.v1`, `skill.proposal.reviewed.v1`,
  `demonstration.started|action.captured|transitioned|converted.v1`, `mcp_app.frame.issued.v1`.
  Audit actions: `SKILL_REVISION_COMMITTED`, `SKILL_PROPOSAL_CREATED`,
  `SKILL_PROPOSAL_REVIEWED`, `DEMONSTRATION_STARTED|TRANSITIONED|CONVERTED`,
  `DEMONSTRATION_PAUSED_ON_SENSITIVE_REGION`, `MCP_APP_FRAME_ISSUED`.
- Read paths (`get_skill`, `get_*`, `list_*`) open a `Deferred` transaction and return without
  `commit()`; mutation paths are `Immediate`. Filtered listing uses a bounded scan budget
  `limit*8` (min 64). Receipt encoding uses plain `serde_json::to_string` (not canonical).
- **No `#[cfg(test)]` module** — the largest untested adapter surface in this bucket.

### crates/storage-sqlite/src/user_request_resolutions.rs (1507)
`SqliteUserRequestResolutionStore`: `respond_to_user_request`, `dismiss_user_request`.
- Owner auth first (`workspaces.owner_principal_id` + `ACTIVE`), then dedup by
  `principal_id + idempotency_key` stored in `request_dedup.request_id`.
- `check_pending_and_version`: version CAS, `USER_REQUEST_EXPIRED` when `expires_at <= at`,
  `PENDING` only. Parent identity re-verified against `agent_sessions` and either
  `conversation_turns` (`WAITING_USER`) or `tasks` (READY/RUNNING/WAITING_USER/BLOCKED/NEEDS_USER/
  INCOMPLETE/PAUSED) plus the exact attempt.
- Response contract: EXTERNAL_URL accepts exactly `{"action": "accept"|"decline"|"cancel"}`;
  FORM validates a bounded JSON-Schema profile (28 keywords) and rejects
  `pattern`/unsupported keywords with `PROVIDER_INPUT_UNSUPPORTED`; sensitive names/phrases
  (`password`, `api_token`, `AKIA…`, `ghp_…`, JWT shape) → `SENSITIVE_INPUT_UNSUPPORTED`.
  Depth ≤32, response ≤ `USER_REQUEST_RESPONSE_MAX_BYTES`, choices ≤500.
- Tables: `user_requests`, `user_request_responses`, `request_dedup`, `domain_events`; event
  `user.request.resolved.v1` with payload `{request_id, from:"PENDING", to, resolved_by,
  aggregate_version, response_digest?}`.
- Tests (5): `external_authorization_accepts_only_the_bounded_action_enum`,
  `form_response_must_match_the_pinned_bounded_schema`,
  `unsupported_or_secret_bearing_form_contracts_fail_closed`,
  `response_depth_and_size_are_bounded_before_persistence`,
  `choice_id_resolves_only_to_a_unique_stored_value_without_returning_other_choices`.

### crates/storage-sqlite/tests/linux_secret_service.rs (236)
`#![cfg(target_os = "linux")]`, `#[ignore]`, opt-in `LITECOWORK_RUN_LINUX_SECRET_SERVICE_TEST=1`;
service `com.litecowork.workspace-blob-key`; `CreatedCredentials` deletes only accounts it
recorded (never enumerates the collection). 1 test:
`native_secret_service_provisions_rotates_reopens_and_rejects_a_malformed_pointer` — provision
v1 → rotate v2 → reconstruct provider → active v2 / retained v1 → write
`"not-a-version-pointer"` into the live active account → both `current_key` and `rotate_key`
return `StoreError::Integrity`.

## Test inventory by proof kind

**Admission (dispatch/start gates).** `rejects_any_dispatch_transition_without_combined_admission`;
`INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE` guard in `transition_invocation`; `Started` refusal in
`transition_effect`; `ready_lifecycle_request_requires_matching_runtime_local_binding`;
`provision_success_requires_exact_provider_source_isolation_and_budget_proof` (8 mutated proofs);
`nonusable_compatible_offer_cannot_create_an_agent_binding`;
`nonusable_compatible_offer_cannot_enable_an_agent_binding`;
`durable_starting_planner_can_activate_after_its_binding_is_disabled`;
`admitted_planner_can_commit_after_binding_is_disabled_and_lead_eligibility_changes`;
`runtime_cannot_submit_observed_or_verified_evidence_or_claim_another_producer`;
`a_first_observation_creates_no_occurrence_and_finds_the_next_slot`.

**Fencing / CAS / epoch.** `stale_status_and_version_cas_rejects_commands`;
`only_positive_absence_proof_marks_destroyed_and_terminal` (second apply → `InvalidTransition`);
`live_fence_rejects_attempt_grant_for_another_capability_identity`;
`exhausted_change_budget_unsafe_boundary_and_stale_candidate_block`;
`stale_child_result_revision_never_escalates`;
`acquire_takeover_release_commit_events_and_exact_receipts_atomically` (epoch 1→2, stale
`Conflict{1,2}`); `stale_task_spec_verification_cannot_verify_effect`;
`changed_input_head_invalidates_verification_for_effect`;
`stale_verification_subject_cannot_verify_effect`;
`conflicted_resource_heads_invalidate_verification_for_effect`;
`conversation_agent_response_commit_settles_turn_and_session_atomically` (stale turn, stale
session, foreign principal all rejected).

**Idempotent replay.** `disable_receipt_replays_exactly_rejects_payload_reuse_and_checks_stale_version`
(exact replay identical, `request_dedup` and audit counts unchanged, payload reuse →
`Conflict{None,None}`); `replay_response_is_marked_as_replayed`;
`acquire_takeover_release_commit_events_and_exact_receipts_atomically` (`replayed: true`,
no duplicate events); `settlement_is_deterministic_for_the_same_cursor_and_window`;
`conversation_host_resolution_uses_turn_pinned_binding_after_selection_changes` (exact replay
returns the original durable receipt after host eligibility changes); replay-before-state-check
ordering is an explicit fix in both `effect_evidence::transition_effect` and
`invocations::transition_invocation`.

**Restart / recovery / rollback.** `failed_event_insert_rolls_back_the_binding_change_audit_and_receipt`;
`conversation_starting_session_recovery_uses_exclusive_keyset_pages` (plus the in-file
`list_starting_conversation_agent_sessions` keyset test);
`durable_starting_planner_can_activate_after_its_binding_is_disabled`;
`resource_edge_migration_is_atomic_and_adds_immutable_guards` (v24 rollback, then v26/v27);
`ambiguous_destroy_keeps_destroying_and_requires_reconciliation`;
`ambiguous_provision_does_not_claim_ready_or_failed`;
`a_backwards_clock_never_fires_or_rewinds_the_persisted_deadline`;
`native_secret_service_provisions_rotates_reopens_and_rejects_a_malformed_pointer`
(provider reconstruction reads persisted OS state); `acquire_takeover_release...` re-reads via
`load_control_lease` after each commit.

**Denial / authorization.** `disabling_current_default_returns_stable_conflict_without_writes`;
`agent_binding_create_rejects_auth_refs_without_writing_any_state`;
`duplicate_agent_binding_id_rejection_does_not_write_aggregate_state`;
`invalid_agent_binding_event_schema_does_not_write_aggregate_state`;
`every_active_or_unsettled_hold_denies_suspend_and_destroy` (all 5 hold kinds);
`destroy_denies_retention_and_uncommitted_output_holds`;
`sharing_scope_change_rejects_active_status_invalid_target_and_each_hold`;
`a_failed_first_fallback_never_promotes_a_later_one`;
`the_current_lead_is_never_offered_as_its_own_fallback`;
`unconfirmed_or_unconfigured_trigger_never_fails_over`;
`stale_wrong_lead_or_expired_observations_are_rejected`;
`invalid_policy_and_duplicate_fallbacks_fail_closed`;
`a_claim_supported_only_by_poisoned_content_is_rejected`;
`unsupported_or_secret_bearing_form_contracts_fail_closed`;
`malformed_child_ids_are_rejected_before_storage_access`;
`malformed_parent_context_is_rejected_at_adapter_construction`;
`source_digest_and_private_network_denial_are_validated`; plus inline negative branches in
`conversation_activation_commits_session_turn_and_local_host_binding_atomically`
(wrong host, foreign owner, degraded host, malformed event, event-id collision — all rolled back,
row asserted still `("STARTING","OPEN","READY",0)`).

## Cross-cutting observations

- **Invariant "Unknown is not success" is genuinely encoded.** `LifecycleDecision::
  ReconciliationRequired`, `InvocationStatus::Ambiguous` gated behind
  `reconcile_ambiguous_terminal`, `ProviderDestroyResult::Ambiguous` never → `Destroyed`, and
  `EffectState::Started` unreachable through this adapter.
- **Reported/observed/verified separation is enforced in SQL**, not just prose:
  `validate_evidence_producer` allows only `REPORTED` from a Runtime principal and forbids
  `verification_run_id`; `VERIFIED` requires a PASSED run whose `verifier_kind` matches the
  evidence producer and whose pinned inputs are still heads.
- **Four deliberate fail-closed holes** (consistent with the docs, not silent stubs):
  dispatch admission (`StoreError::DispatchAdmissionUnavailable` + `INVOCATION_DISPATCH_ADMISSION_UNAVAILABLE`),
  Environment destroy admission (`ENVIRONMENT_DESTROY_ADMISSION_UNAVAILABLE`),
  DelegationProfile `EnablementUnavailable` / `SessionOptionsUnsupported`,
  Automation recurring trigger hosting (README only).
- **Blob-before-SQL is the shared pattern.** `put_environment_state`, `put_state`, `write_state`
  run before/inside the transaction; a rejected commit may leave one unreferenced immutable blob
  for delayed GC — asserted explicitly in `enable_rechecks_offer_after_blob_publication_before_sql_commit`
  (`blobs +1`, events/receipts/audit unchanged) and documented in the responsibility README.
- **Auth-before-replay** is implemented via `validate_workspace_mutation_scope` /
  `authorize_owner` / `authorize_runtime` preceding `read_receipt` / `load_replay` /
  `find_receipt`; replay of a receipt is also digest-verified (`response_digest`).
- **Untested load-bearing surface:** `skill.rs` (1943 lines, 16 public methods, 4 aggregates,
  capture consent/caps), `invocations.rs` (1001 lines), `needs_you.rs` (645),
  `resource_edges.rs` (363), `delegation_profiles.rs` (981), `runtime_identity.rs`,
  `isolation_digest.rs` all have zero unit tests. Their invariants are only enforced by SQL
  predicates, so regressions there would surface only through daemon-level tests.
- **Test sources vs README claim:** `domain-responsibility/README.md` cites "all 21" tests
  (2026-10-09); the bucket alone contains 25 (`schedule_tests` 24 + `suggestions` 1), so the
  recorded count is stale/understated — evidence is CODE-level and dated, as the README itself says.
- **Panic in a pure planner:** `schedule::local_from_instant` uses `.expect("schedule instants are
  bounded")` on `chrono::DateTime::from_timestamp_millis`; a hostile or corrupt persisted cursor
  reaching `plan_settlement`/`next_slot_after` panics rather than returning `DomainError`.
- **Ordering curiosity:** `environments.rs` uses `validate_workspace_mutation_scope` after loading
  the aggregate in `transition_environment` (checked twice), while `create` splits owner-scope and
  runtime-scope checks around the replay lookup — deliberate, but the asymmetry is easy to misread.
- **Platform evidence is honestly scoped.** Both Linux qualifications are `#[ignore]`d and
  env-var gated; the systemd test asserts cgroup primitives only and says so. No macOS/Windows
  native evidence exists in this bucket beyond `#[cfg]`-gated test-support paths (WSL2 not proof).

## Coverage check

All 29 listed files read to the last line; nothing skipped, no file needed truncation. Gaps that
are *structural*, not reading gaps:
- `domain-environment`, `domain-invocations` declare submodules (`browser_control`,
  `browser_pointer`, `capability_bridge`) whose files are outside this bucket.
- `storage-sqlite` helpers used but defined elsewhere: `insert_domain_event`,
  `canonical_json`, `canonicalize_utc_timestamp`, `map_database_error`, `Command`,
  `SqliteWorkspaceStore::execute_command`, `FileBlobStore`, `accept_initial_plan_transaction`,
  `activate_task_planning_session_transaction`, `list_starting_conversation_agent_sessions`,
  `validate_plan_acceptance_commit`, `agent_binding_created_payload` (referenced by tests).
- `docs/schemas/error-codes.schema.json` is `include_str!`-compiled into `needs_you.rs`; its
  contents were not part of this bucket.
- Migration DDL constants (`SQLITE_V26_DDL`, `SQLITE_V27_DDL`, `SCHEMA_VERSION`,
  `Failpoint`) live in the storage-sqlite root module, outside this bucket.
