# Desktop/local V1 implementation re-audit — 2026-10-13

## Scope and evidence

Owner requested: commit/push pending work first, then re-evaluate architecture,
documentation and implementation and list omissions. Source checkpoint `c8158ac`
was committed and pushed before this audit. It adds the production Unix stream
write-deadline regression; no production transport behavior changed.

Inventory at that checkpoint: **160 tracked Markdown files / 57,532 lines**, plus
135 Rust, 66 TypeScript, 24 TSX, 16 SQL, 15 JSON, 10 YML and 3 YAML files in the
relevant tracked-file inventory. The owner-consistency checker read all 160 Markdown
files mechanically. Three independent read-only audit lanes reviewed current
contracts and source for Core/runtime, UI, and documentation/release. The lead
inspected returned source evidence. Mechanical ingestion and focused semantic review
are distinct: this report does **not** certify semantic review of every statement,
absence of all defects, or live execution of every feature. Historical CURRENT-RUN
entries remain historical evidence. No owner acceptance was supplied by an agent.

Severity: **High** requires correction before the affected product flow is trusted;
**Medium** is a correctness/contract or qualification problem; **Low** is clarity or
traceability. Known unfinished gates are listed separately from discovered defects.
Paths/line references below refer to the source checkpoint; documentation corrections
in this audit may move their line numbers. Source findings are control-flow evidence,
not newly executed reproduction tests. Each repair needs its own regression.

## Newly identified source defects — remediation and evidence status

The findings below were recorded against the `c8158ac` audit checkpoint. The original
source remediations for A01–A04 and A06–A13 are present in pushed `5fb33ea`. The durable
Conversation-create journal described for A05 was added later in the working tree and is
not part of that commit. This table distinguishes source and test definitions from exact
test execution, packaged/provider evidence, and owner acceptance.
The original inventory and discovery methods above remain historical evidence for the
`c8158ac` checkpoint.

