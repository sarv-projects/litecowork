# LiteCowork — Repository Documentation and Reset Audit

**Audit date:** 2026-10-31  
**Main baseline:** c7e9c05ab09563a554515bdd7e0e7d86c99fffa0  
**Scope:** active Markdown contracts after the docs-first reset, historical documentation archive, preserved local source archive, the SDD workflow and the dirty Conversation Session Supervisor worktree.

## 1. Executive finding

The docs-first reset has preserved the prior material, but it has deliberately changed what can be claimed about the checkout: the active main working tree is a documentation/architecture workspace, not a runnable LiteCowork implementation. The normative product and architecture contracts remain usable as design authority. Any CODE, SERVED, INTEGRATED, or QUALIFIED status must be established again from a recovered build and exact provider/OS evidence.

The active documentation was inventoried and received a line-numbered review pass in the reset review. The source set supplied for the reset was also read in full. During this continuation, the feature SDD package, SDD standard, templates, documentation tooling, and relevant Conversation/UserRequest owner contracts were read and cross-checked. The historical archives were inventoried and cross-compared for duplicates and unique deltas; they were not all independently re-read line by line. This distinction is intentional: an archive is preservation and provenance, not current product authority.

## 2. Corpus inventory

| Corpus | Files | Lines | Treatment |
|---|---:|---:|---|
| Four user-supplied source specifications/audits | 4 | 21,722 | Read in full and reconciled |
| Active root and current docs Markdown | 121 | 43,907 | Full active-document line-numbered review pass plus the 2026-10-31 desktop reference audit; docs/archive excluded; root Markdown 6/1,604 and current docs 115/42,274 |
| Feature specification packages (specs/) | 10 | 966 | Read/reviewed; one package structurally validated |
| SDD guide/templates (.specify/) | 6 | 184 | Read/reviewed; validator syntax checked and executed |
| Historical docs archive | 116 | 39,531 | Inventoried; duplicates cross-compared; selected unique evidence reviewed, not all files independently reread |
| Local ignored source archive Markdown | 255 | 94,259 | Inventoried; includes duplicated worktree docs and generated/design-tool metadata; not all files independently reread |

The earlier reset review recorded 116 active root/docs Markdown files / 42,825 lines; adding that audit record produced its 117-file / 42,911-line snapshot. The post-promotion snapshot before the desktop reference audit was 120 root/current-docs Markdown files / 43,602 lines. After that audit, the current snapshot is 121 root/current-docs Markdown files / 43,940 lines, plus 10 feature-package Markdown files / 966 lines and 6 .specify Markdown files / 184 lines: **137 active Markdown files / 45,090 lines total**. The current root/doc split is 6 root files / 1,614 lines and 115 current docs files / 42,326 lines. These counts exclude docs/archive and the ignored local archives tree. The historical docs archive is docs/archive/legacy-implementation-c7e9c05; the local source archive is archives/legacy-codebase-c7e9c05 and is ignored by Git. Current docs tooling counts separately: .specify/scripts/validate_feature_package.py is 280 lines.

The source archive Markdown total is not 255 unique engineering documents. It includes the preserved dirty worktree (231 files), design-tool material under .superdesign, and duplicated copies of repository documentation. Comparing the feature worktree with the main documentation snapshot found 99 of 103 matching docs byte-identical, with four domain docs changed. Comparing its implementation inventory with the historical implementation archive found 111 of 114 byte-identical, with three generated/audit inventory files changed.

## 3. Source authority and reconciliation rules

