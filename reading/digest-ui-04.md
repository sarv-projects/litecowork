# Digest ui-04 — Operator shell, Tauri native bridge, presentation stream

Files: 13 read fully + `src-tauri/src/lib.rs` ~91% / 60 listed | Lines read: ~7,600 of 16,261 listed (+6,133 in unlisted `lib.rs`) | Bytes: ~0.55 MB of 0.80 MB

**Scope deviation (declared):** `reading/plan/ui-04.txt` does **not** contain `src-tauri/src/lib.rs`, but the bucket brief requires the native command bridge. `lib.rs` (6,133 lines, the `generate_handler!` hub) was read instead of 46 listed files. See Coverage check.

## Per-file notes

### apps/litecowork-ui/src-tauri/src/lib.rs (6,133 lines) — adapter/bridge host
Not in plan; read because it is the only place commands are registered and the daemon is spawned.

**Daemon transport (the load-bearing finding).** No HTTP, no bearer token, no loopback port.
- `LocalOperatorClient` holds `UnixEndpoint` + `expected_peer_uid`, or `WindowsEndpoint`; both derived from `app.path().app_data_dir()`. `operator_ipc::require_os_local_ipc()` gates platform support.
- Unix: `endpoint.connect(|peer: PeerCredentials| peer.uid == expected_peer_uid)` — `SO_PEERCRED` checked **before** frame IO.
- Windows: `endpoint.connect()` with admission via the named pipe's protected logon-SID DACL (comment at `exchange_with_deadline`).
- One framed request per connection: `RequestFrame { protocol_version: PROTOCOL_VERSION, request_id, method, path_and_query, headers, body_length }`; request IDs `tauri-{pid}-{seq}` / `tauri-stream-{pid}-{seq}` from `OPERATOR_REQUEST_SEQUENCE`.
- Readiness gate on every call: `GET /v1/operator/readiness` must return `operator_state == "SERVING"` and `api_contract_version == 1`; optional `runtime_id`/`local_incarnation_id` pinning. Budget `OPERATOR_READINESS_PROBE_TIMEOUT = 400 ms`. Shared `tokio` runtime (`worker_threads(2)`), `InFlightBodyBudget::new(DEFAULT_MAX_IN_FLIGHT_BODY_BYTES)`.
- Request bodies bounded by `BoundedJsonWriter` (`MAX_BODY_BYTES`); responses by `read_bounded_response` (1 MiB hard cap, `take(1024*1024+1)`).
- Comment of record: "No endpoint or handle crosses into the WebView." Verified true for this client — `LocalOperatorClient` is never serialized; commands return `*View` structs only.

**Sidecar spawn (no Tauri sidecar/bundle API).** `start_runtime` → `start_local_runtime`:
- `daemon_executable()` order: debug-only `LITECOWORKD_PATH` → `resource_dir()/binaries/litecoworkd[.exe]` → `resource_dir()/litecoworkd` → `current_exe().parent()/litecoworkd` → (debug only) every `PATH` entry. `is_file()` only — no signature/permission check.
- Non-Linux: `Command::new(daemon).arg("run").arg("--data-dir").arg(state_dir).stdin(null).stdout(null).stderr(null).spawn()`.
- Linux: **no spawn**; `systemd_user_service::start(&config_dir, &daemon, &state_dir, deadline)`.
- Startup deadline `RUNTIME_STARTUP_DEADLINE = 5 s`, 50 ms poll; child handle deliberately dropped after readiness ("Runtime has its own single-instance lock"); `child_exited` path tolerates another process winning the single-instance race.
- `get_runtime_status` on Linux: `systemd_user_service::verify_active` failure forces `state = "DEGRADED"` + blocker `LINUX_SYSTEMD_SERVICE_UNVERIFIED`, even when the Operator API answers. Good separation of API readiness vs containment readiness.
- Status probe shells out: `Command::new(daemon).arg("status").arg("--data-dir")`, stdout piped into a named reader thread, `MAX_RUNTIME_STATUS_BYTES = 1 MiB`, `STATUS_COMMAND_PROBE_TIMEOUT = 400 ms`, kill+wait on deadline.