| ID | Severity / owner | Finding and concrete trigger | Remediation and CODE evidence | Remaining release evidence |
|---|---|---|---|---|
| A01 | High / E02-S03, E08-S03 | Resource A's unsaved draft/file could survive selecting B and be published as B's revision. | The editor pins the target and requires Save/Discard/Stay; the current UI helper suite passed 218/218 and rendered Chrome passed 31/31, including dirty-draft, Workspace-switch, focus, failed-save, and pending-operation cases. | Packaged Tauri/daemon operation, provider/OS qualification, and owner acceptance remain pending. |
| A02 | High / E08-S08 | Cached RichPresentation could remain visible after its reference became POLICY_OMITTED, UNAVAILABLE, or INTEGRITY_FAILED. | Cache visibility is bound to every immutable reference field and AVAILABLE state. The pinned UI helper suite passed 227/227. The rendered Conversation group passed 8/8 on Chrome 149.0.7827.200, explicitly evicting cached output for all three invalidations while semantic text remains visible. | Packaged stream/fetch behavior, verified renderer/provider integration, and owner acceptance remain pending. |
| A03 | Medium / E08-S08 | An equivalent live snapshot could replace the reference object during fetch, cancel the effect, and strand LOADING. | The fetch uses a stable immutable cache key. UI helper suite passed 218/218; rendered Chrome passed 31/31, including equivalent live snapshots, READY, failure, and retry recovery. | Packaged live-stream behavior, reconnect recovery, and owner acceptance remain pending. |
| A04 | Medium / E03-S02, E08-S04 | Late Conversation creation could update shared state after switching Workspace. | Creation captures Workspace identity and applies results only while that Workspace is active. UI helper suite passed 218/218; rendered Chrome passed 31/31, including delayed create and delayed acknowledgement after switching Workspace. | Real Operator/Tauri SYSTEM behavior and owner acceptance remain pending. |
| A05 | Medium / E03-S02 | Conversation Create could allocate a new RequestId after an ambiguous response and duplicate a committed Conversation. | Working-tree source adds a bounded Tauri app-data journal written before POST, exact `(Workspace, RequestId)` restoration/retry and acknowledgement, cleanup only for confirmed 4xx/pre-POST failures, and Workspace-fenced UI state. Focused Tauri tests passed 26/26, Conversation helper tests 18/18, full UI suite 218/218, and rendered Chrome 31/31, including acknowledgement failure and exact retry. A later Windows source slice uses flushed same-directory temporary files plus `MoveFileExW` with replacement/write-through flags; helper Windows-target tests and strict Clippy compile. | The browser uses a mock journal; Rust reopen tests do not restart the WebView/Runtime process. Actual Windows replacement/reopen, inherited app-data ACL, same-volume power-loss durability, packaged restart, and owner USER acceptance remain pending. The full Tauri Windows target check is blocked by the unavailable `x86_64-w64-mingw32-windres` tool. |
| A06 | Medium / E03-S02 | Desktop exposed only the first 50 Conversations; Tauri omitted cursor forwarding and UI dropped `next_cursor`. | The Tauri bridge validates/forwards the opaque bounded cursor; UI parses, appends, deduplicates, and rejects non-advancing cursors. The pinned UI helper suite passed 227/227 and the rendered Conversation group passed 8/8, requesting page two to display Conversation 51. An authenticated Axum Operator route test with real temporary SQLite passes 51 committed records across a tied-timestamp 50+1 boundary, deterministic cursor replay, exact order/no gaps or duplicates, same-owner Workspace isolation, cross-Workspace cursor rejection, and denial for a different authenticated owner. The full daemon and root gates pass on the current working tree. | Packaged Tauri/daemon process, OS peer authentication, restart behavior, stale/repeated cursor failure injection, and owner acceptance remain pending. |
| A07 | Medium / E02-S03 | Late Resource detail/history/head-preview responses could populate a reused editor after selection changed. | Source and harness fence/defer detail, history, and head-preview responses by Workspace, Resource, and request generation, including success/error interleavings. The three affected UI helper suites passed 45/45; rendered Chrome passed 31/31, including the deferred races. | Packaged IPC, provider/OS qualification, and owner acceptance remain pending. |
| A08 | Medium / E02-S04 | Needs You load-more could remain busy after refresh/reconnect/Workspace change. | The reducer resets busy state on invalidation and ignores stale completion for a newer request. The authenticated read-only Needs You projection is served and mounted; storage projection tests passed 7/7, daemon route/IPC tests 5/5, Tauri bridge tests 3/3, focused mounted Chrome interaction 2/2, and the UI helper suite 226/226. | Approval/UserRequest details and source-owned resolution actions, notification deep links, packaged Tauri/daemon reconnect, accessibility acceptance, and owner USER acceptance remain pending. |
| A09 | High / E01-S04, E03-S03 | Exact Task creation retry could fail before durable receipt replay if the default lead was cleared after commit. | The route checks owner/Workspace and exact durable receipt before mutable default-agent resolution. `instruction_create_route_commits_first_revision_and_replays_after_advance` includes a real-SQLite isolated Task route case: it creates with a 200-byte key, clears the default, retries the exact request, denies a foreign owner, and asserts one Task row/event. Exact full-daemon output for `5fb33ea` is not captured here. | Packaged authenticated IPC crash/restart/lost-ACK SYSTEM run and owner workflow remain pending. |
| A10 | Medium / E01-S04, E08-S02 | Suggestion preference mutation kept a route-specific 128-byte cap after the shared Operator key contract was 200 bytes. | The `PUT` operation now references the reusable 1–200 printable-ASCII OpenAPI parameter. Daemon, domain, SQLite, and Tauri use their existing shared validators. A real temporary-SQLite authenticated Axum route test accepts a 200-byte key, replays the same result, rejects changed-payload reuse and 201 bytes, and asserts one preference/event/receipt. The focused route, domain, storage, and Tauri tests pass. | Packaged authenticated IPC, supported-OS qualification, and owner acceptance remain pending. |
| A11 | Medium / E01-S04, E02-S02 | Archive, policy, and default-agent handlers misreported same-key/different-payload receipt conflicts as STALE_WORKSPACE_VERSION. | The handlers map receipt digest conflicts to `CONFLICT` and reserve `STALE_WORKSPACE_VERSION` for actual CAS conflicts. Route tests are `workspace_archive_route_is_owner_scoped_quiescent_and_idempotent`, `workspace_policy_route_is_owner_scoped_versioned_and_idempotent`, and `workspace_default_agent_route_distinguishes_receipt_conflict_from_stale_version`. Final full-daemon output for `5fb33ea` is not captured here. | Packaged authenticated IPC/Workspace UI recovery and owner acceptance remain pending. |
| A12 | High / E02-S03 | Resource revision upload failed to advance its local chunk offset after an accepted chunk, so multi-chunk files could repeat chunk zero and never commit. | The loop advances `offset = end` after an accepted receipt. UI helper suite passed 218/218; rendered Chrome passed 31/31, including a 65,537-byte input, both byte ranges, and one commit. | Packaged Tauri + authenticated-daemon multi-chunk and interrupted/resumed upload, provider/OS qualification, and owner acceptance remain pending. |
| A13 | Medium / E02-S02 | An exact default-AgentBinding receipt retry could be rejected after the selected binding's eligibility changed; changed `If-Match` also has to remain part of request identity. | The route resolves the durable receipt before rechecking mutable binding eligibility and includes `expected_version` in the receipt payload. `workspace_default_agent_route_distinguishes_receipt_conflict_from_stale_version` passed in the full 160-test daemon suite and as a focused 1/1 real-SQLite authenticated Axum route test on 2026-10-14. It covers exact replay after disabling the binding, changed `If-Match`/payload `CONFLICT`, new disabled-binding denial, and genuine stale-version classification. | Packaged IPC, process-restart recovery, Workspace UI behavior, and owner acceptance remain pending. |