1. Root ARCHITECTURE.md is the normative system boundary and ownership contract.
2. docs/PRODUCT.md and docs/V1-PRODUCT-RELEASE-CONTRACT.md define the user promise and V1 boundary.
3. Domain owner contracts and accepted ADRs are authoritative within their domains.
4. docs/schemas/ owns promoted JSON Schema, OpenAPI, event and immutable SQLite migration contracts.
5. Function/settings/coverage/UI ledgers and generated types provide traceability; they do not override owner contracts.
6. The supplied Revision 9 master specification composes cross-domain decisions; it cannot override domain owners or machine schemas.
7. docs/IMPLEMENTATION-MASTER-PLAN.md orders delivery; Product Strategy, Coding Cockpit UI Spec, LiteSPM Integration Plan and Connector Factory Design remain subordinate planning/design documents and do not redefine domain semantics.
8. Recovered executable source establishes what exists/serves and does not override normative contracts.
9. Dated CODE/SYSTEM/USER evidence establishes what is proven for an exact build/platform/agent/provider.

Keep TARGET, CODE, SERVED, INTEGRATED, and QUALIFIED distinct. The older docs describe a historical c7e9c05 state at their individual timestamps. After the reset, none of those old source-level statements proves an executable implementation exists in the active checkout.

## 4. Findings and corrections made

### 4.1 Broken links caused by the archive move

The first active-tree link scan found 44 broken relative Markdown links pointing to the former implementation/ directory. Their targets were present in the preserved historical archive. The links have been rewritten to docs/archive/legacy-implementation-c7e9c05/ with source-relative paths, preserving their historical destination rather than silently replacing the old evidence with a new plan.

The post-SDD baseline link scan recorded 821 relative Markdown links in 136 files with zero missing targets. The 2026-10-31 desktop-reference update scan now covers 863 relative Markdown links in 143 active files, again with zero missing targets. The scan excludes docs/archive and the ignored local archive; the feature-package validator separately scans all Markdown files inside each package. Plain-text historical path mentions in dated plans and archived records may remain intentionally; they are not current instructions.

### 4.2 Architecture header and invariant numbering

ARCHITECTURE.md referred to an ignored archives_docs/ directory that does not exist in this checkout. The header now states that the former ARCH corpus is not present in the active docs-first checkout and distinguishes the local source archive from the versioned historical docs archive.

The architecture invariant list had two consecutive entries numbered 15. It is now numbered 1–31. A status note makes explicit that embedded source/status observations refer to the historical baseline, not the reset checkout.

### 4.3 Documentation entry point

docs/README.md no longer links the supplied Revision 9 source to the repository homepage as though that URL were the document. It identifies Revision 9 as user-supplied source material, clarifies that the source file is not stored in this checkout, and explains that the executable code snapshot is local-only. It also states that CODE/SERVED/INTEGRATED/QUALIFIED claims cannot be made from this checkout until a source slice is recovered.

The authority order in docs/README.md now matches AGENTS.md: architecture; product/release contract; domain owners and accepted ADRs; machine schemas; traceability ledgers; Revision 9 composition; subordinate delivery planning; recovered source; and exact CODE/SYSTEM/USER evidence. The index now links the normative Specification Standard, .specify workflow and specs package index.

### 4.4 LiteSPM endpoint authority remains unresolved

The current Capability Fabric owner contract distinguishes the previously selected machine-service candidate hostname from the public LiteSPM catalogue hostname. The machine-service candidate is explicitly unqualified and is not a verified machine API; the public site is a browser product/catalog surface, not a LiteCowork RPC endpoint. The two hosts must not be treated as aliases. The new LiteSPM integration plan therefore leaves exact IPC methods, authentication and schemas to the published upstream local protocol and requires a fresh version/protocol probe before implementation.

### 4.5 Preserved Conversation Session Activation work

The dirty worktree at archives/legacy-codebase-c7e9c05/.worktrees/conversation-session-supervisor remains registered on branch feature/conversation-session-supervisor at 3052341473e964bececf7f34c9d45923848e5218. After the Desktop move, its Windows-style gitdir pointers were repaired with git worktree repair; it now resolves under WSL with no prunable warning. It was not reset, stashed, committed, merged, pruned or source-edited.

