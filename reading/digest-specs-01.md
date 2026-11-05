# Digest specs-01

Files: 122 read / 122 listed | Lines read: 5,631 | Bytes: 311 KB (plan totals; per-package line sums reconcile exactly to 5,631). No file truncated.

## Per-file notes

### specs/README.md (43)
- Canonical index of feature SDD packages; state "Feature IDs are stable; do not rename a folder after acceptance"; packages must conform to `docs/SPECIFICATION-STANDARD.md` + `.specify/`.
- Index table lists 001–011 with **every row `DRAFT`**, including 001 whose own package says ACCEPTED/LOCKED.
- Package-001 manifest table names all 11 required roles and cites `001-T01–T09`; validate via `python3 .specify/scripts/validate_feature_package.py [pkg]`.
- Delegates 002–011 to `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md`, `docs/LITECOWORK-BUILD-TASKS.md`, `docs/SPEC-TASK-TRACEABILITY.md`, `docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md` (outside bucket, unverified).
- States `docs/superpowers/plans/` are working records, "not equivalent to a complete accepted feature package".

### specs/001-conversation-session-supervisor/README.md (28)
- Declares **Status: ACCEPTED/LOCKED — normative specification locked and ready for execution**, Revision 1.0, Updated 2026-11-01. Contradicts `specs/README.md` (DRAFT) and its own DATA-MODEL/RESEARCH/WIRING (`DRAFT`, `spec_revision 0.1`).
- Owners: Conversation, Agent Runtime, Storage, Operator/API, Desktop UI, Security/Trust, Product.
- "Current truth": USER+OPEN turn commit, immutable host resolution, context-package read, STARTING/LOST recovery, storage-only atomic readiness activation exist; no qualified supervisor, no dispatch, no final AGENT write, no end-to-end Send; route fail-closed.
- No explicit dependencies section (only outcome, current truth, package links, canonical owners) — misses the repo README requirement to state dependencies.

### specs/001-conversation-session-supervisor/SPEC.md (346)
- The only fully decision-closed package: "All blocking decisions B-01 through B-10 are fully resolved and locked."
- Goals G-01…G-09; LOCKED L-01…L-14 (L-03 session scoped to exactly one turn; L-04 STARTING is reservation only, no adapter/process/network inside SQLite; L-06 handle excluded from Serde/Debug except presence boolean; L-14 final message + terminal turn atomic, "write boundary still to be promoted").
- FLEXIBLE F-01…F-05 (coordinator shape, host reuse, transport, first UI progress, module names).
- B-01…B-10 resolutions: serialized single-flight lane → `CONVERSATION_TURN_IN_PROGRESS` **HTTP 409**; Windows 11/10 `x86_64-pc-windows-msvc` + Claude Code CLI & OpenCode/Codex CLI, adapter `1.0.0`, harness-managed native pass-through auth; three-phase handshake (`AdapterVersionProbe` → `CreateSessionRequest` → SQLite CAS `activate_conversation_agent_session`); `commit_conversation_agent_response` (steps: verify `expected_turn_version`/`expected_session_version`, insert `conversation_messages` `message_kind='AGENT'`, settle `conversation_turns` `outcome=COMPLETED|FAILED|CANCELLED`, `settled_at=datetime('now')`, `agent_sessions`→`CLOSED`, append `conversation.message.added.v1` + `conversation.turn.settled.v1`, receipt to `request_dedup`); best-effort `TURN_DELTA` + REST resync; 80% budget sliding window with `[Notice: Messages 1..N omitted...]`; dual-mode materialization via `--file <path>`; Job Object/cgroup + **10-minute idle TTL + 5 s graceful wait**; `attribution_source = PROVIDER_REPORTED | ESTIMATED_LOCAL` with `tokens = max(1, utf8_byte_count / 4)`.
- §7.1 observed signatures (worktree `3052341473e964bececf7f34c9d45923848e5218`): `start_conversation_agent_session`, `mark_starting_conversation_agent_session_lost`, `list_starting_conversation_agent_sessions`, `activate_conversation_agent_session`; activation fields `expected_session_version`, `expected_turn_version`, `host_instance_id`, `native_session_ref`; generic OPEN→RUNNING via `transition_conversation_turn` "remains forbidden".
- §7.5 limits: `ConversationContextPackage` **1,024 messages / 4 MiB** (SCHEMAS.md); API detail prompt **256 KiB** declared "not the adapter request budget"; `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` named in SCHEMAS.md but absent from the **131-code** registry, hence "not a promotable error". §13 command `cargo test -p litecoworkd --test conversation_session_supervisor --locked -- --nocapture` is PROPOSED/absent; only foundation evidence `conversation_activation_commits_session_turn_and_local_host_binding_atomically` passed 2026-10-31.
- AC-001…AC-019 (EARS). O-01…O-08 open questions. Stale text: §14 says "No implementation changes are authorized while this package is DRAFT" despite ACCEPTED/LOCKED; §18 repeats "before status leaves DRAFT".

### specs/001-conversation-session-supervisor/PLAN.md (106)
- Status ACCEPTED/LOCKED, spec revision 1.0. Strategy: one qualified native chat vertical slice for one pinned OS/build/harness/adapter/auth tuple; durable transaction before UI enablement; activation and final settlement are separate atomic operations.
- Dependency DAG T1→T2/T3, T2→T6, T3→T4→T5→T6, T6→T7, T3/T4/T6/T7→T8→T9.
- Phases A–H with exit gates (A contract lock; B durable completion; C supervisor; D one adapter; E context dispatch; F durable result; G lifecycle/recovery; H qualification).
- Candidate paths: `apps/litecoworkd/src/conversation_operator.rs`, `crates/storage-core/src/conversation.rs`, `crates/storage-sqlite/src/conversations.rs`, `apps/litecowork-ui/src-tauri/src/conversation_stream_bridge.rs`; migration chain `sqlite-v1`…`sqlite-v28`, forward-only/checksummed.
- Risks table names "Duplicate prompt on timeout", "Host READY confused with session readiness", "Daemon crash between reservation and activation".
- §9 handoff still says blocked until B-01–B-05 and B-07–B-10 resolved — stale versus §5 "fully resolved".

### specs/001-conversation-session-supervisor/TASKS.md (160)
- IDs `001-T01`…`001-T09` (two-digit; **not** `001-T1`). T01 COMPLETED; T02 READY; T03/T05/T07/T08/T09 NOT STARTED; T04 BLOCKED – B-02/B-03/B-05; T06 BLOCKED – T02.
- T02 acceptance marks `[x]` "Exact request/result types … approved (`commit_conversation_agent_response` designed)" while SPEC §7.3 and DATA-MODEL §5 call the same contract PROPOSED and "blocking owner work" — direct contradiction.
- Allowed paths mirror PLAN; every task names a `cargo test` filter "to be added" (e.g. `conversation_agent_response_commit`, `conversation_session_supervisor_lifecycle`, `conversation_context_dispatch`, `conversation_session_recovery`) — none exist.
- "Dependency order" paragraph allows T02‖T03 only after T01 locks interfaces. "Completion evidence" requires revision, exact command, result, OS/harness/auth, reviewer — no rows recorded.

### specs/001-conversation-session-supervisor/DATA-MODEL.md (78)
- **Status: DRAFT** (package elsewhere ACCEPTED/LOCKED).
- Reuses canonical records: Workspace, Conversation, ConversationMessage, ConversationTurn (version/status/retry ordinal/pinned AgentBinding/current session pointer), AgentBinding, AgentSession (scope CONVERSATION, exactly one Turn), AgentHostInstance, AgentSessionHostBinding, DomainEvent, request receipt, ConversationContextPackage ("not persisted aggregate"), RichPresentation.
- State ownership delegated: turn `OPEN → RUNNING` only after readiness; AgentSession `STARTING → ACTIVE → INTERRUPTING/CLOSING → CLOSED|LOST`; AgentHostInstance `STARTING/READY/BUSY/DEGRADED/FAILED/STOPPING/STOPPED`; "READY or BUSY is not adapter session readiness".
- Bounds: 1,024 messages / 4 MiB; **"Adapter request bound is separately negotiated; currently observed OpenCode/Codex transport is 256 KiB per request"** — contradicts SPEC §7.5/§10 and RESEARCH §5.4 which say the 256 KiB figure is the API UserRequest cap and not the adapter budget.
- Events listed incl. `conversation.turn.created.v2`, `agent.session.starting.v1`, `agent.session.started.v1`, `conversation.turn.settled.v1`, `conversation.turn.retried.v1`, `conversation.turn.resumed.v1`; "No new event is approved by this DRAFT".

