# Digest daemon-gap

7 files read in full + `operator.rs` wiring (≈10,650 lines). No `operationId` exists in Rust or `docs/schemas/` for these paths: identity = method + path + handler fn; operationIds **undetermined**.

## 1. `trust.rs` (1,891) — NOT MOUNTED

Pure Trust evaluator + approval producer; **no routes, no reads, no dispatch**. Doc: "Policy evaluation is a pure decision primitive… CapabilityBroker admission remains fail-closed until its authorization/effect/audit transaction is implemented."

- `CapabilityRef{capability_id, identity_kind, source, package_version, component, digest}`; `validate()` needs `sha256:`+64 lowercase hex; `PACKAGE_COMPONENT` requires `package_version`, `MCP_SKILL` forbids it. Scopes `CONVERSATION|TASK_PLANNING|ATTEMPT_EXECUTION`; `DecisionKind` = `ALLOW|DENY|REQUIRE_APPROVAL`; `PolicyEvaluation` = `VERSIONED|UNAVAILABLE`; `ExecutionMethod` incl. `NATIVE_AGENT_TOOL`, `UNKNOWN`.
- `ReasonCode` (30): 13 `*Mismatch`, 4 `*NotActive`, plus `EffectRequired, EffectUnexpected, OperationNotClassified, ExecutionMethodUnknown, SecretLeaseInvalid, EffectRequiresAttemptScope, WorkspacePolicyDenied, ApprovalBindingMismatch, ApprovalDenied, ExecutionMethodNotAllowed, ApprovalRequired, ApprovalRequiredByPinnedPolicy, AllowedReadOnlyOperation, PolicyUnavailable`.
- `PolicyEngine::core_v1()` = `"litecowork.standard"` rev 1, **two rules only**: `workspace.resource.read`→`ALLOW`+`[STRUCTURED_API]`, `external.message.send`→`REQUIRE_APPROVAL`+`[STRUCTURED_API]`; all else `DENY/OperationNotClassified`.
- **Fail-closed (exact):** bundle `None`, digest not self-verifying, or `policy_id != "litecowork.standard"` → `Deny` + `PolicyUnavailable` in the `Unavailable` variant. `Denied|Expired|Revoked|Cancelled` approval → `ApprovalDenied` *before* any read allow; `Pending` → `RequireApproval`.
- `request_approval(...)` persists only for `RequireApproval`/`ApprovalRequired|ApprovalRequiredByPinnedPolicy`, else `StoreError::Invalid("Trust policy did not require a new Approval for this exact request")`; Conversation scope → `StoreError::Invalid("Approvals require a Task or Attempt scope")`; needs `approval_status.is_none()`, owner `USER`, actor `AGENT`. `APPROVAL_TTL_MINUTES=15`; risk `SAFE` only for `workspace.resource.read`; event `approval.requested.v1` via `ApprovalStore::request_approval(RequestApprovalCommit{..})`, one tx.
- **Discrepancy:** `ApprovalActor.assurance_level` hard-coded `"VIEW_ONLY"` even with `risk:"SENSITIVE"`. ~28 tests incl. `unknown_core_policy_bundle_fails_closed_without_versioned_decision`, `corrupt_policy_digest_fails_closed_without_versioned_decision`, `denied_approval_for_the_exact_action_is_not_treated_as_a_read_allow`.
- **Wiring:** `main.rs:36` `#[allow(dead_code)] mod trust;` — "intentionally compiled but not wired until the atomic CapabilityBroker admission transaction and audit persistence exist." No `trust::` reference outside the file; no caller, no route. `trust_operator.rs` unrelated.

## 2. `automation_operator.rs` (3,071) — MOUNTED (13)

`GET|POST /v1/automations` (`list_automations`/`create_automation`) · `GET|PATCH /v1/automations/{automation_id}` (`get_automation`/`revise_automation`) · `GET …/revisions`, `…/occurrences`, `…/health` · `POST …/pause`|`…/resume`|`…/disable` · `POST …/run` (`run_automation_now`) · `POST …/test-runs` (`run_automation_test`) · `POST /v1/automations/host/settle` (`settle_automation_triggers`). Body limit 128 KiB → 413.