**Error mapping (single funnel).** `read_bounded_response` decodes `{code, message, retryable, correlation_id, details}` and returns `"{message} [{CODE}; retryable={bool}; reference={correlation_id}]"`, but only if `code` matches `^[A-Z0-9_]{1,64}$` and `correlation_id` is ≤128 graphic ASCII; otherwise the bare message. `details` is dropped ("may contain internal context; keep them native-only") — sole exception `WORKSPACE_NOT_QUIESCENT`, where `workspace_archive_blocker_summaries()` maps 17 known blocker `kind`s to human labels + counts. Non-JSON errors become `"Local Operator API returned HTTP {status}"`. `operator_error_code()` later re-parses the code out of the bracketed suffix — a lossy string round-trip used by `workspace_root_scan_operator_error` / `workspace_policy_error`.

**Registered commands: 105** (`tauri::generate_handler!` at 5374). Owned here (36): `get_runtime_status`, `start_local_runtime`, `list_workspaces`, `create_workspace`, `update_workspace_policy`, `archive_workspace`, `set_workspace_default_agent_binding`, `list_workspace_roots`, `add_workspace_root`, `scan_workspace_root`, `revoke_workspace_root`, `pause_workspace_root`, `resume_workspace_root`, `list_workspace_instructions`, `create_workspace_instruction_revision`, `list_agent_installations`, `list_agent_profiles`, `probe_agent_profile`, `list_agent_bindings`, `create_agent_binding`, `enable_agent_binding`, `disable_agent_binding`, `list_workspace_runtime_bindings`, `enroll_local_runtime`, `list_tasks`, `create_task`, `get_task`, `list_task_spec_revisions`, `get_task_planning_readiness`, `revise_task_spec`, `get_resource_detail`, `set_context_document_status`, `list_resource_revisions`, `create_resource_revision_upload`, `commit_resource_revision_upload`, `import_resource`, `create_resource_upload`, `get_resource_upload`, `upload_resource_chunk`, `commit_resource_upload`, `list_resources`, `search_resources`, `rebuild_resource_text_index`, `preview_resource_text`, `preview_resource_text_with_provenance`.

Exact signatures (representative, camelCase keys cross the IPC):
`get_runtime_status() -> RuntimeStatus`, `start_local_runtime() -> RuntimeStatus`
`list_workspaces() -> Vec<WorkspaceView>`, `create_workspace(name, request_id) -> WorkspaceView`
`update_workspace_policy(workspace_id, replication_policy, expected_version, request_id)`
`archive_workspace(workspace_id, expected_version, request_id)`
`set_workspace_default_agent_binding(workspace_id, agent_binding_id: Option<String>, expected_version, request_id)`
`list_workspace_roots(workspace_id, cursor: Option<String>) -> WorkspaceRootPageView`
`add_workspace_root(workspace_id, expected_workspace_version, watch_policy, replication_policy, request_id) -> Option<WorkspaceRootView>` (None = user cancelled picker)
`scan_workspace_root(workspace_id, workspace_root_id, expected_version, request_id) -> WorkspaceRootScanView`
`revoke_workspace_root / pause_workspace_root / resume_workspace_root(workspace_id, workspace_root_id, expected_version, request_id)`
`create_task(workspace_id, objective, preferred_lead_agent_binding_id, coworker_id, expected_coworker_version, input_refs: Vec<PinnedResourceRefInput>, request_id) -> TaskDetailView`
`get_task(workspace_id, task_id)`, `list_task_spec_revisions(workspace_id, task_id)`, `get_task_planning_readiness(workspace_id, task_id, expected_task_version)`, `revise_task_spec(workspace_id, task_id, expected_task_version, parent_revision, objective, request_id)`
`probe_agent_profile(workspace_id, provider_key)` — `provider_key` allowlist is exactly `CODEX | OPENCODE`.
`create_resource_upload(workspace_id, display_name, media_type, size_bytes, expected_digest, folder_relative_path, context_document, request_id)`
`upload_resource_chunk(workspace_id, upload_id, chunk_index, content_range, chunk_sha256, content_base64, request_id)`
`import_resource(workspace_id, display_name, media_type, content_base64, request_id)`
All `async fn`, all wrapped in `tauri::async_runtime::spawn_blocking` (blocking framed IPC on a worker thread), all `-> Result<T, String>` (no structured error type — codes are re-encoded into the message string).