### specs/001-conversation-session-supervisor/RESEARCH.md (82)
- Status DRAFT, checked 2026-10-31. Source table maps ARCHITECTURE (c7e9c05), PRODUCT, V1 release, STATE-MACHINES, SERVICES, API, STORAGE, EVENTS/SCHEMAS, AGENT-FABRIC/CONTROL, NATIVE-COMPOSER-LLD, TESTING to findings + limits.
- Worktree evidence: branch `feature/conversation-session-supervisor`, HEAD `3052341473e964bececf7f34c9d45923848e5218`, path `archives/legacy-codebase-c7e9c05/.worktrees/conversation-session-supervisor`.
- `cargo test -p storage-sqlite --locked` = **593 unit + 2 AgentRegistryCache integration** tests, passed before handle-redaction additions, not rerun after; Linux Secret Service tests deliberately ignored.
- Confirms "No caller for `activate_conversation_agent_session` … outside its storage trait/implementation/tests."
- Mismatches §5: (4) "OpenCode/Codex transport input is 256 KiB while product ConversationContextPackage allows 4 MiB" — the third conflicting statement of the 256 KiB question. R-01…R-07 open research; "No web research has been used to claim current upstream API behavior."

### specs/001-conversation-session-supervisor/WIRING.yaml (228)
- `status: "DRAFT"`, `spec_revision: "0.1"` — the wiring graph is not certified at SPEC revision 1.0.
- 12 nodes (desktop-ui, operator, conversation-service, sqlite-store, turn-coordinator, session-supervisor, invocation-runner, host-supervisor, agent-adapter, native-host, presentation-operator, tauri-bridge) and 20 edges e01–e20; e19/e20 model `ProviderInputBinding` exact-key reconciliation ("update acknowledgement is not acceptance").
- 5 transactions: `tx-turn-admission`, `tx-session-reservation`, `tx-session-activation`, `tx-user-request-response` ("INHERITED OWNER CONTRACT - runtime qualification required"), `tx-final-response` ("PROPOSED - B-04 blocking"); all `external_calls_inside: false`.
- `tx-final-response.atomic_records` says `COMPLETED ConversationTurn` only, narrower than B-04's `COMPLETED | FAILED | CANCELLED`.
- External boundaries (native-start, native-event-stream, provider-input-acceptance, native-interrupt) all "first-route value unresolved".

### specs/001-conversation-session-supervisor/contracts/README.md (10)
- Index to api-events.md, errors-recovery.md, ui-behavior.md; names canonical owners `docs/API.md`, `EVENTS.md`, `STORAGE.md`, `SCHEMAS.md`, `SERVICES.md`, `STATE-MACHINES.md`, `AGENT-CONTROL.md`, `NATIVE-COMPOSER-LLD.md`.
- States the promotion rule: a new global contract must reach owner + machine schema before being canonical.

### specs/001-conversation-session-supervisor/contracts/api-events.md (63)
- `POST /v1/conversations/{id}/turns` bounds: **at most 256 blocks, 1 MiB, 256 ResourceRefs**; `presentation_preference: AUTO | SIMPLE | RICH`; principal-scoped `Idempotency-Key`.
- Mounted behavior: settles FAILED with `CONVERSATION_SESSION_ADMISSION_UNAVAILABLE` and returns **503 `DEPENDENCY_UNAVAILABLE`**; "must not change that behavior to a 202/accepted status".
- Routes: `PATCH /v1/conversations/{id}/agent-binding` (If-Match + Idempotency-Key, future turns only), `.../turns/{turn_id}/retry`, `.../cancel`, `GET .../presentation?cursor=&limit=`, `GET .../messages?cursor=` (status unverified), `GET .../usage` target-only.
- Event reuse list matches DATA-MODEL §6 exactly. §4 restates the four-part atomic final commit; §5 transient delta identity = Workspace + Conversation + turn + retry ordinal + AgentSession + monotonic sequence; "Current local Operator does not emit `turn.delta`, `turn.settled` or `rich.draft`."

### specs/001-conversation-session-supervisor/contracts/errors-recovery.md (78)
- 10-row classification table (owner/scope denial, request conflict, pinned target unavailable, session readiness failure, input preparation failure, provider uncertain, provider terminal failure, user cancellation, persistence uncertainty, projection gap).
- Retry classes: `SAFE_LOCAL_REPLAY`, `SAFE_AFTER_RECONCILE`, `EXPLICIT_USER_RETRY`, `NEVER_AUTOMATIC`.
- 15-row failure/crash matrix from "Before submit transaction" to "During daemon restart"; each row names durable truth and recovery.
- 12 mandatory behaviors, incl. "STARTING never means running", "Only atomic activation moves ConversationTurn OPEN -> RUNNING", "Stop is a request", and the full UserRequest/MCP exact-key rule (§11).
- §6 recovery sweep: enumerate STARTING "ordered by binary AgentSessionId with exclusive cursor", bounded pages/retries per daemon tick, idempotent, "No provider prompt is redispatched by a recovery sweep." §5 forbids coining error codes here.

### specs/001-conversation-session-supervisor/contracts/ui-behavior.md (82)
- Flows F1–F7 (composer visible → edit → send → streaming → stop → failure → reload with `RESYNC_REQUIRED`); "No navigation to Task or Coworker is required for F1–F7."
- Composer eligibility states: `EMPTY, NEEDS_AGENT, AUTH_REQUIRED, ELIGIBLE, SENDING, WAITING, AMBIGUOUS, SETTLED, OFFLINE, STALE`; agent picker `LOADING/READY/AUTH_REQUIRED/STALE`; message stream `LOADING, STREAMING, COMMITTED, DEGRADED, ERROR, PARTIAL`; runtime `UNAVAILABLE/DEGRADED`.
- Command map binds reducer actions (`TurnAdmissionRequested`, `CancelRequested`, `RetryRequested`, `SelectedBindingChanged`, `UserRequestResponded`) to domain commands.
- Test IDs: `UIV1-016–022`, `UIV1-023–036`, `UIV1-049–057`, `UIV1-140–147`, `AC-001–019`, `FL01–FL05`.
- Note: §3 says B-01 policy was "reject vs queue — until locked", stale because SPEC B-01 locked rejection; §4 defers operationIds to api-events.md, which never lists any.

### specs/002-native-agent-install-auth-binding/README.md (27)
- Status DRAFT "does not authorize implementation"; owners to approve Agent Runtime, Security/Trust, Desktop UI, Product; outcome = detect agent, complete native sign-in, enabled AgentBinding with install/auth/descriptor/readiness kept separate.

### specs/002-native-agent-install-auth-binding/SPEC.md (171)
- Problem: universal chat blocked until install/auth/binding are observed separately without a LiteCowork model router. Goals 4, non-goals 4 (no universal Providers screen, no silent rewrite of `CLAUDE.md`/`AGENTS.md`/Codex config).
- LOCKED: no global provider/model catalog (AGENT-CONTROL §2.1); registry listing ≠ readiness (§2.3–2.4); secret bytes never in config/events/bindings/logs (§2.6); no silent option substitution (§2.8); LiteSPM owns package lifecycle, LiteCowork scoped use (§2.10); installed≠authenticated≠bound≠granted≠session-ready.
- Blockers B-02 (shared with 001), B-11 (additional AgentModules), B-12 (descriptor TTL and probe budget). AC-001…AC-007.
- §7a facts: field owners exactly `NATIVE_AGENT | LITECOWORK_NON_SECRET | LITECOWORK_SECRET_REF | LITESPM_CAPABILITY`; `AgentAuthDescriptor.state` = `UNKNOWN, NEEDS_AUTH, AUTHENTICATING, AUTHENTICATED, EXPIRED, DEGRADED, ERROR`; new bindings created **disabled**; `lead_eligible=false` → `AGENT_NOT_LEAD_ELIGIBLE`; no enabled binding → `AGENT_UNAVAILABLE`; enable/disable commit one ALLOW `AuditRecord`; `agent.binding.configuration.changed.v1` carries credential slot ids only; credential submit `Idempotency-Key` **1–200 printable ASCII**; `open_native_config` takes a module-known target id only.
- Routes cited: `GET /agent-registry`, `POST /agent-registry/refresh`, `GET /agent-installations`, install/update, profile refresh, control-descriptor, begin auth, credential submit, native-config open, binding list/create/configuration/enable/disable; "Several remain target-only".
- Structural defect: section headed `## 7a. Owner facts incorporated 2026-11-01` sits **after §18 and before §19**, duplicating the §7 numbering and holding the densest interface content.