- Auth: `authorized_workspace` (owner + ACTIVE local binding) for reads/definition; `run`/`test-runs` = `selected_workspace` + `ensure_workspace_owner` (binding fenced in tx); `host/settle` = `authorized_trigger_host` → 403 `TRIGGER_HOST_UNAVAILABLE` unless this incarnation holds an ACTIVE binding with role `TRIGGER_HOST`. Mutations via `ResponsibilityService::execute(OwnerCommandScope{..}, …)`, then revision re-read (`STALE_WORKSPACE_VERSION`) + `ensure_workspace_owner`.
- Storage/events: `SqliteCoworkerStore`, `TaskService::prepare_standalone`→`create_task(commit)`, `SqliteRoutineStore`, `SqliteAutomationHostStore` (`health_facts`, `list_due_schedules`, `occurrence_for_slot`, `settle_started_occurrences_from_task_outcomes`, `record_skipped_occurrence`, `advance_cursor`), `plan_host_settlement`, `materialize_routine_inputs`; one `EventDraft`/admission (entity `Task`, payload + `routine_id`, `routine_revision`, `automation_id`, `automation_occurrence_id`|`automation_test_run_id`); `no-store`.
- Idem: `idempotent_id("tsk"|"occ"|"trn"|"cor"|"ev", …)`; replay resolved **before** mutable Coworker/Resource/binding/head reads via `get_task_create_receipt(…, canonical_payload)` (changed payload → 409 `CONFLICT`), replayed commit field-validated else 503. One `create_task` tx = occurrence claim + READY Task + event + receipt; `claim_expires_at` +2 min; `trigger_kind` `MANUAL`/`SCHEDULE`; `If-Match` required for revise/pause/resume/disable/run/test.
- **Fail-closed (exact):** `DomainError::ReconciliationRequired` → 503 `DEPENDENCY_UNAVAILABLE` "Automation activation requires a trigger host and occurrence reconciliation service that is not available"; `domain-responsibility/README.md:132` "Resume returns `ReconciliationRequired` while a retained Automation or occurrence needs trigger reconciliation" → **`POST …/resume` is the gated command**. Exactly one `Manual` trigger else 409 `INVALID_TRIGGER`; `Hub`/foreign `SpecificRuntime` → 409 `TRIGGER_HOST_UNAVAILABLE`.
- Errors: `INVALID_ARGUMENT, DEPENDENCY_UNAVAILABLE, NOT_FOUND, FORBIDDEN, CONFLICT, AUTOMATION_DISABLED, AUTOMATION_NOT_PAUSED, ROUTINE_ARCHIVED, COWORKER_PAUSED, TRIGGER_UNSUPPORTED, WORKSPACE_ARCHIVED, INVALID_TRIGGER, TRIGGER_HOST_UNAVAILABLE, ROUTINE_REVISION_NOT_FOUND, AGENT_UNAVAILABLE, STALE_VERSION, INTERNAL`.
- Settlement blockers (24 literals): `TRIGGER_HOST_CLOCK_UNAVAILABLE, TRIGGER_HOST_STORE_UNAVAILABLE, TRIGGER_HOST_LOOKUP_FAILED, OCCURRENCE_SETTLEMENT_UNAVAILABLE, TRIGGER_CURSOR_MISSING, OCCURRENCE_COUNT_UNAVAILABLE, SCHEDULE_PLAN_UNAVAILABLE, OCCURRENCE_IDENTITY_INVALID, OCCURRENCE_LOOKUP_FAILED, OCCURRENCE_ALREADY_SKIPPED, OCCURRENCE_TASK_MISSING, ROUTINE_REVISION_UNAVAILABLE, MISSING_REQUIRED_INPUT, INPUT_MATERIALIZATION_FAILED, COWORKER_PIN_MISSING, AGENT_UNAVAILABLE, TRIGGER_HOST_BINDING_UNAVAILABLE, CLAIM_WINDOW_UNAVAILABLE, TASK_ADMISSION_REJECTED, AUTOMATION_OR_DEPENDENCY_CHANGED, AUTOMATION_ADMISSION_REJECTED, PINNED_DEPENDENCY_UNAVAILABLE, TASK_PROVENANCE_MISMATCH, TRIGGER_CURSOR_ADVANCE_FAILED`; skips `MISFIRED_SLOT_SKIPPED/OVERLAP_SKIPPED/OVERLAP_QUEUED/OVERLAP_CANCEL_OLD`; `TRIGGER_HOST_GRACE_MS` 5 min; a deferred slot keeps naming its instant.
- Worker: `AutomationTriggerHostWorker`, `TRIGGER_HOST_POLL_INTERVAL` 30 s, started `operator.rs:1252`; **start failure aborts endpoint startup**.
- **Fixed answers:** `occurrence_version: 3` + `occurrence_status: "STARTED"` literals (never read back); `routine_health` always `observed_costs: []`, `required_dependency_health: []`, `drift_evidence_refs: []`; drift from `transient_failures` only → `UNKNOWN`; success rate `None` under 3 runs, `CANCELLED` excluded.
- **Doc drift:** doc still says "recurring trigger hosting and `/resume` remain unavailable" though worker + settle route are wired. Tests: worker (2) + health/occurrence route tests.

