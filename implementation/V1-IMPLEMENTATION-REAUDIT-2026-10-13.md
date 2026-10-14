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

The findings below were recorded against the `c8158ac` audit checkpoint. Their source
remediations are present in pushed `5fb33ea`; this table distinguishes source and test
definitions from exact test execution, packaged/provider evidence, and owner acceptance.
The original inventory and discovery methods above remain historical evidence for the
`c8158ac` checkpoint.

| ID | Severity / owner | Finding and concrete trigger | Remediation and CODE evidence | Remaining release evidence |
|---|---|---|---|---|
| A01 | High / E02-S03, E08-S03 | Resource A's unsaved draft/file could survive selecting B and be published as B's revision. | The committed editor pins the target and requires Save/Discard/Stay before switching; rendered-harness source covers those choices, Workspace switches, pending operations, and save failure. Focused helper tests also exist. Final pinned UI suite/build output for `5fb33ea` is not captured here. | Packaged Tauri/daemon operation, provider/OS qualification, and owner acceptance remain pending. |
| A02 | High / E08-S08 | Cached RichPresentation could remain visible after its reference became POLICY_OMITTED, UNAVAILABLE, or INTEGRITY_FAILED. | Cache visibility is bound to every immutable reference field and AVAILABLE state. Parser/helper tests and rendered-harness source cover policy/integrity invalidation while preserving semantic text. Final UI/browser rerun output is pending. | Packaged stream/fetch behavior, verified renderer/provider integration, and owner acceptance remain pending. |
| A03 | Medium / E08-S08 | An equivalent live snapshot could replace the reference object during fetch, cancel the effect, and strand LOADING. | The fetch uses a stable immutable cache key; rendered-harness source defers fetch, emits an equivalent live snapshot, and checks READY, failure/retry, and loading-state recovery. Final UI/browser rerun output is pending. | Packaged live-stream behavior, reconnect recovery, and owner acceptance remain pending. |
| A04 | Medium / E03-S02, E08-S04 | Late Conversation creation could update shared state after switching Workspace. | Creation captures Workspace identity and applies results only while that Workspace is active; rendered-harness source includes delayed create and Workspace-switch checks. Final UI/browser rerun output is pending. | Real Operator/Tauri SYSTEM behavior and owner acceptance remain pending. |
| A05 | Medium / E03-S02 | Conversation Create generated a new RequestId for each attempt, allowing a retry after a lost response to duplicate records. | A bounded app-session cache retains `(Workspace, normalized title, RequestId)` across keyed-page remounts; helper tests cover same-command reuse, changed title/Workspace identity, and capacity. Rendered-harness source checks a lost acknowledgement and one committed record. Process-restart recovery is not proven; final UI/browser rerun output is pending. | Packaged response-loss/restart behavior and owner acceptance remain pending. |
| A06 | Medium / E03-S02 | Desktop exposed only the first 50 Conversations; Tauri omitted cursor forwarding and UI dropped `next_cursor`. | The Tauri bridge validates/forwards the opaque bounded cursor; UI parses, appends, deduplicates, and rejects non-advancing cursors. Unit and rendered-harness source checks request page two and display Conversation 51. Final Tauri/UI/browser rerun output is pending. | Real Operator/Tauri run with 51+ durable records, stale/repeated cursors, and supported-OS authorization remains pending. |
| A07 | Medium / E02-S03 | Late Resource detail/history/head-preview responses could populate a reused editor after selection changed. | Reads and preview state are fenced by Workspace, Resource, and request generation. Focused helper tests and rendered-harness source resolve Resource A/B responses out of order and assert B remains selected. Final UI/browser rerun output is pending. | Packaged IPC, provider/OS qualification, and owner acceptance remain pending. |
| A08 | Medium / E02-S04 | Needs You load-more could remain busy after refresh/reconnect/Workspace change. | The committed reducer resets busy state on invalidation and ignores stale completion for a newer request; the two focused reducer regression definitions are in `task-attention-pagination.test.ts`. The final pinned UI suite was not recaptured for this checkpoint. | The full Approval/UserRequest/blocker inbox is still not served or integrated; rendered live reconnect and owner acceptance remain pending. |
| A09 | High / E01-S04, E03-S03 | Exact Task creation retry could fail before durable receipt replay if the default lead was cleared after commit. | The route checks owner/Workspace and exact durable receipt before mutable default-agent resolution. `instruction_create_route_commits_first_revision_and_replays_after_advance` includes a real-SQLite isolated Task route case: it creates with a 200-byte key, clears the default, retries the exact request, denies a foreign owner, and asserts one Task row/event. Exact full-daemon output for `5fb33ea` is not captured here. | Packaged authenticated IPC crash/restart/lost-ACK SYSTEM run and owner workflow remain pending. |
| A10 | Medium / E01-S04 | Shared mutation validation accepted only 128-byte idempotency keys although OpenAPI allows 200. | The shared daemon validator accepts 1–200 printable ASCII bytes; route/helper definitions cover 129, 200, 201, and malformed input. Tauri bridges use the shared 200-byte bound where the route contract requires it. Final UI/Tauri/daemon suite outputs for `5fb33ea` are pending. | Packaged authenticated IPC and owner acceptance remain pending. |
| A11 | Medium / E01-S04, E02-S02 | Archive, policy, and default-agent handlers misreported same-key/different-payload receipt conflicts as STALE_WORKSPACE_VERSION. | The handlers map receipt digest conflicts to `CONFLICT` and reserve `STALE_WORKSPACE_VERSION` for actual CAS conflicts. Route tests are `workspace_archive_route_is_owner_scoped_quiescent_and_idempotent`, `workspace_policy_route_is_owner_scoped_versioned_and_idempotent`, and `workspace_default_agent_route_distinguishes_receipt_conflict_from_stale_version`. Final full-daemon output for `5fb33ea` is not captured here. | Packaged authenticated IPC/Workspace UI recovery and owner acceptance remain pending. |
| A12 | High / E02-S03 | Resource revision upload failed to advance its local chunk offset after an accepted chunk, so multi-chunk files could repeat chunk zero and never commit. | The loop advances `offset = end` after an accepted receipt. Rendered-harness source now checks a 65,537-byte upload advances through byte ranges `0-65535/65537` and `65536-65536/65537`, then commits once. The focused browser/full suite was not rerun in this evidence capture. | Packaged Tauri + authenticated-daemon multi-chunk and interrupted/resumed upload, provider/OS qualification, and owner acceptance remain pending. |
| A13 | Medium / E02-S02 | An exact default-AgentBinding receipt retry could be rejected after the selected binding's eligibility changed; changed `If-Match` also has to remain part of request identity. | The route resolves the durable receipt before rechecking mutable binding eligibility and includes `expected_version` in the receipt payload. `workspace_default_agent_route_distinguishes_receipt_conflict_from_stale_version` uses real SQLite and an authenticated isolated Axum route to check exact replay after disabling the binding, changed `If-Match`/payload `CONFLICT`, new disabled-binding denial, and genuine stale-version classification. Exact full-daemon output for `5fb33ea` is not captured here. | Packaged IPC, process-restart recovery, Workspace UI behavior, and owner acceptance remain pending. |

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

