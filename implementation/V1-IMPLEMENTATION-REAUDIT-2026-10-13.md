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

## Newly identified source defects — open

| ID | Severity / owner | Finding and concrete trigger | Source evidence | Required repair evidence |
|---|---|---|---|---|
| A01 | High / E02-S03, E08-S03 | Resource A's unsaved draft/file can survive selecting B and be published as B's revision. The reused editor changes target identity without applying its dirty-draft transition. | `apps/litecowork-ui/src/App.tsx:3343,3358`; `src/resources/ResourceRevisionEditor.tsx:124–148,402–405,529–534` | Rendered editor: dirty A → select B → Save/Discard/Stay; no A bytes written to B; Workspace switch and pending status/upload variants. |
| A02 | High / E08-S08 | Cached RichPresentation remains displayed after current reference becomes POLICY_OMITTED, UNAVAILABLE or INTEGRITY_FAILED. It can contradict the unavailable notice. | `apps/litecowork-ui/src/conversations/ConversationsPage.tsx:47,79,83` | Gate/reset cache by complete current identity/digest/availability; revoke/change reference while READY and assert semantic fallback. |
| A03 | Medium / E08-S08 | A live snapshot replaces the reference object during a rich fetch, cancels the effect, then LOADING prevents a replacement fetch. No loading retry is shown. | `ConversationsPage.tsx:47,69,73`; `conversation-view.ts:87,181` | Delayed fetch + equivalent live reference; eventual READY/FAILED, bounded retry and no permanent loading. |
| A04 | Medium / E03-S02, E08-S04 | Late Conversation creation updates shared state after switching Workspace; create lacks the read paths' request generation. | `ConversationsPage.tsx:190–204` | Delayed create A → switch B; no A list/selection/title/snapshot applied in B. |
| A05 | Medium / E03-S02 | Conversation Create generates a new RequestId for each attempt; lost committed response followed by retry can create another record. | `ConversationsPage.tsx:195`; compare Task recovery `App.tsx:1009–1073` | Preserve exact unresolved envelope/key; committed response loss → one Conversation, original receipt. |
| A06 | Medium / E03-S02 | Desktop can only list the first 50 Conversations. Tauri provides no cursor and UI parser drops next_cursor. Older records become unreachable. | `src-tauri/src/conversation_bridge.rs:118`; `conversation-view.ts:135`; server `apps/litecoworkd/src/conversation_operator.rs:163–204` | 51+ records; stable bounded pagination, authorization and repeated/stale cursor tests. |
| A07 | Medium / E02-S03 | Resource detail/history and current-head preview can apply a late A response in B's reused editor. Comparing against the captured request Resource does not fence current selection. | `src/resources/ResourceRevisionEditor.tsx:159–179,246–260` | Delayed A/B responses resolve out of order; only selected identity is displayed and edited. |
| A08 | Medium / E02-S04 | Needs You load-more remains busy after refresh/reconnect/Workspace change. Generation invalidates the old finally, while the new generation never resets loadingMore. | `src/needs-you/TaskAttentionPage.tsx:56–72,102–104,137` | Delayed page → invalidate generation → fresh page; next load-more works and stale rows do not apply. |
| A09 | High / E01-S04, E03-S03 | Exact Task creation retry can fail before durable receipt replay if the default lead was cleared after the original commit. Current default resolution precedes receipt access. | `apps/litecoworkd/src/operator.rs:3681–3694`; default clearing `crates/domain-workspace/src/lib.rs:1404` | Create using default → lose ACK → clear default → exact retry returns original Task without new events or rows. Recheck authentication/scope before replay. |
| A10 | Medium / E01-S04 | Shared mutation helper accepts only 128-byte idempotency keys; canonical OpenAPI allows 200. Valid ASCII keys of length 129–200 fail on some routes. | `apps/litecoworkd/src/operator.rs:6186–6193`; `docs/schemas/operator-api.openapi.yaml:3670–3675` | Consistent canonical bound across handlers/IPC; boundary and malformed-key route fixtures. |
| A11 | Medium / E01-S04, E02-S02 | Archive, replication-policy and default-agent handlers collapse receipt digest conflicts into STALE_WORKSPACE_VERSION instead of CONFLICT. Wrong recovery advice can encourage changed retry identity. | `apps/litecoworkd/src/operator.rs:7314,7440,7543`; receipt conflict `crates/storage-sqlite/src/lib.rs:16768–16773` | Same key/different payload → CONFLICT; actual aggregate CAS → STALE_WORKSPACE_VERSION; exact replay unchanged. |

