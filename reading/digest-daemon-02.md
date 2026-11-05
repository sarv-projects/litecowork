# Digest daemon-02 — `apps/litecoworkd/src/` operators, IPC tests, workers

Files: 16 read in full or substance of 20 listed | Lines read: ~11,344 of 21,117 | Bytes: ~415 KB of 784 KB

## Per-file notes

### apps/litecoworkd/src/main.rs (79 lines)
- CLI entry (adapter): `--help`, `--version`, `run --data-dir`, `status --data-dir`, `backup <create|verify|restore|upgrade-check>`; unknown args exit 2.
- Dead-code gated: `browser_consent`, `browser_control_service`, `environment_action_batch`, `environment_computer_use`, `environment_isolation`, `environment_office_qualification`, `environment_provider`, `trust`. Live: `agent_credentials_operator`, `agents`, `artifact_operator`, `attempt_process_scope`, `backup_operator`, `conversation_operator`, `local_filesystem`, `operator`, `runtime`, `workspace_root_cleanup_worker`.
- `HELP` is evidence: "This build does not accept work: the Runtime remains DEGRADED … and it does not launch agents"; "there is no authenticated Operator stop command yet".

### apps/litecoworkd/src/workspace_root_cleanup_worker.rs (105 lines)
- Thread `litecowork-workspace-root-cleanup`; `sync_channel(1)` stop; `EMPTY_OR_FAILED_POLL=1s`, `COMPLETED_BATCH_PAUSE=25ms`; calls `store.process_next_workspace_root_cleanup_batch(&runtime_id, &now)`.
- `Err(_)` → 1s poll then retry (no success claim); `Drop` joins.
- Test asserts retry after `StoreError::Io` then exit (2 calls). No DB assertions.

### apps/litecoworkd/src/runtime_windows_security_tests.rs (124 lines)
- Shells `powershell.exe -NoProfile -NonInteractive` (PSModulePath removed) asserting `AreAccessRulesProtected` and exactly two Allow rules: `S-1-3-4` (OWNER RIGHTS), `S-1-5-18` (SYSTEM), none inherited — for state dir, `runtime.lock`, `runtime-state.json`.
- Junction (`mklink /J`) state dir must be rejected; file-symlink `runtime-state.json` must be rejected by `read_state` and must not alter the target ACL. Symlink test prints SKIP on `PermissionDenied`/`1314` — a security assertion that silently no-ops without Developer Mode.

### apps/litecoworkd/src/conversation_operator.rs (1386 lines)
- Operator layer; header still claims "Turn dispatch is intentionally absent here" (stale).
- Routes: `GET|POST /v1/conversations`; `GET /v1/conversations/{id}`; `POST …/turns`; `POST …/turns/{turn_id}/retry`; `POST …/turns/{turn_id}/cancel`; `PATCH …/agent-binding`.
- `ConversationApiState{store, principal_id, runtime_id, supervisor: Option<Arc<AgentSessionSupervisor>>}`; `FromRef<ApiState>` constructs a **new** supervisor per request; shared supervisor only in `#[cfg(test)] test_router_with_supervisor`.
- Submit: idempotency key → workspace header → `validate_turn_request` (≤256 blocks/refs, ≤1 MiB text, presentation `AUTO|SIMPLE|RICH`) → `load_authorized_conversation` → canonical digest (`serde_json_canonicalizer`+sha256) → `idempotent_id("msg"|"turn"|"cor", principal, request_id, …)` → `replay_conversation_turn` → `resolve_turn_agent_binding` → one `create_conversation_turn` commit (USER message, OPEN turn, Conversation bump, `conversation.message.added.v1`, `conversation.turn.created.v2`, receipt).
- Supervisor branch: `tokio::spawn(supervisor.dispatch_turn(...))`; failure only `eprintln!`; returns `202 ACCEPTED` + `ConversationTurnReceipt{Open}` + `x-correlation-id`.
- No-supervisor branch: second txn `TurnCommand::Fail` → `conversation.turn.settled.v1` `OPEN→FAILED`, `reason_code=CONVERSATION_SESSION_ADMISSION_UNAVAILABLE`, then `503 DEPENDENCY_UNAVAILABLE` "message was saved … not sent to an agent", `no-store`; conflict with `actual > expected` treated as same 503; other errors `500 INTERNAL` settlement failure.
- Binding: `conversation.active_agent_binding_id` else `workspace.default_agent_binding_id`; no fallthrough; archived workspace → 404; none → `422 AGENT_UNAVAILABLE`. Body `agent_binding_id` identity-checked only.
- Retry: `If-Match` (`409 STALE_VERSION`), only from `FAILED` (else `409 INVALID_STATE`), new `ses_` id, `retry_ordinal+1`, `conversation.turn.retried.v1`, 202.
- Cancel: `Completed|Failed|Cancelled` → `409 INVALID_STATE`; `RUNNING`→`CANCEL_REQUESTED` (`conversation.turn.status.changed.v1`, `USER_REQUESTED`)→`CANCELLED`; `CANCEL_REQUESTED`→`CANCELLED`; `OPEN`→`FAILED` (`USER_CANCELLED`); else 503.
- Codes: `FORBIDDEN`, `INVALID_ARGUMENT`, `NOT_FOUND`, `AGENT_UNAVAILABLE`, `AGENT_NOT_LEAD_ELIGIBLE`, `WORKSPACE_ARCHIVED`, `STALE_CONVERSATION_VERSION`, `CONFLICT`, `DEPENDENCY_UNAVAILABLE`, `INTERNAL`, `STALE_VERSION`, `INVALID_STATE`.
- Risk: 202 precedes any dispatch outcome; dispatch failure leaves durable `OPEN` with only stderr and no compensation. `resolve_turn_agent_binding` and retry/cancel `get_conversation_turn` call the store synchronously on the Tokio worker.