### specs/002-native-agent-install-auth-binding/PLAN.md (37)
- 4 phases: resolve §5 blockers (no code) → promote machine schema in owner + `docs/schemas/` together → implement in dependency order on a recovered tree → cargo filters + desktop USER path.
- The 002–011 PLAN bodies are verbatim identical apart from title/owners; same "Risks" trio (implementing from DRAFT, treating archive CODE as qualification, collapsing states into one UI boolean) and same verification gates.

### specs/002-native-agent-install-auth-binding/TASKS.md (53)
- `002-T01`…`002-T04`; T01 BLOCKED, T02–T04 NOT STARTED; scopes: registry/lifecycle observations, binding commit separate from auth, registry UI chips.
- Every task's acceptance ends with the generated fragment **"Criteria discussed include AC-001 AC-002."** — including T03 ("AC-003, AC-004 and AC-006") and T04 ("AC-005 and AC-007"), so the sentence is copy-paste noise.
- Commands: `agent_registry_observation`, `agent_binding_auth_split` (storage-sqlite) plus the package validator for UI tasks.

### specs/002-native-agent-install-auth-binding/DATA-MODEL.md (32)
- Records: AgentRegistryEntry ("cache, not authority"), AgentInstallationObservation (Runtime-local), AgentControlDescriptor (time-bounded, non-secret), AgentBinding (Workspace-scoped durable eligibility, "not a session handle").
- Only AgentBinding carries a Workspace id; registry/observations/descriptors/lifecycle are Runtime-local. "Neither receipt means authentication success or session readiness."
- New events require same-change edits to `docs/EVENTS.md` + `docs/schemas/domain-event.schema.json`; migration "None in this revision".

### specs/002-native-agent-install-auth-binding/RESEARCH.md (27)
- Authorities `AGENT-CONTROL.md`, `AGENT-FABRIC.md`, `AGENT-ACCOUNT-ROUTE-MATRIX.md`, `ARCHITECTURE.md`; competitor pattern rejected = single connected toggle; reference desktops ZCode, DeepSeek Harness, AionUi, OpenDot.
- "Findings" is the shared 3-bullet template incl. the irrelevant "Hosted competitors often collapse connection, permission and execution into one toggle"; "Rival web pages were not re-fetched on 2026-11-01"; "No runtime was executed."

### specs/002-native-agent-install-auth-binding/WIRING.yaml (62)
- 5 nodes (ui, operator, owner, storage, native `AgentLifecycleAdapter`), 6 edges, 2 transactions (`tx-observation` → `installation_observation`; `tx-binding` → `agent_binding`), `external_calls_inside: false`, recovery `reload-last-commit-and-reconcile`.
- Generic: no per-operation contract strings, unlike 001's edge contracts.

### specs/002-native-agent-install-auth-binding/contracts/README.md (7) / api-events.md (7) / errors-recovery.md (5) / ui-behavior.md (17)
- README: 3 links + "Canonical owners stay in `docs/`."
- api-events: "Use existing … operations in docs/API.md and docs/AGENT-CONTROL.md §12"; forbids a universal `/v1/providers` catalog; "This DRAFT does not declare new event types"; error codes only via `docs/schemas/error-codes.schema.json`.
- errors-recovery: one paragraph copied from SPEC §11 (stale descriptor, expired auth, missing binary, unsupported version, cancelled login; "Retry re-probes; it does not replay secret bytes") + the shared QUALIFIED sentence.
- ui-behavior: shared header (V1-UI-SCREEN-CONTRACT, V1-SETTINGS-CONTRACT-LEDGER, SEMANTIC-SERVED-UI-MAP, V1-UI-ACTION-STATE-LEDGER) + 3 flows (separate install/auth/binding/readiness chips; model+effort only after fresh descriptor; unavailable shows repair text) + shared "Commands"/"Validation" paragraphs.

### specs/003-workspace-roots-context-ingestion/README.md (27)
- DRAFT; owners Resource, Conversation, Storage, Security/Trust, Desktop UI; outcome = attach folder as persistent WorkspaceRoot, grounded answer with citations from exact revisions, revoke fences retrieval.

### specs/003-workspace-roots-context-ingestion/SPEC.md (162)
- LOCKED: no-follow directory handles + exact OS identity (WORLD-RESOURCES); canonical paths stay Runtime-local; overlapping non-revoked roots rejected; revoked roots not reactivatable; root observation is not a capability grant; context instructions cannot grant capabilities. Blockers B-13 (ingestion formats), B-14 (watcher vs rescan). AC-001…AC-007.
- §7a bounds: metadata census **1,000 directory entries, depth 32, 1 MiB of relative locator bytes, 8-second deadline that stops traversal at 7 seconds, 2 concurrent jobs**; census "reads no content, and creates no ResourceRevision" and a partial scan does not tombstone.
- Resume of PAUSED needs AVAILABLE location + incarnation READY/DEGRADED; UNAVAILABLE restored only by identity revalidation; keyring identity loss blocks startup *before* the root transaction.
- Revocation commits REVOKED + deletes private bindings + removes selected-root replication membership in one transaction. Overlap rejects same/ancestor/descendant of ACTIVE, PAUSED, UNAVAILABLE; `SELECTED_FOLDERS` stores root ids; instruction revisions ≤ **64 KiB UTF-8**, stale write → `STALE_WORKSPACE_VERSION`.
- Error names: `RESOURCE_CONFLICT` (unpinned ref), `INTEGRITY_FAILURE` (digest mismatch), `ContextDocumentStatus ACTIVE` required for reads. Cites `listWorkspaceRoots`, `pauseWorkspaceRoot`, `resumeWorkspaceRoot`, `revokeWorkspaceRoot`, `scanWorkspaceRoot`, `searchResources`, `groundedRetrieval`; `addWorkspaceRoot`/`updateWorkspaceRoot` "target-only and have no mounted handler"; events `workspace.root.created.v1`, `workspace.root.status.changed.v1`.

### specs/003-workspace-roots-context-ingestion/PLAN.md (37)
- Identical template (4 phases, DAG bullets "T01 blocks later tasks", "External side effects never run inside the transactions declared in WIRING.yaml", "Feature 001 remains a prerequisite…"), owners line only difference.

### specs/003-workspace-roots-context-ingestion/TASKS.md (53)
- `003-T01`…`003-T04`; T01 BLOCKED; T02 admits AC-001…AC-005 though traceability assigns AC-001/002 to T01; T04 (UI) has **no AC in the traceability table**. Same generated "Criteria discussed include AC-001 AC-002." on all four. Commands: `workspace_root_lifecycle`, `context_attachment_receipts`.

### specs/003-workspace-roots-context-ingestion/DATA-MODEL.md (32)
- Records: WorkspaceRoot `ACTIVE | PAUSED | UNAVAILABLE | REVOKED`; resource revision and digest; ContextAttachment and retrieval receipt; Runtime-local private locator. Everything else is shared boilerplate (relations/lifecycle/retention/migration).