## 3. `rag_retrieval_operator.rs` (889) — MOUNTED (2)

`POST /v1/resources/retrieval` (`grounded_retrieval`) · `GET /v1/resources/{resource_id}/index-state` (`resource_index_state`) · `route_layer(retrieval_no_store)`.

- Auth: `selected_workspace` + `ensure_workspace_owner_for` — owner only, **no local-binding check**. Provider (stated limit): `LocalLexicalRetrievalProvider`, `litecowork.local-lexical`/`LEXICAL_ONLY`/`LOCAL`; "It is not semantic retrieval."
- Outcomes: `GROUNDED` | `INSUFFICIENT_EVIDENCE` (`NO_AUTHORIZED_MATCH` or `PINNED_REVISION_NOT_INDEXED`) | `PROVIDER_UNAVAILABLE`. `scope:None` = whole Workspace; `[]` → 400. `request_id` derived `retrieval-<sha256>` — "a correlation handle, not an idempotency receipt" → **no idempotency, no writes, no events**.
- Errors: `INVALID_ARGUMENT, RESOURCE_INDEX_UNAVAILABLE, PROVIDER_UNAVAILABLE, INTEGRITY_FAILURE, INTERNAL, NOT_FOUND`; `resolve_index_state(&ResourceIndexFacts{..})`; missing resource → 404. 7 tests (revision-bound citations, insufficient evidence, cross-Workspace isolation, non-owner 403, revoked ContextDocument, index-state READY/404, local-models `LOCAL_MODEL_PROVIDER_NOT_QUALIFIED`).

## 4. `workspace_root_scan_operator.rs` (1,645) — MOUNTED (2)

`POST /v1/workspace-roots/{workspace_root_id}/scan` (`scan_workspace_root`) · `GET …/observations` (`search_workspace_root_observations`) · `route_layer(root_scan_no_store)`.

- Auth: `selected_workspace` + `get_workspace` owner equality → **404 `NOT_FOUND`** non-disclosing. Scan also needs `If-Match` root version + `Idempotency-Key`; `WorkspaceRootRevalidationBindings::CurrentIncarnation` must match runtime *and* incarnation plus `location_id`/`locator_ref_id`, live keyed identity, then re-open + `revalidate_with_cancel`.
- **Platform gate (exact):** filesystem path `#[cfg(target_os = "linux")]`; other targets → **501 `SCAN_UNSUPPORTED_PLATFORM`** "Metadata scanning is not available on this platform", after auth/If-Match/Idempotency-Key. macOS/Windows scan permanently unsupported here.
- Limits: `MAX_SCAN_TIME` 8 s (`MAX_FINAL_IDENTITY_FENCE_TIME` 1 s), semaphore 2 (`SCAN_UNAVAILABLE`), `MAX_ENTRIES = storage_core::MAX_WORKSPACE_ROOT_SCAN_ENTRIES`; TIME_LIMIT partial usable only if the closing fence succeeds.
- Storage: `WorkspaceRootService::{get_scan_receipt (replay), get_scan_candidate, commit_scan}` (one tx: observation + `ResourceRecord`/`ResourceLocationRecord` + bindings); partials `TIME_LIMIT_REACHED, SCAN_BUDGET_REACHED, ENTRY_CHANGED_OR_UNREADABLE, MOUNT_BOUNDARY_REACHED`; cursor `SignedRootObservationCursor` tagged by `keyed_pseudonym("litecowork.operator.workspace-root-observation-cursor.v1", …)`, constant-time compare.
- Errors: `INVALID_ARGUMENT, NOT_FOUND, STALE_WORKSPACE_VERSION, RESOURCE_IDENTITY_UNAVAILABLE, SCAN_UNAVAILABLE, RUNTIME_UNAVAILABLE` (unversioned `Conflict`), `SCAN_UNSUPPORTED_PLATFORM, CONFLICT, BUSY, INTERNAL`. 17 tests (6 fence/admission, 5 error-mapping, 6 route — every rejection `no-store`).