### apps/litecoworkd/src/runtime_offers_operator.rs (328 lines)
- `GET /v1/runtimes/{runtime_id}/offers`; cursor `{version:1, workspace_id, runtime_id, runtime_incarnation_id, limit, offer_kind, offer_ref}`; `offer_kind` must be `AGENT_ENDPOINT`; limit 1–200.
- `StoreError::NotFound` → `403 FORBIDDEN` (non-disclosure); `Conflict` → `409 STALE_VERSION` "cursor no longer matches the current Runtime incarnation"; `no-store`.
- Tests: missing `x-workspace-id` → 400; `limit=201` → 400.

### apps/litecoworkd/src/goal_operator.rs (495 lines)
- Passive Goals (`GoalService`, `GoalCommand`); "Goal commands record intent and references only; they never schedule work." 64 KiB body.
- Error map: `GOAL_ARCHIVED`, `INVALID_TRANSITION`, `WORKSPACE_ARCHIVED` (409), `NOT_FOUND`, `DEPENDENCY_UNAVAILABLE`; `goal_view` re-checks goal/revision/workspace identity; ownership rechecked after commit before disclosure.

### apps/litecoworkd/src/agents/module_registry.rs (638 lines)
- `AgentModuleRegistry{modules_by_id: BTreeMap}`; `AgentModuleManifest{module_id, module_version, descriptor_api_version, registry_agent_ids, display_name, distribution_kinds, supported_os, supported_architectures, lifecycle_features}`; `AgentModuleLifecycleFeatures{discover_installation, install, update, uninstall, native_auth, secure_credential_slots, open_native_config}`.
- `match_registry_agent`: host API 0 → `Ok(None)`; highest compatible descriptor; tie → `Err(AmbiguousMatch)`.
- `assess_registry_agent` → `NoAdapter|NoCompatibleDescriptor|UnsupportedRuntime|Supported`; registry record alone never `Supported`.
- Tests: flags never inferred; exact-OS/arch matching; duplicates fail closed; `../`, `module//agent`, `01.2.3`, 123-char build suffix, `../../run-this` distribution all rejected.