### specs/003-workspace-roots-context-ingestion/RESEARCH.md (27)
- Authorities `WORLD-RESOURCES.md`, `CONTEXT.md`, `STORAGE.md`, `API.md`, `TRUST.md`; rejects "wholesale machine scan (competitor desktop agents often index home by default)". Findings block is the same three template bullets as 002 (including the connection-toggle claim that has nothing to do with roots).

### specs/003-workspace-roots-context-ingestion/WIRING.yaml (62)
- 5 nodes (ui, operator, owner, storage, `fs` = OS directory identity), edges `e-owner-fs` (reopen) / `e-fs-owner` (identity); `tx-root` → `workspace_root` + `private_binding`; `tx-attach` → `context_attachment`.

### specs/003-workspace-roots-context-ingestion/contracts/* (7/7/5/17)
- api-events: "POST/PATCH workspace-roots that OpenAPI marks without a mounted handler stay target-only"; no new events; error codes only from SCHEMAS. errors-recovery: SPEC §11 paragraph (UNAVAILABLE/PAUSED/REVOKED distinct, keyring startup blocker) + shared QUALIFIED sentence. ui-behavior: 3 flows ("Add root uses the OS picker then an explicit confirm"; distinct repair per state; Context Used lists citations, search hits need explicit attach).

### specs/004-agent-relay/README.md (27)
- DRAFT; owners Conversation, Agent Runtime, Product; outcome = A→B→A on one Conversation without copying private transcripts or credentials.

### specs/004-agent-relay/SPEC.md (152, thinnest SPEC)
- Non-goals explicitly separate relay from Task delegation: "HOST_DELEGATED work belongs to Task Runtime (DELEGATION.md)". Blockers B-15 (portable context schema/size), B-16 (second AgentModule, depends on 002). AC-001…AC-006.
- §7a: relay is not `NATIVE_INTERNAL`, `HOST_DELEGATED`, or `CAPABILITY_EXECUTOR`; creates no `DelegationProfile`, Environment, lease, grant, `ResultEnvelope`; `enabled=false` denies a new session; `lead_eligible=false` cannot be Conversation lead; a disabled binding leaves an admitted session to ordinary settlement.
- Error codes: stale option → `AGENT_SESSION_OVERRIDE_UNSUPPORTED` or `AGENT_NATIVE_CONFIG_CHANGED`; quota → `AGENT_QUOTA_EXHAUSTED`; otherwise `AGENT_UNAVAILABLE` with no substitute. Switching binding emits no `task.lead_agent.changed.v1` and builds no `LeadHandoffPacket`.
- Cites `setConversationAgentBinding`, `submitConversationTurn`, `retryConversationTurn`; events `conversation.agent_binding.changed.v1`, `conversation.turn.created.v2`, `agent.session.starting.v1`, `agent.session.started.v1`.

### specs/004-agent-relay/PLAN.md (37)
- Same 4-phase template; "Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn."

### specs/004-agent-relay/TASKS.md (53)
- `004-T01`…`004-T04`; T01 BLOCKED; T02 accepts AC-002 while traceability assigns AC-002 to T01 (double mapping); commands `portable_context_export`, `conversation_agent_relay`.

### specs/004-agent-relay/DATA-MODEL.md (31)
- Records: `PortableContextPackage — derived, not a native transcript`; per-turn AgentBinding id on the committed message; distinct AgentSession per harness admission. Contradicts its own WIRING `tx-export` atomic record `portable_context_package`, i.e. a "derived" package treated as a transactional durable record.

### specs/004-agent-relay/RESEARCH.md (27)
- Authorities `DELEGATION.md` (to avoid collapsing relay into HOST_DELEGATED), `NATIVE-COMPOSER-LLD.md`, `FLOWS.md`, feature 001; "Reject vendor multi-agent routers that share one hidden transcript". Same template findings.

### specs/004-agent-relay/WIRING.yaml (65)
- 6 nodes incl. `harness_a`/`harness_b`; `tx-export` → `portable_context_package`, `tx-turn` → `conversation_turn`; no transaction for cross-harness session isolation.

### specs/004-agent-relay/contracts/* (7/7/5/17)
- api-events: "Reuses Conversation send/turn operations from feature 001. No new Task routes." errors-recovery: "If B is unavailable, turn admission fails and A's history remains." ui-behavior: 3 flows (picker between turns; transcript labels producer; failure on B does not auto-send to A).

### specs/005-safe-task-path/README.md (27)
- DRAFT; owners Task Runtime, Environments, Trust, Storage, Desktop UI; outcome = promote Conversation outcome into Task, accept immutable plan, one isolated Attempt edits one approved file with receipt surviving cancel/restart.

### specs/005-safe-task-path/SPEC.md (161)
- LOCKED: task creation does not imply Step/Attempt/lease/Environment; no persisted Task DRAFT; unavailable lead → `AGENT_UNAVAILABLE` with no substitute; unknown external effect reconciled before retry; grants explicit and scoped. Blockers B-17 (first Environment provider + attestation), B-18 (one edit class: replace vs patch). AC-001…AC-007.
- §7a: `If-Match` + TaskSpec head on plan acceptance; mismatches **409 `STALE_TASK_VERSION` / 409 `STALE_SPEC_REVISION`**; non-active planner → **403 `FORBIDDEN`** or 409 `CONFLICT`; `IsolationAttestation` fields must be `ENFORCED` (UNKNOWN/NOT_ENFORCED/caller-supplied fail closed); releasing a lease or destroying Environment needs `QuiescenceObservation QUIESCENT`; no Task retry command — continuation is `recover_step` with a new Attempt id; new incarnation cannot renew a lease → EXPIRED/REVOKED + Attempt `ABANDONED`, stale fence → `STALE_FENCE`; provider returns `INTEGRITY_FAILURE` on digest mismatch.
- `LeadFailoverPolicy` default: explicit Task request → selected Coworker revision → DISABLED with empty triggers/fallbacks and `max_lead_changes 0`; omission must not be treated as DISABLED when a Coworker revision supplies ASK/ALLOW_LISTED.

### specs/005-safe-task-path/PLAN.md (37)
- Template; owners line differs only.

### specs/005-safe-task-path/TASKS.md (53)
- `005-T01`…`005-T04`; T01 BLOCKED; T02 claims AC-001…AC-004 (AC-001/002 also assigned to T01); T04 (UI) has no AC row; commands `task_explicit_plan`, `task_one_edit_reconcile`.

### specs/005-safe-task-path/DATA-MODEL.md (32)
- Records: Task and TaskSpecRevision; PlanRevision and Steps; Attempt, ExecutionLease, CapabilityGrant, Effect, Evidence; idempotency receipt. Rest boilerplate.

### specs/005-safe-task-path/RESEARCH.md (27)
- Authorities `TASK-RUNTIME.md`, `ENVIRONMENTS.md`, `TRUST.md`, `DELEGATION.md`, V1 release §6; "Reject worktree-as-sandbox". Same template findings.

### specs/005-safe-task-path/WIRING.yaml (62)
- `env` = Environment provider; `tx-task` → `task` + `task_spec_revision`; `tx-attempt` → `attempt`, `lease`, `grant`, `idempotency_receipt`; note the edition prose says Attempt+lease+grant+intent+receipt commit *before* the Environment is invoked, matching SPEC AC-005.

### specs/005-safe-task-path/contracts/* (7/7/5/17)
- api-events: task create/plan accept/attempt/cancel in API.md + TASK-RUNTIME, "No new route names in this DRAFT". errors-recovery: cancel fences lease, timeout reconciles, restart does not reissue an ambiguous write. ui-behavior: "Run as Task is a separate review, not a send side effect"; plan accept precedes edit; unknown result not shown as verified.

### specs/006-golden-connection-capability-write/README.md (27)
- DRAFT; owners Connectors, Capability Fabric, Security, LiteSPM boundary, Desktop UI; outcome = one real Connection, one read, one approved write with reconciliation, revoke fences later use.

