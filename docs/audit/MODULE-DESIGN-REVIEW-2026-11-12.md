# Module design review — 2026-11-12

**Baseline:** `64c435149a82ba952c6c3bf3049e26cf2d9344b1`, `main`, three commits ahead of `origin/main`. The working tree was already modified and had untracked design/audit documents when this review began; it was not clean. One worktree. Ignored dependency/build directories were observed and preserved. This report is an audit artifact, not owner acceptance, implementation authorization, or qualification evidence.

**Scope and limitation:** this reviewer inspected the 18 feature packages other than 008 and 016. Those two are assigned to other reviewers. Every module's UI/API/error/data contract and HLD/LLD section was semantically read; selected wiring and plans were also read. The remaining package sections, TASKS, RESEARCH and README files were inventoried but were **not all read semantically**. The exact coverage ledger below is the authority for this report. A structural scan, file hash, or line count is not semantic review. The user's requested all-document/all-line audit remains OPEN; this report must not be represented as its completion.

Hashes and line coverage refer to baseline Git blobs, not concurrent fixes by the coordinating reviewer. Findings below describe that baseline. A later correction does not make the original reading disappear, but must be rechecked before closing the finding. Root ARCHITECTURE.md was reread in bounded complete chunks after an initial truncated command; PRODUCT.md and V1 release contract were read. CURRENT-RUN and IMPLEMENTATION-STATUS were consulted but a combined read truncated; they are not counted as completely reviewed here. This subreview itself did not open external URLs. The coordinating review separately checked official A2UI launch/index/README/releases, OpenCode ACP/MCP documentation, and Tauri frontend communication/Channel references on 2026-11-12; the relevant current research is recorded in the linked LLDs and Feature 016 RESEARCH. That source check found that GitHub Releases does contain A2UI CLI/core/agent-SDK releases, while the exact React renderer package and its v1.0 conformance remain unpinned. Those mutable upstream pages are research evidence, not an adopted dependency or provider qualification.

## Assessment

The design has useful ownership boundaries, real Rust/Tauri/React source, transactional foundations, and explicit fail-closed states. It is not ready to claim complete end-to-end implementation or qualification. Several documents within the same package disagree about current implementation, and some target signatures and wiring disagree about transaction contents or admission order. The new selected-screen/window Computer request also changes the previously browser-only observation decision; it needs an explicit owner revision and downstream trace, not a UI relabel.

Highest priority is to resolve the concrete contract disagreements below, including the Conversation waiting-state Stop mismatch (M16/A-79), then qualify vertical slices. More screens or green structural validators will not resolve missing native provider input, Environment admission, real PTY, live observations, durable context receipts, or professional Artifact producers.

## Confirmed disagreements and design gaps

| ID | Priority | Evidence at baseline | Consequence and required owner action |
|---|---|---|---|
| M01 | P1 | 001 DATA-MODEL lines 15, 48, 60 says no durable dispatch intent/outbox or startup sweep. Its UI cross-layer trace says the opposite. Full `agents/conversation_recovery.rs` and storage-core intent methods were read; Operator header and admission comments confirm durable intent. | An implementer could rebuild existing durability or sequence dependent work on a false blocker. Conversation owner must replace current-state prose with exact outbox/claim/recovery behavior, preserving the distinct unqualified provider journey. Update WIRING recovery and admission known-gap fields as well. |
| M02 | P1 | 001 API contract lines 10, 26, 50, 58 describes unresolved detached crash window, absent identity fields and no delta/settled producer; later UI trace records implemented intent, retry ordinals, stream and Stop path. SPEC HLD/LLD repeats earlier state. | Reconcile source-backed CODE facts across SPEC/data/API/wiring/TASKS/status. Do not turn CODE fixes into live-provider evidence. Separate remaining service/host ownership work from already implemented durability. |
| M03 | P1 | 002 UI line 73 and API operation table line 22 say enable lacks descriptor checks; UI trace line 101 says `X-Agent-Descriptor-Revision` CAS. SQLite `lib.rs` around 19044 calls descriptor revision validation. UI trace also says sign-in/native config have no Tauri command while its later paragraph says commands exist. | Agent owner must specify the exact conditional descriptor rule, module-less behavior, and remaining unqualified producer/launcher boundary. A mounted command that refuses all methods is neither missing transport nor supported auth. |
| M04 | P1 | 003 SPEC target `CommitContextAttachment` requires retrieval_receipt_id; `ContextUsedEntry.root_id` is nonoptional. DATA-MODEL explicitly permits null receipt for explicit references and null root for uploaded Resources. | The ordinary uploaded-file attachment path cannot satisfy both interfaces. Context owner should define optionality and explicit-versus-retrieved source classes once, promote canonical schema, and test explicit upload, rooted ref, retrieved citation, revoke-before-dispatch and receipt replay. |
| M05 | P1 | 005 SPEC actor step 4 attaches Environment before Attempt admission; 015 actor step 3 and wiring `e-delegation-attempts` attach the child's Environment after child/lease/budget commit. Child transaction list does not show the same attestation pin. | Define one prepare/admit/launch protocol. A CREATED child may reserve budget before attachment only if dispatch is fenced until equivalent current-incarnation attestation, exact roots and grants are committed/rechecked. Explain crash cleanup and reservation release. This is an unresolved target sequencing ambiguity, not a demonstrated containment bypass. |
| M06 | P1 | 005 SPEC/API says Attempt, lease, grant, invocation intent and idempotency receipt commit together; WIRING `tx-attempt` lists only Attempt/lease/grant/receipt. | Add the exact owner record/Effect proposal relationship and event/snapshot requirements to wiring. Otherwise the architecture graph does not trace the external-write intent needed for crash reconciliation. |
| M07 | P1 | 007 SPEC/API requires paired ArtifactVersion/ResourceRevision/head updates/events/receipt; WIRING `tx-version` lists only ArtifactVersion/provenance. Export prose requires durable export event/replay while `tx-export` lists only export_receipt. | Make target transaction inventories agree with Artifact owner atomicity. Show stage/validate, head CAS, event/snapshot/receipt, export ambiguity and local file commit reconciliation explicitly. Existing text append is not a professional format engine. |
| M08 | P1 | 011 DATA-MODEL line 24 and WIRING local-preference/recovery entries say preferences exist only in component state/reset. UI trace says applied and persisted. SettingsView line 118 explicitly delegates applied/persisted preferences to App. WIRING end says rename unserved, while `operator.rs:1541` mounts PUT Workspace name. | Repair the source boundary consistently. Retain missing stop request, power wiring and update as separate gaps. The same package must not instruct one contributor to persist preferences and another to preserve an already persisted store without identifying the distinction. |
| M09 | P1 | 012 DATA-MODEL migration paragraph acknowledges v32 control-event and side-note tables; its later current-source block and errors contract say no store/records exist. Storage SQL insert paths were found in conversations.rs. | State storage-only CODE accurately. Preserve absent service/route/UI qualification. Conversation Take control still depends on exact observed native Stop; browser/Computer input takeover remains a different control authority. |
| M10 | P1 | 014 SPEC/API permits null title to clear; current `conversation_operator.rs` line 40 explicitly refuses null, saying durable title is never empty. SPEC still calls rename/archive routes proposed while source mounts them. | Conversation owner must decide null/empty semantics and align schema, handler, UI, error contract and tests. A source choice does not silently amend accepted target semantics. Preserve search/delete as separate missing journeys. |
| M11 | P1 | 017 SPEC trust boundary 4 requires manifest authentication and every digest before any key derivation. Source backup.rs derives key around 1399 and decrypts around 1414/1460. | Correct the achievable ordering: structurally bound/validate untrusted header and KDF parameters before expensive derivation, derive bounded key, authenticate encrypted manifest/chunks, verify digests, then stage. Require malformed-header/KDF-cost/resource-cap tests. This is a documentation ordering conflict, not evidence that crypto is broken. |
| M12 | P2 | 017 `BackupOperationRecord.workspace_id` is String in SPEC, nullable in DATA-MODEL for unbound VERIFY. API says every route's Workspace header matches path Workspace, but verify/import have no path Workspace and may operate on an absent Workspace. | Define authorization for installation-local/unbound archive verification versus Workspace create/list/import; use an optional target identity where necessary. Test unauthorized existing Workspace archive, absent import, foreign rows and no cross-Workspace disclosure. |
| M13 | P1 | 019 UI Retry always reissues with a new request ID; DATA-MODEL retains a single durable UpdateAttempt and recovery distinguishes HANDOFF/interrupted work. | Retry policy must reconcile the original uncertain install/handoff first. New request IDs are appropriate only after a known refusal/failure or changed explicit intent. Add lost-command-response, app-restart-after-handoff and double-click tests. |
| M14 | P1 | 013 SPEC HLD assumes second OS-local IPC to LiteSPM daemon; root architecture describes the currently observed upstream local MCP stdio bridge and requires the published pin to govern wire details. Entire feature is deferred. | Revalidate exact upstream transport, advertised operations, host mapping, peer assurance, plan/approval and reconciliation contract when deferral lifts. Do not implement inferred daemon RPC or infer names from CLI/catalog. Keep the plan digest bound owner approval and one package authority. |
| M15 | P2 | 020 HLD says learning work item is recorded after turn settlement; plan says it stays off Send. Candidate queue durability is separate, with restart lease recovery. | Specify the crash window between settled event and enqueue: consume durable event cursor/dedupe or atomically record an outbox reference. An asynchronous callback alone would lose eligible learning after a crash. Test settle-before-enqueue crash, duplicate consumer delivery, source deletion and policy revision races. |
| M16 | P1 | Accepted Feature 001 AC-024 requires Stop for OPEN/RUNNING/WAITING_USER/WAITING_DEPENDENCY/CANCEL_REQUESTED. Current Operator cancel handles RUNNING and OPEN only; it rejects the waiting states, while the UI omits Stop. The state machine allows waiting states to enter CANCEL_REQUESTED, so normative contract, service route and UI do not agree. | Keep the waiting composer fenced and do not render a working control that the route rejects. The Feature 001 owner must either implement provider-aware cancel/reconciliation for exact pending requests or approve a revision narrowing AC-024 with an explicit recovery path. No claim of AC-024 completion until resolved; tracked as A-79. |
| M17 | P2 | The compact-navigation proposal's UX-02 requires focus to remain discoverable when More opens/closes and a secondary route is selected. In `App.tsx`, the `<details>` key changes from `"primary"` to the selected secondary `Page`, remounting the disclosure and its focused button; there is no explicit focus restoration. The rendered-browser check did not run because `LITECOWORK_CHROME` is unset. | Keep this as an open shell UX issue, not a new accepted contract. Before claiming keyboard/focus conformance, use a stable disclosure identity or explicitly restore focus to the selected destination/More summary, then verify mouse, keyboard, close, route change and narrow-drawer behavior in a rendered desktop/browser run. |