### apps/litecoworkd/src/trust_operator.rs (875 lines)
- Only `POST /v1/capability-grants` and `/{grant_id}/revoke`; 24 KiB; `authenticate` + `no_store_middleware`; never consumes Approval, issues SecretLease or dispatches.
- Hard-coded `CapabilityGrantScope::TaskPlanning`, `allowed_operations=["workspace.resource.read"]`, `secret_refs: []`, `assurance_level="LOCAL_STRONG"` (not body-selectable), expiry `> now && <= now+24h`; ref digest `sha256:<64 lowercase hex>`.
- Events `capability.grant.created.v1` / `capability.grant.revoked.v1`, entity `capability_grant`; `SqliteApprovalStore`.
- Tests: issue+revoke; replay identical 201 + same correlation; changed payload 409; revoke `If-Match:1` → `REVOKED` v2; replay `replayed:true` same `event_id`; stale 409; DB = 1 revoked grant, 2 `audit_records` (`CAPABILITY_GRANT_ISSUED|REVOKED`), 2 `domain_events`; approval_id/secret_refs/25h/`external.message.send`/attempt_id all 400 with 0 grants.

### apps/litecoworkd/src/capability_invocations_operator.rs (584 lines)
- Read-only `GET /v1/tasks/{task_id}/invocations`, `GET /v1/capability-invocations/{id}`; cursor bound to workspace+task; limit 1–200.
- `failure_view` allowlist: `code`, `category ∈ {TRANSIENT,PERMANENT,POLICY,USER_REQUIRED,AMBIGUOUS}`, `safe_message`, `retryable`, `signature`, `correlation_id`; `details` dropped.
- `NotFound`→404; all other store errors (incl. Integrity)→503 `DEPENDENCY_UNAVAILABLE`; `no-store` on every response.
- Tests assert `provider_task_id`, `provider_handle_ref`, `failure.details` absent; cursor rejects foreign scope; 401/404 are `no-store`.

### apps/litecoworkd/src/agent_credentials_operator.rs (489 lines)
- Library-only write-only boundary: no `routes()`, not mounted; `AgentCredentialSecretSubmission{descriptor_digest, secret_base64: Zeroizing<String>}` (`deny_unknown_fields`, no `Debug`).
- Order: validate (digest 71 chars `sha256:` lowercase; base64 → `Zeroizing<Vec<u8>>`, 1..=48 KiB decoded) → descriptor identity+digest+freshness → exactly one slot with `storage_owner == LiteCoworkSecretStore` (NativeAgent refused) → `SecretStore::write` → `SecretRef` must be `LocalOnly`.
- `From<StoreError>` → `SecretStoreUnavailable` (no provider text).
- Tests: one write, response only slot + ref (raw secret absent); stale/mismatched/NativeAgent never write; unknown/duplicate slot and bad base64 fail closed; store failure mapped.
- Source contains literal `let encoded_secret=[redacted];` — file as preserved is sanitized and would not compile.

### apps/litecoworkd/src/agent_quota_observation_operator.rs (288 lines)
- `GET /v1/agent-bindings/{id}/quota-observation`; owner + binding existence, then always `Json(Option::<Value>::None)` → body `null`, `no-store`; doc: no AgentModule publishes authoritative quota, do not infer.
- Tests: 200/`null`, 403 foreign, 404 missing.

### apps/litecoworkd/src/artifact_operator.rs (801 lines)
- Routes incl. `POST /v1/artifacts/{id}/text-version` (7 MiB), `promote`, `archive`, `GET …/edit-head`, `GET /v1/tasks/{task_id}/artifacts`.
- Text append preconditions: `If-Match` + `expected_content_version` + `expected_resource_version` + `expected_parent_resource_revision_id`; replay resolved before head checks → 200 `replayed:true`; commit emits `resource.revision.observed.v1` + `artifact.version.created.v1`.
- Only managed `text/plain` ≤1 MiB; codes `INVALID_ARTIFACT_TRANSITION`, `ARTIFACT_ARCHIVED`, `IDEMPOTENCY_CONFLICT`, `STALE_VERSION`, `ARTIFACT_READ_LIMIT_EXCEEDED`, `PROVIDER_UNAVAILABLE`, `INTEGRITY_FAILURE`; download sets `nosniff`, CSP `sandbox; default-src 'none'`, `attachment`, `no-store`.