### specs/006-golden-connection-capability-write/SPEC.md (160)
- LOCKED: LiteCowork stays control plane (CONNECTORS-WORKFLOWS §1); installed/connected/assigned/granted/activated/invoked distinct; tokens stay in SecretStore; TLS on with SSRF/redirect/timeout/byte limits; package approval bound to exact plan digest, no self-approval. Blockers B-19 (golden provider + OAuth/PKCE recipe), B-20 (the one write operationId + idempotency key). AC-001…AC-007.
- §7a: `Connection.status` = `CONNECTING, CONNECTED, DEGRADED, REAUTH_REQUIRED, REVOKED, DISCONNECTED` and **"The OpenAPI Connection status enum omits REVOKED"** — a declared schema/owner mismatch to fix before implementation; `account_display_hint` is not authenticated identity.
- Loopback binds only **127.0.0.1** on OS-chosen port, `state` single-use, callback body carries no token, secrets stored before CONNECTED, failed commit purges new secrets, no plaintext fallback; first terminal callback wins; per-connection refresh lock, `invalid_grant`→REAUTH_REQUIRED else DEGRADED.
- `ConnectorActionDescriptor.operation_class` = `READ, WRITE, SEND, DELETE, PURCHASE_LIKE, ADMIN`; idempotency = `PROVIDER_KEY, CLIENT_KEY, READ_ONLY, UNKNOWN`; `CapabilityInvocation.execution_method` = `STRUCTURED_API, STRUCTURED_BROWSER, ACCESSIBILITY_BROWSER, SCREEN_COMPUTER_USE, DETERMINISTIC_LOCAL, NATIVE_AGENT_TOOL, UNKNOWN`; `AMBIGUOUS` non-terminal; CONVERSATION and TASK_PLANNING invocations read-only.
- Errors: `SENSITIVE_INPUT_UNSUPPORTED`, `PROVIDER_INPUT_UNSUPPORTED` (unknown MCP methods incl. sampling do not become a UserRequest). Egress rejects loopback/private/link-local/multicast/cloud-metadata even under a Task grant; every redirect revalidated; "A PROPOSED Effect is not a dispatch permit". Cites `listConnections`, `getConnection`, `disconnectConnection`, `createCapabilityGrant`, `revokeCapabilityGrant`, `resolveApproval`; event `connection.state.changed.v1`; "No golden-provider write operationId exists until B-20".

### specs/006-golden-connection-capability-write/PLAN.md (37)
- Template.

### specs/006-golden-connection-capability-write/TASKS.md (53)
- `006-T01`…`006-T04`; T01 BLOCKED; T02 accepts AC-001/002/006; T03 accepts AC-003/004/005/007; T04 claims the "six-way distinction" with no AC. Commands `golden_connection_custody`, `golden_capability_write`.

### specs/006-golden-connection-capability-write/DATA-MODEL.md (32)
- Records: Connection with opaque id; Secret handle ("never token bytes in projections"); CapabilityGrant, Invocation, Effect, Evidence; approval bound to plan digest.

### specs/006-golden-connection-capability-write/RESEARCH.md (27)
- Authorities `CONNECTORS-WORKFLOWS.md`, `CAPABILITY-FABRIC.md`, `NETWORK-SECURITY.md`, `LITESPM-LITECOWORK-INTEROP-LLD.md`; "Reject embedding third-party automation runtimes". Same template findings.

### specs/006-golden-connection-capability-write/WIRING.yaml (65)
- Nodes incl. `provider` (external HTTP) and `secrets` (SecretStore) with `e-owner-secrets` kind `custody`; `tx-connection` → `connection`; `tx-invoke` → `invocation`, `approval`, `idempotency_receipt`.

### specs/006-golden-connection-capability-write/contracts/* (7/7/5/17)
- api-events: "Exact LiteSPM wire names stay in the version-pinned LiteSPM contract, not this DRAFT." errors-recovery: cancelled OAuth leaves no Connection; refresh serialization; UNKNOWN write blocks retry. ui-behavior: "Revoke explains that grants die with the Connection"; write disabled until approval.

### specs/007-professional-artifacts/README.md (27)
- DRAFT; owners Artifacts, Presentation, Storage, Desktop UI; outcome = create/revise/export/reopen one deliverable with provenance and version differences.

### specs/007-professional-artifacts/SPEC.md (153)
- LOCKED: `ArtifactStore` owns bytes and versions (ARTIFACTS-EVIDENCE); presentation does not own persistence; format declared viewable/editable/exportable/unsupported; provenance links Conversation or Task. Blocker B-21 (the one format + edit/export engine). AC-001…AC-006.
- §7a: Artifact moves only TRANSIENT→SAVED by explicit promotion and SAVED→ARCHIVED; publish to ARCHIVED → `ARTIFACT_ARCHIVED`; concurrent publish → `STALE_VERSION`; `listLibrary` returns only SAVED; same key+payload replays, changed payload → **409 `CONFLICT`**.
- Local text-append route: managed `text/plain` up to **1 MiB**, pins If-Match + content version + Resource aggregate version + exact parent ResourceRevision, rejects archived Workspace/Artifact, external-resource content, HTML. Desktop Save As: one `MANAGED_BLOB` version up to **10 MiB IPC bound**, creates no Artifact/Resource revision or event, destination bytes do not return through WebView IPC.
- Evidence level `REPORTED | OBSERVED | VERIFIED`; REPORTED cannot satisfy OBSERVED/VERIFIED; Runtime append path admits only REPORTED. Interactive run states `PREVIEW_ONLY, STARTING, RUNNING, STOPPING, STOPPED, FAILED, REVOKED, INTERRUPTED`; does not increment `Artifact.version`; unproved quiescence is INTERRUPTED not STOPPED. Presentation forbids raw HTML, scriptable SVG, inline handlers, CSS, arbitrary JS; `PresentationStatus COMPLETE` ≠ Task complete.
- Cites `getArtifact`, `getArtifactTextEditHead`, `appendArtifactTextVersion`, `promoteArtifact`, `archiveArtifact`, `downloadArtifactVersion`, `listLibrary`.

### specs/007-professional-artifacts/PLAN.md (37)
- Template.

### specs/007-professional-artifacts/TASKS.md (53)
- `007-T01`…`007-T04`; T01 BLOCKED; T02 accepts AC-001/002/006; T03 AC-003/005; T04 AC-004 only. Commands `artifact_versions`, `artifact_one_format`.

### specs/007-professional-artifacts/DATA-MODEL.md (32)
- Records: Artifact identity; immutable version + content digest; provenance links; export receipt. Rest boilerplate.

### specs/007-professional-artifacts/RESEARCH.md (27)
- Authorities `ARTIFACTS-EVIDENCE.md`, `PRESENTATION-RUNTIME.md`, V1 release area 9; "Reject preview-as-editor". Same template findings.

### specs/007-professional-artifacts/WIRING.yaml (62)
- `editor` = format editor engine; `tx-version` → `artifact_version` + `provenance`; `tx-export` → `export_receipt`; `e-editor-owner` kind `bytes`.

### specs/007-professional-artifacts/contracts/* (7/7/5/17)
- api-events: "No new global schema until B-21 promotes a format id." errors-recovery: crash during save rolls back uncommitted version; failed export does not move the current version pointer. ui-behavior: "Read-only formats do not show a save control"; render failure still shows source text.

### specs/008-workbench-browser-terminal-computer/README.md (27)
- DRAFT; owners Desktop shell, Environments, Browser provider, Trust, Desktop UI; outcome = real browser and terminal work, takeover/return, stale handle cannot act.