Some M01/M03/M08/M09/M10 observations were sent to the coordinating reviewer for source-backed corrections. This report does not assume those corrections are complete or owner accepted.

## Module-by-module interconnection review

Each row is a semantic assessment of the read sections, not a package readiness decision. Coverage is narrower than the full package where the ledger says so.

| Module | HLD/LLD and ownership assessment | UI/API/event/tool/recovery and acceptance work needed |
|---|---|---|
| 001 Conversation | Correct central ordinary-chat owner; native reasoning stays with harness; durable answer must commit independently of presentation. Target coordinator/supervisor/host topology remains distinct from current integration. | Resolve M01/M02/M16. New intelligent answer intent must be optional through native pipeline, correlate turn/retry/session and use semantic digest. UserRequests/native permissions need actual provider producer/reply/denial/timeout evidence. Keep native tools REPORTED, not fictitious mediated Effects. |
| 002 Agent setup | Registry metadata, installation, authentication, bindings and session readiness correctly separated. AgentModule owns exact native capabilities/options. | Resolve M03. ACP client session/input/permission/tool/resource/terminal negotiation must be version-pinned and module owned. Do not make ACP listing mean dispatch readiness. Revalidate model/effort on admission and preserve native sign-in/credential custody. |
| 003 Context | Root identity and factual indexing are distinct from explicit attachment/retrieval. Selected Computer frames are not WorkspaceRoot grants. | Resolve M04. Add durable exact-revision receipt/Context Used; distinguish uploads from rooted files and observations from extracted content. Screenshot use as attachment needs explicit frame selection/provenance/redaction/retention, not automatic durable screen history. |
| 004 Relay | Fresh per-turn AgentSessions and semantic portable history avoid private transcript migration. | Qualify two actual selectable providers and A→B→A. Preserve producer identity and bounded context. Existing contract still describes attachment/materialization and provider-case status inconsistently; reconcile against 001/002/003. |
| 005 Safe Task | Explicit Task creation, accepted Plan, containment and reconciled write are the correct dependency floor for consequential actions. | Resolve M05/M06 before child input/terminal/Computer actuation. Computer observation alone need not create Task; consequential agent input through Core requires the normal admitted authority path. Native unmediated input cannot be advertised as intercepted. Qualify one write with denial, stale fence, ambiguous outcome and restart. |
| 006 Connection | OAuth, SecretStore, grant and ApprovalUse are correctly independent; brokered HTTP is the use path. | OAuth recipe remains a real owner gate; approval is not execution. Discover secondary placement must preserve connect/scope/reauth/revoke repair entry points from Coworker/Conversation. No arbitrary URLs or tokens reach the agent. Test exact-target post-timeout reconciliation. |
| 007 Artifacts | Semantic answer, rich enhancement, Resource and Artifact identities must remain distinct. Immutable content versions and source pins are suitable. | Resolve M07. First Markdown create/edit/export-receipt producer is still required. Optional saved answer view state is presentation data, not an Artifact version until explicit save. Save as Artifact and Library reopen need actual owner routes, exact source identity and deletion-origin handling. |
| 009 Coworkers | Optional identity/context layer reuses Conversation and Task owners; no autonomous engine should arise from memory/defaults. | Coworker-owned chats, context scopes, memory ports and policy consumers remain separate gaps. Put roster/chat under Coworkers; place knowledge in Library/Coworker context and approvals in Needs You. Paused versus explicit-owner Task admission needs current owner conformance. |
| 010 Automations | TriggerHost occurrence creation distinct from Task execution and delivery status; cron timezone/misfire are explicit. | Four-primary sidebar makes Routines a subview of Automations and keeps history/Continue reachable. Occurrence older-page pagination and real notification settlement must be completed. Selected window observation cannot silently create monitor responsibility or unattended control. Connector events remain brokered/pinned, no public ingress assumption. |
| 011 Shell | IPC peer auth, Runtime/window distinction and owner-specific Settings avoid a second authority. | Resolve M08. Replace top-level Home/Dashboard/Goals/Ideas/Notifications sprawl with explicit destination ownership. Work, Coworkers, Library, Automations are primary; Needs You is inbox; Discover/Settings secondary. New Chat/history stay compact and first-class. Add route migration, keyboard/focus, deep links, selected-screen layout and authorization-before-restoring preferences. |
| 012 Take control | Conversation takeover means stop/quiescence/terminal control-event; side-notes do not steer the native turn. | Resolve M09. Never reuse this command for Computer mouse/keyboard lease takeover. Simultaneous turn Stop, browser/Computer takeover, Task pause and terminal control need independent visible pending/result states. Side-note is separate stored owner content and needs purge/export rules. |
| 013 LiteSPM | One package authority, exact reviewed plan and digest-bound approval are correct. | Resolve M14 only after deferral lifts. Secondary Discover still needs reachable inventory/adopt/install/repair/remove flows; package availability never becomes Connection, Coworker assignment or grant. Reconcile UNKNOWN against original upstream operation. |
| 014 History | Search, archive and tombstone/purge are separate Conversation owner capabilities; cross-reader fence inventory is useful. | Resolve M10. Compact history must preserve search/pagination/archive actions and pending-create recovery. Saved view state, remembered notices, rich documents and context/retrieval references must join deletion/export fence inventory. Physical purge and retained provenance must not contradict each other. |
| 015 Task control | Durable controls, exact child lineage and verified results keep host delegation separate from native subagents. | Resolve M05. Actual Task allowed_commands must drive controls. Native ACP tool/terminal/permission requests correlate to exact child grants/session/epoch. A report of a native subagent is never a host child Attempt. Work Activity must link to the exact real Browser/Computer/terminal session without implying liveness from history. |
| 017 Backup | Offline full restore and live import of an absent Workspace have distinct authority; restored grants/leases must remain dead. | Resolve M11/M12. Add saved-view-state inclusion policy, ephemeral screen/PTY exclusions, deleted-origin fences and renderer/schema compatibility. Never restore screen/window handles, capture consent, input leases, native sessions or terminal process IDs as live. |
| 018 Usage/diagnostics | Read-only usage and diagnostics stay outside domain authority. Unknown counters/costs cannot become zero. | UI states may be saved per Workspace but queries must reauthorize. Count native response usage once; optional presentation compilation failure must not duplicate charged inference or block semantic settlement. Capture/control/PTY health should expose typed source/age/denial reasons without screenshot, terminal byte, secret or private-path logging. |
| 019 Release | Exact OS/build/provider/auth cell and USER sign-off remain required. Stop/update depends on global positive quiescence. | Resolve M13. Add distinct cells for selected-window capture, display/permission revocation, actual PTY and input takeover. Old managed-browser viewport evidence cannot qualify full desktop/window observation. No signed updater/installer qualification implied by scripts. |
| 020 Proactivity/learning | Suggestions remain proposals; learning eligibility and Resource-backed memory do not grant authority. | Resolve M15. Remove top-level Ideas/Home dependency by mapping suggestions into Work/Coworker context and review into Needs You. Optional saved view interactions/clicks are not automatically learning candidates or permission. Screen/terminal content remains ineligible unless explicit source/scope policy admits it. |

## New UX request: coordinated contract changes

The request is a new product design input. It does not silently overturn the existing accepted B-32 browser-only/whole-desktop deferral. The owner resolution must state its intended replacement precisely and propagate it through domain authority, schemas, feature package revision, ledgers, wiring, tests and status.

1. **Navigation:** 011 owns shell arrangement. Product/Architecture and screen/action authorities currently enumerate more persistent destinations. Map all existing journeys into four primary destinations: Work (chats/tasks/activity and optional status views), Coworkers (identity/chat/configuration), Library (files/knowledge/artifacts), Automations (Routines/schedules/history). Needs You is a distinct compact inbox. Discover and Settings are secondary. New Chat and Recent conversations remain accessible without Coworker/Task. Goals, ideas and notifications need explicit subview destinations and preserved deep links; hiding their navigation without reassignment loses required journeys.
2. **Live Computer observation:** separate observation authority from input authority. Define selected screen/window identity, native picker, capture-consent owner, source changes, bounds/latest-frame delivery, stale/paused/offline labels, revocation, lock/sleep, excluded/secure windows, redaction, screen size/DPI changes and Runtime-incarnation invalidation. An embedded page/managed browser viewport is not selected-window capture. No raw frames belong in the domain journal or ordinary backups.
3. **Input and takeover:** observation permission cannot imply mouse/keyboard permission. Bind any input lease to exact target, controller, scope, incarnation and epoch; show grant/revoke/return state independently. LiteCowork is normally backgrounded during Computer input, so the in-app strip must have a qualified out-of-app trigger (global shortcut or native tray); refuse mediated input otherwise. Reconcile an already dispatched action before retry or ownership transition. Distinguish Computer input takeover, browser takeover, Conversation Take control and Task pause/cancel. Record only real mediated action provenance; direct native tools stay reported/observed. Feature 008 A-78 and Feature 011 shell review own the trigger gate.
4. **Actual Terminal:** selected shell, actual PTY/ConPTY process, cwd/environment isolation, ownership, terminal identity, ordered/bounded byte stream, resize, exit code, reconnect, cancellation, quiescence and secret input need provider contracts. Transcript UI or command text is not process output. Native ACP terminal methods must reference the same owner process/session model or be clearly native-owned; do not invent a second terminal authority.
5. **Intelligent answers:** native agent produces complete semantic answer plus optional bounded declarative intent; compiler validates/binds exact authorized objects and registered components. No HTML/JS/eval in chat. Semantic content commits immediately, and rich-render failure preserves text/citations/warnings/deliverables. Optional saved view state should be separately scoped/versioned presentation preference with bounded schema, identity/digest compatibility and reset behavior; it cannot settle Task state or carry grants. 016 owns detailed renderer design; 001/002 own the native transport and correlation.
6. **ACP:** discovery/client support, lifecycle descriptor negotiation, session start/prompt/update/permission, file/resource/terminal surfaces, tool result normalization, stop and recovery must be wired through AgentModule adapters. A protocol path should avoid hardcoded provider branches in Core and must not claim native tool interception. Qualify native auth/model semantics and permission denial on the exact agent/version/OS tuple.