**Daemon routes touched (exact paths).** `/v1/operator/readiness`; `/v1/workspaces`, `/v1/workspaces/{id}`, `/{id}/archive`, `/{id}/default-agent-binding`, `/{id}/instructions/revisions`, `/{id}/runtime-bindings/current-local`, `/{id}/runtime-bindings/local-enrollment`; `/v1/workspace-roots`, `/{id}/{scan|revoke|pause|resume}`; `operator_ipc::PRIVATE_LOCAL_ROOT_SELECTION_PATH`; `/v1/tasks`, `/{id}`, `/{id}/spec-revisions`, `/{id}/plan-revisions`, `/{id}/steps`, `/{id}/planning-readiness`; `/v1/agent-installations`, `/v1/agent-profiles`, `/v1/agent-profiles/probe`, `/v1/agent-bindings`, `/{id}/{enable|disable}`; `/v1/resources`, `/v1/resources/quick-import`, `/v1/resources/uploads`, `/v1/resources/uploads/{id}`, `/uploads/{id}/chunks/{i}`, `/uploads/{id}/commit`, `/resources/{id}`, `/revisions`, `/revision-uploads`, `/resources/{id}/context-document/status`, `/resources/{id}/context-document/deletion`; `/v1/stream`; `/v1/needs-you`; `/v1/delegation-profiles`, `/{id}/revisions`, `/{id}/performance?task_category=`.

**Invariants actually enforced in code (verified, not claimed):**
- Idempotency: `valid_idempotency_key` = non-empty, ≤200 bytes, all `is_ascii_graphic()`. Every mutating command requires `Idempotency-Key` + `If-Match` (version).
- Response echo verification before returning: `create_task` re-checks `workspace_id`, `origin_coworker_id`, `current_spec_revision == 1`, and *exact* `input_refs` equality; `revise_task_spec` requires `revision == parent_revision + 1`; `pause/resume` require `version == expected_version + 1` and the exact status transition; `revoke` requires `status == "REVOKED"`; `set_context_document_status` requires `version + 1` and echoed status; `get_task` builds plan/step projections and rejects duplicate `logical_key`, mismatched title/objective, or a plan whose `task_spec_revision > current_spec_revision`.
- `add_workspace_root` HMAC: `derive_workspace_root_add_idempotency_key` = HMAC-SHA256 over domain tag `litecowork.workspace-root.add.v1`, request seed, workspace, version, watch/replication policy and **raw path bytes**, length-prefixed; key from `getrandom::fill` in a `OnceLock`. Path is sent base64url only; test asserts the key is 64 hex chars and contains no path substring.
- `create_resource_upload` restricts `context_document` to exactly `{kind: "WORKSPACE_NOTES", owner_ref: {kind: "WORKSPACE", workspace_id}}` and forbids it combined with `folder_import`.
- `upload_resource_chunk` validates `Content-Range` arithmetic against the decoded body length (`end - start + 1 == body_size`, `total ≤ 100 MiB`), 1 B–4 MiB per chunk, lowercase hex SHA-256.
- `set_context_document_status` / `begin_context_document_deletion` require `Cache-Control: no-store` on the response (`ensure_no_store`) — the deletion status decoder also enforces an exact 6-field object and a hand-written RFC3339 validator.
- `TaskPlanningReadinessView::try_from` **rejects** any daemon that claims `dispatch_available/planning_started/agent_session_started/plan_created = true` — the bridge can only report blocked readiness. Strong "unknown is not success" enforcement.