The 26 October plan/spec describe a dedicated SQLite activation operation that, after the native supervisor observes the exact readiness handshake outside SQLite, atomically commits the ACTIVE AgentSession, RUNNING ConversationTurn, canonical events/snapshots, and Runtime-local host binding. The opaque native session handle is excluded from events, snapshots, and public results. A later selector/default change does not rebind the already pinned turn. Generic OPEN-to-RUNNING transition remains fail-closed.

The preserved worktree's verification record was updated after the earlier reset audit: the focused test conversation_activation_commits_session_turn_and_local_host_binding_atomically passed on 2026-10-31 after the final handle-redaction assertions. The broader cargo test -p storage-sqlite --locked run had passed 593 unit tests plus 2 AgentRegistryCache integration tests before those last handle-redaction-only additions; the full suite was not rerun afterward. Linux Secret Service integration was intentionally ignored because of active-user side effects, and Windows Credential Manager tests were unavailable on Linux. The offline architecture validator could not run in that environment because pydantic-core 2.46.5 was absent from the dependency cache. Read-only inspection confirms no native supervisor caller and no served Conversation Send route; Send remains fail-closed. Treat the slice as focused storage CODE evidence only, not a served or qualified Conversation path.

The preserved branch contains its own variants of STORAGE.md, AGENT-CONTROL.md, and EVENTS.md plus corresponding generated audit inventories and uncommitted storage changes. A documentation-only review selectively promoted the tested Conversation activation contract semantics from those three owner documents into main: immutable pinned binding/readiness fences, same-transaction ACTIVE session + RUNNING turn + local binding, and handle exclusion from snapshots/events/committed result. Main's NATIVE-COMPOSER-LLD.md was already more recently reconciled on 2026-10-31 than the older branch copy, so the branch variant was not copied. Rust implementation changes, tests and generated audit inventory changes remain isolated in the dirty feature worktree and were not merged or committed.

### 4.6 Feature SDD workflow and Conversation Supervisor package

The repository now has a normative Specification Standard, workflow/templates under .specify, and an indexed feature package under specs/001-conversation-session-supervisor. The package contains SPEC, PLAN, TASKS, DATA-MODEL, RESEARCH, WIRING and request/event plus error/recovery contracts. It remains DRAFT and does not authorize implementation. WIRING now models InvocationRunner exact-key provider-input reconciliation, its response/turn transaction, and the external provider boundary outside SQLite. The package validator checks required artefacts/sections, a single canonical acceptance command, package-wide relative Markdown links, task fields/dependency cycles, acceptance-ID traceability, unique WIRING IDs and transaction owners/atomic records/no-external-call boundaries. This is structural validation, not owner acceptance or runtime proof.

The feature review found that B-06 (Conversation-scoped UserRequest continuation) was incorrectly listed as an open blocker even though the canonical docs/STATE-MACHINES.md, docs/API.md, docs/STORAGE.md and docs/TRUST.md already define it. The DRAFT now inherits that owner contract and adds AC-017 through AC-019 for immutable response commit, exact provider-input acceptance, MCP polling, rejection/new-key handling and ambiguous acceptance. B-06 is removed from the blocking-decision table; it is not an unresolved design question. Remaining feature blockers are B-01 through B-05 and B-07 through B-10, including the final Agent-message/terminal-turn atomic write (B-04), first qualified harness/OS/auth tuple, readiness handshake, event replay behavior and context/resource/process/usage policies.

A read-only storage check also reconciled the apparent native-handle contradiction: agent_session_host_bindings intentionally stores the opaque handle in Runtime-local SQLite state and is explicitly excluded from Workspace backups/replication. Schema triggers enforce origin identity, immutability and delayed deletion until AgentSession settlement. This is not a domain-event/snapshot/public API leak; supervisor-owned close/recovery is still unimplemented.

### 4.7 Machine contract reconciliation

A structural comparison was run against the promoted machine contracts after the SDD updates:

- schemas/operator-api.openapi.yaml is OpenAPI 3.1.0 with 203 paths, 229 operations and 229 unique operationIds. After normalizing the API.md /v1 prefix, query annotations and path parameter names, all 229 operations are accounted for. Two are explicitly commented in API.md as target-only and are likewise described as unmounted in OpenAPI: POST /v1/workspace-roots and PATCH /v1/workspace-roots/{id}. There are no unexplained prose-only or machine-only routes.
- schemas/domain-event.schema.json defines 148 unique versioned event type constants. EVENTS.md contains exactly the same 148 names: zero missing and zero extra.
- schemas/error-codes.schema.json defines 131 unique error codes. All 131 occur in SCHEMAS.md.
- The 25 numbered ADRs are all indexed in docs/adr/README.md and each has explicit status metadata. ADR-0022/0023 are Accepted; ADR-0024 is Accepted target with implementation/schema migration/owner acceptance pending; the others are Accepted for vNext. This is a deliberate acceptance-vs-implementation distinction.

The 28-file SQLite chain was also applied to a fresh in-memory database using the transaction/PRAGMA protocol stated in STORAGE.md and the migration preambles: v1-v3 bootstrap group; v4 rebuild with foreign_keys OFF and legacy_alter_table ON; v5 normal immediate transaction; v6 rebuild with foreign_keys ON and legacy_alter_table ON; v7-v28 normal immediate transactions. All applied; final foreign_key_check returned zero rows, quick_check returned ok and 135 application tables existed. This replay does not exercise the production migration runner's immutable source-checksum/schema-fingerprint ledger or its user_version bookkeeping (user_version remained 0 in this stand-alone SQL replay).

This comparison is structural and name-based. It does not prove every prose request/response field, schema constraint, handler, storage function and UI state is semantically implemented; owner contracts, schemas and runtime qualification remain separate gates.

### 4.8 Repository SDD governance and validation

The global Specification Standard now defines the minimum feature-package contents, lifecycle, acceptance discipline, review/lock gate and implementation handoff. The .specify package includes templates and a documentation-only validator. A feature package index provides a single discoverable entry point. The validator checks required package files/sections, exactly one canonical bash acceptance command, package-wide Markdown links, task metadata and dependency DAGs, requirement-to-acceptance traceability, unique WIRING node/edge/transaction IDs, resolved edge references, non-empty transaction owners/atomic records and external_calls_inside=false. The WIRING interaction graph itself is not forced to be acyclic; native request/response and streaming paths can legitimately be cyclic. Task dependency cycles are rejected.

AGENTS.md already contains the required SDD workflow and preservation boundary. A search confirmed it contains no subagent-development instruction that needs removal. Product/architecture references to native subagents elsewhere are valid ownership semantics and must not be removed as a blanket cleanup.

### 4.9 Cross-domain target/current boundary register

This register summarizes the deliberate implementation boundaries that recur across owner documents. It is a navigation aid only; the first owner named remains authoritative and its wording wins.