### Evidence update — 2026-10-14

The current local command `cargo test --locked -p storage-sqlite` passed **193 tests,
0 failed, 0 doc tests** in 241.96 seconds with rustc `1.98.1 (48a229cea 2026-09-01)`
and Cargo `1.98.1 (797e8a9bc 2026-08-05)`. This is storage CODE evidence only; it does
not verify the daemon, Tauri bridge, rendered UI, or package integration. It predates a
later uncommitted owner edit to `crates/storage-sqlite/src/blob.rs` that appeared during
the documentation pass, so that edit is not covered by this result. The previously
recorded 192/192 UI, 60/60 Tauri, and 147-unit + 4-CLI daemon counts predate this pushed
source checkpoint and are historical, not final-checkpoint results. Exact current UI,
rendered-browser, build, Tauri, and daemon outputs remain pending recapture.

Hosted CI at `5fb33ea` failed in the UI job because nested package scripts could not find
`pnpm`. A Corepack availability correction exists only as an uncommitted local workflow
change; a hosted rerun is pending. No hosted-green result is claimed. The CODE source and
test definitions in A01–A13 do not close packaged/provider, OS, crash/restart, SYSTEM, or
OWNER USER gates, and no backlog status is changed by this audit update.

UI paths abbreviated after A01 are relative to `apps/litecowork-ui/` unless stated.
No newly demonstrated authorization bypass, secret leak or SQL recovery corruption was
found in the inspected paths. That limited result is not a security certification.

### Evidence update — 2026-10-15

Fresh current-working-tree verification adds the following CODE evidence without changing
story status:

- Pinned Node `24.21.0` / pnpm `12.10.1` UI helper suite: **242/242**. The same-ID
  RichPresentation snapshot regression is included. Production UI API freshness,
  TypeScript, and Vite build passed. Rendered Chrome `149.0.7827.200` passed 54 checks
  before that same-ID guard was added, so it does not cover the final guard.
- `cargo test --locked -p litecoworkd --bin litecoworkd`: **211/211** on Linux WSL2 x86_64,
  Rust/Cargo `1.98.1`. This includes the Task receipt replay after clearing the default
  AgentBinding, Workspace receipt-vs-CAS error classification, and route-level 129/200-byte
  acceptance plus 201-byte rejection/no-write checks for archive, replication policy,
  default AgentBinding, and private native WorkspaceRoot selection.
- `cargo fmt --all -- --check`; strict Clippy for `storage-sqlite` and `litecoworkd`; strict
  contract audit (0 errors), Markdown audit (174 files, 0 broken links), generated coverage
  (103 documents / 1,669 sections / 6,492 rows), implementation plan (67 stories),
  architecture (142 typed events), strict design depth (0 blockers), design-owner
  consistency (34 checks, 0 issues), and `git diff --check` passed.