## Implementation order and evidence gates

This is a proposed dependency ordering for review, not authorization of BLOCKED/DEFERRED TASKS.

1. Close the identified owner/schema/request/transaction disagreements. Record the compact-shell and selected-window observation scope decisions and exact feature revisions. Check the master plan and named TASKS authorization before implementation.
2. Preserve and qualify one native Conversation setup→binding→Send→stream→semantic commit→Stop→retry→restart path, including real native permission/form denial and response correlation. Complete AgentModule/ACP seams where required; do not couple plain chat to Task readiness.
3. Complete exact explicit attachment and retrieval receipt/Context Used path. Then validate its deletion/export/replay behavior.
4. Implement real terminal and observation providers as independent bounded services with truthful source/health. Observation-only can precede input; input remains unavailable until target-specific permission/fencing/reconciliation qualifies. Global Runtime stop/update must count their real activity and unknown quiescence.
5. Finish safe Task/Environment/Effect/verification slice before consequential automated Computer/connector/delegated execution. Resolve child admission/attestation ordering before fan-out.
6. Complete first Artifact create/edit/export and optional declarative presentation with fallback. Add saved view state only with explicit ownership, retention and compatibility semantics.
7. Bring Coworker scope, Automations delivery, learning, backup, diagnostics and release-matrix gates through their actual dependent routes. LiteSPM, Windows execution and release-dependent deferrals remain explicit until lifted.

For every path, record User outcome → exact control → typed action → authenticated IPC → authorization → service → transaction/version/fence → event/receipt → projection/stream → restart/recovery → acceptance evidence. Required cases include denial, stale scope/descriptor/target, duplicate replay, timeout-after-dispatch, cancellation race, crash/restart/reconnect and no silent fallback. Acceptance must name exact build/OS/agent/provider/auth method. This audit ran no packaged desktop, provider, capture, terminal, restore or E2E tests.

## Read coverage ledger

The following generated ledger uses SHA-256 of exact baseline Git blob bytes and physical line counts. `FULL` means every line was returned in readable bounded output (overlapping rereads repaired initial truncated batches). `PARTIAL` lists exact included one-based ranges; unlisted lines remain semantically unreviewed. `INVENTORY ONLY` means a path/hash/line count or heading/search scan, with zero lines counted as semantic coverage. Feature 008 and Feature 016 are outside this ledger. Separate review findings for them informed the new Computer and ACP presentation design documents, but no merged per-line coverage ledger was produced; their complete semantic review is therefore not claimed. No inference from similarly named files fills a gap.


Baseline package inventory: **199 files / 22910 lines; 10356 lines semantically read; 12554 lines unreviewed** in this subreview. Counts exclude 008/016. Baseline SHA is fixed above.