| Domain | Canonical owner(s) | Reconciled current/target boundary | Remaining gate / dependency |
|---|---|---|---|
| Plain Conversation and native session execution | API, NATIVE-COMPOSER-LLD, AGENT-CONTROL, AGENT-FABRIC, SERVICES, STORAGE, EVENTS, STATE-MACHINES | Submit commits USER + OPEN, then fails closed with admission unavailable because no qualified supervisor is wired. The dirty worktree contains a storage-only atomic activation foundation; it is not served or end-to-end. | Feature 001 remains DRAFT; resolve first OS/harness/auth tuple, session-ready handshake, final AGENT response + terminal-turn transaction, event handling, stop/recovery and E2E. |
| Workspace roots | WORLD-RESOURCES, STORAGE, API, schemas/operator-api.openapi.yaml | Public generic POST create and PATCH policy update are explicitly target-only in API prose/OpenAPI. Current desktop flow uses authenticated native folder selection; owner list/observations and pause/resume/revoke/scan are separate operations. | Do not expose arbitrary absolute path or enable the two target operations until native identity/locator and policy contracts are promoted/served. |
| Task plan acceptance and Task controls | TASK-RUNTIME, SERVICES, STATE-MACHINES, API, OpenAPI | Plan acceptance has a detailed target contract but its POST is not mounted because producer-scoped AgentSession/Attempt context is missing. Task steer/pause/resume/cancel/recovery, Attempt takeover/return-control and timeline/effects/usage inspection are explicitly target-only. | Add trusted producer assertion and durable owner transactions; do not render controls as operational before route and UI qualification. |
| Agent install/auth/native configuration | AGENT-CONTROL, AGENT-FABRIC, AGENT-ACCOUNT-ROUTE-MATRIX, API | Inventory/registry refresh, profile probe, descriptor refresh, credentials and binding controls are described as served. Native authentication launch, native config open, binding config update, session options, harness capabilities, install/update handlers remain target-only. | Per-agent AgentModule lifecycle adapters, exact vendor flow and OS/harness version conformance. Only QUALIFIED route combinations may be offered ready. |
| Coworkers, Responsibilities and automatic Memory | ADR-0024, COWORKERS-TARGET, COWORKER-FLOWS, RESPONSIBILITIES, LEARNING, CONTEXT, API, schemas | Coworkers are optional chat-first identities, not mandatory for ordinary Conversation. ADR-0024 accepts the product/architecture target but explicitly leaves implementation, schema migration and owner acceptance pending. Coworker-target API route additions are not promoted to canonical OpenAPI/served routes. | Complete owned schema/migration/API, scoped memory learning, responsibility scheduling, UI and E2E before making operational claims. |
| LiteSPM | ADR-0003, CAPABILITY-FABRIC, LITESPM-LITECOWORK-INTEROP-LLD, LITESPM-INTEGRATION-PLAN | LiteSPM is an independent shared package lifecycle authority. Exact local machine service methods/auth/schema/version remain unqualified; no endpoint/API is inferred from catalog/browser or CLI. The catalog/provider status is truthfully unqualified and install/test commands fail closed. | Upstream version-pinned protocol publication/probe and declared OS/host conformance suite, followed by the integration release gate. |
| Connector Factory / provider operations | CONNECTOR-FACTORY-DESIGN, CONNECTORS-WORKFLOWS, NETWORK-SECURITY, CAPABILITY-FABRIC | Activepieces is an operation example source; Nango supplies auth/provider/scope metadata; OpenAPI yields endpoint candidates. No external runtime is embedded. Generated candidates remain non-invokable until implementation and qualification. The inspected upstream connector package kind is marked deprecated/deferred to its V2; do not advertise a first-class connector listing based on stale snapshots. | Pin current upstream sources; implement brokered operations with TLS/SSRF/redirect/deadline/byte/redaction/idempotency/reconciliation controls and provider test-account evidence. |
| Automations and Routines | AUTOMATION, ROUTINES, STORAGE, STATE-MACHINES, API | ManualTrigger “Run once” is a one-shot Task admission/settlement path. Recurring TriggerCoordinator hosting, cursor and misfire recovery remain unavailable; starter templates create editable drafts only and do not install, connect, authorize or enable triggers. | Recurring host/cursor/misfire lifecycle and owner USER evidence before enabling recurring responsibility. |
| Interactive app Artifacts | INTERACTIVE-ARTIFACT-RUNTIME, ARTIFACTS-EVIDENCE, PRESENTATION-RUNTIME, SECURITY, API | Explicit V1 target contract for immutable web bundles and local run protocol; no implementation or OS qualification is implied. Rich/interactive presentation is a separate aggregate/runtime and cannot replace semantic ConversationMessage or gain privilege. | Actual OS-isolated local runtime, host protocol, permission/read/save mediation, fallback and cross-version qualification. |
| Channels | CHANNELS, RUNTIME-MESH, STATE-MACHINES, NETWORK-SECURITY, API | Owner-approved identity/binding and ingress/receipt/host-assignment semantics are defined. Provider-specific setup/reauthentication is deferred to each integration; cross-Runtime replication is post-V1. | Per-provider account setup, callback/webhook trust, host assignment and USER qualification; never infer a provider auth flow. |
| Cloud/remote/multi-device | V1-PRODUCT-RELEASE-CONTRACT, RUNTIME-MESH, ARCHITECTURE | Cloud Runtime, remote Runtime, cross-device continuation/synchronization, teamwork/RBAC and mobile continuation are post-V1. Durable Task portability does not teleport a live native process or opaque session. | A separately planned V2 release with portable-state, lease/effect reconciliation, identity and host qualification. |
| UI and product surface | V1-UI-SCREEN-CONTRACT, DESIGN-SYSTEM, DESKTOP-SHELL-LLD, EXPERIENCE, V1 ledgers | Screen and interaction contracts define the target; README states that a capability is available only after its release gate/provider qualification. Mock, screenshot, disabled control or design spec is not shipped evidence. | Map each affordance to a served API, persisted owner state, error/recovery path and actual packaged UI USER evidence. |
| Usage and provider cost | V1-TELEMETRY-USAGE-RESILIENCE-LLD, OBSERVABILITY, API, SCHEMAS | Measured, provider-reported, estimated and UNKNOWN remain distinct. Conversation-wide usage and its ingestion/query are target-only in API status; missing usage is not zero. | Real provider usage ingestion, deduplication and provenance before displaying billed/cost claims. |