- A separate read-only UI re-review found no concrete remaining A01/A03/A04/A07 race in
  the inspected current source. It is not a replacement for the tests or owner acceptance.

These are local CODE and documentation-consistency results. Hosted CI after the latest
changes, packaged desktop/daemon transport, provider and cross-OS qualification, SYSTEM,
and owner USER gates remain pending. E02-S03's root scan/membership/search behavior is not
implemented; the standalone Linux traversal remains only a scanner primitive.

### Evidence update — 2026-10-16

The current working tree was rechecked against A01–A11. Read-only reviews found the listed
source remediations present; the lead added the missing immediate Workspace-identity fence
to Library search and tightened ContextDocument deletion owner/state validation and its
OpenAPI cache/count contract. The full pinned UI helper suite passed **285/285**; the
rendered Chrome `149.0.7827.200` suite passed **61 checks**, including A01–A06, A12,
ContextDocument deletion and Workspace search interleavings. The full serial daemon binary
suite passed **231 tests**, with one manual OpenCode smoke ignored. It includes A05 durable
Conversation replay after a lost acknowledgement/restart, A09 Task replay after clearing
the default Agent, A10 129/200/201-byte idempotency boundaries, A11 receipt-conflict versus
stale-version classification, and authenticated deletion-route checks. Focused Resource
race coverage passed **7/7**; ContextDocument deletion helpers passed **11/11**.

TypeScript, generated API freshness, Vite production build, strict contract audit (0 errors),
Markdown audit (176 files / 0 broken links), coverage (103 documents / 6,536 rows),
implementation plan (67 stories), architecture (143 typed events), and `git diff --check`
passed. One isolated Resource race run timed out waiting for its browser harness before
assertions; the exact rerun and complete UI suite passed. An initial UI command used system
Node `22.23.1` and stopped at the declared engine check; rerun with Node `24.21.0` passed.
These are current-tree CODE and rendered-test-harness results, not packaged Tauri/provider
SYSTEM qualification or owner USER acceptance. No A01–A11 story status is changed; all
required external/platform/owner gates remain pending. E02-S03 root scan, membership,
freshness and search are still not implemented.

## Documentation, tooling and evidence findings