### specs/008-workbench-browser-terminal-computer/SPEC.md (157)
- LOCKED: browser provider and presentation adapter separate (CHROME-BROWSER-LLD); "UNAVAILABLE is not a browser mode"; terminal UI owns no privileged shell (V1-INTEGRATED-TERMINAL-LLD); Workbench = views over authorized objects; stale handles cannot act (master plan P7). Blockers B-22 (browser provider mode), B-23 (shell profile; whether computer-use is same milestone). AC-001…AC-007.
- §7a enum inventory: `AvailabilityStatus` = `READY, NEEDS_SETUP, UNAVAILABLE, OFFLINE, UNKNOWN`; `BrowserMode` = `MANAGED_CHROMIUM, CONNECT_EXISTING_CHROME, MANUAL_ONLY, HEADLESS_TASK`; `BrowserControlOwner` = `AGENT, USER, NONE, TRANSFER_PENDING`; session health = `STARTING, READY, DEGRADED, DISCONNECTED, RECOVERING, CLOSED`; `viewMode` = `LIVE_INTERACTIVE, OBSERVATION_FRAMES, EXTERNAL_WINDOW, UNAVAILABLE`; `BrowserActionReceipt.state` = `REJECTED, ADMITTED, IN_PROGRESS, APPLIED, AMBIGUOUS, BLOCKED`; `shellProcessState` = `STARTING, ACTIVE, EXITED, RECONNECTING, LOST, DETACHED, STOPPING`; `ownerKind` = `USER_MANUAL, TASK_ATTEMPT, PROVIDER_PRIVATE`.
- Take control is an atomic epoch transfer AGENT epoch n → HUMAN epoch n+1; return needs explicit confirmation and another increment; closing the side pane does not close the browser or cancel the Task; "A matching URL after restart is not proof that a pre-crash write succeeded".
- Forbidden: password fields/payments/CAPTCHA/permission prompts/MFA without a human; copying encrypted cookies or importing history/passwords/login state; Chrome remote debugging against the default user directory; Playwright against the default Chrome profile. `reconcileBrowserAction` leaves UNKNOWN ambiguous and never submits the same form twice; `detachBrowserSession` does not kill a user-owned profile. Terminal: unauthorized root rejected, silent fallback to home forbidden, closing view ≠ Stop Task.
- Cites `takeEnvironmentControl`, `returnEnvironmentControl`; event `environment.control.lease.changed.v1`; "No browser-session or terminal-PTY operationId exists yet."

### specs/008-workbench-browser-terminal-computer/PLAN.md (37)
- Template.

### specs/008-workbench-browser-terminal-computer/TASKS.md (53)
- `008-T01`…`008-T04`; T01 BLOCKED; T02 accepts AC-001…AC-004 + AC-007 (browser); T03 AC-005/006; T04 no AC. Commands `browser_epoch`, `terminal_environment_bind`.

### specs/008-workbench-browser-terminal-computer/DATA-MODEL.md (32)
- Records: browser session epoch and consent; terminal session bound to Environment id; redacted handle refs; `AvailabilityStatus`.

### specs/008-workbench-browser-terminal-computer/RESEARCH.md (27)
- Authorities `DESKTOP-SHELL-LLD.md`, `CHROME-BROWSER-LLD.md`, `V1-INTEGRATED-TERMINAL-LLD.md`, `ENVIRONMENTS.md`, `ZCODE-UI-INSPIRATION-AUDIT.md` (inspiration only); "Reject WebView-as-Chrome".

### specs/008-workbench-browser-terminal-computer/WIRING.yaml (65)
- Nodes `browser`, `pty`; `tx-epoch` → `browser_epoch` + `consent`; `tx-term` → `terminal_session`; edges `e-owner-browser` (action) and `e-owner-pty` (session).

### specs/008-workbench-browser-terminal-computer/contracts/* (7/7/5/17)
- api-events: "Provider routes stay in the browser and terminal LLDs … This DRAFT does not mint operationIds." errors-recovery: provider crash marks epoch lost; a half-applied browser action is never replayed without reconciliation. ui-behavior: Browser/Terminal/Computer shown "only when qualified"; stale epoch shows lost, not a live page.

### specs/009-coworkers-scoped-memory/README.md (27)
- DRAFT; owners Coworkers, Conversation, Memory, Trust, Desktop UI; outcome = optional Coworker, scoped context attach/revoke, restart, cross-scope denial.

### specs/009-coworkers-scoped-memory/SPEC.md (156)
- LOCKED: ordinary Conversation needs no Coworker; "A memory change cannot grant permissions, Connections, packages or schedules" (release contract §6.4); Goals/Suggestions grant no authority (COWORKERS-TARGET); ADR-0024 governs optional chat-first Coworkers. Blockers B-24 (minimal memory record + purge semantics), B-25 (Quick Create pins default lead binding or starts unbound). AC-001…AC-006.
- §7a: `Coworker.status` = `ACTIVE, PAUSED, ARCHIVED`; archive only when no Coworker-owned Automation active and every Task terminal; archive deletes nothing. Task stores immutable `origin_coworker_id` + `origin_coworker_revision`. Presence projection keeps `proactive_status`, `activity_status`, `runtime_status` separate on `GET /v1/coworkers/{id}/presence` and "is not synthetic typing". `CoworkerInteractionPolicy` = `STANDARD_TRUST_POLICY, REQUIRE_OWNER_APPROVAL, HANDOFF_TO_OWNER`.
- `Goal.status` = `ACTIVE, PAUSED, COMPLETED, ARCHIVED`; mutating archived Goal → `GOAL_ARCHIVED`; `GoalProgressProjection COMPLETE` means readers were readable, not criteria passed. `Suggestion.status` = `PROPOSED, ACCEPTED, DISMISSED, EXPIRED`; accepting a TASK suggestion commits one READY Task and no Plan/AgentSession. Quick Create uses two idempotency keys; a failed first chat leaves the Coworker created; Add Memory writes a user-authored ContextDocument and does not forge a `LearningCandidate`.
- Cites `createCoworker`, `reviseCoworker`, `changeCoworkerStatus`, `getCoworkerRevision`, `setPrimaryCoworker`, `setContextDocumentStatus`, `requestContextDocumentDeletion`.

### specs/009-coworkers-scoped-memory/PLAN.md (37)
- Template.

### specs/009-coworkers-scoped-memory/TASKS.md (53)
- `009-T01`…`009-T04`; T01 BLOCKED; T02 AC-002…AC-005; T03 AC-003 + AC-006; T04 AC-001. Commands `coworker_scope`, `coworker_retrieval_fence`.

### specs/009-coworkers-scoped-memory/DATA-MODEL.md (32)
- Records: Coworker id and revision; scoped ContextDocument; Conversation coworker origin; purge tombstone without payload bytes.

### specs/009-coworkers-scoped-memory/RESEARCH.md (27)
- Authorities `COWORKERS-TARGET.md`, `RESPONSIBILITIES.md`, `LEARNING.md`, `adr/0024-optional-chat-first-coworkers.md`, `CONTEXT.md`; "Reject memory that implies permission".

### specs/009-coworkers-scoped-memory/WIRING.yaml (51, no external node)
- Nodes ui/operator/owner/storage only; `tx-coworker` → `coworker_revision`; `tx-memory` → `context_document_status`; `external_boundaries` still declares a generic `outside-sqlite` although no external node exists.

### specs/009-coworkers-scoped-memory/contracts/* (7/7/5/18)
- api-events: "Proposed routes stay unmarked CURRENT until promoted." errors-recovery: stale `expected_coworker_version` conflicts; "Archive rejects new proactive work" (SPEC §11 phrases it as PAUSED blocks proactive/scheduled, ARCHIVED cannot be a Task origin — slight wording drift). ui-behavior: "Memory editor cannot toggle grants"; purge confirms scope.

### specs/010-automations-recurring-host/README.md (27)
- DRAFT; owners Automation, Task Runtime, Runtime lifecycle, Desktop UI; outcome = exactly one Task per occurrence across sleep/restart with truthful history and notification status.