| Baseline file | Lines | Semantic coverage | SHA-256 |
|---|---:|---|---|
| `specs/001-conversation-session-supervisor/DATA-MODEL.md` | 79 | FULL: 1-79 | `ce15f348c6eaee4736df477e4c5e1389d4b3da482000400bbc3219c118b08b72` |
| `specs/001-conversation-session-supervisor/PLAN.md` | 107 | FULL: 1-107 | `393285484e52d263efefb445c3efeeca617bde109feb33f218a75780bb87818d` |
| `specs/001-conversation-session-supervisor/README.md` | 31 | FULL: 1-31 | `498335a8338a4014ad9501724219fd80f91adbfae4d4508b07e681435ce33cd3` |
| `specs/001-conversation-session-supervisor/RESEARCH.md` | 118 | FULL: 1-118 | `40bfdab9bc71ff30f3b1be9df48ab915ee5d6aa23b789ec9f8153f0d1163774c` |
| `specs/001-conversation-session-supervisor/SPEC.md` | 385 | FULL: 1-385 | `93ea63a3227d186840af4bac67a869318aa05ac51e7cf9af44b26c9de1ae2494` |
| `specs/001-conversation-session-supervisor/TASKS.md` | 559 | FULL: 1-559 | `ef02808108c52eeca7de3b8569f67d691e42160688a3be2dc3a786b2836d653e` |
| `specs/001-conversation-session-supervisor/WIRING.yaml` | 236 | FULL: 1-236 | `26978acdce51b2a38e118e53b7b93b573909f14a399433d5ee5d4ce4dcc682f3` |
| `specs/001-conversation-session-supervisor/contracts/README.md` | 12 | FULL: 1-12 | `db996aef02f977b4f33446470050accb843a7e9fc2ea323c9393a9eb8e7a737f` |
| `specs/001-conversation-session-supervisor/contracts/api-events.md` | 63 | FULL: 1-63 | `7579c4b6b3b0b3865a6464168c891323d886bd83dd361ec7fd824333d8601c5b` |
| `specs/001-conversation-session-supervisor/contracts/errors-recovery.md` | 78 | FULL: 1-78 | `47de1c535b262d4f804ba14eb166f7ce267f325acad83788fd66d93bfc64bb59` |
| `specs/001-conversation-session-supervisor/contracts/ui-behavior.md` | 109 | FULL: 1-109 | `8395bc23740b1915719ace64d5ff091256336f110728fc322701e154f742f0c0` |
| `specs/002-native-agent-install-auth-binding/DATA-MODEL.md` | 44 | FULL: 1-44 | `395d73b9442ae14103ebf42d6aa1d4286085128af3596484284edfd7c7ab0585` |
| `specs/002-native-agent-install-auth-binding/PLAN.md` | 52 | FULL: 1-52 | `63d51687b260e122e933820da038e66f09d98fcf71fde3c084f2136de4b2e616` |
| `specs/002-native-agent-install-auth-binding/README.md` | 40 | FULL: 1-40 | `ae40408136c1931bc165c8cf20ff0f8d9782f0273081a1a79782fffb20147e38` |
| `specs/002-native-agent-install-auth-binding/RESEARCH.md` | 107 | FULL: 1-107 | `b35b7f2db8777d7034e414b673f44449c42507991ff87640689d2eac69b15c11` |
| `specs/002-native-agent-install-auth-binding/SPEC.md` | 470 | FULL: 1-470 | `1d30894da5cee2aaef02d8a79b34927d57c037836d5f57459702c6be8ba97135` |
| `specs/002-native-agent-install-auth-binding/TASKS.md` | 401 | FULL: 1-401 | `859ae64126b1cb9ac0dd0e7d5bb2df8af7bccdf084d20aae2b24094164b40af9` |
| `specs/002-native-agent-install-auth-binding/WIRING.yaml` | 122 | FULL: 1-122 | `a44d9b9a0e304a7ea1023ba49a93543030eeb206e3dc55ee95a779e2fb36837b` |
| `specs/002-native-agent-install-auth-binding/contracts/README.md` | 9 | FULL: 1-9 | `650e7a2f3b6ee610b458ff550a8c1e02ae54ff89fcb8c1ed2d7885ca46754e42` |
| `specs/002-native-agent-install-auth-binding/contracts/api-events.md` | 28 | FULL: 1-28 | `5ae5f5673f99b2db20cd1ecd55288caf625caf24f9aef7560713ab693e033e2d` |
| `specs/002-native-agent-install-auth-binding/contracts/errors-recovery.md` | 9 | FULL: 1-9 | `d1caa1de6e7d6b2f2a1123fb0e6d6aa4feb7d9f413ea8d43f82b0cca36fe71ed` |
| `specs/002-native-agent-install-auth-binding/contracts/ui-behavior.md` | 107 | FULL: 1-107 | `a95a8142a8e02fa6cab164eaf6d9a17eae6ed2cc9ed9d04e31c25753bdcf4331` |
| `specs/003-workspace-roots-context-ingestion/DATA-MODEL.md` | 34 | FULL: 1-34 | `aac743cd60e0a4c8c15cc8e9c25285fe4fe896e91b2775f3401dfdbafbe42a14` |
| `specs/003-workspace-roots-context-ingestion/PLAN.md` | 43 | FULL: 1-43 | `0a66d755d261b4e1c9219a265513acd518504e6328b9d53999ab21748318c13e` |
| `specs/003-workspace-roots-context-ingestion/README.md` | 36 | FULL: 1-36 | `6828640d7b1e934e2cf22ae7688266bd19159bd4f467b298dbdbf16133935b98` |
| `specs/003-workspace-roots-context-ingestion/RESEARCH.md` | 60 | FULL: 1-60 | `00c5d08d51269defeac606ceb870f7200252ca89b0ece71f41e3274547e64b55` |
| `specs/003-workspace-roots-context-ingestion/SPEC.md` | 339 | FULL: 1-339 | `0791579e8e750b7a0eb8a1a2d0590b9c1e7e8f7df96eef26c21ec975cb7848dc` |
| `specs/003-workspace-roots-context-ingestion/TASKS.md` | 378 | FULL: 1-378 | `fadaf65abc55bbf95c6acc6208ab84bebf09786641af6f2e2ea8131cda7d4db0` |
| `specs/003-workspace-roots-context-ingestion/WIRING.yaml` | 124 | FULL: 1-124 | `95bcdb48298ff32210288c7835e13230d1fd452949ec2b4dfcbda574e47c65b9` |
| `specs/003-workspace-roots-context-ingestion/contracts/README.md` | 9 | FULL: 1-9 | `5eec3c904cd05b4d9de68f96e0d1c224c90479c85016796d77ebfbef11a71271` |
| `specs/003-workspace-roots-context-ingestion/contracts/api-events.md` | 11 | FULL: 1-11 | `815c53e7fbeb742921fd29b1dc4bfd6272fc87769de719f5cf291ceddad7e677` |
| `specs/003-workspace-roots-context-ingestion/contracts/errors-recovery.md` | 9 | FULL: 1-9 | `23b50b52397b4c9aa2fc0464eb2034ab57a2e8a773a0e0a97d809cac529fd5b8` |
| `specs/003-workspace-roots-context-ingestion/contracts/ui-behavior.md` | 30 | FULL: 1-30 | `a9f9801466a1332c5ffd7d83f690380a62bda28d47973e0cfcc9f011d7fc25ff` |
| `specs/004-agent-relay/DATA-MODEL.md` | 41 | FULL: 1-41 | `6e60781f5d23aa681851a97d31c709a019a6ca22f040e871bde39c18b58b695a` |
| `specs/004-agent-relay/PLAN.md` | 44 | FULL: 1-44 | `bb51ae4751afdedcf0603592d66a861ad586c7bf946643742707c065ca6d65db` |
| `specs/004-agent-relay/README.md` | 34 | FULL: 1-34 | `de07eb5407dd343743bc2b380d7aa16af33d20cfcb6e2392853f95a36c47d7da` |
| `specs/004-agent-relay/RESEARCH.md` | 67 | FULL: 1-67 | `1ae58880005506216dde137b337712f47eee45805486954439a1f1b467e04f79` |
| `specs/004-agent-relay/SPEC.md` | 317 | FULL: 1-317 | `2d3e74af7e123368a1c9b2b1d513661d86c9d6200f36ebe79a20e76a9d0dc093` |
| `specs/004-agent-relay/TASKS.md` | 239 | FULL: 1-239 | `d099ccd54042c1adeb093b3bd6907c37d9418474656e110b3086f0b9d3a3d4db` |
| `specs/004-agent-relay/WIRING.yaml` | 69 | FULL: 1-69 | `676dd56cd6603a618193945fa3e231b62803b79cdba094d03e135a9223478c20` |
| `specs/004-agent-relay/contracts/README.md` | 9 | FULL: 1-9 | `dcd3f2509e555ea04e9df04adbe7785751be4a25061fd6f5ea3b26bb14794e31` |
| `specs/004-agent-relay/contracts/api-events.md` | 27 | FULL: 1-27 | `5ce6b4c6b36a5e74071f62b9daf357aeb0168e44ff7e1418645f3068a08b5e45` |
| `specs/004-agent-relay/contracts/errors-recovery.md` | 22 | FULL: 1-22 | `85b0437b167fc1e8bbd6492257e48ee5b54d10a5f2f9c1f7e4805518afb126c3` |
| `specs/004-agent-relay/contracts/ui-behavior.md` | 35 | FULL: 1-35 | `25c45aadeff4c907c2eb5aea6d90ef57edfd35290e35ea235e489985222f8dec` |
| `specs/005-safe-task-path/DATA-MODEL.md` | 41 | FULL: 1-41 | `a953175fcd0b0e54b9f171c6b4380004979416fb3638ee4cad733c1406b6078c` |
| `specs/005-safe-task-path/PLAN.md` | 49 | INVENTORY ONLY: none | `0e3d977b33ccc2777c80c45673430cf56ad2a181931d4b4f76b1c6b0b8e6fd8b` |
| `specs/005-safe-task-path/README.md` | 28 | INVENTORY ONLY: none | `d8d910ff3b1577e001508325bab9d1c79704436de69dc6dd6f8b8580fff26e71` |
| `specs/005-safe-task-path/RESEARCH.md` | 101 | INVENTORY ONLY: none | `6797846c168ca3ddb7455648c5547efc40abf789c75f1bde9395cf3fbb4e7f54` |
| `specs/005-safe-task-path/SPEC.md` | 343 | PARTIAL: 88-205 | `7d02459ee665356360ee5735ce19ae93180589881977f5520d0ce5ca00dd5568` |
| `specs/005-safe-task-path/TASKS.md` | 440 | INVENTORY ONLY: none | `849c73628347a0ebd449b59670a86117eff00249866d1e64a0c68fbae8f1f538` |
| `specs/005-safe-task-path/WIRING.yaml` | 98 | FULL: 1-98 | `9df259cf04cfcc9b41f2b3243cce596ee3dd3e2d3db7096a7286cc84eaa22045` |
| `specs/005-safe-task-path/contracts/README.md` | 9 | INVENTORY ONLY: none | `17f83e0c247dded860f400051fd12885da0fce956fb02f922cc37779c26f205c` |
| `specs/005-safe-task-path/contracts/api-events.md` | 28 | FULL: 1-28 | `687241adbde35ed5dd0ed25a8b5ae7b7e8022139994574eb0a1d3bfd493294e5` |
| `specs/005-safe-task-path/contracts/errors-recovery.md` | 36 | FULL: 1-36 | `79c7330173f26918f24db0261c29f0e2e073b96e35e6975f21cab9537edcf156` |
| `specs/005-safe-task-path/contracts/ui-behavior.md` | 44 | FULL: 1-44 | `41ed977a4a07cfcffefd12ab16be62b652b14091135d8facfda00c2323c3b2ac` |
| `specs/006-golden-connection-capability-write/DATA-MODEL.md` | 41 | FULL: 1-41 | `0ee0ff9231941e6f0d15d6fba4004342b50a523f9401c31f3468e0ece9d9be7d` |
| `specs/006-golden-connection-capability-write/PLAN.md` | 41 | INVENTORY ONLY: none | `f0aec8687dc1936949f820c919b4da2193eec41ef2044eb2c3248c9c1e730645` |
| `specs/006-golden-connection-capability-write/README.md` | 32 | INVENTORY ONLY: none | `0aa65d388a4bc7c3077c236a062fd8822207190ea743deb1da3e4be72f0a54c4` |
| `specs/006-golden-connection-capability-write/RESEARCH.md` | 98 | INVENTORY ONLY: none | `674462d556915ae924951ad9c65bf086d6b22eb5ba898159af886eaf882cf0f0` |
| `specs/006-golden-connection-capability-write/SPEC.md` | 328 | PARTIAL: 91-188 | `bb9cbf190ef6d012e4f26a50dfe7bb11867210a077548972b4a34124778407cf` |
| `specs/006-golden-connection-capability-write/TASKS.md` | 355 | INVENTORY ONLY: none | `ef22836186b2d447d18de6a247bba63bc64d86168f218d525a6d34407be423d3` |
| `specs/006-golden-connection-capability-write/WIRING.yaml` | 106 | INVENTORY ONLY: none | `411874a85b054761e4b5661bb14b93e5bbbd1bf8e8ceb4ba994db6b201c439fa` |
| `specs/006-golden-connection-capability-write/contracts/README.md` | 9 | INVENTORY ONLY: none | `835fa319e33639158d85f6d17aa577821ec39c5748c02a87341430c61e3aedf1` |
| `specs/006-golden-connection-capability-write/contracts/api-events.md` | 32 | FULL: 1-32 | `3d5bf3f0ae57110824b9c7cad44827a61a8954748f1d6183afa9813619607fde` |
| `specs/006-golden-connection-capability-write/contracts/errors-recovery.md` | 38 | FULL: 1-38 | `d769d59fa3d0d1178560118385702713fc3174945076cde13b28d173e4e7a7f4` |
| `specs/006-golden-connection-capability-write/contracts/ui-behavior.md` | 45 | FULL: 1-45 | `cef671b3390c275bcec4d42ab3c0edd8c53213fb0dc42197c1367923e3ec853a` |
| `specs/007-professional-artifacts/DATA-MODEL.md` | 43 | FULL: 1-43 | `0feb260179d36f6fe4434d07ff09eaa4971cc4318702a22785292fcf714c527f` |
| `specs/007-professional-artifacts/PLAN.md` | 42 | INVENTORY ONLY: none | `0b134ea1d5c532f2736a6f3e11586329c4d73fb2724c6e6f9b79bd09a17275c7` |
| `specs/007-professional-artifacts/README.md` | 34 | INVENTORY ONLY: none | `29bf5af99608f1f9dbdf3a8910292246443ebb0284120c789cf6cce2af52858b` |
| `specs/007-professional-artifacts/RESEARCH.md` | 89 | INVENTORY ONLY: none | `399d4ef289881380cbf9c1f5c004357734c98d283f696aeae0c6f7c899fc3ffd` |
| `specs/007-professional-artifacts/SPEC.md` | 362 | PARTIAL: 99-221 | `c154a43949ecdbf2a1294c1b57c0d3f571c031911674d9cb2145e10f46299fbe` |
| `specs/007-professional-artifacts/TASKS.md` | 345 | INVENTORY ONLY: none | `c50facabe5fc3382331022716bde316566263813469952b561499467af0bbc3d` |
| `specs/007-professional-artifacts/WIRING.yaml` | 112 | FULL: 1-112 | `5eba5d76848aec855490569aee276ae1694c3e27140aa3c8d9826e90d25169a2` |
| `specs/007-professional-artifacts/contracts/README.md` | 9 | INVENTORY ONLY: none | `6851fe2bea36897ed19bc2d207423b4e7b265d9713a282cb473c731d9e590ae4` |
| `specs/007-professional-artifacts/contracts/api-events.md` | 44 | FULL: 1-44 | `43d8d4df3de015f5d666db614ce9d4443af873021d5e09181ae6a8903b277f9f` |
| `specs/007-professional-artifacts/contracts/errors-recovery.md` | 34 | FULL: 1-34 | `ff59c2d0d62f117687fc296df40c686d5665b50714279b9853de92c052169e2d` |
| `specs/007-professional-artifacts/contracts/ui-behavior.md` | 42 | FULL: 1-42 | `c57a24333c722e0be3d580314fb7edd9297c1c10e025ea343ea2db87ff9197ed` |
| `specs/009-coworkers-scoped-memory/DATA-MODEL.md` | 44 | FULL: 1-44 | `ef0b286842f52dbd60bb0093bd06c68c303dea16a50e58be75a0eba0c3f03d05` |
| `specs/009-coworkers-scoped-memory/PLAN.md` | 41 | INVENTORY ONLY: none | `6aba18622a302eed4f61b7b4becfa8f7fa1964f14e239192b6a019731987a12a` |
| `specs/009-coworkers-scoped-memory/README.md` | 28 | INVENTORY ONLY: none | `ef72adb87d65fd470fe976d7dc7ba1ce828b95edfc64e6a8e6248b7769f676bd` |
| `specs/009-coworkers-scoped-memory/RESEARCH.md` | 71 | INVENTORY ONLY: none | `8804137cbc79c800098634a3d093de14deb9cc0f4e22848f0687ccfdbc2e2447` |
| `specs/009-coworkers-scoped-memory/SPEC.md` | 369 | PARTIAL: 99-227 | `edbac854c85292a2a085f9072f0444b885f19aca3705bac6ee52c71207e707b9` |
| `specs/009-coworkers-scoped-memory/TASKS.md` | 374 | INVENTORY ONLY: none | `217844323983b291a8ae6ea29dcd4b4e22801fdb7eb4fb6bea1bb3a3701375ee` |
| `specs/009-coworkers-scoped-memory/WIRING.yaml` | 74 | INVENTORY ONLY: none | `f2a5f37310ade5a78382a270ed09df341d7718e1f64f7352ae73ca25598f4f9b` |
| `specs/009-coworkers-scoped-memory/contracts/README.md` | 9 | INVENTORY ONLY: none | `2d1e501647932ae04ae0a03904fff83a9595706bbfb3bbcd4d527730c684360e` |
| `specs/009-coworkers-scoped-memory/contracts/api-events.md` | 51 | FULL: 1-51 | `307deeb1dc8e06d905226948435c6e1d8a2fb59f2a9750fae7d3791fbb914034` |
| `specs/009-coworkers-scoped-memory/contracts/errors-recovery.md` | 31 | FULL: 1-31 | `677fde43c92b7691a7e8eaec8069a68d3ff489385e31bfe8c8fe3a041989b5db` |
| `specs/009-coworkers-scoped-memory/contracts/ui-behavior.md` | 43 | FULL: 1-43 | `a1699fd90a5c66e270f57f797ce4ebc5c7edc268fa11cce46a1f59ae44462bb1` |
| `specs/010-automations-recurring-host/DATA-MODEL.md` | 39 | FULL: 1-39 | `218698c04034959b8bceeb09d93452127a5a05366c3c1101d678a4adb30494a0` |
| `specs/010-automations-recurring-host/PLAN.md` | 51 | INVENTORY ONLY: none | `bdb90a5837a3b3b0c0434647ad65b0ba23a086b3432588d0544453b02159f48c` |
| `specs/010-automations-recurring-host/README.md` | 33 | INVENTORY ONLY: none | `08480ce173eba01fc46a4869176ed38039628a92bee32c9d89115c77de39bfa0` |
| `specs/010-automations-recurring-host/RESEARCH.md` | 127 | INVENTORY ONLY: none | `eb5f0a9e923dfe21d1c5a877e37aa6b5ea26a4891802ae8bb377ecee3032a53a` |
| `specs/010-automations-recurring-host/SPEC.md` | 411 | PARTIAL: 96-206 | `1f555275b96e2d939cd5690c44807caec1b2e962e94aca98ab5882e52ca32853` |
| `specs/010-automations-recurring-host/TASKS.md` | 392 | INVENTORY ONLY: none | `3fc640b0187851c6c9cd3b4d1c7100933df9734007b7a0218381ff4c934152f2` |
| `specs/010-automations-recurring-host/WIRING.yaml` | 94 | INVENTORY ONLY: none | `7d1fa2bddb3adb8a65a81b9a65224782409392073a65c9bc4b5dea3ff90a192f` |
| `specs/010-automations-recurring-host/contracts/README.md` | 9 | INVENTORY ONLY: none | `24741dab3370e445f2a34e24d907fec21c72722cf3cad00c6b58618980baf307` |
| `specs/010-automations-recurring-host/contracts/api-events.md` | 16 | FULL: 1-16 | `ac5add414becc8d428bd00ec1f06250dcaae5f6c69a6620c6fef9ff15a28d040` |
| `specs/010-automations-recurring-host/contracts/errors-recovery.md` | 16 | FULL: 1-16 | `3045692e91b1ccf1cca0360dc36ca8356929af4f64b13ef034d9dbff4dc0be58` |
| `specs/010-automations-recurring-host/contracts/ui-behavior.md` | 42 | FULL: 1-42 | `40fb60ac7664d2be324feb4cb6341751e7679abe691c8c7eeb4b8c10d7d06991` |
| `specs/011-desktop-shell-ipc-settings-runtime/DATA-MODEL.md` | 40 | FULL: 1-40 | `e68508947e6e81e7f52457f50183718cbba8d7c20ac865cdabf99e526fea9637` |
| `specs/011-desktop-shell-ipc-settings-runtime/PLAN.md` | 55 | FULL: 1-55 | `0f666af39b46e0563f10495c6b48e1bc8e59bb94fa79e88d98e90b3cb0cf09a7` |
| `specs/011-desktop-shell-ipc-settings-runtime/README.md` | 40 | INVENTORY ONLY: none | `b3e708895b832da01a6e3d50ffd59813c04c679760c575f435626c08e64c6a3e` |
| `specs/011-desktop-shell-ipc-settings-runtime/RESEARCH.md` | 83 | INVENTORY ONLY: none | `2e8c613fc46acf89a3408da1963ea221f4d8fad3664f73965bc6e9c8ed86d95a` |
| `specs/011-desktop-shell-ipc-settings-runtime/SPEC.md` | 371 | PARTIAL: 88-181 | `1908aba0690db2e726d34a3197785355a568a818f950c5011f63c7b9e3ca6f7f` |
| `specs/011-desktop-shell-ipc-settings-runtime/TASKS.md` | 503 | INVENTORY ONLY: none | `52e38f04c139d8c37880e32dce1179a7400467f73d183e9e3fb4b4cd921e9984` |
| `specs/011-desktop-shell-ipc-settings-runtime/WIRING.yaml` | 127 | FULL: 1-127 | `23ad8ecd5e45c96f72ef06b215e34c0f41125cb391e67465b6284c5abaf0e286` |
| `specs/011-desktop-shell-ipc-settings-runtime/contracts/README.md` | 9 | INVENTORY ONLY: none | `2900cede772888e025fc1e77e0c9225873a367b6d444e4682202bb90f4ecc3bd` |
| `specs/011-desktop-shell-ipc-settings-runtime/contracts/api-events.md` | 9 | FULL: 1-9 | `ad24e72dd1abd610a22382dde496ae2aae51f1a52be0863f2464af9a6be32410` |
| `specs/011-desktop-shell-ipc-settings-runtime/contracts/errors-recovery.md` | 15 | FULL: 1-15 | `53ec41475b98d4c186ba059e8abb96e2e769d7839ec40f6c784dde1de205f770` |
| `specs/011-desktop-shell-ipc-settings-runtime/contracts/ui-behavior.md` | 72 | FULL: 1-72 | `7d68433b738d800676a3e63d55a2203608573b8f66fe0be0f29372cfd27d79a0` |
| `specs/012-conversation-coedit-take-control/DATA-MODEL.md` | 37 | FULL: 1-37 | `68957fe88617b57f7b573095e87828537536b387a57bf42d21c47de6c525166e` |
| `specs/012-conversation-coedit-take-control/PLAN.md` | 37 | INVENTORY ONLY: none | `d664014406fe1dd8cfd5de808cc4f5cbe1d413879612e31854bb6a292ef2a4a9` |
| `specs/012-conversation-coedit-take-control/README.md` | 28 | INVENTORY ONLY: none | `ff55b88926199bba9306cad0d94e9cff47b0901f95543b383c8665547b9c28a3` |
| `specs/012-conversation-coedit-take-control/RESEARCH.md` | 72 | INVENTORY ONLY: none | `4d21e2852024f8a0524cead04b85b3ab78f22e474c9c2b162251bdc09661021a` |
| `specs/012-conversation-coedit-take-control/SPEC.md` | 266 | FULL: 1-266 | `05436505bb3590f106fd8564b097ed3fd4f2bf75b6f9eda4b77186b0ef8f48c8` |
| `specs/012-conversation-coedit-take-control/TASKS.md` | 240 | INVENTORY ONLY: none | `0d12c90b75ac257597c37d66313f3caa751bbe90dc2d17d2bd6eac14ac9efadd` |
| `specs/012-conversation-coedit-take-control/WIRING.yaml` | 70 | FULL: 1-70 | `ce2e5a5644f32ffb0c333601a8007ae83e727dd891652f340b4904a18777f920` |
| `specs/012-conversation-coedit-take-control/contracts/README.md` | 9 | INVENTORY ONLY: none | `ac227c419c46e6850af0f19d8b81bc5031caedba601bb8b8d8f9e4da7d342d04` |
| `specs/012-conversation-coedit-take-control/contracts/api-events.md` | 28 | FULL: 1-28 | `e0d2d16a60726fffb06cf46702be0909bd829e93985316d58d88dd1031d31952` |
| `specs/012-conversation-coedit-take-control/contracts/errors-recovery.md` | 35 | FULL: 1-35 | `cc03339138fc71477b4c5458e22c8075f087885bf86ab79dd6f64b889c7197b3` |
| `specs/012-conversation-coedit-take-control/contracts/ui-behavior.md` | 43 | FULL: 1-43 | `87c950899af477b00aac46c86c2f3531af65c5796af10d0afdcf39cbfd183b4a` |
| `specs/013-litespm-package-lifecycle-bridge/DATA-MODEL.md` | 60 | FULL: 1-60 | `7a909da32c63575a9d8674add82a901d02fd4b49542dad021cc6312a3f9a0da7` |
| `specs/013-litespm-package-lifecycle-bridge/PLAN.md` | 113 | INVENTORY ONLY: none | `cf6326eb8c891f971fa45598863f422a3b8051574ca3fc75d339501a7f9fc240` |
| `specs/013-litespm-package-lifecycle-bridge/README.md` | 30 | INVENTORY ONLY: none | `d1ef099e69b0705ee83f671ba073b6046888eca6d420ea70c7a57a10b6523888` |
| `specs/013-litespm-package-lifecycle-bridge/RESEARCH.md` | 68 | INVENTORY ONLY: none | `7b99263c77a48978d0622d84bdf6705bc432c7972222319370b5f079bca4a01f` |
| `specs/013-litespm-package-lifecycle-bridge/SPEC.md` | 323 | PARTIAL: 88-217 | `97dec8997894b12cafacfae09b94d7c44e599fbd830176fb30f4a1a3910c638b` |
| `specs/013-litespm-package-lifecycle-bridge/TASKS.md` | 444 | INVENTORY ONLY: none | `aa59ec1dad2f676cc0057ab03f10eebe1ecd854f05655b996dc58ec32c57fa19` |
| `specs/013-litespm-package-lifecycle-bridge/WIRING.yaml` | 168 | INVENTORY ONLY: none | `e75e6b90f976206d34dcfe2273efbf2ae2f1f8028d1d194807f842b15a464380` |
| `specs/013-litespm-package-lifecycle-bridge/contracts/README.md` | 12 | INVENTORY ONLY: none | `1fe6a14e99e977516dc479e50aa676b3834e98947347f1826ee589272163ccbb` |
| `specs/013-litespm-package-lifecycle-bridge/contracts/api-events.md` | 43 | FULL: 1-43 | `ae346d2f848429a4510e7ed04abb66c98ea5aa9056d61b371771684f6307bfb2` |
| `specs/013-litespm-package-lifecycle-bridge/contracts/errors-recovery.md` | 44 | FULL: 1-44 | `d9138839bfe8a2c2720d26092d84a56a789d70b5acc7544ee0487143a3b907c0` |
| `specs/013-litespm-package-lifecycle-bridge/contracts/ui-behavior.md` | 57 | FULL: 1-57 | `939daecdf4b2e84f652f8564a24779a0a387ba203a08462f0b60ecf7fc981774` |
| `specs/014-conversation-history-management/DATA-MODEL.md` | 65 | FULL: 1-65 | `26722aff9aff258b9ddbc023e935322bdab1485b0294fd56c2e0a07fa7ce1db2` |
| `specs/014-conversation-history-management/PLAN.md` | 101 | INVENTORY ONLY: none | `f631c50fbcb2b2b086a7106b603461c2e4193d7e5c2b21b0e45a77b84bbe0f66` |
| `specs/014-conversation-history-management/README.md` | 28 | INVENTORY ONLY: none | `a69d363751252a16a98976e3389bb9cb6b691e077ff910d623874efd53337f47` |
| `specs/014-conversation-history-management/RESEARCH.md` | 52 | INVENTORY ONLY: none | `63b921a45eaa52b3cba8fa6e13ebdd0977579bc599c413d362a2aa5a4df27a71` |
| `specs/014-conversation-history-management/SPEC.md` | 306 | PARTIAL: 83-199 | `af339a466218f034ae9451698a2802024c88334fb53df4cfb6d5d680b1d0f96a` |
| `specs/014-conversation-history-management/TASKS.md` | 380 | INVENTORY ONLY: none | `29e4bd439f72a1393279de76c98c7c8d10ef8a5b2828bf36733969881b7b98f2` |
| `specs/014-conversation-history-management/WIRING.yaml` | 158 | INVENTORY ONLY: none | `41bf10a36197bf8de24076e9aa2cba4e1d6526ed4533981abb6100ed8aca67f8` |
| `specs/014-conversation-history-management/contracts/README.md` | 12 | INVENTORY ONLY: none | `328c357ab855f686d78c14dc4b6bddb3a3b7691dfe9b353cca68b436d6cf221a` |
| `specs/014-conversation-history-management/contracts/api-events.md` | 33 | FULL: 1-33 | `dfd8e0d6c386d4e79f85e41354abe217fd7b6b97f945e857508928e064ab0581` |
| `specs/014-conversation-history-management/contracts/errors-recovery.md` | 55 | FULL: 1-55 | `1f9821d823e8e8189813db345f1ba40e7396e4584e70a0f993b8fce26bb517b2` |
| `specs/014-conversation-history-management/contracts/ui-behavior.md` | 56 | FULL: 1-56 | `2aad9e19708dfac0ae708905cf753c5012540bce579bc187d2f0e71c5b9f1f57` |
| `specs/015-task-control-delegation/DATA-MODEL.md` | 63 | FULL: 1-63 | `49a7633daa5a14f325b962b62628a7d031f4e873e941d9bf61a4100051ea726d` |
| `specs/015-task-control-delegation/PLAN.md` | 97 | INVENTORY ONLY: none | `ca6d6715ee7f89c05dfb2eceffe43d11938c54eaac426fc0536a3f7d6920d354` |
| `specs/015-task-control-delegation/README.md` | 35 | INVENTORY ONLY: none | `6a7311aeaa250e22f32e080df56061535ff444528880a9a03088c5fbdb1b467d` |
| `specs/015-task-control-delegation/RESEARCH.md` | 70 | INVENTORY ONLY: none | `eccd080a9a6caba67675aec120cf2e96afc1e618bdf5ebe949cc14c312c63241` |
| `specs/015-task-control-delegation/SPEC.md` | 350 | PARTIAL: 89-245 | `899f4f04d704af70aa40717e6254a233eee044d18e5052f7ad221bac318ff8da` |
| `specs/015-task-control-delegation/TASKS.md` | 411 | INVENTORY ONLY: none | `bcb34df4f5fe5b9a1c204a2796dfc9b29dd5031ed735f54541eea87419a28a1d` |
| `specs/015-task-control-delegation/WIRING.yaml` | 209 | FULL: 1-209 | `73a7abd37945689c14e866c4899e06472f22114960aac482ea23ed043c14e663` |
| `specs/015-task-control-delegation/contracts/README.md` | 12 | INVENTORY ONLY: none | `4c4aa4fe225c0bc0e1ff6fe9d921db1f0f2a03888fb3c842cd7154c433ff8924` |
| `specs/015-task-control-delegation/contracts/api-events.md` | 42 | FULL: 1-42 | `f0830840d027397ec01c347c390b46fcb7f02d32227410c4fdb1ae2b8c629d68` |
| `specs/015-task-control-delegation/contracts/errors-recovery.md` | 46 | FULL: 1-46 | `0a7c35e1e0e4b18bafe3b2be7970dc69f44ed25d4ccfc0f212a273c81e278d35` |
| `specs/015-task-control-delegation/contracts/ui-behavior.md` | 73 | FULL: 1-73 | `177a983018e152f7b86f7ef8114e22842fb198826dd9a4279389f5581a1fe6fe` |
| `specs/017-data-portability-backup-restore/DATA-MODEL.md` | 76 | FULL: 1-76 | `69a3a9c3f446a8172d1ebfcf8d163686360032021ac3873f4c3708ebdfcfa308` |
| `specs/017-data-portability-backup-restore/PLAN.md` | 88 | FULL: 1-88 | `a6ae2367c1ee8c4b181db5da3e50cac516e95ac7aef8cf2e607b3c8efca005cf` |
| `specs/017-data-portability-backup-restore/README.md` | 32 | INVENTORY ONLY: none | `c2741565a8cabccad49258209f33e452ef76b484e0bc1d909c2760d776bfc881` |
| `specs/017-data-portability-backup-restore/RESEARCH.md` | 77 | INVENTORY ONLY: none | `b7abefbb65faa2829047f9fbe9f4a33b6680c71d1844aaaaf3529a19623ccd0c` |
| `specs/017-data-portability-backup-restore/SPEC.md` | 347 | PARTIAL: 93-236 | `a45743f32503f21201f51acc77fddc1b98288ee671624ff7b99f558420d6a1a1` |
| `specs/017-data-portability-backup-restore/TASKS.md` | 387 | INVENTORY ONLY: none | `53a32188f574cb35dcf713d3e4ccab4858b437189a40c33b7e111c03a2ada331` |
| `specs/017-data-portability-backup-restore/WIRING.yaml` | 206 | INVENTORY ONLY: none | `95cdc3499c0c01f2aae13f96673cf5fd4c9c80f1f2dbfb399a5d5e32e4be8b54` |
| `specs/017-data-portability-backup-restore/contracts/README.md` | 12 | INVENTORY ONLY: none | `3b10fe62d11a1b5681925166f36f56fd35bb8842212f76c4d5595e5457f40722` |
| `specs/017-data-portability-backup-restore/contracts/api-events.md` | 46 | FULL: 1-46 | `37a61f99e911dd769448a69633456648690f9be587d2e57d2c36f07ed4cba735` |
| `specs/017-data-portability-backup-restore/contracts/errors-recovery.md` | 40 | FULL: 1-40 | `1d238da6043f861128bb1cd379a26c6937621e99764a1cc25dbd39535d3c3800` |
| `specs/017-data-portability-backup-restore/contracts/ui-behavior.md` | 57 | FULL: 1-57 | `6d384846d21663f26d576fe768c6b562697eaa2ee5bde64e29bdfc1b1db85463` |
| `specs/018-usage-diagnostics-observability/DATA-MODEL.md` | 68 | FULL: 1-68 | `1345f470f1ce352697f4dd804bc6f0d5ef933a962d66c3fc3b560aea7c75828c` |
| `specs/018-usage-diagnostics-observability/PLAN.md` | 88 | INVENTORY ONLY: none | `3401015488eaf41febb3306742440a28e151f93fa7777ef8fff7d23b4ad3fdc6` |
| `specs/018-usage-diagnostics-observability/README.md` | 34 | INVENTORY ONLY: none | `5addb625b192e2d3a8f3d745ad6487d14c1da46433e2dc3e4f5468a44fea147d` |
| `specs/018-usage-diagnostics-observability/RESEARCH.md` | 67 | INVENTORY ONLY: none | `b126d60a06bdb70579d63acf0ded7617b7265f28e8e9d38cd9859913da00d94c` |
| `specs/018-usage-diagnostics-observability/SPEC.md` | 342 | PARTIAL: 92-233 | `9f652b364df80d7df42f4a75755f516b51fd678845e66b79df694efbb534c283` |
| `specs/018-usage-diagnostics-observability/TASKS.md` | 418 | INVENTORY ONLY: none | `a47009b181837a9106ed9049868cbb98b71ca258a1e90bcdb49bcae7dac9837a` |
| `specs/018-usage-diagnostics-observability/WIRING.yaml` | 206 | INVENTORY ONLY: none | `345fee4341710b4c527947e870f7a5184d22b2406802af6b6857f728ddbe8529` |
| `specs/018-usage-diagnostics-observability/contracts/README.md` | 12 | INVENTORY ONLY: none | `d9a3aab101159de96c3c521a0af05cfdaa622287cc7fdacab02251f66c38b9c0` |
| `specs/018-usage-diagnostics-observability/contracts/api-events.md` | 38 | FULL: 1-38 | `dd5ddf4b16b59d73a286282fe5ac2374b609953dd05815a336a43d529574c80c` |
| `specs/018-usage-diagnostics-observability/contracts/errors-recovery.md` | 33 | FULL: 1-33 | `4c17242c0c1605a5d523e64c290f4202c5fdda021fcf3f119b67d4d4fc958c2b` |
| `specs/018-usage-diagnostics-observability/contracts/ui-behavior.md` | 62 | FULL: 1-62 | `71238b20f96ab6b58cb7ccbbed410771b0f76baa9d6b8c37bd15bed478804f95` |
| `specs/019-release-engineering-qualification/DATA-MODEL.md` | 54 | FULL: 1-54 | `3685c2e8a16e8bb23472143c35336b5361c552d851950c4dede64824b1752223` |
| `specs/019-release-engineering-qualification/PLAN.md` | 106 | INVENTORY ONLY: none | `d6b46795c632b8352a8e54719bccbda3573059ac183b7012e951217f013d3c64` |
| `specs/019-release-engineering-qualification/README.md` | 29 | INVENTORY ONLY: none | `61f6df91414726ddf80fcbd9e693eaa4d83fc5f827bc64066fa68d1503755a54` |
| `specs/019-release-engineering-qualification/RESEARCH.md` | 67 | INVENTORY ONLY: none | `b6b927ee0052cf9cd7930e50e2005624d9d54e17009e9c74d22b018854462bc7` |
| `specs/019-release-engineering-qualification/SPEC.md` | 299 | PARTIAL: 93-188 | `07e5c721e340987f5579b76e6806d7a99feeae951024a4ea38d22e391a8aaf98` |
| `specs/019-release-engineering-qualification/TASKS.md` | 589 | INVENTORY ONLY: none | `105a380f45bb4d9976fe03835afe20309708214fa07e22328877f3115153153b` |
| `specs/019-release-engineering-qualification/WIRING.yaml` | 261 | INVENTORY ONLY: none | `a233bc98537912a0646424e73e96ca5ac76dd692c13978301f08f53a3f49e893` |
| `specs/019-release-engineering-qualification/contracts/README.md` | 12 | INVENTORY ONLY: none | `5676ead5973ccd3ea11ee178072fc402318a64bb9ca3fe89ffc8310c4d28115e` |
| `specs/019-release-engineering-qualification/contracts/api-events.md` | 50 | FULL: 1-50 | `5ae3472a846380f24643b223de4f558a3a61973b05098e65f416f965186bafbb` |
| `specs/019-release-engineering-qualification/contracts/errors-recovery.md` | 37 | FULL: 1-37 | `c579f283bc194e48dd2757eb72169646749b2628e685098af93e863ce4c3f8b3` |
| `specs/019-release-engineering-qualification/contracts/ui-behavior.md` | 62 | FULL: 1-62 | `6a5ca7492e3123040f4fde8aea27002c48163fec22aa109e721e9a5c48ef8c28` |
| `specs/020-proactivity-quiet-learning/DATA-MODEL.md` | 60 | FULL: 1-60 | `eef14b13c0a008b1a187c4e367ceb20ddc0c8926dbf205d56bc06622df45bdbf` |
| `specs/020-proactivity-quiet-learning/PLAN.md` | 82 | FULL: 1-82 | `c8d209e253dabef7b447889bcb525833aa62c788cbcfddb806f85d20e06a7ee9` |
| `specs/020-proactivity-quiet-learning/README.md` | 29 | INVENTORY ONLY: none | `3658046a4c80cc86c188c41098f779254fecef9bd2363e59c6b887df81eafcbc` |
| `specs/020-proactivity-quiet-learning/RESEARCH.md` | 44 | INVENTORY ONLY: none | `e3bef086755dbc90560e4346cf2e81a2b3fe833110c0373ac24d6d5ce9694faa` |
| `specs/020-proactivity-quiet-learning/SPEC.md` | 326 | PARTIAL: 86-222 | `383294a0df47281fe7e81dd413f85bf24146468cf9ca33a68d04981117fbcfa1` |
| `specs/020-proactivity-quiet-learning/TASKS.md` | 359 | INVENTORY ONLY: none | `3ffadca3df28cfdc2f77e5963739bfe771d8e292cf19f8db50c30e5e77398b9d` |
| `specs/020-proactivity-quiet-learning/WIRING.yaml` | 260 | INVENTORY ONLY: none | `58c8ffdaafeb115c5d82ceb3794da29adfd52567b1d1156de37df161b06808d6` |
| `specs/020-proactivity-quiet-learning/contracts/README.md` | 12 | INVENTORY ONLY: none | `f19b22c4351109982a82e6ca6e6c106074ad8c3a6d7b762f6204722be9061257` |
| `specs/020-proactivity-quiet-learning/contracts/api-events.md` | 52 | FULL: 1-52 | `d72d2fc847d9a997432d44073b7b209fad468dfc0fd6a53cee22aebf4b602172` |
| `specs/020-proactivity-quiet-learning/contracts/errors-recovery.md` | 45 | FULL: 1-45 | `6711886e9a27a86ea884fa5612e5fd4ddd4ff39376cb6ea91b5fda2ea54097b3` |
| `specs/020-proactivity-quiet-learning/contracts/ui-behavior.md` | 64 | FULL: 1-64 | `284c7456032cdd1d9209b156d09fc71648a32982284e4fbfa2adb7fe58fd88d2` |
| `specs/README.md` | 64 | INVENTORY ONLY: none | `9801dee725cdffe609fc171b1350fd99cf1ec5e58384d71b6740a80aa6eef361` |