| ID | Severity | Finding / evidence | Disposition |
|---|---|---|---|
| D01 | Medium | Hosted CI run `38592926531` on `3fd5c62` exposed stale generated coverage after Markdown inventory refresh. | `scripts/check.sh` rechecks Markdown freshness after coverage generation updates `ARCHITECTURE-COVERAGE.md`; the test-first script-order regression passed. The full local gate now passes end to end on this tree: format, strict workspace Clippy, workspace tests/build, Python (**29/29**), strict contract and Markdown audits, coverage, design depth, owner consistency, plan, architecture, and diff checks. Hosted run `38633270629` on `942089d` still failed root `check`; a hosted run containing this fix remains pending. |
| D02 | Medium | Next-session handoff still identifies the older storage-only frontier and omits later source repairs and current audit evidence. | The owner explicitly deferred handoff work in the active request. This finding remains pending; CURRENT-RUN contains the latest implementation evidence. |
| D03 | Medium | WORKFLOWS U15/U16 said “no invisible autonomous memory proposal V1,” contradicting accepted quiet private learning in LEARNING/ADR-0024/E08-S03. | Corrected workflow row to qualified private learning with eligibility, visibility/opt-out, conflict/revocation gates. No learning implementation claim. |
| D04 | Medium | TESTING owner suite, platform matrix and release CI mixed local G2/G3 with cloud continuation, remote Runtime and external human channel. SCOPE/ROADMAP defer E11/E12. | Qualified cloud/remote/channel cases by their post-V1 gates; desktop/local acceptance remains required. |
| D05 | Medium | `scripts/audit_v1_markdown.py` omitted the four local archive directories and local `.superpowers` / `superpowers` planning-artifact directories from traversal pruning. Historical or owner-local files could contaminate the first-party Markdown inventory with private filenames and hashes. | Traversal now prunes all six directories before visiting their children. Focused filesystem fixtures prove excluded trees are never visited while first-party docs remain inventoried; the three-test module passes. Owner artifacts were preserved and not deleted. |
| D06 | Medium | At the audit checkpoint, 13 active stories lacked dedicated standard review packets: E02-S04, E03-S01/S03, E05-S01, E07-S00/S01, E08-S02/S03/S04/S07/S08, E09-S02/S05. | Packet files now exist for all 20 active stories; the implementation-plan validator checks presence. This closes the missing-file finding. Evidence completeness/quality remains per-packet review work; E03-S02 is planned and receives a separate packet for this repair slice. |
| D07 | Medium | The root Cargo workspace omitted the separate Tauri workspace, and CI previously ran neither UI tests/build nor rendered UI checks. | The workflow defines separate root, pinned Node/pnpm UI (unit, rendered Chrome, build), and Tauri Cargo jobs. The Corepack shim is committed in `942089d`; hosted run `38633270629` confirms UI and Tauri jobs passed. Root `check` failed and remains under D01. macOS/Windows/provider/SYSTEM/owner evidence and screen-reader/physical zoom remain open. |
| D08 | Medium | The default shell may not match the pinned Node/pnpm toolchain; prior dependency bootstrap had failed before installation. | The package, `.node-version`, and CI pin Node 24.21.x/pnpm 12.10.1. Hosted run `38633270629` passed the pinned UI job; local pinned tests pass 227/227, the production build passes, and the full current rendered Chrome suite passes 37/37 after adding the explicit A02 UNAVAILABLE invalidation case. Packaged Tauri, cross-OS/SYSTEM/USER gates remain open. |
| D09 | Low | ARCHITECTURE-COVERAGE said product behavior “is not implemented or qualified,” although tested partial source exists. Generator repeated this wording. | **Fixed in `48e5470`:** generator now says its coverage index does not establish implementation or qualification; focused wording test and coverage, plan, and architecture validators pass. |
| D10 | Low | Reference decision map says “+ background Conversation,” while AUTOMATION permits unique follow-up Conversation only on demand and forbids hidden Conversations for each run. | Corrected reference map to on-demand Task follow-up; `docs/AUTOMATION.md:479–480` remains authority. |
| D11 | Medium | OpenAPI/ledger counts can be read as served feature counts. E.g. producer-scoped POST plan revisions exists in machine spec but router serves GET only. Structural auditors do not discover that handler gap. | The scoped representative inventory separates OpenAPI targets, mounted handlers, providers, transport, and CODE/SYSTEM/OWNER USER evidence and disclaims an exhaustive served-route count. The WorkspaceRoot row now records the passed authenticated native-selection route test (owner create/exact replay, foreign-principal denial, one committed root, and generic public POST absence) and retains GET/list, packaged transport, SYSTEM, and USER limits. The Conversation-list row links to authenticated SQLite evidence for 51+ records. |
| D12 | Medium | Desktop README said Windows peer authentication was unimplemented despite existing daemon/Tauri named-pipe source and security helpers. `apps/litecowork-ui/README.md:8,55` vs `src-tauri/src/lib.rs:42,3983,4167` and `apps/litecoworkd/src/operator.rs:318,335`. | Corrected to source present, real Windows builds/OS/SYSTEM/USER qualification pending. No Windows support claim. |

## What is actually present

The repository contains real bounded production components, not an empty architecture:
SQLite single-writer transactions, event/projection/receipt atomicity and guarded recovery;
OS identity/key-provider boundaries; Runtime incarnation and lock/lifecycle state;
authenticated Unix IPC and Windows named-pipe source; Operator owner-scoped handlers;
Workspace policy/instructions, roots, Resource revision intake/FTS/history; durable
Conversation reads and snapshot streams; Task draft/spec/readiness and attention
projections; Coworker/Goal/Suggestion/ContextDocument/DelegationProfile slices;
manual Routine/Automation READY-Task admission; Artifact immutable history/edit/compare/
Save As; bounded optional RichPresentation parsing/rendering; fail-closed Trust,
Invocation/Effect primitives and Linux process-scope/staging primitives.