### apps/litecoworkd/src/resource_archive_operator.rs (882 lines)
- `POST /v1/resources/{id}/archive/preview`; 16 MiB cap; process-wide `Semaphore::new(2)` with `try_acquire_owned` → `429 RATE_LIMITED`; pin re-verified (workspace/resource/revision/digest/size) else `409`.
- Errors: 413 read-limit, 422 external content, 409 `CONTEXT_DOCUMENT_NOT_ACTIVE`, 404, 500 `INTEGRITY_FAILURE`, 503 `RESOURCE_LOCATION_UNAVAILABLE`; worker `ARCHIVE_REJECTED` 422, 408 `TIMEOUT`, 502/503 `DEPENDENCY_UNAVAILABLE`.
- Tests: closed body, lowercase digest only, cross-workspace 404, non-owner 403, wrong digest 409, success only under `cfg!(linux && source-zip-worker)` else 503, saturated slots 429 with zero new `domain_events`/`request_dedup` and no child revision, 16 MiB+1 → 413.

### apps/litecoworkd/src/resource_archive_worker.rs (630 lines)
- Inputs: `SourceRevision{workspace_id, resource_id, revision_id, content_sha256}` + bytes; `MAX_ARCHIVE_BYTES 16 MiB`, `MAX_REQUEST_BYTES 24 MiB`, `MAX_RESPONSE_BYTES 8 MiB`, `OUTER_DEADLINE 15 s`, `PYTHON=/usr/bin/python3`, `CLI=capabilities/zip_intake/runtime_cli.py`.
- Without `cfg(linux && source-zip-worker)`: always `Err(Unavailable)` — never a synthetic manifest.
- With feature: `python3 -I`, `current_dir("/")`, `env_clear()`, `process_group(0)`, stderr null, reader/writer threads, deadline poll, SIGKILL to process group.
- `parse_manifest` binds `provider_version=="litecowork-zip-intake/0.1.0"` and all limits (`max_entries 2048`, entry 8 MiB, expanded 32 MiB, ratio 100, path 1024/32, seconds 10.0); rejects `observed_bytes`/`content_sha256`, status outside `READY|DIRECTORY|REJECTED`, reason/status contradictions, totals over limits.
- Path safety: no leading `/`, `\`/`:`, NFC, no trailing space/dot, Windows reserved stems, `<>"|?*`; NFKC casefold collisions must be `REJECTED/PATH_CONFLICT`.
- Tests: hostile paths, reserved names, compressed empty dir accepted, `A.txt`/`Ａ.txt` and `Straße`/`STRASSE` rejected, file-ancestor rejected, `EXTRACTED` rejected.

### apps/litecoworkd/src/browser_control_service.rs (840 lines)
- Compiled but unwired (`#[allow(dead_code)]`); decides/records intent only, "not evidence that an action was applied".
- `BrowserControlOwner{Agent,User,None,TransferPending}`; `BrowserActionKind` (15); `BrowserControlLease{control_epoch, session_revision, observation_revision, expires_at_ms, in_flight_inputs, owner_agent_session_id, owner_principal_id}`.
- Reason codes `BROWSER_ACTOR_MISMATCH`, `…_PRINCIPAL_NOT_PERMITTED`, `…_SESSION_MISMATCH`, `…_STALE_CONTROL_EPOCH`, `…_LEASE_EXPIRED`, `…_NOT_THE_LEASE_OWNER`, `…_CONTROL_TRANSFER_PENDING`, `…_STALE_SESSION_REVISION`, `…_STALE_TAB_GENERATION`, `…_INPUTS_IN_FLIGHT`, `…_OBSERVATION_NOT_USABLE`, `…_OBSERVATION_REVISION_NOT_ADVANCED`, `…_CONTROL_EPOCH_EXHAUSTED`, `…_MODE_DISALLOWS_AGENT_INPUT`.
- `admit_action`: session→expiry→epoch→session revision→transfer-pending→derived vs claimed actor→mode. `request_takeover`: user only, zero in-flight. `return_to_agent`: user owner, fresh observation in current epoch, revision advanced, age ≤ max.
- Tests cover every refusal, queued pre-takeover action → `StaleEpoch`, exact `execution_method_for` map.

### apps/litecoworkd/src/suggestions_operator.rs (1200 lines)
- Listing settles `SYSTEM_EXPIRY` first; `more_due` → 503 `DEPENDENCY_UNAVAILABLE` (no partial page). `validate_action_workspace` requires ACTIVE local Runtime binding for the current incarnation.
- `accept-task`: atomic Task + Suggestion resolution (`create_from_suggestion`), replay via `get_suggestion_task_acceptance_receipt`; refuses archived workspace/Coworker, disabled/non-lead binding; 201/200, ETag `"<version>"`.
- Test: 200-byte key accepted, replay identical, changed payload 409, 201-byte key 400, DB 1 preference / 1 `suggestion.preference.changed.v1` / 1 `request_dedup`.