### specs/010-automations-recurring-host/SPEC.md (154)
- LOCKED: TriggerHost observes, ordinary Task admission selects execution Runtime (master plan P9); at-least-once input → exactly-once occurrence; polling is default local reachability; logical occurrence + idempotency durable; window close may leave Runtime up. Blockers B-26 (first trigger type + misfire policy), B-27 (notification channel, local only). AC-001…AC-006.
- §7a: only a terminal Task settles an occurrence — COMPLETED→COMPLETED, terminal FAILED→FAILED, CANCELLED→SKIPPED with reason; `INCOMPLETE, PAUSED, WAITING_USER, BLOCKED, NEEDS_USER, VERIFYING, READY, RUNNING, PAUSE_REQUESTED, CANCEL_REQUESTED` leave it STARTED and holding its concurrency slot. `MisfirePolicy` = `SKIP, RUN_ONCE_WHEN_AVAILABLE, CATCH_UP_BOUNDED`; `ambiguous_local_time` = `EARLIER|LATER`; `nonexistent_local_time` = `SKIP|NEXT_VALID`; occurrence states `PENDING→CLAIMED→STARTED→COMPLETED|FAILED` plus `WAITING_DEPENDENCY`/`SKIPPED`; CLAIMED returns to PENDING only on lease expiry; stable delivery id with a different payload digest → `OCCURRENCE_CONFLICT`.
- Saving an Automation creates it **PAUSED** and admits no Task; `POST /v1/automations/{id}/run` allowed while PAUSED and does not enable recurring hosting; DISABLED terminal; resume only PAUSED + locally reconcilable Manual/Schedule triggers and does not fire at enable. `POST /v1/routines/{id}/run` needs Idempotency-Key + exact `routine_revision`, rejects non-null `conversation_id` until Conversation+Task commit atomically, accepts at most **64 top-level TEXT or RESOURCE_REF bindings**, creates one READY Task + TaskSpecRevision 1 and no Plan/Step/session/Attempt/lease/Environment/Effect/grant.
- `SIGNATURE` webhook verified with constant-time HMAC over raw bytes before parsing; local default hosting `LOCAL_POLL`; "This slice still does not add a public webhook listener". Cites `createAutomation`, `updateAutomation`, `pauseAutomation`, `resumeAutomation`, `runAutomationNow`, `settleAutomationTriggers`, `runRoutine`; events `automation.occurrence.created.v1`, `automation.occurrence.claimed.v1`, `automation.occurrence.settled.v1`, `automation.cursor.changed.v1`, `notification.delivery.changed.v1`.

### specs/010-automations-recurring-host/PLAN.md (37)
- Template.

### specs/010-automations-recurring-host/TASKS.md (53)
- `010-T01`…`010-T04`; T01 BLOCKED; T02 AC-001/002/004; T03 AC-003/005/006; T04 no AC. Commands `automation_occurrence`, `automation_recovery`.

### specs/010-automations-recurring-host/DATA-MODEL.md (32)
- Records: Automation revision; occurrence id and cursor; Task link; notification status.

### specs/010-automations-recurring-host/RESEARCH.md (27)
- Authorities `AUTOMATION.md`, `ROUTINES.md`, `RUNTIME-LIFECYCLE.md`; explicitly "Historical plans under docs/superpowers/plans/ are not authoritative"; "Reject always-on cloud workers".

### specs/010-automations-recurring-host/WIRING.yaml (62)
- `clock` = local clock and Runtime wake; `e-clock-owner` (fire) / `e-owner-clock` (cursor); `tx-occurrence` → `automation_occurrence`; `tx-link` → `task` + `occurrence_link`.

### specs/010-automations-recurring-host/contracts/* (7/7/5/17)
- api-events: "No new webhook ingress in this DRAFT." errors-recovery: restart replays cursor without double-create; missed fire recorded as misfire/skip, "never as silent success". ui-behavior: "Disabled and paused are distinct"; misfire visible.

### specs/011-desktop-shell-ipc-settings-runtime/README.md (27)
- DRAFT; owners Runtime, Operator IPC, Desktop UI, Product; outcome = launch desktop, reach real local Runtime over authenticated IPC, change a setting only when its handler exists, truthful health across restart.

### specs/011-desktop-shell-ipc-settings-runtime/SPEC.md (162)
- LOCKED: Operator IPC does not change API authorization; principal from authenticated transport, never client-supplied id; display preferences cannot restore grants; three independent lifecycles (Runtime, AgentHost, Environment); V1 local-first desktop; untrusted presentation cannot forge host actions. Blockers B-28 (GA OS + IPC qualification bar), B-29 (first settings groups). AC-001…AC-008.
- §7a IPC limits: **16 KiB header, 100 MiB request body, 10 MiB response body, 8 concurrent accepted requests, 128 MiB combined in-flight bodies, 3-second connect/header deadline, 30-second body-read deadline, handler deadline ≤ 60 seconds**.
- Allowed logical headers only `X-Workspace-ID, Idempotency-Key, If-Match, Content-Type, Content-Range, X-Chunk-SHA256`; `Authorization`, `Host`, `Origin`, cookies and hop-by-hop headers forbidden; `request_id` is correlation only, not an Idempotency-Key. Unix socket `0600` in `0700` dir, symlinks rejected, peer UID must match installation binding; Windows named-pipe DACL grants only current logon SID + SYSTEM; if DACL creation fails, "startup does not open an unauthenticated pipe and does not fall back to loopback bearer authentication".
- Three distinct lifecycle vocabularies: `recovery_state` = `STARTING, RECOVERING, READY, DEGRADED, DRAINING, STOPPING, STOPPED`; `runtime.availability.changed.v1` payload enum = `PAIRING, ONLINE, DEGRADED, DRAINING, OFFLINE, REVOKED`; lifecycle view availability adds `STARTING` + `RECOVERING`. `GET /v1/operator/readiness` confirms API serving for the reported incarnation, "not execution readiness"; `GET /v1/runtime-lifecycle` probes nothing.
- Drain stops admission before persisting DRAINING, allows admitted handlers ≤ 60 s; "A handler deadline is unsettled because work may continue after HTTP 504"; Operator stop route absent until Task/Effect/lease/local-service settlement exist. `RuntimeStartupPolicy` = `MANUAL, LOGIN_BACKGROUND, ALWAYS_ON_SERVICE`, LOGIN_BACKGROUND recommended. Non-goal explicitly rejects "Claiming all 187 settings are served". Cites `getLocalOperatorReadiness`, `getLocalRuntimeLifecycle`, `enrollLocalRuntimeInWorkspace`, `previewLocalRuntimeStop`, `requestLocalRuntimeStop`; event `runtime.availability.changed.v1`.

### specs/011-desktop-shell-ipc-settings-runtime/PLAN.md (37)
- Template.

### specs/011-desktop-shell-ipc-settings-runtime/TASKS.md (53)
- `011-T01`…`011-T04`; T01 BLOCKED; T02 (IPC gate, accepts AC-001/AC-002) **has no row in the traceability table**, while decision-only T01 is credited with AC-001/002; T03 AC-005/006; T04 AC-003/004/007/008. Command `cargo test -p operator-ipc --locked ipc_peer_auth`.

### specs/011-desktop-shell-ipc-settings-runtime/DATA-MODEL.md (32)
- Records: Runtime and RuntimeIncarnation; authenticated IPC session; local UI preference "distinct from domain aggregates"; per-control map status.

### specs/011-desktop-shell-ipc-settings-runtime/RESEARCH.md (27)
- Authorities `RUNTIME-LIFECYCLE.md`, `LOCAL-OPERATOR-IPC.md`, `DESKTOP-SHELL-LLD.md`, `V1-SETTINGS-CONTRACT-LEDGER.md`, `SEMANTIC-SERVED-UI-MAP.md`, `DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md`; "Reject Electron Host as the LiteCowork daemon".

### specs/011-desktop-shell-ipc-settings-runtime/WIRING.yaml (62)
- `ipc` = local IPC peer check; `e-ui-ipc` (frame) → `e-ipc-operator` (authenticated-request); `tx-health` → `runtime_incarnation`; `tx-pref` owner is `Desktop UI local store` → `local_preference` (the only non-domain transaction in the bucket).

### specs/011-desktop-shell-ipc-settings-runtime/contracts/* (7/7/5/18)
- api-events: "IPC is a transport for docs/API.md operations"; settings classes follow the ledger. errors-recovery: "Daemon crash shows disconnected, not an empty successful workspace"; drain stops admission before claiming stopped. ui-behavior: shell shows Runtime health; settings render from semantic-map class; "Disabled controls explain why"; semantic transcript survives rich-UI failure.

## Cross-cutting observations