These are partial integrated paths and tested components. They do not establish a
complete provider-backed Task or full supported desktop release. Existing safeguards
include Artifact read cancellation/authorization clearing, Task pinned Resource preview
identity fencing, exact pending Artifact/Workspace-instruction/Resource-status request
identities, native Save As integrity/status checks and transport failure propagation.
Do not replace them with permissive shortcuts when fixing the defects above.

## All remaining desktop/local V1 work

Backlog authority remains `backlog.json`: **60 local V1 stories**, 1 IN_REVIEW,
19 IN_PROGRESS, 40 PLANNED, **zero ACCEPTED**. There are 67 total including seven
post-V1 E11/E12 stories. Full story/dependency table is in the refreshed
[next-session handoff](NEXT-SESSION-HANDOFF-2026-10-13.md). Every applicable story
still needs CODE, actual SYSTEM and owner USER evidence before acceptance.

| Track | Remaining implementation/qualification |
|---|---|
| E01-S01–S04 | Clean reproducible CI/desktop bootstrap; physical database/WAL/blob exhaustion and OS keystore qualification; successful managed daemon lifecycle/systemd+cgroup/sleep/drain/recovery; packaged authenticated transport/reconnect/slow-consumer and different-identity tests. Local stream CODE test does not close packaged SYSTEM. |
| E02-S01–S04 | Installed shell/tray/startup/Quick Entry; complete Workspace/instruction/read-write recovery; Resource import/root/freshness across claimed OSes; source-owned Needs You resolution; A01/A07/A08/A12 source corrections are present. The read-only Needs You projection has CODE evidence; packaged IPC/reconnect, full source actions, actual UI/accessibility, and owner acceptance remain open. |
| E03-S01–S11 | Qualified native Conversation Send/Stop/retry/reconnect; scoped isolated planner and producer-authenticated atomic Plan/Steps/receipt submission; integrated Attempt launch/lease/settlement/recovery; native feature parity; AgentModule registry/discovery/install/update/repair; descriptor/config/auth/secure slots/native options/composer; exact per-agent/OS conformance. A04–A06/A09/A11/A13 source corrections are present; final suite, packaged, provider, and owner evidence remain open. |
| E04-S01–S06 | Atomic Trust admission/audit/grants/Secret issuance; Gateway progressive discovery; real Capability dispatch, idempotency and ambiguous Effect reconciliation; actual LiteSPM/MCP authority/integration; independently verified Artifact/Evidence completion; cross-agent package bridges. Pure decision/persistence primitives remain insufficient. |
| E05-S01–S05 | Full DelegationProfile flow, accepted-plan child admission/isolation, qualified routing and measured/estimated/unknown cost, quota/budget, independent escalation and lead failover, heterogeneous native/provider corpus and B83 evidence. |
| E06-S01–S05 | Runtime-integrated archive preview/extraction with bounded provider, child provenance/deletion/cancel; hybrid semantic retrieval and grounded attachments; incremental invalidation/purge; local-model hardware/no-network conformance; full Knowledge readiness/placement UI. ZIP storage/standalone preview is not extraction/RAG. |
| E07-S00–S05 | Real enforcing EnvironmentProvider connected to exact Resource staging, attestation/incarnation/admission, output quotas, approved egress, native tool mediation and quiescence; persistent previews/lifecycle; real Chrome/profile/consent/control epochs/takeover; ActionBatch/deadline/warmth; office/data/research and desktop/computer/PTY providers. Windows roots remain UNAVAILABLE until actual identity/reparse/volume/WSL/restart tests qualify them. |
| E08-S01–S08 | Optional chat-first Coworker ownership/last-active/Quick Create and connections; qualified Goals/Suggestions; private automatic learning with canonical candidate/memory policy/provenance/consent/filter/revoke/recall; full Live Work/Workbench/browser/terminal shell; keyboard/screen-reader/zoom/reduced-motion; typed transient/final Presentation Runtime; exact Artifact versions; semantic-first RichPresentation/Host Guidance and matched OpenIntelligentUI viability/performance tests. A02/A03 source and rendered-harness cases are present; final UI/browser, provider, SYSTEM, and owner evidence remain pending. |
| E09-S01–S05 | Routine revision/reuse; recurring/event TriggerCoordinator and durable occurrence/cursor/misfire/overlap admission; restart-safe manual request recovery; actual Automation Test/health/drift; notifications/quiet hours/background behavior; StandingResponsibility grouping without another scheduler. Manual READY Task creation does not execute scheduled work. |
| E10-S01–S02 | Qualified Skills/MCP Apps/Discover and connection repair UX; bounded redacted demonstration, SkillProposal review/test, drift handling and authorized LiteSPM publication. No automatic unsafe replay/publication. |
| E13-S01–S04 | Backup/restore and migration/update rollback; measured numeric performance/soak/resilience; all assigned real workflows/fault injection/platform matrix; signed clean-host installers/update/uninstall/data retention, SBOM/licenses/security/accessibility/runbooks/incident drills; owner G3 approval. No advertised path receives PASS from a mock or missing host. |