### apps/litecoworkd/src/skill_operator.rs (1962 lines; read 1–700)
- Routes `/v1/skills`, `/v1/skill-proposals` (+approve/reject), `/v1/demonstrations` (+actions/complete/pause/resume/abort), `/v1/mcp-apps/frames|messages`; 256 KiB.
- Approving a proposal "records the owner's decision only: it never installs a package, never activates a Skill, and never grants authority"; LiteSPM publication reported blocked, not simulated.
- `WriteSkillBody` has no `status`; status derives from `evaluate_skill_definition_write`; views expose revision refs, never inline instruction text; cursors `{version:1, workspace_id, scope, created_at, entity_id}`; guard errors → 400 `{"reason": code}`.

### apps/litecoworkd/src/operator_ipc_tests.rs (8035 lines; read 1–~900)
- Aggregates sibling test modules by `#[path]` (incl. `resource_dependencies_operator_tests`, `resource_upload_archive_operator_tests`, `runtime_capability_hosts_tests`, `workspace_idempotency_key_bounds_operator_tests`).
- IPC surface: `operator_ipc::{DEFAULT_MAX_IN_FLIGHT_BODY_BYTES, InFlightBodyBudget, LogicalHeader, MAX_STREAM_FRAME_BYTES, PROTOCOL_VERSION, RequestFrame, RequestHeader, unix::UnixEndpoint}`, `dispatch_ipc_exchange`, `is_expected_unix_stream_write_disconnect`, `AuthenticatedLocalPeer`.
- `advance_runtime_inventory_revision` stops at `MAX_SAFE_JSON_INTEGER` and errors without mutation.
- Context-document deletion route test: 400 unknown field, 401 unauthenticated, 403 foreign POST+GET, 400 missing `If-Match`, 202 `DELETION_PENDING` (`required_replica_count>0`, no `provider_ref|locator_ref_id|resource-content-gc`), byte-identical replay, 200 poll matching `manifest_digest`, 409 stale; all `no-store`.
- Idempotency key bound 129/200 accepted, 201 and spaces rejected. Instruction pin survives Workspace advance (1 vs 2). Auth middleware 401 without `AuthenticatedLocalPeer`; instruction history 403 with canonical `FORBIDDEN` message; invalidation paging scoped/paginated/`no-store`.
- Entire file is `#[cfg(unix)]`-style (`PermissionsExt`, `MetadataExt`); no Windows counterpart observed.

## Cross-cutting observations
- IPC trust root: `authenticate` requires the `AuthenticatedLocalPeer` extension, inserted only by `dispatch_ipc_exchange` after Unix peer-credential auth; a header cannot substitute.
- Idempotency: deterministic IDs + `request_dedup` receipt; replay resolved before preconditions; payload change → conflict.
- `no-store` uniformly on success and error.
- "Unknown is not success": quota `null`, registry never infers features, archive worker `Unavailable` instead of empty manifest, invocation failures allowlisted. Exception: conversation 202 precedes dispatch; supervisor failure only logged, no settlement here.
- Drift: conversation header denies dispatch that exists; `main.rs` dead-codes `trust`/`browser_control_service` although trust routes are mounted; archive preview route is 503 in release builds.
- `AgentSessionSupervisor` lives in `crate::agents` (not listed) and is rebuilt per request — restart/reconnect recovery of OPEN turns is UNVERIFIED in this bucket.
- Tests use real SQLite/blob stores with fixed keys, but fixtures often set `PRAGMA foreign_keys=false`.

## Coverage check
- `skill_operator.rs`: lines 1–700 of 1962 only (demonstration/approve/frame tests unread).
- `operator_ipc_tests.rs`: ~900 of 8035 lines; tail read cancelled at deadline — conversation/supervisor, runtime recovery and stream tests unread.
- `resource_dependencies_operator_tests.rs` (313) and `resource_upload_archive_operator_tests.rs` (1063) not read.
- No build/test/cargo/daemon execution; all findings are static reading.
