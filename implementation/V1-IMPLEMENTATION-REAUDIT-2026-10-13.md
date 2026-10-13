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

The findings below were recorded against the `c8158ac` audit checkpoint. The current
working-tree repairs and 2026-10-13 verification are summarized here and recorded with
exact commands in `CURRENT-RUN.md`; the original inventory and discovery methods above
remain historical evidence for that checkpoint.

| ID | Severity / owner | Finding and concrete trigger | Remediation and CODE evidence | Remaining release evidence |
|---|---|---|---|---|
| A01 | High / E02-S03, E08-S03 | Resource A's unsaved draft/file could survive selecting B and be published as B's revision. | Editor retains the current target and requires Save/Discard/Stay before switching; selection/read generations are fenced. Four focused helper tests and the full pinned UI suite (192/192), TypeScript and Vite build pass. | Real rendered editor test covers dirty A → B, Save/Discard/Stay, Workspace switch and pending-operation states; packaged UI and owner acceptance remain pending. |
| A02 | High / E08-S08 | Cached RichPresentation could remain visible after the current reference became POLICY_OMITTED, UNAVAILABLE or INTEGRITY_FAILED. | Cache visibility is keyed by the full immutable binding and only AVAILABLE refs; response parser checks every reference field. RichPresentation and conversation tests pass within the 192/192 UI suite. | Rendered update from READY to policy/integrity failure must immediately remove rich DOM and retain the complete semantic answer and unavailable state. |
| A03 | Medium / E08-S08 | A live snapshot could replace the ref object during fetch, cancel the effect and strand LOADING. | Fetch effect depends on a stable immutable cache key, not ref object identity; equivalent live references preserve the in-flight request. UI suite 192/192, typecheck and Vite build pass. | Browser test must defer fetch, send equivalent live update, then prove READY or recoverable FAILED and keyboard-accessible retry. |
| A04 | Medium / E03-S02, E08-S04 | Late Conversation creation could update shared state after switching Workspace. | Create captures Workspace and only applies its result while that Workspace is active; Conversation page is keyed by Workspace. UI suite 192/192 passes source/helper regression. | Rendered delayed-create A → B test and real Operator/desktop SYSTEM test remain pending. |
| A05 | Medium / E03-S02 | Conversation Create generated a new RequestId for each attempt, allowing a retry after a lost response to duplicate records. | Exact `(Workspace, normalized title, RequestId)` is retained in a bounded module-scoped cache across page remounts and Workspace switches; 64-entry boundary and unchanged-retry tests pass in UI suite. | Browser lost-response retry must return one record and original receipt. The cache is app-session scoped; process-restart recovery is not proven. |
| A06 | Medium / E03-S02 | Desktop exposed only the first 50 Conversations; Tauri omitted cursor forwarding and UI dropped `next_cursor`. | Tauri bridge now validates and forwards bounded base64url cursors; UI parses, appends, deduplicates, and detects non-advancing cursors. UI suite 192/192 and Tauri workspace tests 60/60 pass. | Real Operator/Tauri run with 51+ records, stale/repeated cursors and authorization across supported OS transports remains pending. |
| A07 | Medium / E02-S03 | Late Resource detail/history/head-preview responses could populate a reused editor after selection changed. | Requests are fenced by selected Workspace/Resource plus generation; preview results/errors/busy state use the same fence. Four focused Resource helper tests and full UI suite/typecheck/build pass. | Browser test must resolve delayed A/B responses out of order and inspect actual selected editor state; owner acceptance pending. |
| A08 | Medium / E02-S04 | Needs You load-more could remain busy after refresh/reconnect/Workspace change. | The committed reducer resets busy state on invalidation and ignores stale completion for a newer request. The two pagination regressions pass within the pinned UI suite (192/192). | Rendered inbox and actual approval/UserRequest/blocker flow remain incomplete; live reconnect and owner acceptance pending. |
| A09 | High / E01-S04, E03-S03 | Exact Task creation retry could fail before durable receipt replay if the default lead was cleared after commit. | Route now checks owner/Workspace, then replays the exact durable receipt before mutable default-agent resolution. `cargo test --locked -p litecoworkd` passes 147 unit + 4 CLI tests; route test clears default, retries, denies foreign owner, and asserts one row/event. | Packaged authenticated IPC crash/restart/lost-ACK SYSTEM run and owner workflow remain pending. |
| A10 | Medium / E01-S04 | Shared mutation helper accepted only 128-byte idempotency keys although OpenAPI allows 200. | Helper accepts 1–200 printable ASCII bytes; route/unit cases cover 129, 200, 201 and malformed input. Full daemon tests pass 147 unit + 4 CLI. | Transport-level test through packaged IPC and owner acceptance remain pending. |
| A11 | Medium / E01-S04, E02-S02 | Archive, policy and default-agent handlers misreported receipt digest conflicts as STALE_WORKSPACE_VERSION. | All three map same-key/different-payload receipt conflicts to CONFLICT while real CAS conflicts remain STALE_WORKSPACE_VERSION; route tests also retain exact replay. Full daemon tests pass 147 unit + 4 CLI. | Packaged authenticated IPC/Workspace UI recovery and owner acceptance remain pending. |
| A12 | High / E02-S03 | Resource revision upload failed to advance its local chunk offset after an accepted chunk. Files larger than one chunk could repeatedly submit chunk zero and never reach commit. | The upload loop now assigns `offset = end` after a successful chunk receipt. Focused rendered Chrome harness asserts one accepted chunk advances to the commit stage for a file smaller than one negotiated chunk; storage/daemon/UI suites are being rerun for this checkpoint. | Multi-chunk upload through packaged Tauri and authenticated daemon, interrupted/resumed multi-chunk upload, provider/OS qualification, and owner acceptance remain pending. |