No contradiction is resolved by upgrading a target-only or unqualified design to “served”. These boundaries are intentionally preserved until their owner and release gates are met.

## 5. Priority follow-up before implementation resumes

1. Keep main docs-first. Do not stage or push the ignored archives/ directory.
2. Keep the preserved feature worktree isolated. Do not reset, stash, rebase, commit, merge, prune or mutate it during documentation reconciliation.
3. Before any promotion/merge, review the preserved feature worktree's complete diff and reconcile the Conversation activation patch against current owner contracts and schema/event definitions.
4. The focused activation storage test has passed; before any merge, rerun the full storage suite after the final redaction additions, then relevant event, concurrency and recovery suites from a dependency-complete environment.
5. Keep native supervisor readiness, context loading, prompt dispatch, stream commit, Stop/cancel, restart recovery and actual desktop Send as separate follow-on gates. The activation storage operation alone does not complete Conversation Send.
6. Start one end-to-end user journey through the actual packaged UI, daemon, IPC, storage and real agent/provider before broad feature expansion. Record exact platform/provider/version evidence.
7. Run LiteSPM service/protocol reconciliation in parallel, but do not advertise agent auto-connection, marketplace install, package apply or invocation until the published protocol and required plan/approval/receipt paths are proven.
8. Resolve the remaining Conversation feature blockers with their owners, then submit the package for IN_REVIEW and ACCEPTED/LOCKED. No implementation starts while it remains DRAFT.

## 6. Explicit limitations of this audit

- The historical docs archive and local source archive were inventoried and duplicate-compared, but every archived Markdown file was not independently re-read line by line. The active root/docs Markdown and four supplied source documents received the prior full line-numbered review pass; the feature package and .specify guides/templates were also read in this continuation.
- The current active-tree relative Markdown scan covers 137 files and 877 local links with zero broken targets; the feature-package validator separately scans each package Markdown links. This is not a schema/API semantic validator.
- This documentation-only continuation ran Python syntax and structural validators, link checks and YAML parsing. It did not run a Rust/TypeScript product build or runtime suite, and no current user-visible product capability was re-qualified.
- The dirty feature worktree was preserved as-is. Its uncommitted changes were inspected selectively through its plan/spec and diffs; they were not normalized, committed or merged.
- No commit or push was made. The main working tree still contains the expected file moves/deletions and newly created docs until an owner chooses a commit strategy.