## Documentation, tooling and evidence findings

| ID | Severity | Finding / evidence | Disposition |
|---|---|---|---|
| D01 | Medium | Hosted CI at `40b1233` passed workspace checks but failed generated coverage freshness. Workflow and review-packet changes were not reflected in coverage. Run `38448659899`. | The current local `scripts/check.sh` refreshes first-party Markdown and contract/design inventories before regenerating coverage, then checks freshness. `b003e9e` run `38452400483` still failed on stale coverage; the fix and current generated artifacts are uncommitted and require a fresh hosted run before closure. |
| D02 | Medium | Next-session handoff still identified the older storage-only frontier and omitted later source repairs and current audit evidence. | CURRENT-RUN now records pushed `5fb33ea` and the evidence limits. The handoff file is outside this correction's write allowlist and remains stale; this finding stays open until that separate file is refreshed. |
| D03 | Medium | WORKFLOWS U15/U16 said “no invisible autonomous memory proposal V1,” contradicting accepted quiet private learning in LEARNING/ADR-0024/E08-S03. | Corrected workflow row to qualified private learning with eligibility, visibility/opt-out, conflict/revocation gates. No learning implementation claim. |
| D04 | Medium | TESTING owner suite, platform matrix and release CI mixed local G2/G3 with cloud continuation, remote Runtime and external human channel. SCOPE/ROADMAP defer E11/E12. | Qualified cloud/remote/channel cases by their post-V1 gates; desktop/local acceptance remains required. |
| D05 | Medium | `scripts/audit_v1_markdown.py:16,40` omitted `archive_code`, `archives_docs`, `ARCHIVE`, `archives` from traversal pruning despite AGENTS historical boundary. Future local archives could contaminate first-party tracked inventory with owner filenames/hashes. | **Fixed in `b003e9e`:** traversal prunes all four directories; focused fixture proves those trees are not visited and current docs remain inventoried. The hosted run exposed the separate D01 coverage generation-order issue. |
| D06 | Medium | At the audit checkpoint, 13 active stories lacked dedicated standard review packets: E02-S04, E03-S01/S03, E05-S01, E07-S00/S01, E08-S02/S03/S04/S07/S08, E09-S02/S05. | Packet files now exist for all 20 active stories; the implementation-plan validator checks presence. This closes the missing-file finding. Evidence completeness/quality remains per-packet review work; E03-S02 is planned and receives a separate packet for this repair slice. |
| D07 | Medium | The root Cargo workspace omitted the separate Tauri workspace, and CI previously ran neither UI tests/build nor rendered UI checks. | The workflow defines separate root, pinned Node/pnpm UI (unit, rendered Chrome, build), and Tauri Cargo jobs. At pushed `5fb33ea`, hosted UI CI failed because nested scripts could not find `pnpm`; a local Corepack shim correction is uncommitted and has no hosted rerun. macOS/Windows/provider/SYSTEM/owner evidence and screen-reader/physical zoom remain open. |
| D08 | Medium | The default shell is Node 22.23.1 / pnpm 12.11.2 while the project pins Node 24.21.0 / pnpm 12.10.1; UI dependency bootstrap had failed before installation. | The package remains pinned to Node 24.21.0 / pnpm 12.10.1. Earlier 192/192 and 195/195 counts are historical, not final `5fb33ea` evidence; exact final pinned UI test, rendered-browser, API freshness, and build outputs are pending recapture. Hosted, packaged Tauri, OS/SYSTEM/USER gates remain open. |
| D09 | Low | ARCHITECTURE-COVERAGE said product behavior “is not implemented or qualified,” although tested partial source exists. Generator repeated this wording. | **Fixed in `48e5470`:** generator now says its coverage index does not establish implementation or qualification; focused wording test and coverage, plan, and architecture validators pass. |
| D10 | Low | Reference decision map says “+ background Conversation,” while AUTOMATION permits unique follow-up Conversation only on demand and forbids hidden Conversations for each run. | Corrected reference map to on-demand Task follow-up; `docs/AUTOMATION.md:479–480` remains authority. |
| D11 | Medium | OpenAPI/ledger counts can be read as served feature counts. E.g. producer-scoped POST plan revisions exists in machine spec but router serves GET only. Structural auditors do not discover that handler gap. | The scoped representative inventory now separates OpenAPI target, mounted route, provider, transport/OS, CODE, SYSTEM and OWNER USER evidence. It links Conversation create to its real-SQLite route test and Task create to the nested real-SQLite route case; it does not claim an exhaustive served-route inventory or a served-route count. |
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
| E02-S01–S04 | Installed shell/tray/startup/Quick Entry; complete Workspace/instruction/read-write recovery; Resource import/root/freshness across claimed OSes; Needs You/approval end-to-end; A01/A07/A12 source corrections are present, while packaged tests, actual UI states/accessibility, and A08's full inbox remain open. |
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

## Cleanup and next work

One main worktree; no branches/worktrees/native profiles/background servers created
by audit lanes. Shared Cargo target 2,288,252,691 bytes, UI dist 923,931 bytes,
owner `.playwright-mcp` 133,783 bytes retained. Node helper tests created no persistent
output. Existing caches/MCP processes/owner files preserved. Generated documentation
outputs are intentional and reviewed; no secret/provider data was collected.

Source remediations A01–A13 are present in pushed `5fb33ea`; do not reopen those code
changes without a failing regression. Immediate evidence work is to recapture the exact
pinned Node/pnpm UI unit, rendered-browser and build outputs, then current Tauri and daemon
full-suite outputs, followed by the real packaged/provider/OS and owner gates where their
environments are available. The broader release remains blocked by the independently
unfinished E01/E02/E03/E04/E07 foundations and the full backlog above. Preserve all fail-
closed planner, Trust, Effect, containment and Windows-root gates.