**On-disk state written by the bridge itself (only one):** see `conversation_create_journal.rs`. Plus two writer commands registered but not in this bucket: `artifact_bridge::artifact_save_as`, `resource_save_bridge::resource_save_as` (native save dialogs) — flagged, unread here.

**Tests (in-file, `#[cfg(test)]`):** `runtime_status_bridge_tests` (camelCase payload incl. `processRunning`/`operatorReady`/`daemonAvailable`), `workspace_root_add_idempotency_tests`, `workspace_instruction_request_tests` (empty parent list OK, `[0]` and 257 parents rejected), `workspace_archive_request_tests` (rejects `/`, `?`, `#` and unknown blocker kinds), `workspace_policy_request_tests` (+`operator_error_code` parsing), `bounded_json_tests`, `presentation_stream_subscription_tests`, `stream_setup_cancellation_tests` (3 cancellation races, incl. drop of in-flight connection), `resource_search_provenance_tests`, `resource_preview_provenance_tests`, `task_planning_readiness_tests`, `idempotency_key_validation_tests`.

### apps/litecowork-ui/src-tauri/src/conversation_create_journal.rs (879) — storage/adapter
The only durable state the bridge owns. `ConversationCreateJournal::new(app_data_dir)`, dir `conversation-create-recovery/`, files `pending.v1.json` + `pending.v1.lock`. `list / record / acknowledge`; `PendingConversationCreate { workspace_id, title: Option<String>, request_id }`.
Guards: process `Mutex` + `fs2` `lock_exclusive` on the lock file; `symlink_metadata` everywhere (symlink → fail closed); Unix `0700` dir / `0600` files, `mode & 0o077 == 0`, uid equal to app-data dir, `nlink == 1`, `dev`/`ino` re-check after open; atomic temp (`create_new`, `sync_all`) + rename, Windows via `windows_local_security::replace_file_write_through`, Unix `sync_directory`.
Bounds: `MAX_PENDING_CREATES = 64`, `MAX_JOURNAL_BYTES = 128 KiB`, title ≤160 chars, workspace ≤256, request ≤200; `version != 1` or unknown fields → reject **without** rewriting bytes; duplicate `(workspace_id, title)` rejected so an ambiguous create cannot be silently re-issued; same `request_id` + identical body is idempotent, different body errors. `acknowledge` is idempotent and scoped to `(workspace_id, request_id)`.
12 tests incl. reopen/replay, corruption & oversize preservation, symlinked journal and lock paths, `0o500` directory write failure preserving bytes, independent-handle lock exclusion.
Note: `deserialize_required_title` is a no-op wrapper around `Option::deserialize` — it does **not** make `title` required (the `deny_unknown_fields` + missing-field test still rejects because `title` has no `#[serde(default)]`). Misleading name, not a bug.

### apps/litecowork-ui/src-tauri/src/needs_you_bridge.rs (326) — adapter
`list_needs_you(app, workspace_id, status, cursor, limit, kind, task_id) -> NeedsYouPageView`. `status` ∈ `OPEN|RESOLVED`, `kind` ∈ `APPROVAL|USER_REQUEST|TASK_BLOCKER`, `limit` 1–200 (default 50). Route `GET {base}/needs-you`.
`canonical_item_id(kind, source_ids)` = `sha256:` over `litecowork.needs-you.item.v1\0` + u32-length-prefixed parts — the bridge **recomputes** the item id and rejects a mismatch, plus per-kind identity shape (APPROVAL requires `approval_id`+`task_id`, forbids `user_request_id`/`blocker_id`; TASK_BLOCKER requires `status == OPEN`). Duplicate ids rejected. 3 tests.