## 5. `delegation_profiles_operator.rs` (1,220) — MOUNTED (7)

`GET|POST /v1/delegation-profiles` · `GET /v1/delegation-profiles/{id}` · `GET|POST …/revisions` (POST = `revise_delegation_profile`) · `GET …/performance` · `POST …/duplicate` · `POST …/status` · `route_layer(profile_no_store_middleware)`, 96 KiB limit.

- Auth: `authorized_workspace`; `execute_command` re-authorizes **after** commit (workspace `ACTIVE` + ACTIVE local binding) → 403 `FORBIDDEN`. Commands `DelegationProfileService::execute(DelegationProfileCommand::{Create,Revise,Duplicate,SetStatus})`; ids `idempotent_id("dp", …)`; `If-Match >0` for revise/duplicate/status.
- **Fail-closed (exact):** `DelegationProfileError::EnablementUnavailable` → 503 `DEPENDENCY_UNAVAILABLE` "Enabling requires validated adapter, Trust, and Environment admission integration" → **create/revise allowed, enable is not**. `SessionOptionsUnsupported`→409 `AGENT_SESSION_OVERRIDE_UNSUPPORTED`; `OptionsInvalid`→400 `DELEGATION_PROFILE_OPTIONS_INVALID`; `Archived|AlreadyExists|VersionConflict|IdempotencyConflict`→409 `CONFLICT`.
- Quirk: `store_error(StoreError::NotFound)` → **403 `FORBIDDEN`**, not 404. **Fixed projection:** `…/performance` passes `source_observations_complete: false` ("VerificationRun has no exact Attempt attribution") → `confidence:"LOW"`, `verifier_pass_rate:null`, `failure_categories: []`; fractional `UsageQuantity` → unknown, never rounded.

## 6. `agents/codex_app_server.rs` (1,486) — no routes

`#![allow(dead_code)]`: "The unqualified native transport remains staged until admitted dispatch and descendant settlement are integrated." Header: "No payload is logged, persisted, or auto-approved… Neither proves Environment writer quiescence or supplies Trust grants."

- Limits: line 1 MiB, queued 64, wire 8 MiB, pending RPC 16, startup 15 s, rpc 30 s; `spawn` rejects out-of-range (`InvalidLimits`); exe/cwd absolute. `NativeEnvironment::apply` = `env_clear()` + 9 vars (`HOME, USERPROFILE, APPDATA, LOCALAPPDATA, CODEX_HOME, PATH, SYSTEMROOT, TMPDIR, TEMP, TMP`); `args(["app-server","--listen","stdio://"])`.
- `codex-host-reaper` owns the child, kills the tree on `stop_requested` **or expired `next_rpc_deadline`** (watchdog independent of event pumping). `TransportState{…, writer_quiescence_proven}` — **always `false`**.
- RPC: `initialize, account/read, model/list, thread/start, thread/resume, turn/start, turn/steer, turn/interrupt`; constructors incl. `thread_start_read_only`, `turn_start_with_conversation_context`, `turn_start_read_only_plan`. **`TurnSteer` has no constructor** → unreachable.
- Approvals: one-shot `AcceptOnce|Decline|Cancel` only via `ServerReply::{ExecutionApproval,FileChangeApproval,DenyPermissions,DeclineElicitation,CancelElicitation,Unsupported}`; mismatch → `UnsupportedReply`; `Unsupported` → `-32601 "Unsupported client request"`. No session/policy amendment.
- Fail-closed: malformed/partial `initialize` → poison, `initialized` false, reads `NotInitialized`; `TerminalObservation` = "diagnostic evidence only: never an RPC acknowledgment". `TransportError` (16): `InvalidLimits, SpawnFailed, PipeUnavailable, WorkerUnavailable, Io, InvalidMessage, InvalidRequestInput, MessageTooLarge, QueueFull, Timeout, Closed, NotInitialized, AlreadyInitialized, UnknownRequest, UnsupportedReply, IdExhausted`.
- 3 tests (`malformed_or_partial_initialize_never_acknowledges_or_admits_read_rpcs` etc.); consumers `agents/mod.rs:10`/:27, `agents/codex_profile_probe.rs:284` `spawn` → **profile probe only**, never dispatch. Read artifact: two literals render `{"refreshToken=[redacted]` (file-read redaction, not a source defect).