## Supplemental current-tree review — 2026-11-12

The continuation reviewer re-read every line in the 11 normative files for Features 006, 007, 008 and 016, including each package's README, SPEC, PLAN, TASKS, DATA-MODEL, RESEARCH, WIRING and four contract files. The line spans below are the exact physical lines present at source checkpoint `8432397befc69bafd3335a0c30b2f1e9e82bef5e`, before the documentation-only continuation edits were committed. A full-file diff review was then done for each edited path. Package manifest hashes are SHA-256 over UTF-8/LF records sorted by repository path, where each record is `<individual-file-SHA256><two spaces><repository-path><LF>`.

| Feature | Files | Lines read | Per-file full semantic coverage | Package manifest SHA-256 |
|---|---:|---:|---|---|
| 006 Golden Connection and capability write | 11 | 1,137 | `DATA-MODEL.md` 1-41; `PLAN.md` 1-41; `README.md` 1-34; `RESEARCH.md` 1-106; `SPEC.md` 1-328; `TASKS.md` 1-357; `WIRING.yaml` 1-106; `contracts/README.md` 1-9; `contracts/api-events.md` 1-32; `contracts/errors-recovery.md` 1-38; `contracts/ui-behavior.md` 1-45 | `a16269b48dacb3573e855227cff0413a1f3439973568bb6c79dfbfcfeffebd47` |
| 007 Professional Artifacts | 11 | 1,159 | `DATA-MODEL.md` 1-43; `PLAN.md` 1-42; `README.md` 1-36; `RESEARCH.md` 1-89; `SPEC.md` 1-362; `TASKS.md` 1-346; `WIRING.yaml` 1-112; `contracts/README.md` 1-9; `contracts/api-events.md` 1-44; `contracts/errors-recovery.md` 1-34; `contracts/ui-behavior.md` 1-42 | `05c1b71db3e1c8d45cb231f7452c54df95f621e9184506e67dc386db57916832` |
| 008 Workbench, Browser, Terminal and Computer | 11 | 1,580 | `DATA-MODEL.md` 1-64; `PLAN.md` 1-62; `README.md` 1-38; `RESEARCH.md` 1-114; `SPEC.md` 1-377; `TASKS.md` 1-548; `WIRING.yaml` 1-176; `contracts/README.md` 1-11; `contracts/api-events.md` 1-50; `contracts/errors-recovery.md` 1-52; `contracts/ui-behavior.md` 1-88 | `7b19c57c8ca69e026a354127dbdaa45ef8bfc00812d69befaf3ef19f3a941791` |
| 016 Rich Presentation and Intelligent UI | 11 | 1,508 | `DATA-MODEL.md` 1-60; `PLAN.md` 1-120; `README.md` 1-39; `RESEARCH.md` 1-100; `SPEC.md` 1-324; `TASKS.md` 1-458; `WIRING.yaml` 1-205; `contracts/README.md` 1-14; `contracts/api-events.md` 1-47; `contracts/errors-recovery.md` 1-57; `contracts/ui-behavior.md` 1-84 | `bc1583f807597dd10b65b2f6058a748844be03cf2385ccbb475cebc549a419d8` |