### apps/litecowork-ui/src-tauri/src/delegation_profile_bridge.rs (227) — adapter
`list_delegation_profiles`, `list_delegation_profile_revisions`, `get_delegation_profile_performance` → all return `DelegationProfileResponse { status: u16, content_type: String, body_base64: String }`.
`performance_path()` validates `profile_id` (no `/`, `\`, `.`, `..`), percent-encodes the segment, allowlists 8 `task_category` values, and hard-codes the suffix `/performance`. `Cache-Control: no-store` sent on revisions + performance.
**Divergence:** unlike every other command here, this bridge does **not** decode a typed `*View`, does **not** require `is_success()`, and hands the raw daemon body (base64) + status + content-type to the WebView. A non-2xx body therefore reaches the renderer unvalidated. 1 unit test (`performance_path_tests`).

### apps/litecowork-ui/src-tauri/src/context_document_deletion_bridge.rs (297) — adapter
`begin_context_document_deletion(app, workspace_id, resource_id, expected_version, request_id) -> ContextDocumentDeletionStatusView` (expects `202`), `get_context_document_deletion_status(app, workspace_id, resource_id)` (expects `200`). Route `POST|GET /v1/resources/{id}/context-document/deletion` with `X-Workspace-ID`, `If-Match`, `Idempotency-Key`, JSON `{}`.
Hardened decoder: exact 6-field object (`STATUS_FIELDS`), rejects unknown/private fields (`provider_locator` test), status ∈ `ACTIVE|REVOKED|DELETION_PENDING|DELETED`, `acknowledged <= required`, `sha256:` 71-char digest, hand-rolled RFC3339 validator (leap years, offsets, fractions), `DELETED` requires `acknowledged == required`, `ACTIVE/REVOKED` require empty digest and zero counts. `ensure_no_store` required. 2 tests. Admission text explicitly says "outcome is unknown. Check its status or retry the same request" — correct UNKNOWN reconciliation.

### apps/litecowork-ui/src/presentation/presentation-stream.ts (421) — UI reducer
`reducePresentationFrame(state, frame: unknown, expectedScope?) -> PresentationStreamState`. Frame union: `stream.ready`, `projection`, `stream.resync_required`, `turn.delta`, `turn.settled`, `rich.draft`. Draft events: `DRAFT_STARTED|DRAFT_FAILED|DRAFT_FINALIZING|DRAFT_PUBLISHED|BLOCK_OPENED|TEXT_APPENDED|BLOCK_CLOSED|BLOCK_REPLACED`.
Pure allowlist validation: `exactKeys` / `hasOnlyKeys` per frame and per payload; any unknown key ⇒ frame dropped (no partial apply). Bounds: `MAX_FRAME_BYTES 1 MiB`, `MAX_ACTIVE_DRAFTS 4`, `MAX_ACTIVE_TURNS 4`, `MAX_RETAINED_TURNS 8`, `MAX_TRANSIENT_TURN_BYTES 256 KiB`, `MAX_TRANSIENT_TOTAL_BYTES 512 KiB`, `MAX_TRACKED_TURN_FENCES 512`, `MAX_DRAFT_BLOCKS 200`, `MAX_OPEN_BLOCKS 32`, `MAX_DRAFT_DEPTH 8`, `MAX_DRAFT_BYTES 512 KiB`, `MAX_DRAFT_FRAMES 1000`, `MAX_REPLACEMENT_BYTES 64 KiB`.
Fencing is explicit: `retryFences` (monotonic per `[conversation,turn]`, evicts oldest beyond 512), `settledRetries`; out-of-order `sequence` ⇒ `RECONNECTING`, never concatenated. `BLOCK_REPLACED` re-serializes and re-parses (JSON round-trip) and forbids `kind` change. `DRAFT_BLOCK_KINDS` closed set of 11. Header comment: "Transport frames are untrusted and only update an ephemeral rendering projection." — matches ARCHITECTURE.md §2.
Residual: `childDepth` is recursive (bounded to 8, fine); `failDraft` retains the draft key with empty blocks.

### apps/litecowork-ui/src/discover/discover-api.ts (237) — UI API/decoder
`createDiscoverApi(workspaceId, searchTransport, itemTransport)`; types `DiscoverItem`, `DiscoverItemPage`, `DiscoverItemDetail`, `DiscoverProviderStatus`, `DiscoverApiError`. Enums: kind `APP|CAPABILITY|SKILL|AGENT`; availability `AVAILABLE|NEEDS_AUTH|UNSUPPORTED|ERROR|NOT_QUALIFIED`; compatibility `COMPATIBLE|INCOMPATIBLE|UNKNOWN`; install `NOT_INSTALLED|INSTALLED|INSTALL_PENDING|UNKNOWN`; auth `NOT_REQUIRED|REQUIRED|AUTHENTICATED|UNKNOWN`; source `LITE_SPM_PACKAGE|MCP_SERVER|LITECOWORK_LOCAL`.
`decodeDiscoverItem` forces `authoritative: false` and **throws** if the daemon claims authority; cross-checks `AVAILABLE` against incompatible/unknown install/required-or-unknown auth; digest regex `^sha256:[a-f0-9]{64}$`. Comments cite `docs/CAPABILITY-FABRIC.md`, `docs/CONNECTORS-WORKFLOWS.md`, `docs/API.md`. Transport is injected and `@tauri-apps/api` is *not* imported here (no direct IPC).

### apps/litecowork-ui/src/workspaces/workspace-policy.ts (102) — UI state machine
`WorkspacePolicyUpdateIntent/Pending/Projection`, `WorkspacePolicyRefreshOutcome = CONFIRMED|SUPERSEDED|UNCHANGED|MISSING`; `workspacePolicyUpdateRequest` (dedupes by `JSON.stringify([workspaceId, expectedVersion, policy])`, drops obsolete same-workspace intents, reuses the existing `requestId` — this is the client half of the server idempotency contract), `clear…`, `reconcile…`, `isWorkspacePolicyUpdateResponse` (requires `version === expectedVersion + 1` and policy echo), `mergeWorkspacePolicyProjection`, `mergeWorkspaceListProjection` (never downgrades a higher local version).

### apps/litecowork-ui/src/resources/zip-intake-api.ts (26) — UI decoder (stub)
`ZipIntakeReadiness` is a *closed* literal type: `capability_id: "litecowork.zip-intake"`, `status: "UNAVAILABLE"`, `resource_behavior: "OPAQUE_RESOURCE_ONLY"`, `provider_integrated: false`, `extraction_enabled: false`, `reason_code: "ISOLATED_WORKER_NOT_QUALIFIED"`. `decodeZipIntakeReadiness` rejects anything else. Honest non-qualification, no extraction path.

### apps/litecowork-ui/src/agents/agent-catalog-api.ts (70) — UI types only
`AgentCatalogApi` interface: `listInstallations`, `listRegistry({query,cursor,limit})`, `refreshRegistry`, `listProfiles(workspaceId)`, `listBindings`, `probeCodex`, `probeOpenCode`, `createBinding`, `enableBinding`, `disableBinding`. Types `AgentDistribution`, `AgentRegistryEntry`, `AgentRegistryPage` (`stale`), `AgentRegistrySnapshot`, `AgentProviderKey = "CODEX" | "OPENCODE"` — matches the `probe_agent_profile` allowlist. No invoke here.

### apps/litecowork-ui/src/agents/desktop-agent-control-api.ts (17) — adapter
`invoke<unknown>("get_agent_control_descriptor", { workspaceId, profileId })` / `("refresh_agent_control_descriptor", …)`, result piped through `parseAgentControlDescriptor`. Exact evidence of camelCase arg keys crossing IPC.

### apps/litecowork-ui/src/main.tsx (10), index.html (14) — entry
`ReactDOM.createRoot(#root).render(<React.StrictMode><App/></React.StrictMode>)`, imports `./styles.css`. HTML: no CSP meta tag, `#root`, module script `/src/main.tsx`, theme-color `#f6f8fb`. No Tauri CSP declared in the HTML — CSP therefore comes from `tauri.conf.json` (not read here) — **unverified**.

### apps/litecowork-ui/tests/idempotency-key-boundary.test.ts (128) — test
Installs a fake `window.__TAURI_INTERNALS__.invoke` that records `{command, args}` and throws. Asserts 129- and 200-byte keys **do** reach native IPC with `args.requestId` intact for `goal_request`, `automation_request`, `routine_request`; 201-byte, space-containing and non-ASCII keys are rejected *before* IPC (`/request identity/i`). Also exercises `createRoutineApi` generic transport (asserts `Idempotency-Key` header forwarding). Confirms the 200-byte limit is enforced identically in TS and Rust.

## Cross-cutting observations

1. **Authority model is correct and enforced, not just documented.** The renderer never receives an endpoint, socket path, PID, or token; every command is a fixed route + validated ids, and mutating routes carry `Idempotency-Key` + `If-Match`. Daemon authority is not bypassed by any command I read — with one exception (finding 2).
2. **Raw-body pass-through is the weakest seam.** `DelegationProfileResponse` (and, by the struct shape visible in `automation_bridge.rs`, `AutomationResponse`) ship `status`/`content_type`/`body_base64` to the WebView without `is_success()` gating or typed decoding. This is the only place a daemon error body crosses the bridge unvalidated; it is a validation-bypass, not a credential leak (no handles/tokens in those projections that I observed).
3. **Error contract is stringly-typed.** `Result<T, String>` plus a formatted `"{message} [{CODE}; retryable=…; reference=…]"` and a re-parse (`operator_error_code`) is fragile; a message containing `[` shifts the split. Lower severity than it looks (response text is daemon-controlled and validated), but it is a parallel error encoding next to `docs/schemas/`.
4. **Verified-vs-reported is unusually rigorous.** Response echo checks (`version == expected + 1`, exact `input_refs`, plan/step identity) and the readiness decoder that *refuses* any `planning_started=true` claim are real "reported ≠ committed" defenses.
5. **The bridge owns exactly one durable artifact** — `conversation-create-recovery/pending.v1.json` — with lockdown-grade file handling. Everything else is daemon-owned, which matches ARCHITECTURE.md ownership.
6. **Sidecar discovery is path-based only** (`is_file()`), no signature/hash check; `LITECOWORKD_PATH` is debug-only, but a writable `resource_dir()/binaries` or `PATH` (debug) entry would be trusted. Mitigated by the peer-UID/DACL handshake, which would refuse a foreign daemon.
7. **No CSP visible in `index.html`** while `invoke` is exposed to the WebView; the CSP must be asserted from `tauri.conf.json` before this is called qualified.
8. `import_resource` accepts up to ~14 MiB base64 from the renderer and forwards it in one frame — the largest single IPC body in the surface, and `media_type` is forwarded without an allowlist.

## Coverage check
- **Not read (46 of 60):** all remaining listed TS/TSX/CSS/tests — notably `ArtifactWorkbench.tsx`, `WorkbenchPanel.tsx`, `RoutinesPage.tsx`, `DiscoverPage.tsx`, `UserRequestDetail.tsx`, `TaskSpecRevisionHistory.tsx`, `SkillProposalsPage.tsx`, `styles.css` (1,241), `automations-page.css`, `discover-page.css`, `conversations-page.css`, `coworker-settings.css`, `check-rendered-ui.mjs` (1,913 / 177 KB), `tests/browser/harness.tsx` (1,824), `build-daemon-sidecar.mjs`, `systemd_user_service.rs` (662), `delegation_profile_bridge` peers, and the 9 remaining tests.
- **Read partially:** `lib.rs` lines 3701–4223 (`search_resources`, `rebuild_resource_text_index`, `preview_resource_text`, `preview_resource_text_with_provenance` bodies) — names/paths known from registration and constants (`MAX_RESOURCE_INDEX_MATCHES = 32`, `MAX_INDEXED_RESOURCE_BYTES = 1_048_576`, `valid_resource_sha256`), bodies not read.
- **Why:** bucket brief prioritised the native bridge; `lib.rs` was outside the plan yet mandatory for it, and per-turn deadline forced a stop. Nothing above is inferred from filenames alone; unverified items are marked.