Critical unfinished source gates were reconfirmed: planner rejects dispatch at
`apps/litecoworkd/src/agents/planning.rs:64`; contained Attempt launch at
`attempt_process_scope.rs:478` has no production dispatch caller; Trust explicitly
lacks integrated admission at `trust.rs:1–6`; Effect persistence has no provider
transport at `crates/storage-sqlite/src/effect_evidence.rs:1–4,112`; live Linux
systemd/Bubblewrap conformance tests remain ignored; Windows root revalidation returns
unsupported at `apps/litecoworkd/src/runtime.rs:897`. Preserve these fail-closed gates.

E11 cloud continuation/channel/mesh and E12 remote Runtime follow local V1. External
credentials, physical OS/device work, signing or subjective owner acceptance block
only their exact evidence gates. Publication/release/paid provisioning remains outside
current authorization. Commits and pushes are authorized.

## Verification observed at the original audit checkpoint (historical)

- `cargo test --locked -p operator-ipc`: 20 unit + 4 integration passed before commit;
  focused production deadline test passed in 5.06 seconds.
- `cargo clippy --locked -p operator-ipc --all-targets -- -D warnings`,
  `cargo fmt --check`, `git diff --check`: passed before commit.
- Hosted [run 38448659899](https://github.com/sarv-projects/litecowork/actions/runs/38448659899)
  at `40b1233`: workspace Rust formatting/Clippy/tests/build and 15 Python tests passed;
  architecture passed; plan gate failed stale coverage. Earlier Bubblewrap failure
  did not recur in this run. The subsequent run 38449616382 at c8158ac reached the same stale-coverage failure; its IPC regression also passed in the hosted workspace suite.
- `.venv/bin/python scripts/validate_architecture.py`: passed (142 typed events,
  schema/OpenAPI/SQL/name/link consistency).
- `.venv/bin/python scripts/validate_implementation_plan.py`: initial FAIL because
  coverage was stale; generated refresh/final results are recorded in CURRENT-RUN.
- `.venv/bin/python scripts/audit_v1_contracts.py --strict`: 0 errors, 206 machine
  routes, 158 supplemental UI cases, 67 stories. Not served-handler proof.
- `.venv/bin/python scripts/audit_v1_design_depth.py --strict`: 20 domains,
  226 functions, 187 Settings, zero structural blockers. Not runtime proof.
- `.venv/bin/python scripts/validate_design_owner_consistency.py`: 160 documents,
  57,532 lines mechanically read, 34 assertions, zero issues; its generated JSON
  changed inventory counts/hash. Not universal semantic proof.
- `PATH=/home/sarvesh/.nvm/versions/node/v24.21.0/bin:$PATH node --experimental-transform-types --test apps/litecowork-ui/tests/*.test.ts apps/litecowork-ui/tests/*.test.mjs`:
  **169 passed, 2 failed** (171 total, 13.94 seconds). Both failures are imports of
  missing `@tauri-apps/api` in desktop-suggestion-api/resource-location-api tests.
  Focused retry from UI cwd reproduced missing dependency. No new UI bootstrap/build,
  rendered-component, native desktop, accessibility or SYSTEM/USER pass is claimed.

## Current CODE evidence update — 2026-10-14

This update supersedes table entries above that said the final UI/browser rerun or full
suite output was pending. Current-worktree UI verification used Node `24.21.0`, pinned pnpm
`12.10.1`, Chrome `149.0.7827.200`, Linux WSL2 x86_64, and Rust/Cargo `1.98.1`:

- `corepack pnpm@12.10.1 test` — **215/215 passed**.
- `corepack pnpm@12.10.1 test:browser` — **27/27 rendered Chrome checks passed**, including
  A01–A07, A12's 65,537-byte two-chunk upload ranges and exactly one commit, plus F90.
- `corepack pnpm@12.10.1 build` — API freshness, TypeScript no-emit, and Vite passed; the
  minified JS chunk is 811.38 kB (224.55 kB gzip).
- `cargo test --workspace --locked` — current run passed **214 storage-sqlite tests, 160
  litecoworkd tests, 4 CLI tests**, and the remaining workspace crate tests. The focused
  `resource_upload_gc_tests::` module passed **18/18**. Full workspace strict Clippy,
  format, and build passed.
- E02-S03 now has real-SQLite/FileBlobStore tests proving ineligible Resources do not
  require ResourceIndex key history, eligible content fails closed without partial SQL
  projection when the key is unavailable, and an upload retry succeeds after key restore.

These remain CODE/browser-harness checks, not packaged Tauri + authenticated daemon
acceptance. Packaged/provider, cross-platform OS, crash/restart where not explicitly
exercised, SYSTEM, and owner USER gates remain pending. Hosted CI run `38592926531` on
`3fd5c62` completed: UI and Tauri passed; root `check` failed only because generated
coverage was stale after Markdown inventory refresh. The local ordering fix is covered by
the 14-test architecture-validator suite, and manual regenerated contract/depth/Markdown/
coverage/plan/architecture gates pass. A full `scripts/check.sh` rerun with the fix and a
fresh hosted run on the next pushed commit remain pending.

### Source re-review findings requiring follow-up

- **A05 Conversation Create retry identity:** the bounded private Tauri app-data journal
  and exact-identity UI restore/retry path are now present in the working tree. Focused
  Tauri, helper, full UI, and rendered Chrome checks passed 26/26, 18/18, 218/218, and
  31/31 respectively. The rendered acknowledgement-failure case preserves the exact retry
  identity; the browser journal remains a test mock. `docs/API.md` states that rejected
  4xx creates have no committed Conversation/event/receipt, which is the basis for clearing
  only confirmed rejections. Windows now requests write-through replacement after flushing
  the same-directory temporary file; helper tests cross-compile, but full Tauri Windows
  checking is blocked by the missing `x86_64-w64-mingw32-windres` tool. Actual Windows
  restart/failure behavior, inherited ACL, power-loss qualification, packaged restart, and
  owner acceptance remain pending. E03-S02 is still partial because
  native turn admission/execution is not implemented.
- **A07 rendered race evidence:** source and harness now independently defer detail,
  history, and head-preview responses and exercise success/error interleavings; 45 affected
  UI helper tests pass. The current rendered Chrome run passed all 31 named checks,
  including those interleavings.
- **A13 receipt classification:** `cargo test --locked -p litecoworkd
  workspace_default_agent_route_distinguishes_receipt_conflict_from_stale_version --
  --nocapture --test-threads=1` passed **1/1**; it also passed in the full 160-test daemon
  suite. Packaged IPC/restart/UI/owner acceptance remain pending.

## Cleanup and next work

One main worktree; no branches/worktrees/native profiles/background servers created
by audit lanes. Shared Cargo target 2,288,252,691 bytes, UI dist 923,931 bytes,
owner `.playwright-mcp` 133,783 bytes retained. Node helper tests created no persistent
output. Existing caches/MCP processes/owner files preserved. Generated documentation
outputs are intentional and reviewed; no secret/provider data was collected.

The original source remediations for A01–A04 and A06–A13 are present in pushed `5fb33ea`;
do not reopen those code changes without a failing regression. The A05 durable journal
extension is a later working-tree change with the CODE evidence listed above; the pushed
baseline does not contain that extension. Current pinned UI, rendered-browser, storage,
and combined daemon evidence is recorded above. Remaining work is packaged/provider/OS and
owner gates where their environments are available, plus independently unfinished
E01/E02/E03/E04/E07 foundations and the full backlog above. Preserve all fail-closed
planner, Trust, Effect, containment and Windows-root gates.