Supplemental total: **44 files / 5,384 lines**. Features 006 and 007 were partly or not semantically covered in the original baseline, while Features 008 and 016 were excluded from that reviewer assignment. Because some 006 lines overlap baseline coverage, do not add 5,384 mechanically to the original 10,356 and call the result a unique-line total.

### Cross-layer and source cross-check performed

- Rechecked the accepted root architecture, product/V1 boundary, screen/action contracts, compact workspace design, Rich Response, Presentation Runtime, ACP pipeline and the active feature owners against the supplemental packages.
- Read current source paths for ACP session creation/normalization, the five-kind rich-presentation validators and store, read/publish route call sites, Tauri bridge, React renderer and Workbench surface. Those source paths did not change between `64c4351` and `8432397`; the relevant findings remain CODE/SERVED foundations only. This was a bounded source cross-check, not a line-by-line read of all `apps/` and `crates/` source.
- Checked official interoperability/product material for ACP content/tool/permission semantics, A2UI, AG-UI, MCP Apps, ChatGPT Canvas/navigation and Claude Artifacts. Feature 016 RESEARCH records useful patterns and rejected adoptions. A2UI status remains internally inconsistent across official pages; the React package/version and conformance pin are unresolved. No dependency or upstream code was adopted.
- Found and reconciled these additional cross-owner issues: A-80 Connection/read versus Task-write qualification ordering; A-81 Artifact file-write/export-receipt crash gap; A-82 GitHub issue-comment OAuth permission mismatch; A-83 interactive PTY per-command receipt evidence boundary; A-84 Feature 016 baseline/extension acceptance split (design conflict resolved, owner gates remain open); A-85 MCP proposal input, host sequence and replay identity. M17 remains a secondary-navigation focus recovery issue.
- A final stale-index scan found old 006/007 blocker summaries and obsolete Feature 016 coverage wording. The current status entries in `specs/README.md`, `docs/V1-FEATURE-SDD-REGISTER.md`, `docs/SPEC-TASK-TRACEABILITY.md`, `docs/V1-PRODUCT-FEATURE-COVERAGE.md` and the contradiction register were corrected to reflect A-80/A-81/A-82/A-84/A-85. Dated 2026-11-09 research and owner-acceptance records were left historical rather than rewritten.