UI paths abbreviated after A01 are relative to `apps/litecowork-ui/` unless stated.
No newly demonstrated authorization bypass, secret leak or SQL recovery corruption was
found in the inspected paths. That limited result is not a security certification.

## Documentation, tooling and evidence findings

| ID | Severity | Finding / evidence | Disposition |
|---|---|---|---|
| D01 | Medium | Hosted CI at `40b1233` passed workspace checks but failed generated coverage freshness. Workflow and review-packet changes were not reflected in coverage. Run `38448659899`. | The current local `scripts/check.sh` refreshes first-party Markdown and contract/design inventories before regenerating coverage, then checks freshness. `b003e9e` run `38452400483` still failed on stale coverage; the fix and current generated artifacts are uncommitted and require a fresh hosted run before closure. |
| D02 | Medium | Next-session handoff still identified the older storage-only frontier and omitted later source repairs and current audit evidence. | Refreshing the handoff and CURRENT-RUN to the live pushed checkpoint plus this verified working-tree slice; keep owner/provider/system gates pending and preserve the full V1 backlog. |
| D03 | Medium | WORKFLOWS U15/U16 said “no invisible autonomous memory proposal V1,” contradicting accepted quiet private learning in LEARNING/ADR-0024/E08-S03. | Corrected workflow row to qualified private learning with eligibility, visibility/opt-out, conflict/revocation gates. No learning implementation claim. |
| D04 | Medium | TESTING owner suite, platform matrix and release CI mixed local G2/G3 with cloud continuation, remote Runtime and external human channel. SCOPE/ROADMAP defer E11/E12. | Qualified cloud/remote/channel cases by their post-V1 gates; desktop/local acceptance remains required. |
| D05 | Medium | `scripts/audit_v1_markdown.py:16,40` omitted `archive_code`, `archives_docs`, `ARCHIVE`, `archives` from traversal pruning despite AGENTS historical boundary. Future local archives could contaminate first-party tracked inventory with owner filenames/hashes. | **Fixed in `b003e9e`:** traversal prunes all four directories; focused fixture proves those trees are not visited and current docs remain inventoried. The hosted run exposed the separate D01 coverage generation-order issue. |
| D06 | Medium | At the audit checkpoint, 13 active stories lacked dedicated standard review packets: E02-S04, E03-S01/S03, E05-S01, E07-S00/S01, E08-S02/S03/S04/S07/S08, E09-S02/S05. | Packet files now exist for all 20 active stories; the implementation-plan validator checks presence. This closes the missing-file finding. Evidence completeness/quality remains per-packet review work; E03-S02 is planned and receives a separate packet for this repair slice. |
| D07 | Medium | The root Cargo workspace omitted the separate Tauri workspace, and CI previously ran neither UI tests/build nor rendered UI checks. | The workflow now has separate root, pinned Node/pnpm UI (unit, rendered Chrome, build), and Tauri Cargo jobs. Root `scripts/check.sh` includes strict V1 audits and generated-output freshness checks. Hosted execution is pending; macOS/Windows/provider/SYSTEM/owner evidence and screen-reader/physical zoom remain open. |
| D08 | Medium | The default shell is Node 22.23.1 / pnpm 12.11.2 while the project pins Node 24.21.0 / pnpm 12.10.1; UI dependency bootstrap had failed before installation. | Bootstrap and local UI test path are fixed. Current pinned Node/pnpm suite passes 195/195; API freshness and the production build are being rerun. The workflow now installs locked dependencies and runs UI unit/rendered/build gates. Hosted, packaged Tauri, OS/SYSTEM/USER gates remain open. |
| D09 | Low | ARCHITECTURE-COVERAGE said product behavior “is not implemented or qualified,” although tested partial source exists. Generator repeated this wording. | **Fixed in `48e5470`:** generator now says its coverage index does not establish implementation or qualification; focused wording test and coverage, plan, and architecture validators pass. |
| D10 | Low | Reference decision map says “+ background Conversation,” while AUTOMATION permits unique follow-up Conversation only on demand and forbids hidden Conversations for each run. | Corrected reference map to on-demand Task follow-up; `docs/AUTOMATION.md:479–480` remains authority. |
| D11 | Medium | OpenAPI/ledger counts can be read as served feature counts. E.g. producer-scoped POST plan revisions exists in machine spec but router serves GET only. Structural auditors do not discover that handler gap. | A scoped representative inventory now separates OpenAPI target, mounted route, provider, transport/OS, CODE, SYSTEM and OWNER USER evidence. Its Conversation create row is being refreshed with the real-SQLite route test; Task evidence links are being corrected. It explicitly does not claim an exhaustive served-route inventory. |
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
| E02-S01–S04 | Installed shell/tray/startup/Quick Entry; complete Workspace/instruction/read-write recovery; Resource import/root/freshness across claimed OSes; Needs You/approval end-to-end; repair A01/A07/A08 and complete actual UI states/accessibility. |
| E03-S01–S11 | Qualified native Conversation Send/Stop/retry/reconnect; scoped isolated planner and producer-authenticated atomic Plan/Steps/receipt submission; integrated Attempt launch/lease/settlement/recovery; native feature parity; AgentModule registry/discovery/install/update/repair; descriptor/config/auth/secure slots/native options/composer; exact per-agent/OS conformance. Repair A04–A06/A09–A11. |
| E04-S01–S06 | Atomic Trust admission/audit/grants/Secret issuance; Gateway progressive discovery; real Capability dispatch, idempotency and ambiguous Effect reconciliation; actual LiteSPM/MCP authority/integration; independently verified Artifact/Evidence completion; cross-agent package bridges. Pure decision/persistence primitives remain insufficient. |
| E05-S01–S05 | Full DelegationProfile flow, accepted-plan child admission/isolation, qualified routing and measured/estimated/unknown cost, quota/budget, independent escalation and lead failover, heterogeneous native/provider corpus and B83 evidence. |
| E06-S01–S05 | Runtime-integrated archive preview/extraction with bounded provider, child provenance/deletion/cancel; hybrid semantic retrieval and grounded attachments; incremental invalidation/purge; local-model hardware/no-network conformance; full Knowledge readiness/placement UI. ZIP storage/standalone preview is not extraction/RAG. |
| E07-S00–S05 | Real enforcing EnvironmentProvider connected to exact Resource staging, attestation/incarnation/admission, output quotas, approved egress, native tool mediation and quiescence; persistent previews/lifecycle; real Chrome/profile/consent/control epochs/takeover; ActionBatch/deadline/warmth; office/data/research and desktop/computer/PTY providers. Windows roots remain UNAVAILABLE until actual identity/reparse/volume/WSL/restart tests qualify them. |
| E08-S01–S08 | Optional chat-first Coworker ownership/last-active/Quick Create and connections; qualified Goals/Suggestions; private automatic learning with canonical candidate/memory policy/provenance/consent/filter/revoke/recall; full Live Work/Workbench/browser/terminal shell; keyboard/screen-reader/zoom/reduced-motion; typed transient/final Presentation Runtime; exact Artifact versions; semantic-first RichPresentation/Host Guidance and matched OpenIntelligentUI viability/performance tests. Repair A02/A03. |
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

## Verification observed during this audit

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

First implementation repairs: A01/A07 (Resource identity and draft safety), A02/A03
(RichPresentation identity/cache lifecycle), A09/A11/A10 (receipt/error/key contract),
then remaining UI races and pagination. Agree interface ownership and use disjoint
agents; rerun focused meaningful regression, then affected integration gates. Continue
foundational E01/E02/E03/E07 containment/native Task spine rather than opening unrelated
features. Keep full remaining V1 scope and separate owner evidence obligations.
