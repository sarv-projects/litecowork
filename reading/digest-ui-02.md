# Digest ui-02 — `apps/litecowork-ui/` (Operator shell: reasoning/workbench, settings, shared components)

Files: 30 read / 60 listed | Lines read: ~10,600 of 15,083 | Bytes: ~560 KB of 803 KB
Status: **partial** — a hard deadline cut the remaining 30 files (mostly `tests/`, `automations/`, `artifacts/`, `coworkers/CoworkerSettings.tsx`, `presentation/TaskPresentationPanel.tsx`). See Coverage check.

## Per-file notes

### apps/litecowork-ui/README.md (77)
- Owner doc for E02-S01/E02-S03. Declares partial status explicitly: provider-backed Conversation send, native Task/Attempt execution, Coworker-owned Conversation association, StandingResponsibilities, trigger hosting, Browser/Computer/Terminal Workbench, ZIP extraction, folder watching all **not qualified**.
- Key sentence for the invariant: "A registered persistent folder is therefore a saved scope, not proof that its contents are being watched or indexed. The shell shows blockers/empty states rather than fabricated work."
- Version authorities named: `package.json`, `.node-version`, `pnpm-workspace.yaml`, `rust-toolchain.toml`. Node 24.21.x / pnpm 12.10.1 / Rust 1.98.1.
- Unknown identity: "the `implementation/CURRENT-RUN.md`" referenced — path does not match root `CURRENT-RUN.md`; unverified link.

### apps/litecowork-ui/package.json (37)
- `@litecowork/desktop` 0.1.0, private, ESM. Deps only `@tauri-apps/api`, `react`, `react-dom` (React 19.1). Dev: `openapi-typescript 7.13.0`, `typescript ~5.8.3`, `vite ^6.3.5`.
- Scripts: `build` = `check:node && api:check && tsc --noEmit && vite build`; `test` = `node --experimental-transform-types --test --test-concurrency=1 "tests/*.test.ts" "tests/*.test.mjs"`; `test:browser` = `scripts/check-rendered-ui.mjs`; `tauri` = `scripts/run-tauri.mjs`.
- **Divergence:** `engines.node` = `">=22.20.0"` but `pnpm-workspace.yaml` sets `nodeVersion: 24.21.0` + `engineStrict: true` and README says 24.21.x. Three authorities, two answers.

### pnpm-workspace.yaml (4)
- `nodeVersion: 24.21.0`, `engineStrict: true`, `allowBuilds: {'esbuild@0.25.12': true}` only.

### src-tauri/Cargo.toml (40)
- `litecowork-ui`, lib `litecowork_ui_lib` (staticlib/cdylib/rlib). `unsafe_code = "forbid"` workspace lint.
- Deps: `operator-ipc` (path), `tauri 2`, `rfd = "=0.15.4"` (xdg-portal), `sha2`, `hmac`, `getrandom 0.3`, `fs2`, base64. Windows: `windows-local-security`. Linux: `rustix` process feature.

### src-tauri/src/main.rs (5)
- `windows_subsystem = "windows"` in release; delegates to `litecowork_ui_lib::run()`.

### src-tauri/tauri.bundle.conf.json (5)
- `bundle.externalBin = ["binaries/litecoworkd"]` — the daemon ships as a sidecar binary.