### Scope remains open

This supplement does not complete the user's all-document/all-code line-by-line request. The original ledger still includes inventory-only and partial coverage for other feature packages; most global design documents and the active Rust/Tauri/React source have not received a complete semantic reread. Keep the overall audit OPEN and continue with explicit path/range/hash accounting; do not claim a full repository audit from this supplement or from structural validators.

## Supplemental Feature 005 review — 2026-11-12

The reviewer read all 11 current Feature 005 normative files line by line before the latest corrections below. The table's counts and hashes are for the reviewed post-correction content; edited paragraphs and the full intended-path diff were reread after each correction. Per-file SHA-256 values hash the exact current file bytes. The package manifest is recomputed from UTF-8/LF records sorted by repository path, each record `<raw-file-SHA-256><two spaces><repository-path><LF>`.

| Feature | Files | Lines | Current full-file review coverage | Package manifest SHA-256 |
|---|---:|---:|---|---|
| 005 Safe Task Path | 11 | 1,430 | `README.md` 1-28; `SPEC.md` 1-393; `PLAN.md` 1-49; `TASKS.md` 1-446; `DATA-MODEL.md` 1-47; `RESEARCH.md` 1-105; `WIRING.yaml` 1-240; `contracts/README.md` 1-9; `contracts/api-events.md` 1-30; `contracts/errors-recovery.md` 1-37; `contracts/ui-behavior.md` 1-46 | `259b72204b31fba6da4bf181131ebd6b916e06a21936de25ce7124462ab6161a` |