UI paths abbreviated after A01 are relative to `apps/litecowork-ui/` unless stated.
No newly demonstrated authorization bypass, secret leak or SQL recovery corruption was
found in the inspected paths. That limited result is not a security certification.

## Documentation, tooling and evidence findings

| ID | Severity | Finding / evidence | Disposition |
|---|---|---|---|
| D01 | Medium | Hosted CI at `40b1233` passes workspace checks but fails generated coverage freshness. Workflow and review-packet changes were not reflected in coverage. Run `38448659899`. | Refresh generated coverage after this reviewed audit and rerun validators; hosted result remains its observed failure until a new run proves otherwise. |
| D02 | Medium | Next-session handoff still identified `5ced2ab` and the older storage-only frontier, omitting `c8158ac`, D-Bus/Bubblewrap fixes and current audit. | Corrected handoff checkpoint, current evidence, source-defect priority and environment limits in this audit. Historical storage evidence retained. |
| D03 | Medium | WORKFLOWS U15/U16 said “no invisible autonomous memory proposal V1,” contradicting accepted quiet private learning in LEARNING/ADR-0024/E08-S03. | Corrected workflow row to qualified private learning with eligibility, visibility/opt-out, conflict/revocation gates. No learning implementation claim. |
| D04 | Medium | TESTING owner suite, platform matrix and release CI mixed local G2/G3 with cloud continuation, remote Runtime and external human channel. SCOPE/ROADMAP defer E11/E12. | Qualified cloud/remote/channel cases by their post-V1 gates; desktop/local acceptance remains required. |
| D05 | Medium | `scripts/audit_v1_markdown.py:16,40` omits `archive_code`, `archives_docs`, `ARCHIVE`, `archives` from traversal pruning despite AGENTS historical boundary. Future local archives could contaminate first-party tracked inventory with owner filenames/hashes. | OPEN latent tool defect. Those directories are absent now; no current contamination observed. Add exclusions and a focused pruning test before future archive use. |
| D06 | Medium | 13 active stories lack a dedicated standard review packet: E02-S04, E03-S01/S03, E05-S01, E07-S00/S01, E08-S02/S03/S04/S07/S08, E09-S02/S05. Required fields are in PROCESS review packet. | OPEN traceability gap. Consolidate existing source/commands/versions/limits from CURRENT-RUN; absence of a filename is not proof of absent tests. |
| D07 | Medium | Only CI workflow calls `scripts/check.sh`; root Cargo workspace excludes separate Tauri workspace. It runs neither frontend tests/build nor real rendered-component/accessibility gates, strict V1 ledgers or owner-consistency checks. | OPEN automation coverage gap, distinct from the script's documented Rust/Python scope. Add appropriate separate lanes; OS/native proof still requires actual hosts. |
| D08 | Medium | Current default shell is Node 22.23.1 / pnpm 12.11.2, while project pins Node 24.21.0 / pnpm 12.10.1. UI dependency tree is absent; two UI test files cannot import @tauri-apps/api. | OPEN environment/bootstrap qualification. Use installed pinned Node; qualify frozen-lockfile install, lifecycle policy and UI build before a new green frontend claim. Do not delete owner caches. |
| D09 | Low | ARCHITECTURE-COVERAGE says product behavior “is not implemented or qualified,” although tested partial source exists. Generator repeats this wording. | OPEN clarity issue in `scripts/validate_implementation_coverage.py` generated disclaimer. Prefer “this audit does not establish implementation or qualification.” |
| D10 | Low | Reference decision map says “+ background Conversation,” while AUTOMATION permits unique follow-up Conversation only on demand and forbids hidden Conversations for each run. | Corrected reference map to on-demand Task follow-up; `docs/AUTOMATION.md:479–480` remains authority. |
| D11 | Medium | OpenAPI/ledger counts can be read as served feature counts. E.g. producer-scoped POST plan revisions exists in machine spec but router serves GET only. Structural auditors do not discover that handler gap. | OPEN verification limitation; maintain served-handler + provider + OS + evidence inventory separately from design coverage. Do not downgrade target contract. |
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