## 7. Desktop reference application cross-check - 2026-10-31

A source-level audit now compares the actual ZCode packages/desktop application and DeepSeek Harness apps/desktop application at pinned 2026-10-30 commits. See [DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md), with a separate link from the existing [ZCode screenshot/UX audit](ZCODE-UI-INSPIRATION-AUDIT.md). The paired review is deliberately non-normative: ZCode per-window Host and CLI-owned Conversation, DeepSeek app-owned Node Host, narrow preload API, update handoff, plugin boundary, file transfer and tests are compared against LiteCowork Tauri + durable Runtime + Core/LiteSPM owners. It does not promote upstream code, behavior, tests or feature coverage into LiteCowork evidence.

The current active Markdown inventory is 137 files / 45,090 lines: root 6 / 1,614; docs/ excluding docs/archive 115 / 42,326; feature specs 10 / 966; .specify/ 6 / 184. The current relative-link pass found 877 local links and zero broken targets. Both upstream clones are shallow, clean and ignored under repo_take/; upstream tests were not run. No code was copied. LiteCowork has no root LICENSE in this checkout; this does not automatically prohibit private/internal adaptation of permissively licensed code, but the intended distribution posture, exact-file provenance, dependencies and required notices must be reviewed before shipping. git diff --check reported no whitespace errors; Windows Git emitted line-ending conversion warnings on existing LF files.

## 8. Final reset-era stale-reference and navigation sweep - 2026-10-31

- `docs/README.md` now distinguishes its quick-start map from the complete owner/design indexes and links to Product Contract Index, Coverage Matrix, V1 Design Index, ADR index and the actual machine-contract index (`SCHEMAS.md` plus `schemas/`). A nonexistent `schemas/README.md` was not created; the navigation was corrected to existing authorities.
- Root `README.md` no longer directs readers to the archived epics/roadmap as current status. It now directs current status to `CURRENT-RUN.md`, the repository audit and acceptance/E2E evidence policy. The referenced historical epics/backlog paths were verified under `docs/archive/legacy-implementation-c7e9c05/`.
- `PRODUCT-CONTRACT-INDEX.md` now points to the one active feature-package validator under `.specify/scripts/` and explains that the historical code/design validators are not active commands in this docs-first checkout.
- `V1-DESIGN-INDEX.md` now marks its source-layout and script paths as historical target landing-zone references after the reset. Its implementer handoff requires current `AGENTS.md`/`CURRENT-RUN.md`, an accepted/locked SDD and deliberate source recovery into a clean worktree before implementation; the old backlog and epics are no longer described as the live queue.
- `IMPLEMENTATION.md` now labels its directory tree as TARGET architecture, not an active source inventory. `V1-UI-SCREEN-CONTRACT.md` now labels its dated 2026-10-24/25 source-correction notes as observations from the pre-reset source snapshot, not current build claims.
- The 13 ZCode source URLs in `ZCODE-UI-INSPIRATION-AUDIT.md` now use one exact 40-character reviewed commit. The audit/authority numbering was corrected to 4.7 machine contracts and 4.8 SDD governance.
- Final inventory: **137 active Markdown files / 45,090 lines** (root 6/1,614; current `docs/` excluding `docs/archive` 115/42,326; feature specs 10/966; `.specify/` 6/184). The final relative Markdown scan found **877 links, zero broken targets**. The only active documentation package validator is `.specify/scripts/validate_feature_package.py`; it passes. `git diff --check` passes after normalizing the added note to LF.
- This sweep changes navigation/status prose only. No API, event, JSON Schema, SQL migration, runtime code, test, or package manifest was changed. Historical plans remain preserved rather than mass-edited.