Per-file SHA-256:

| File | Lines | SHA-256 |
|---|---:|---|
| `specs/005-safe-task-path/README.md` | 28 | `f699572f659abc824b2a9f0c2a0f4427cbbc5c42933e87fcb03a8ebab854eaa5` |
| `specs/005-safe-task-path/SPEC.md` | 393 | `c012298daa897699577be4ee910a1082fc68b2965c024a88e6cd775719d4bc2b` |
| `specs/005-safe-task-path/PLAN.md` | 49 | `0e3d977b33ccc2777c80c45673430cf56ad2a181931d4b4f76b1c6b0b8e6fd8b` |
| `specs/005-safe-task-path/TASKS.md` | 446 | `df9ac27946ea8c4f4b4643ce8217487c41954ae61dfec51c318a819e210777b0` |
| `specs/005-safe-task-path/DATA-MODEL.md` | 47 | `c328765ae929f7b47dc3ea4935e47f0e9d4fa617cf7d568c709fdaa106b303df` |
| `specs/005-safe-task-path/RESEARCH.md` | 105 | `eefcc259dddfbda56e482e589259a0938f661c0127f128e8357512c7bad6eed6` |
| `specs/005-safe-task-path/WIRING.yaml` | 240 | `cb37e18d11be31224fdc49169296c8fe37b1a9836ce7d6390fed765ba8d86b2e` |
| `specs/005-safe-task-path/contracts/README.md` | 9 | `17f83e0c247dded860f400051fd12885da0fce956fb02f922cc37779c26f205c` |
| `specs/005-safe-task-path/contracts/api-events.md` | 30 | `ac2d7d80355cca8fc5f933a2016f8df8d40ef173eb9c74327b14006af89f4a9b` |
| `specs/005-safe-task-path/contracts/errors-recovery.md` | 37 | `a44b577d74304cca37b3758168af61571955f68a2d78b3b251337399d193e5c8` |
| `specs/005-safe-task-path/contracts/ui-behavior.md` | 46 | `6576389cc74ca846bdd30aa4bc756a624945286e04b77c2e0961d29dbab440c3` |

### Source and owner-contract cross-check

- `apps/litecoworkd/src/agents/planning.rs` prepares the bounded packet but sets `dispatch_available: false`; `require_dispatch()` fails closed. `apps/litecoworkd/src/operator.rs` planning readiness checks Workspace ownership and Task version, and returns the four dispatch/planning/session/plan booleans as false. This is a read-only preflight, not a planner journey.
- `apps/litecoworkd/src/environment_provider.rs::probe_environment_provider` reports observations. A trusted Linux Bubblewrap candidate can expose a candidate LocalWorkspace class, but the probe does not provision or attest a Task Environment. The Windows branch reports no Task-capable LocalWorkspace class/lifetime; Job Object support remains process-lifetime evidence only.
- `crates/storage-sqlite/src/lib.rs::accept_initial_plan_transaction` is CODE: it checks authenticated Workspace ownership, Task/spec/status/version, the current lead planner's active session/runtime/host binding, request digest/replay, and commits Task aggregate/current-plan pointer, PlanRevision, Steps, `task.plan.revised.v1`, `step.created.v1` events and receipt in one immediate transaction. The submit route is target-only and the planner cannot currently start; missing negative/replay/race/rollback coverage is not implied by the active-producer test.
- `crates/storage-sqlite/src/effect_evidence.rs` has separate Effect proposal, non-STARTED transition, and Evidence append writers; the STARTED transition fails closed while atomic Invocation/Trust/ApprovalUse admission is missing. `docs/STORAGE.md` requires mediated invocation dispatch to bind Trust, Effect, ApprovalUse and lease admission atomically.
- `rg admit_step_attempt apps crates` found the port/domain/storage definitions and no production daemon caller. `apps/litecoworkd/src/attempt_process_scope.rs` remains `allow(dead_code)`; neither a production Environment attestation producer nor the recovery journal is wired.
- `apps/litecowork-ui/src/App.tsx` has a standalone Home “Save a Task” flow and read-only Task detail. `apps/litecowork-ui/src/conversations/ConversationsPage.tsx` has no Conversation-to-Task promotion action. The sidebar renders four permanent workspace destinations, Needs You, Recent Chats/New Chat and footer Settings; secondary routes are grouped under More, while the More disclosure key changes on a secondary route and can remount/drop focus (M17).
- Re-read `docs/TASK-RUNTIME.md`, `docs/ENVIRONMENTS.md`, `docs/TRUST.md`, `docs/STORAGE.md` and `docs/EVENTS.md` against the feature. The planner gate and Attempt admission require Runtime-private current-incarnation Environment evidence, but their Environment identity and lifecycle relation remains open under A-70; provider actions stay outside SQLite. Do not assume the direct B-18 writer can commit Attempt/lease and then perform a later independent Trust check: A-88 must resolve how it satisfies Storage's atomic mediated-dispatch rule.

### Corrections and open owner decisions

- M06/A-71 described the baseline conflict between a bundled invocation-intent record and WIRING. The current Feature 005 SPEC no longer prescribes an `invocation_intent` record in Attempt admission, and WIRING now lists owner-separated Attempt, Effect, Evidence and verification boundaries. This removes the baseline mismatch; it does not settle whether the direct B-18 writer needs an intent or which transaction admits it. A-88 remains OPEN, and no intent can substitute for a mediated Effect receipt.
- A-86 removes caller-supplied `IsolationAttestation`/Environment handles from the target request; the proof is Runtime-private and current-incarnation scoped. A-89 keeps unmediated native capabilities as a hard planning blocker, not a disclosed-only limitation.
- A-87 remains OPEN: the approved exact new bytes have no canonical durable content reference, bounded preview, digest binding, lost-response replay or restart lookup.
- A-88 remains OPEN: the prior bundled Attempt/lease/grant/`Effect PROPOSED` prose and grouped settlement inventory conflicted with owner transaction boundaries. The wiring now lists owner boundaries, but the direct writer must still be mapped to mediated Invocation dispatch or another owner-approved atomic composition. Storage's Trust/Effect/ApprovalUse/lease atomicity cannot be weakened by Feature 005.
- A-90 remains OPEN: `AdmitFileWriteAttempt.grant` is not defined as an untrusted scope request versus a durable Trust-issued grant. T01 must define owner lookup/creation, Attempt binding and one-time ApprovalUse semantics; request data cannot assert authority.
- A-70 remains OPEN across Features 005/015 for the planner-gate versus Attempt Environment identity/lifecycle, child Attempt provisioning order and cleanup owner. The Feature 005 source checkpoint has T02's safe type/probe CODE slice ahead of its T01 dependency; preserve it but do not extend it into provider integration or admitted execution. The T03 storage transaction is CODE only, not a served plan submission or usable planner.
- The supplied intelligent-UI essay was compared with existing Feature 016, `docs/RICH-RESPONSE.md`, `docs/INTELLIGENT-UI-AND-IMAGE-CONTENT.md` and `docs/ACP-PRESENTATION-PIPELINE-LLD.md`. The useful declarative registered renderer, deterministic local computation, semantic fallback, inline-to-Canvas continuity and provenance are already represented. ACP itself does not define a component tree; an optional narrow proposal tool remains blocked by A-85/provider-session evidence. Arbitrary generated React/HTML/JS, a hidden second model pass, global BYOK router, Next.js/PostgreSQL replacement and duplicate UI schema are not adopted. Feature 016's persistent state/conversational modification remains revision 0.2 IN_REVIEW, not accepted V1 scope.
- The Computer proposal already specifies a user-picked display/window, separate local-preview / agent-disclosure / input permits, newest-frame-only bounded delivery, no recording by default, measured adaptive quality, a persistent in-app **Take over / Stop run** strip and a qualified out-of-app emergency path. Take over fences input while leaving reasoning active; Stop run separately requests Conversation or Task cancellation. These remain TARGET/IN_REVIEW and provider-gated; the current Computer panel is unavailable.

This remains a bounded module supplement. Other feature packages still have inventory-only or partial semantic coverage, and most active source plus global owner documents have not been read line by line. The full repository audit is still OPEN; the exact ledger, not the word “supplement,” determines coverage.