## 7. `user_request_operator.rs` (~1,200, skimmed) + wiring

`GET /v1/user-requests` · `GET /v1/user-requests/{request_id}` · `POST …/respond` · `POST …/dismiss` · own `.route_layer(authenticate)` + no-store; limit `USER_REQUEST_RESPONSE_MAX_BYTES + 1024`.

- Mutation: `If-Match` positive version; `Idempotency-Key` required; body untagged `{response}`|`{choice_id}`; event `user.request.resolved.v1`, `entity_revision = expected_version + 1`; respond receipt without `response_digest` → 500 `INTEGRITY_FAILURE`; dismiss returns only `UserRequestResolutionReceiptView`.
- Errors: `USER_REQUEST_RESPONSE_TOO_LARGE` 413, `INVALID_ARGUMENT` 400, `NOT_FOUND` 404, `CONFLICT` 409, `USER_REQUEST_EXPIRED`/`SENSITIVE_INPUT_UNSUPPORTED`/`PROVIDER_INPUT_UNSUPPORTED` 422, `INTEGRITY_FAILURE` 500, `DEPENDENCY_UNAVAILABLE` 503. Doc says "Read-only… detail" but respond/dismiss mutate.
- `main.rs`: `mod operator; mod agents; #[allow(dead_code)] mod trust;` plus `#[allow(dead_code)]` on `browser_consent`, `browser_control_service`, `environment_*`; help: "This build does not accept work… it does not launch agents."
- `operator.rs` declares `automation_operator :102`, `delegation_profiles_operator :110`, `rag_retrieval_operator :132`, `trust_operator :156`, `user_request_operator :158`, `workspace_root_scan_operator :162`; `build_operator_router` (:1514) merges `user_request_operator :1545`, `automation_operator::router() :1547`, `delegation_profiles_operator :1550`, `workspace_root_scan_operator :1559`, `rag_retrieval_operator :1569`, then `.layer(DefaultBodyLimit::max(16 MiB))`, `.route_layer(middleware::from_fn(authenticate)) :1695`, `.with_state(state)`. `authenticate` (:6405) accepts only the private `AuthenticatedLocalPeer` extension, inserted solely by `dispatch_ipc_exchange` (:1748); no HTTP listener is mounted.

| File | Mounted |
|---|---|
| `trust.rs` | **NO** — `#[allow(dead_code)]`, no reference, no `request_approval` caller |
| `automation_operator.rs` | YES — 13 routes + 30 s worker (start failure aborts startup) |
| `rag_retrieval_operator.rs` | YES — 2 routes |
| `workspace_root_scan_operator.rs` | YES — 2 routes (scan 501 off Linux) |
| `delegation_profiles_operator.rs` | YES — 7 routes (enable → 503) |
| `agents/codex_app_server.rs` | n/a — no routes; used by `codex_profile_probe` |
| `user_request_operator.rs` | YES — 4 routes, own `authenticate` |

## Cross-file observations

1. **Trust never reaches a mutation path:** no mounted module references `PolicyEngine`/`request_approval`; `automation …/run` and `…/test-runs` admit READY Tasks with no Trust evaluation, while `approval_operator` persists/resolves approvals independently of `trust.rs`.
2. **Two authorization bars:** `authorized_workspace` = owner + ACTIVE binding vs `ensure_workspace_owner_for` = owner only (rag_retrieval, root scan, user_request, automation run/test). Only root scan re-proves the *incarnation* binding; only delegation re-checks after commit.
3. **Four idempotency implementations** (receipt+payload, `get_scan_receipt`, service replay, store key); `rag_retrieval` has none by design. **Three `If-Match` parsers** with different rules.
4. **Platform asymmetry:** folder scan is Linux-only; the same build answers 501 `SCAN_UNSUPPORTED_PLATFORM` elsewhere.
5. **Fixed/empty values to exclude from acceptance claims:** `occurrence_version:3`/`occurrence_status:"STARTED"`; `observed_costs`/`required_dependency_health`/`drift_evidence_refs` `[]`; `confidence:"LOW"`, `verifier_pass_rate:null`, `failure_categories: []`; `writer_quiescence_proven:false`.
6. **Doc drift in four files** (automation trigger hosting, user_request "read-only", codex "unqualified", delegation enablement) — stale prose, not behaviour.
7. **Unverified:** service internals (`ResponsibilityService`, `DelegationProfileService`, `WorkspaceRootService`) at call contract only; no `docs/` status claim reconciled.