- **Status conflict on 001.** `specs/README.md` says DRAFT; 001 README/SPEC/PLAN/TASKS say ACCEPTED/LOCKED (rev 1.0) while 001 DATA-MODEL/RESEARCH/WIRING say DRAFT and `spec_revision: "0.1"`. No owner sign-off artifact is recorded anywhere in the bucket, so "ACCEPTED/LOCKED" is asserted, not evidenced, against the repo rule "Never self-approve".
- **256 KiB contradiction inside 001.** SPEC §7.5/§10/O-05: 256 KiB is the API `UserRequest` detail-prompt cap and explicitly "not the adapter request budget". DATA-MODEL §3 ("currently observed OpenCode/Codex transport is 256 KiB per request") and RESEARCH §5.4 ("OpenCode/Codex transport input is 256 KiB") say the opposite. Unresolved, and it gates AC-007.
- **001 internal contradictions.** TASKS T02 marks "exact request/result types … approved" while SPEC §7.3 and DATA-MODEL §5 call `commit_conversation_agent_response` PROPOSED/"blocking owner work"; PLAN §9 and SPEC §14/§18 still speak as if the package were DRAFT-blocked; B-02 locks three harnesses (Claude Code CLI, OpenCode, Codex) while O-01 still asks which first harness and T04 stays BLOCKED on B-02/B-03/B-05 — and no Claude Code transport is described in RESEARCH §3.
- **002–011 are one template with per-domain content mostly confined to §7a.** PLAN.md (37 lines), DATA-MODEL.md boilerplate paragraphs, and the four contract files are near-verbatim across all ten: identical Risks trio, identical "Feature 001 remains a prerequisite…", identical contracts header/footers, identical "Denial, duplicate replay, timeout-after-dispatch, cancel, crash and restart are all required before QUALIFIED."
- **Copy-paste artifacts.** Every one of the 40 tasks in 002–011 ends with "Criteria discussed include AC-001 AC-002." regardless of its real criteria; every task shares the same "Stop:" sentence; RESEARCH "Findings" repeats the connection-toggle bullet in packages where it is irrelevant (003 roots, 005 task path, 007 artifacts, 008 workbench, 010 automations).
- **Misplaced densest section.** In all of 002–011 the heading `## 7a. Owner facts incorporated 2026-11-01` is inserted between §18 and §19, after the assumptions section, and carries the enums/error codes/operationIds/limits that belong in §7 LLD or `contracts/api-events.md`. Numbering collides with §7.
- **Traceability tables in 002–011 are vacuous.** Every row reads "Spec: EARS acceptance | Canonical contract: canonical owner docs cited in this SPEC" — no owner document is actually named per requirement, so the ledger satisfies shape but not traceability. Contrast 001 §16, which names real owners (API, STORAGE, EVENTS, STATE-MACHINES, TRUST, PRESENTATION-RUNTIME).
- **Systematic AC misattribution.** In all ten packages AC-001 and AC-002 are credited in the traceability table to the decision-only T01, while the implementing tasks (T02/T03) list those same ACs in their own acceptance — double mapping. 011 additionally omits T02 (the IPC auth gate that implements AC-001/002) from the table entirely. UI tasks (003-T04, 005-T04, 006-T04, 008-T04, 010-T04) have no AC row at all.
- **Acceptance is structural, not behavioral, for 10 of 11 packages.** 002–011's only "executable acceptance command" is `python3 .specify/scripts/validate_feature_package.py <pkg>`; none defines even a PROPOSED cargo target. 001 is the sole package with a behavioral command (`cargo test -p litecoworkd --test conversation_session_supervisor`) and with dated CODE evidence (593 unit + 2 integration tests, HEAD `3052341473e964bececf7f34c9d45923848e5218`, 2026-10-31).
- **Contract depth collapses after 001.** 001 contracts = 5,592 / 8,898 / 5,282 bytes (state tables, 15-row crash matrix, UI state enums, UIV1 ranges); every other package's three contracts total ≤ ~1.6 KB and are pointer stubs. The repo standard asks `contracts/ui-behavior.md` to define flows, states and typed UI commands; 002–011 supply 3–4 flows plus a generic "Commands" paragraph that names no operationId — and 001's api-events.md likewise defers operationIds without listing any.
- **Thin-vs-deep ranking (SPEC bytes):** 001 36,357 ≫ 002 12,483 > 011 10,446 > 003 10,125 > 005 9,799 > 006 9,763 > 008 9,457 > 010 9,300 > 009 8,893 > 007 8,542 > 004 8,344. Thinnest relative to its own ambition: 004 (relay has only 6 ACs, no numeric bounds, no transaction for cross-harness isolation) and 007 (one format, no size limits in §7/§10 beyond §7a's 1 MiB / 10 MiB).
- **README standard gap (all 11 packages).** The repo requires README to state "scope, status, owners, dependencies and navigation"; none of the eleven READMEs has a dependencies section — dependencies live only in SPEC §front-matter and RESEARCH prose.
- **Ordering/dependency graph is asserted, not modelled.** Every 002–011 SPEC says "Feature 001 remains the Conversation send authority", and prose edges exist (004 needs two qualified modules from 002 via B-16; 008 terminal needs Environment admission from 005; 010 needs 005 for execution; 009 B-25 needs 002) — but no package carries a cross-feature DAG or ID-level dependency list. 011 (IPC + Runtime health) is numerically last yet structurally underpins all others.
- **Cross-package blocker conflict.** 001 B-02 "resolves" first GA OS as Windows 11/10 MSVC, while 011 B-28 still lists "GA OS and IPC qualification bar" as an open blocker and 002 still lists B-02 as a live blocker "shared with 001". Either 001's lock is not visible to 002/011 or those blockers are stale.
- **Schema/owner drift flagged in-package (good practice, unresolved):** 006 notes "The OpenAPI Connection status enum omits REVOKED"; 001 notes `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` absent from the 131-code registry; 009 notes COWORKERS-TARGET records stay PROPOSED until promoted. All three are correctly left unpromoted.
- **Derived-vs-durable inconsistency.** 004 DATA-MODEL calls `PortableContextPackage` "derived, not a native transcript", but 004 WIRING declares `tx-export` atomic record `portable_context_package`; 001 DATA-MODEL similarly says `ConversationContextPackage` is "not persisted aggregate".
- **Consistent global invariants across all 11:** no native handle/credential in projections, events, logs or backups; external calls never inside SQLite; expected-version/idempotency on every mutation; UNKNOWN reconciled before retry; unknown ≠ success; explicit denial tests (denial, duplicate replay, timeout-after-dispatch, cancel, crash, restart) required before QUALIFIED. Every package's "Trust boundary" paragraph is byte-identical.
- **Evidence vocabulary discipline:** only 001 uses CODE/SERVED/SYSTEM/USER/QUALIFIED with dates; 002–011 uniformly label behavior TARGET and say "The active checkout has no implementation to mark SERVED" — consistent with the repo's evidence rules, but leaves 10 packages with zero CODE evidence and no per-task evidence column.
- **Numeric limits worth keeping as the operational spine:** context 1,024 msgs / 4 MiB; submit 256 blocks / 1 MiB / 256 ResourceRefs; API detail prompt 256 KiB; idle TTL 10 min + 5 s; census 1,000 entries / depth 32 / 1 MiB locators / 8 s (stop 7 s) / 2 jobs; instruction revisions 64 KiB; artifact append 1 MiB, Save As 10 MiB; routine bindings 64; IPC 16 KiB header / 100 MiB request / 10 MiB response / 8 concurrent / 128 MiB in flight / 3 s / 30 s / 60 s.

## Coverage check

- All 122 listed files read in full; per-package line sums reconcile to the plan's 5,631 and every read reported complete (no truncation). 001/SPEC.md (346 lines) and the 110 files of 002–011 were read without paging.
- Nothing unread. Files I could not verify **outside** this bucket: `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/SPEC-TASK-TRACEABILITY.md`, `docs/SCHEMAS.md`, `docs/ERROR-CODES` registry (the 131-code claim), `docs/schemas/*` OpenAPI enums (the 006 REVOKED claim), all `docs/` owner documents cited by §7a blocks, and the archived worktree at `3052341473e964bececf7f34c9d45923848e5218` — so every "authority owner says X" statement above is recorded as the package's claim, not independently verified.