### src-tauri/src/agent_quota_bridge.rs (119)
- IPC `get_agent_quota_observation(workspace_id, agent_binding_id) -> Result<Value,String>`. GET `/v1/agent-bindings/{id}/quota-observation`, `Cache-Control: no-store`.
- `valid_id`: non-empty, ≤256, no control chars, not `.`/`..`, no `/`/`\`; percent-encodes the path segment (`encode_path_segment`).
- `decode_observation` accepts **explicit JSON `null`** and otherwise requires `agent_binding_id` to equal the requested binding → rejects cross-binding data. Tests assert `state:"UNKNOWN"` passes through untouched.
- Consequence: UNKNOWN is representable end-to-end; the UI cannot be fed a fabricated number by this route.

### src-tauri/src/coworker_bridge.rs (207)
- Single narrow multiplexer IPC `coworker_request(...)` → `CoworkerResponse{status, contentType, bodyBase64}`; operations `list|get|presence|get_revision|create|revise|status|primary`. WebView cannot pick URL/method/header.
- Routes: `/v1/coworkers`, `/v1/coworkers/{id}`, `/presence`, `/revisions/{n}`, POST `/v1/coworkers`, POST `/…/revisions`, `/…/status`, `/v1/workspaces/{id}/primary-coworker`.
- Status whitelist `ACTIVE|PAUSED|ARCHIVED`. Idempotency-Key required on every mutation; `If-Match: "{version}"` (quoted).
- Rejects irrelevant fields per operation (e.g. `list` refuses `profile_id`/`revision`/`name`). Non-2xx bodies preserved so `CoworkerApiError` keeps the Operator code.

### src-tauri/src/runtime_lifecycle_bridge.rs (282)
- IPC `get_runtime_lifecycle(runtime_id, incarnation_id)` → `RuntimeLifecycleView{runtimeId, startupPolicy, incarnation, availability, blockers}`. GET `/v1/runtime-lifecycle` via `validated_operator_connection(app, Some(runtime), Some(incarnation))`.
- Strict `deny_unknown_fields`; enum whitelists for `startup_policy` (MANUAL|LOGIN_BACKGROUND|ALWAYS_ON_SERVICE), `availability` (PAIRING…REVOKED), `recovery_state` (STARTING…STOPPED), blocker `code` (DEPENDENCY_UNAVAILABLE|RUNTIME_UNAVAILABLE). Rejects `os_boot_id`, `local_principal_id`.
- `valid_timestamp` = ≤128 bytes, has `T`, ends `Z`/contains `±`, no control chars — lexical only, not a parse.
- Test asserts serialized view has **no** `dependentTasks` key — deliberately no derived task numbers.

### src-tauri/src/resource_location_bridge.rs (144)
- IPC `list_resource_locations(workspace_id, resource_id)`. URL built as `workspace_url.trim_end_matches("/workspaces")` + `/resources/{id}/locations` — **string surgery on a URL to re-derive the base**; brittle coupling to the `validated_operator_connection` URL shape.
- Validates `availability` (AVAILABLE|OFFLINE|PLACEHOLDER|REVOKED|UNKNOWN|UNAVAILABLE), `freshness` (CURRENT|STALE|UNKNOWN|UNAVAILABLE), and `observed_digest` exactly `sha256:` + 64 lowercase hex. Rejects `locator_ref_id` (private) and `LIVE_PROBED`. Note the view serializes **snake_case** (no `rename_all`), unlike sibling bridges.

### src-tauri/src/conversation_bridge.rs (822)
- Largest bridge. Commands: `list_conversations`, `list_pending_conversation_creates`, `acknowledge_conversation_create`, `create_conversation`, `get_conversation`, `set_conversation_agent_binding`, `submit_conversation_turn`, `retry_conversation_turn`, `cancel_conversation_turn`, `get_conversation_presentation`, `get_rich_presentation`.
- Routes: `/v1/conversations` (limit 50), `/{id}`, PATCH `/{id}/agent-binding`, POST `/{id}/turns`, POST `/{id}/turns/{turn}/retry`, `…/cancel`, GET `/{id}/presentation`, GET `/v1/rich-presentations/{id}`.
- Write-ahead journal `ConversationCreateJournal{workspace_id,title,request_id}` in `app_data_dir`: `record()` before POST; cleared only on **4xx** (`read_create_response`) or pre-POST client-init failure (`initialize_create_client`); 5xx / transport error / malformed 2xx **keep** the entry. Tests: `confirmed_client_rejection_clears_only_the_exact_pending_request`, `server_failure_and_malformed_success_preserve_the_ambiguous_request`, `local_operator_client_initialization_failure_clears_before_any_post`.
- `set_conversation_agent_binding` verifies response `version == expected+1` and that `active_agent_binding_id` matches (or is explicitly null). **Sends `If-Match: {bare version}`, unquoted** — inconsistent with every other bridge (quoted).
- `valid_rich_presentation_response` is a closed 10-key check: exact workspace/conversation/message/presentation ids, `schema_version==1`, `renderer_contract_version>=1`, `semantic_content_digest` echo, `document_digest` `sha256:[0-9a-f]{64}`, `document_size_bytes` 1..=1_048_576, `document` object. Rejects extra `agent_session_id`.
- **Gap:** `submit_conversation_turn`, `retry_conversation_turn`, `cancel_conversation_turn` return the decoded JSON **without** the workspace/conversation identity re-check that every reader applies. A misrouted response would be accepted.
- `get_conversation_presentation` validates `cursor` with `valid_id` (≤256) not `valid_cursor` (≤2048) — stricter by accident, inconsistent.

### src-tauri/src/delegation_profile_write_bridge.rs (367)
- IPC `delegation_profile_request(...)` → `DelegationProfileWriteResponse{status, contentType, bodyBase64}`; ops `list|create|revise|duplicate|status`. Plus wrappers `create_delegation_profile`, `revise_delegation_profile`, `duplicate_delegation_profile`, `disable_delegation_profile`, `archive_delegation_profile`.
- Routes `/v1/delegation-profiles` (limit 200), `/{id}/revisions`, `/{id}/duplicate`, `/{id}/status`. Status whitelist `ENABLED|DISABLED|ARCHIVED`. `If-Match` quoted. Doc comment: "the daemon remains responsible for rejecting unsafe enablement"; no model override is expressible from the WebView.

### src-tauri/src/resource_save_bridge.rs (363)
- IPC `resource_save_as(workspace, resource, expectedResourceRevisionId, expectedContentDigest, expectedSizeBytes, expectedMediaType)` → `{status: "SAVED"|"CANCELLED"}`. Bytes and native path never cross IPC.
- Order: GET `/v1/resources/{id}` → `validate_detail` (workspace match, `kind=="FILE"`, `provider_instance_id=="litecowork.local-upload"`, `current_revision_id` match, ContextDocument status ACTIVE; REVOKED/DELETION_PENDING/DELETED/unknown each have distinct copy) → `verify_current_revision` paging `/v1/resources/{id}/revisions` (≤100 pages × 100, cursor dedupe via `HashSet`, requires `is_head==true` and digest/size/media type equality) → `rfd` save dialog → GET `/v1/resources/{id}/content?revision_id=…`, compares `x-resource-media-type`, `content_length`, exact byte count → `write_export_atomically`.
- Cap 10 MiB (`artifact_bridge::MAX_LOCAL_SAVE_BYTES`). **Inference risk:** `content_error` maps HTTP 500 to "failed integrity verification" — a 500 is presented as a specific cause the client did not observe.

### src-tauri/src/workspace_instruction_bridge_tests.rs (420)
- Integration test harness over the **real** `LocalOperatorClient` + a same-UID `operator_ipc::unix::UnixEndpoint` fixture server (`PrivateDirectory` at mode 0o700, `peer.uid == expected_uid`).
- Asserts: `GET /v1/operator/readiness` first; `…/instructions/revisions?limit=200` paging with URL-encoded cursor `after / page?` → `after+%2F+page%3F`; `X-Workspace-ID` present; POST create sends `If-Match: 3` (**bare, unquoted**) + `Idempotency-Key` + body `{parent_revisions, content_ref{workspace_id,resource_id,revision_id}, content_digest}`; `STALE_WORKSPACE_VERSION` surfaces as the exact error string; workspace id encoded to `workspace%3Fsection%3Dnotes%26extra%3D1%23latest`; ids `team/workspace`, `team\workspace`, `.`, `..` rejected.
- **`use std::os::unix::fs::{DirBuilderExt, MetadataExt}` and `operator_ipc::unix::UnixEndpoint` → this module cannot compile on Windows.** Matches the README's Windows-qualification gap.

### src/App.tsx (4,499) — the whole Operator shell
- **Structure:** one module holding routing + `SidebarNavIcon`, `navigation`, `HomePage`, `TaskListPanel`, `TaskDetailPanel`, `SettingsPageProps`, `profileHasFreshAdmissionOffer`, `localServiceCopy`, `SettingsPage`, `PlaceholderPage`, `LibraryPage`, `formatBytes`, `supportsTextPreview`, plus ~60 `useState`/`useRef` hooks in `App()`. Domain modules exist for Conversations/Work/Coworkers/Goals/Notifications/Routines/Automations/Discover/Settings — yet Work/Settings/Library/Home render inline here. A separate `SettingsView` is imported but the inline `SettingsPage` is what `page === "Settings"` renders (the import is unused for routing).
- **Pages:** `Home | Conversations | Work | Library | Discover | Needs You | Ideas | Coworkers | Goals | Notifications | Routines | Automations | Settings`. `navigation[]` carries `displayLabel` + `group` (WORKSPACE/RESOURCES/AUTOMATION/MONITORING/SYSTEM) but the rendered sidebar uses a *different* hand-built list (Needs You, Work, Coworkers, "Automations & Routines", Library, Discover) — `displayLabel`/`group`/`Ideas`/`Goals`/`Notifications` are dead config.
- **IPC/typed commands actually invoked** (`invoke<T>(name, args)`): `get_runtime_status`, `start_local_runtime`, `get_runtime_lifecycle`, `list_workspaces`, `create_workspace`, `update_workspace_policy`, `archive_workspace`, `set_workspace_default_agent_binding`, `list_agent_installations`, `list_agent_profiles`, `list_agent_bindings`, `create_agent_binding`, `enable_agent_binding`, `disable_agent_binding`, `list_workspace_runtime_bindings`, `enroll_local_runtime`, `list_workspace_instructions`, `create_workspace_instruction_revision`, `preview_resource_text`, `preview_resource_text_with_provenance`, `import_resource`, `list_resources`, `create_resource_upload`, `get_resource_upload`, `upload_resource_chunk`, `commit_resource_upload`, `rebuild_resource_text_index`, `search_resources`, `resource_save_as`, `add_workspace_root`, `list_workspace_roots`, `pause_workspace_root`, `resume_workspace_root`, `revoke_workspace_root`, `scan_workspace_root`, `search_workspace_root_observations`, `list_tasks`, `get_task`, `create_task`, `revise_task_spec`, `get_task_planning_readiness`.
- **Idempotency pattern (strong):** per-identity requestId maps held in refs — `pendingTaskSave`, `pendingAgentBindingMutations`, `pendingWorkspacePolicyRequests`, `pendingInstructions` (per Workspace), `rootStatusRequestKeys`, `resourceIndexRequestKeys`, upload resume in `localStorage` key `litecowork.resourceUploadResume.v1` keyed by `[workspaceId, relativePath, size, lastModified]` and re-verified by chunk hashes before resume. Instruction save derives `requestId = ${pending.resourceRequestId}-instructions` for the backing Resource import.
- **Idempotency gaps:** `enrollLocalRuntime()`, `setWorkspaceDefaultAgent()` call `crypto.randomUUID()` inline with **no retained identity** — a lost response makes the retry a new request. `archiveWorkspace` keeps `pendingWorkspaceArchive` but discards it when `workspace.version` changes.
- **Error-code matching is by string**, not typed: `error === "STALE_WORKSPACE_VERSION"` and `error.includes("STALE_WORKSPACE_VERSION")`.
- **Truthful-state machinery (matches "unknown is not success"):**
  - `resourceListFailed` → renders "Saved Resources are unavailable … Nothing is known about whether this Workspace has files, so this is not an empty result." with an explicit code comment that "A failed read proves nothing about the Workspace".
  - `TaskListPanel` keeps `loadedKey/lastLoadedAt`, labels offline/stale data "Showing the last Task list loaded {time}".
  - `TaskDetailPanel` under `COMPLETED`: "Completed means the Task reached a terminal successful state. **Outputs are not verified in this view.**"; plan steps note "Attempt details, Evidence, and verification are not shown in this Task view yet."
  - Planning readiness is read-only with 10 named `PlanningBlocker`s and "this result does not authorize or start work."
  - Version fences everywhere (`WorkspaceListRequestFence`, `AgentBindingRefreshFence`, `workspaceSelectionEpoch`, per-domain `generation` refs) and identity re-checks on every response (`saved.workspaceId !== pending.workspaceId`, `updated.version <= binding.version`, etc.).
- **Fabricated UI values found (violations):**
  1. `TaskListPanel`, compact mode: `{compact && nextCursor && <p>…Showing the 5 most recent Tasks.</p>}` — the literal **5** ignores the `limit` prop. A number the component never derived.
  2. Sidebar chat rows: selected row renders a hard-coded `<span className="chat-row-now">now</span>`; the first row gets `chat-row-dot active` by `idx === 0`. Neither is from conversation data.
  3. Sidebar empty state "No saved chats yet" renders whenever `conversationsByWorkspace[selectedWorkspaceId]` is empty — including **before** any load (it is only ever populated by `ConversationsPage.onConversationsLoaded`). That asserts an empty result the shell has not observed.
  4. Sidebar count shows the full array length while the list renders `.slice(0, 10)` — count and list disagree.
  5. `New Chat` button label `Ctrl N` with no key handler anywhere in `App.tsx`.
  6. `PlaceholderPage` renders `Nothing in {page.toLowerCase()} yet` + "View Runtime status". Reachable only via the final `else`; every page has a real branch, so it is dead-but-dangerous if a page is added to `Page` without a branch.
- **Neutral / explicitly-not-claims:** `localServiceCopy` says "Connected … Task execution is not available yet"; workspace policy shows "saved sync policy · inactive" when not LOCAL_ONLY; persistent-folder status labels each say "folder watching and indexing are not active in this build"; `resourceLocationAvailabilityLabel` for locations; instruction history says sources "do not claim which sources a Task or agent actually consumed".
- **Destructive confirmations use `window.confirm`** (`clearPendingTaskAfterReview`, `revokePersistentFolder`) — native WebView dialog, not the design-system dialog.
- No unwrap/expect, no `any`, no mock data, no `TODO`. Error handling is defensive and copy-heavy.

### src/api/idempotency-key.ts (10)
- `validIdempotencyKey`: `/^[\x21-\x7e]{1,200}$/` — visible ASCII, 1–200. Doc comment states every mutating surface must accept exactly this set. Matches the Rust `valid_idempotency_key` tests (200 ok / 201 rejected).

### src/agents/agent-binding-mutation.ts (199)
- `AgentBindingMutationOperation`, `PendingAgentBindingMutation`, `AgentBindingRefreshFence{begin,isCurrent}` (per-workspace generation + selected-workspace check), `AgentActionFeedbackByWorkspace`, `mergeAgentBindingProjection(s)` (version-monotonic; ignores equal/lower), `agentBindingMutationSuccessMessage` (explicitly refuses to claim success when refresh failed or the binding flipped again), `agentBindingMutationFailureMessage` (default-binding conflict, version race), `getOrCreateAgentBindingMutationRequest` (frozen identity), `clearAgentBindingMutationRequest`, `reconcileAgentBindingMutationRequests` (drops pending when server version > expected). Validation: safe-integer version ≥1.

### src/agents/agent-candidate-catalog.ts (57)
- `AGENT_CANDIDATES`: 15 entries (claude-code, codex-cli, opencode, cline-cli, gemini-cli, github-copilot-cli, qwen-code, kiro-cli, goose, mistral-vibe, cursor-cli, kimi-code-cli, jetbrains-junie-cli, factory-droid, auggie-cli).
- Every row hard-codes `sessionModuleStatus: "NOT_IMPLEMENTED"`, `sessionHandshakeStatus: "NOT_TESTED"`, `qualifiedSupportStatus: "NOT_QUALIFIED"`; `catalogStage` ∈ PROFILE_PROBE_IMPLEMENTED | INSTALLATION_PROBE_IMPLEMENTED | CANDIDATE_ONLY (only codex-cli/opencode get PROFILE_PROBE). Doc: "not runnable adapters or support claims." Model example of unknown-not-success.

### src/agents/agent-offer-expiry.ts (17)
- `scheduleOfferExpiryRefresh(offers, onExpiry, nowMs, schedule, cancelTimer)` → single timer at the earliest future `offerExpiresAt`, clamped to `2_147_483_647`. No timer if none. Real data only.

### src/runtime/runtime-start-readiness.ts (24)
- `canOfferLocalRuntimeStart(status)` = `daemonAvailable && !processRunning && !operatorReady`. Doc explains the DEGRADED-but-started case is deliberately not used as the gate.

### src/conversations/conversation-agent-selection.ts (62)
- `buildConversationAgentChoices(workspaceId, bindings, profiles, currentBindingId)`: filters to same workspace + `enabled` + `leadEligible`, dedupes ids, disambiguates duplicate display names with `· id`, falls back to `` `Agent · ${id}` ``, appends a non-selectable `"Unavailable selected agent"` when the current binding is no longer eligible. Sorted by label.

### src/coworkers/lead-failover-order.ts (59)
- `moveFallbackLead`, `excludeCurrentLeadFallbacks`, `isValidLeadFailoverDraft`. Mirrors the bounded policy: ≤3 fallbacks, unique, current lead excluded; `DISABLED` requires empty triggers/fallbacks and `max_lead_changes===0`; `ASK` requires fallbacks and `max_lead_changes===0`; `ALLOW_LISTED` requires `1 ≤ max_lead_changes ≤ fallbacks.length`.

### src/discover/discover-state.ts (125)
- `discoverRowView` maps `AVAILABLE|NEEDS_AUTH|UNSUPPORTED|ERROR|NOT_QUALIFIED` to exactly one label ("State unknown" for ERROR, "Package service not qualified" for NOT_QUALIFIED) and sets `disabled = availability !== "AVAILABLE"`.
- `connectActionOutcome` **always** returns `allowed:false` — either "Opening or running a Workspace skill is not available from Discover" or "Package installation is unavailable until the LiteSPM package contract is published and qualified." Discover is fully read-only by construction. Matches §6 of the repo instructions (LiteSPM contract not published).

### src/discover/desktop-discover-api.ts (52)
- `desktopDiscoverApi(workspaceId)` → `invoke("discover_request", {workspaceId, operation: "search"|"item", query, kind, cursor, itemRef})` → `operatorBridgeResponse(bridge, {signal, maxEncodedLength: 4 MiB})`.

### src/presentation/README.md (41) + task-presentation-items.ts (51)
- Typed read-only renderer layer for `docs/PRESENTATION-RUNTIME.md`: `presentation-types.ts` (strict `parsePresentationItem`, lenient `projectPresentationEntries`), `PresentationRuntime.tsx` (source order, safe fallbacks, **text rendering only — payloads never interpreted as HTML**), `presentation-stream.ts` (pure reducer for ready/snapshot/upsert/resync, drops duplicate/out-of-order, discards partial text after an unreplayable gap), `task-presentation-items.ts`.
- `partitionTaskPresentationItems(items)` → `{task, activity, artifacts, other, rejected, omittedCount}`; invalid saved items become `REJECTED` placeholders, "never silently dropped" — a direct anti-fabrication rule.
- Self-declared not-integrated: Runtime-inventory stream, transient turn/rich-draft streams, real Artifact preview/editors, browser/terminal control leases, MCP App sandboxing, packaged E2E.

### src/needs-you/task-attention-state.ts (132)
- `NeedsYouItemView{itemId,kind:APPROVAL|USER_REQUEST|TASK_BLOCKER,status:OPEN|RESOLVED,approvalId,userRequestId,taskId,blockerId,title,safeSummary,createdAt,resolvedAt}`.
- `isValidNeedsYouPage` requires every item to match the *requested* status, a strictly advancing `nextCursor`, and unique `itemId`s. `mergeNeedsYouPage` dedupes by stable id. `taskAttentionLoadReducer` scopes `loadingMore` to a generation. `nextNeedsYouMotionBaseline` only emphasises genuinely entering items and drops the baseline when the scope key changes.

### src/resources/resource-intake.ts (77)
- Caps: 100 files, 100 MiB per file, 100 MiB per selection. `isSensitiveOrGenerated` excludes `.git/.svn/.hg/node_modules/target/dist/coverage/.venv`, `.env*`, `.pem/.p12/.pfx`, `id_rsa/id_ed25519/*.key/*credentials*`, `.ssh/.aws`.
- `resourceFolderRelativePath` rejects absolute paths, trailing `/`, backslashes, >4096 chars, >128 segments, `.`/`..`, >255-byte segments, control chars, drive letters.
- `resourceIndexAction` returns `UNAVAILABLE` ("Not indexed (ZIP)") for opaque ZIPs, else `REBUILD` — no claim that ZIP contents are searchable.

### src/agents/quota-observation.css (42) / src/agents/delegation-profile-performance.css (38)
- Pure styling. `quota-observation__summary` renders a `dl` of label/value pairs; `worker-performance__confidence` has `is-high`/`is-medium` tiers. Class names imply quota + worker-performance panels with confidence bands — the components that fill them are **not in this bucket**, so whether they show real or invented numbers is **unverified**.

## Cross-cutting observations

- **Implemented architecture:** Rust Tauri bridges are genuinely narrow (fixed route + method per typed operation), validate ids, bound response sizes (1 MiB coworker/profile, 4 MiB conversation, 1 MiB metadata, 10 MiB Save As), and re-verify response identity before returning. Conversation creation is write-ahead journalled with correct 4xx-vs-5xx semantics. This is above-average boundary discipline and consistent with ARCHITECTURE.md's "provider events are untrusted until normalized/correlated" and "unknown is not success".
- **Divergence from §7 layout intent:** `App.tsx` is a 4,499-line god module; domain folders exist but Work/Settings/Library/Home are implemented inline while an unused `SettingsView` import remains. `navigation[]` config is dead relative to the rendered sidebar.
- **Two ETag conventions coexist:** conversation bridge & instruction bridge send bare `If-Match: 3`; coworker, delegation-profile bridges send `If-Match: "3"`. Both are asserted by their own tests. One of them is wrong against a single daemon parser — **verified difference, inferred defect**.
- **Windows gap is structural, not incidental:** `workspace_instruction_bridge_tests.rs` imports `std::os::unix`, and README names Windows IPC qualification as unavailable.
- **Three Node-version authorities disagree** (`engines` vs `pnpm-workspace.yaml` vs README).
- **`resource_location_bridge` derives its URL by stripping a `/workspaces` suffix** — the only bridge that re-slices a URL instead of using the client's base.
- **Fabrication audit (the requested check):** no token counts, no cost figures, and no "verified" badges exist anywhere in the read surface — verification is explicitly disclaimed in `TaskDetailPanel`. The invented values are small and UI-local: "Showing the 5 most recent Tasks.", the `now` timestamp, the `active` dot, the pre-load "No saved chats yet", and the `Ctrl N` hint. All are cosmetic-to-misleading rather than evaluative, but each is a place where the UI asserts something it did not observe.
- **Unverified risk:** quota observation returns explicit `null`/`UNKNOWN` from the bridge, and `delegation-profile-performance.css` styles confidence tiers, but the consuming components are outside this bucket — cannot confirm they don't render estimated tokens/cost. Recommend a follow-up bucket on `agents/AgentCatalogSettings.tsx` and the worker-performance panel.

## Coverage check

Read fully (30): `README.md`, `package.json`, `pnpm-workspace.yaml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.bundle.conf.json`, `src-tauri/src/{main,agent_quota_bridge,coworker_bridge,runtime_lifecycle_bridge,resource_location_bridge,conversation_bridge,delegation_profile_write_bridge,resource_save_bridge,workspace_instruction_bridge_tests}.rs`, `src/App.tsx` (~99%; a ~10-line span of the HomePage composer tail around line 3040 was clipped by a page boundary), `src/api/idempotency-key.ts`, `src/agents/{agent-binding-mutation.ts,agent-candidate-catalog.ts,agent-offer-expiry.ts,quota-observation.css,delegation-profile-performance.css}`, `src/conversations/conversation-agent-selection.ts`, `src/coworkers/lead-failover-order.ts`, `src/discover/{discover-state.ts,desktop-discover-api.ts}`, `src/needs-you/task-attention-state.ts`, `src/presentation/{README.md,task-presentation-items.ts}`, `src/resources/resource-intake.ts`, `src/runtime/runtime-start-readiness.ts`.

**Not read (30)** — deadline, not judgement: `src/artifacts/{StructuredTextPreview.tsx,desktop-artifact-api.ts}`; `src/automations/{AutomationEditor.tsx,AutomationsPage.tsx,automation-starters.ts,desktop-automation-api.ts}`; `src/coworkers/{CoworkerSettings.tsx,desktop-delegation-profile-api.ts}`; `src/presentation/{TaskPresentationPanel.tsx,task-presentation.css}`; `src/resources/{ResourceSourceReferences.tsx,library-navigation.tsx,resource-index-rebuild.ts}`; `src/suggestions/SuggestionsPage.tsx`; all 15 files in `tests/` (`agent-candidate-catalog.test.mjs`, `agent-catalog-settings.test.mjs`, `agent-control-descriptor-panel.test.mjs`, `agent-quota-observation.test.mjs`, `approval-state.test.ts`, `artifact-download-name.test.ts`, `artifact-save-as.test.ts`, `conversation-view.test.ts`, `e08-surface-motion-guard.test.ts`, `knowledge-placement.test.ts`, `presentation-stream.test.ts`, `resource-index-rebuild.test.ts`, `resource-search-pagination.test.mjs`, `workspace-policy.test.ts`, `workspace-root-observations-rendered.test.mjs`, `workspace-root-scan.test.ts`). Because `tests/agent-quota-observation.test.mjs` was unread, the "invented quota numbers" question stays **open**, not answered.
