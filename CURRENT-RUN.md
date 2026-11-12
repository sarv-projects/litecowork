# Current engineering handoff — 2026-11-12

This is the current source and design handoff. It distinguishes the shipped checkout from the proposed V1 experience and preserves the older dated notes below for provenance. A passing structural check, UI fixture, route, or unit test does not qualify a packaged provider journey.

## Repository state at review checkpoint

- Repository: LiteCowork; branch `main`; reviewed HEAD `64c435149a82ba952c6c3bf3049e26cf2d9344b1`; `origin/main` is `6adadab539a62ca1c8b70621c7f88e1785cfe8c2`. Local main is three commits ahead of origin.
- One worktree. The reviewed 141-path set (137 tracked paths and 4 new design/audit documents) was committed locally as `a6caa8b` (`docs: align workspace design and implementation handoff`). The push attempt failed because Git Credential Manager authentication was canceled/unavailable in this non-TTY session; `origin/main` did not move. This status-only handoff update is also committed locally. At handoff, `main` is six commits ahead of `origin/main` and the working tree is clean.
- Preserve `audit-output/`, `archives/`, `.agents/`, `repo_take/`, ignored build/dependency data, stashes and other user work. Stage only the reviewed intended paths. No reset, clean, force-push or history rewrite.
- Active implementation is in `apps/` and `crates/`: Rust daemon/domain/storage, Tauri v2 desktop and React UI. This is not a documentation-only checkout.

## What LiteCowork is building

LiteCowork is a local-first, conversation-first, bring-your-own-agent desktop workspace. A normal chat stays a Conversation and does not require a Task or Coworker. Native agent harnesses own their reasoning, authentication, models and native tools; LiteCowork owns Workspace boundaries, Conversation/Task truth, permissions for mediated operations, durable evidence, artifacts, context and recovery.

The proposed compact shell keeps four primary destinations—Work, Coworkers, Library and Automations—plus a distinct **Needs You** attention inbox. New Chat and Recent Chats remain easy to reach; Settings stays at the bottom; less-used destinations sit under More. The current code has compacted secondary pages under More, while retaining separate Needs You, Work, Coworkers, Automations and Library controls. The complete navigation proposal is still IN_REVIEW.

The right side is one contextual Workbench, not another permanent navigation rail. Only opened objects should become tabs; **+** offers Computer, Browser, Terminal, Files, Changes and Canvas. Computer shows a native-selected display/window. Browser is a separate managed browser session. Terminal is a real PTY/ConPTY under its admitted Environment. Changes are real version/VCS diffs. Activity comes from records. Inline structured answers can expand into Canvas while preserving the same local document state. Today the Workbench is a fixed launcher/status UI: there are no object tabs/+ flow, live Computer capture/input, managed Browser, actual Terminal, real diffs or Activity provider.

For terminal first use, start no process on app launch or when opening **+**. **New terminal** is explicit. Prefer Windows PowerShell 7 only when that exact shell is available inside a qualified admitted Environment; otherwise offer supported profiles. Never silently choose WSL/Git Bash or launch an uncontained host shell. Current Terminal remains unavailable because there is no qualified provider/Environment path.

Computer observation, agent disclosure and computer input are separate permissions. The in-app **Take over** strip is not enough while the target app covers LiteCowork. The proposed safety gate requires a qualified out-of-app global shortcut or native tray action that reaches the same last-hop broker fence; if no such action qualifies, that OS/provider stays observation-only. **Take over** stops further computer input but does not stop reasoning or unrelated work. **Stop run** separately asks the Conversation or Task owner to cancel and reconcile. Sending a message is not takeover or generic steering. No global emergency route, shortcut or tray command is served today (A-78).

## Intelligent answers and ACP

The useful part of the supplied concept is one validated, declarative presentation path: semantic response first; optional registered components for charts, tables, metrics, forms and bounded calculators; deterministic local calculations; accessible text fallback; and Canvas expansion over the same document identity. No arbitrary model HTML/JavaScript runs in the chat WebView, and no hidden second LLM is inserted into BYOA. View persistence and conversational modification are separate proposals requiring renewed Feature 016 review.

ACP supplies agent lifecycle and event transport, not a UI schema. Current ACP launches do not provide a qualified presentation ingress; the configured MCP server list is empty. A provider proposal tool or structured output needs exact session/turn/retry correlation, host-derived scope, bounded schema validation, cross-session isolation and provider-version evidence. A `tool_call` event is observation, not permission to execute it. A2UI is a candidate adapter into the existing canonical presentation model, not a second renderer/state authority. Official material checked 2026-11-12 reports A2UI 1.0 as stable and lists a React renderer, while README/roadmap still describe 0.9.1 as production and 1.0 as a candidate. GitHub Releases has CLI/core/agent SDK releases; the exact React renderer package/version and its v1.0 conformance remain unpinned. See `docs/ACP-PRESENTATION-PIPELINE-LLD.md` and Feature 016 RESEARCH.

## Current progress and remaining work

- **Conversation:** durable admission/outbox, single-flight, response commit, text ResourceRef materialization, stream, Stop and Retry have CODE/SERVED evidence. The current OpenCode route is not a packaged, qualified journey. Form/UserRequest and permission event production/reply remain unverified; no second provider is qualified. The accepted disabled-until-qualified Send gate is still open (A-24). Waiting-state Stop is a current gap: accepted AC-024 covers WAITING_USER/WAITING_DEPENDENCY, but the Operator rejects those cancel states and the UI hides Stop (A-79).
- **Agent setup:** descriptor-driven controls and ID-only opener paths are served fail-closed. No module currently exposes usable native sign-in/configuration methods, credential submission lacks a durable crash-safe receipt, and there is no qualified sign-in → bind → send journey.
- **Relay:** preserve source identity and bounded portable context, gate on actual dispatchability, qualify a second provider, then prove A→B→A with denial/recovery. A picker or provider registry row is not dispatch qualification.
- **Workspace context:** text ResourceRefs work partially. Root ingestion/indexing, durable retrieval/send receipt, Context Used, deletion/replay/race recovery and Windows-native qualification remain incomplete. A-65/A-69 are open.
- **Task execution/control:** Task records and partial planning/control do not constitute a safe execution slice. Still required: admit Attempt, provision/attest Environment, enforce Trust at the actual write boundary, reconcile a receipt, independently verify, and recover cancellation/timeout/restart. Linux is first. Windows Environment remains unavailable (B-35). Delegation and child Attempt gates are tracked in Features 005/015; A-70 remains open.
- **Workbench:** current Files is a Resource preview, Browser is an empty embedded-page preview, Terminal/Review/Activity/Computer are unavailable or status-only. Real browser, PTY/ConPTY, VCS diff, record-backed Activity, selected-screen capture, safe input and packaged recovery are outstanding. Computer revision 1.1 is IN_REVIEW; A-78 adds the background takeover gate.
- **Artifacts and presentation:** bounded text/plain editing and native Save As exist, but there is no complete professional Markdown create/edit/publish/export-receipt journey. Feature 016 r0.2 proposes richer registered components/Canvas/persistence; it is not accepted or implemented. Arbitrary custom app execution stays outside V1 until its separate containment contract qualifies.
- **Other owners:** LiteSPM remains owner-deferred; Automations/notifications, backup/restore and release qualification retain the task-level gaps in `docs/IMPLEMENTATION-STATUS.md` and feature TASKS.

## Documentation audit status

The requested complete semantic, line-by-line reread of all documentation and source is **still open**. `docs/audit/MODULE-DESIGN-REVIEW-2026-11-12.md` is explicit about its baseline and coverage: one subreview semantically read 10,356 of 22,910 lines in 18 feature packages, excluding Features 008 and 016; many other sections were inventory-only or partial. Separate review notes found Feature 006 partial and Feature 007 not reviewed in that pass. The 008/016 design findings are captured in their new LLDs, but there is no merged per-line coverage ledger for them. Validators that scan/count files are not substitutes for semantic review. Re-read the exact owner package and cross-layer journey before each implementation task; do not claim the global audit is finished.

The follow-up review added A-78 (background Computer takeover reachability) and A-79 (waiting-state Conversation Stop contract gap), and found M17 (secondary-navigation focus loss on route remount). Register A-65, A-66, A-67, A-69, A-70, A-73 and A-77 are OPEN/BLOCKED; A-68, A-72, A-74 and A-75 are IN_REVIEW; A-71 is partially aligned with semantic owner review open; A-76 resolves the current transport-authority conflict while implementation remains deferred. Feature 008 revision 1.1 and Feature 016 revision 0.2 are TARGET/IN_REVIEW proposals, not accepted implementation authority. Check task lifecycle and owner decisions before implementing blocked/deferred work.

## Validation on the current working tree

Run on 2026-11-12 at HEAD `64c4351` with the dirty tree described above:

| Check | Result | Limit |
|---|---|---|
| `pnpm test` | 635 passed, 0 failed, 0 skipped at the preceding full-suite checkpoint; latest focused affected tests below | UI/component/unit fixtures; not packaged provider qualification |
| Focused Conversation/Navigation/Workbench tests | 58 passed across four focused commands after the final UI copy and guard edits | Does not substitute for the full suite or packaged provider qualification |
| `cargo test -p litecoworkd --locked response_commit_identity_is_stable_scoped_and_contains_no_native_identifier` | 1 passed | Focused test; existing unrelated compiler warnings; not a full Rust suite |
| `pnpm run build` | API freshness, TypeScript and Vite build passed | Main JS 1,080.62 kB minified / 296.82 kB gzip, above 500 kB advisory |
| `pnpm exec tsc --noEmit` | Passed | UI type check; not a user journey |
| `cargo fmt --all -- --check` | Passed | Formatting check only |
| `pnpm test:browser` | Did not launch | `LITECOWORK_CHROME` is unset, so rendered browser checks remain unverified |
| `.specify/scripts/validate_feature_package.py` | 20/20 structural checks passed | Not owner acceptance |
| `scripts/audit_v1_contracts.py --strict` | 0 errors; 231 OpenAPI, 178 served, 4 outside OpenAPI, 53 target-only | Contract/route consistency, not integrated behavior |
| `scripts/audit_served_routes.py` | Inventory matches composed router | Does not qualify routes |
| `scripts/validate_architecture.py` | Passed with 1 warning | Optional `openapi-spec-validator` is not installed |
| `scripts/validate_design_owner_consistency.py` | 373 documents / 77,144 lines scanned; 36 assertions, 0 issues | Structural assertions only |
| `scripts/audit_v1_design_depth.py` | 372 files / 76,733 lines; 0 structural findings | Structural inventory, not semantic completeness |
| `scripts/audit_v1_markdown.py` | 369 files / 76,758 lines; 0 broken relative links | Link/corpus audit, not semantic completeness |
| `git -c core.whitespace=cr-at-eol diff --check` | Passed | CRLF-aware whitespace check; the generated inventory retains its existing line-ending warning |

No packaged desktop, real native-agent sign-in/permission, Computer capture/input, Browser, PTY, Task write/verification, Artifact export, backup/restore or release-matrix qualification was run in this pass. Earlier Windows workspace/UI results remain dated evidence at their own source revisions and must not be combined with this focused run as a full qualification claim.

## Next handoff sequence

1. Finish the remaining semantic document/source audit using explicit coverage accounting; reconcile A-65 to A-79 at their owner documents before any conflicting implementation.
2. Complete one exact native Conversation path (descriptor/readiness → send → real events → stop/retry → restart/recovery), including provider event evidence and denial. Resolve A-79. Then run the packaged desktop journey.
3. Complete agent authentication/credential durability and qualify sign-in/bind/send. Add a second genuinely dispatchable provider before A→B→A relay qualification.
4. Complete Workspace-root ingestion and atomic context admission/receipt/Context Used with the Feature 001 transaction boundary reconciled under A-65.
5. Build the first safe Task/Attempt/Environment/Trust/write/receipt/independent-verification slice on Linux. Keep Windows execution unavailable until B-35 qualifies.
6. Promote and implement real Workbench providers, including the out-of-app Computer takeover gate; complete professional Markdown Artifact authoring/export; then implement accepted declarative presentation/Canvas capabilities with fallback.
7. Finish platform-denial/recovery, backup/restore and release-row evidence; obtain owner USER qualification before marking V1 ready.

The existing local commits and all reviewed work must be preserved. Commit/push are still pending at this checkpoint; if attempted, use a normal push only and record the exact result here.

## Historical V1 implementation progress snapshot (2026-11-09; superseded)

This dated narrative is retained for provenance only. Its code/task statuses and test baselines are superseded by the current source-backed summary and per-package task links above.

Done and verified (committed on `docs/audit-competitor-superset`):

- **ACP native-agent core (Feature 001/002, register A-52/A-54, tasks 001-T01/T04, P-11).** The broken top-level-SSE parsing (the A-52 defect where agent output never rendered) is replaced with a real Agent Client Protocol boundary: `apps/litecoworkd/src/agents/adapter.rs` (ACP client: initialize/session.new/session.prompt/session.update/session.cancel behind a per-provider launch catalog), `agents/normalizer.rs` (typed event normalizer mapping ACP update kinds to internal turn events), `agents/native_provider.rs` plus `agents/fixtures/opencode_v2_0_32_turn.jsonl` (a captured real v2.0.32 turn). The supervisor consumes the normalizer under a line-resetting inactivity watchdog that replaces the hard 60 s cap. OpenCode v2 HTTP is kept as one non-ACP adapter until its ACP route qualifies. Commits e10d496, ef2bed7.
- **Verification:** 27 agent/adapter/normalizer/native-provider tests pass; `cargo check -p litecoworkd` clean; the storage-sqlite task/delegation/routine family that was briefly broken is back to green; UI `tsc --noEmit` and `api:check` clean.

Deferred with a recorded reason (not done, not faked):

- **Migration wiring (done in Wave 1, Feature 001).** `SCHEMA_VERSION` is now 32 (v31 adds the Conversation history lifecycle side table, 014-T03; v32 adds Conversation control events and owner side notes, 012-T05; v30 adds REPORTED Conversation turn activity records, 001-T25 storage layer only). v28 (Workspace delegation settings, A-40) is wired together with its production writer: the two `task_spec_revisions` writers in `crates/storage-sqlite/src/lib.rs` resolve and pin `delegation_policy_snapshot_json` from the Workspace settings head, and every full-schema fixture that inserts a TaskSpec revision now supplies a snapshot. v29 (`docs/schemas/sqlite-v29.sql`) adds the Conversation dispatch outbox, response commit identity, usage records and single-flight triggers with their writers (`create_conversation_turn`, `commit_conversation_agent_response`, the dispatch-intent claim/release/list store methods). Verified: storage-sqlite 603 passed, 0 failed; litecoworkd migration-dependent tests green.
- **Remaining packages.** Superseded by "Handoff status by package (2026-11-10)" below; delivery follows the waves in docs/LITECOWORK-BUILD-TASKS.md.

## Historical remaining-work snapshot — superseded by the current status above

The package TASKS are the stable itemized backlog. Recommended next order, after checking current blockers and dependencies:

1. Conversation (001): capture real OpenCode v2.0.32 Form/permission events, settle B-06 semantics with owner evidence, finish normalized answer/deny and recovery paths, then execute the packaged Windows send/stop/retry/restart journey. Keep provider-input work blocked until the event exists in evidence.
2. Agent setup (002): complete descriptor-backed auth/credential flows with durable replay receipts, enforce the fresh descriptor at binding enable and dispatch admission, and qualify a real provider sign-in/bind/send route. Add a second provider only with evidence and a full supported route.
3. Workspace context (003): supported root content ingestion/indexing, retrieval on Send, durable ContextAttachment/retrieval receipt and Context Used UI; cover denial, races, restart and native platform behavior. Windows root operations still need native testing.
4. Safe Task slice (005): follow the accepted Linux-first plan from admission through Environment attestation, Trust at the write boundary, one bounded file edit, receipt reconciliation, independent verification and cancellation/timeout/restart recovery. Windows Environment remains unavailable pending B-35.
5. Workbench (008): implement daemon-owned isolated browser consent/session/observation/takeover, real Environment-owned PTY/ConPTY, version/VCS-backed diffs and record-backed Activity. Computer use and whole-desktop capture stay deferred.
6. Artifacts and presentation (007/016): first Artifact creation/provenance, a real Markdown edit/publish/export-receipt journey, then the approved rich-presentation path, media provenance, security, fallback/preferences and release qualification.
7. Close cross-cutting API/schema/UI-map drift and run the exact packaged E2E/security/platform matrix. Route counts and unit tests alone do not qualify release behavior.

## Historical UI/UX and architecture audit findings (2026-11-11; superseded)

- The requested right Workbench is a target place for managed Browser/Files/Terminal/Changes/Activity surfaces, not a live laptop mirror. Current Desktop/Browser UI is preview/status-only; there is no low-overhead desktop capture or real computer-use provider. The design records a bounded ephemeral managed-tab viewport and a persistent browser Take over control only after the provider owns a session and can fence input epochs. Conversation Take control is a different action that cancels generation, waits for provider quiescence, records ownership transfer and enables a new owner turn.
- Conversation now has durable outbox/single-flight/recovery, version-bound Stop/Retry UI, Activity and a limited text Resource picker at CODE/SERVED levels. Remaining design/logic gaps include pinned provider Form event capture and permission answer normalization, first-message/binary/root attachment coverage, complete ambiguous-error settlement and packaged E2E. The Add context control must not be described as workspace-root ingestion or context retrieval.
- Agent controls now have descriptor read/refresh and ID-only Tauri action wiring; absent qualified modules still fail closed. Credentials still need durable submission receipts; authentication, model/effort choices and provider send are not qualified. Picker selection filters unsupported adapters but does not prove fresh auth/session readiness.
- Workspace context has root lifecycle and metadata observations, plus separate lexical retrieval. It still lacks root content ingestion/indexing, Conversation retrieval wiring, durable context receipts and a Context Used view.
- Task, Workbench providers, professional Artifact authoring/export and production RichPresentation publication are incomplete. Full UI tests and focused Rust/bridge tests provide CODE evidence only. There is no packaged desktop/provider E2E or qualified V1 release row.

Contract audit state: 178 OpenAPI operations are served, four routes remain served outside OpenAPI, and 53 OpenAPI operations are target-only. Promote Feature 014 route contracts and reconcile UI/schema ledgers when implementation is authorized; do not count endpoint registration as a complete journey.

## Historical preservation and delivery notes — current rules are in AGENTS.md

- Inspect the worktree and ignored/untracked paths before every change.
- Preserve unrelated user work; never clean, reset, stash, rebase or delete archives.
- Do not alter repo_take clones as part of LiteCowork implementation.
- Do not stage secrets, private agent profiles, local provider credentials, build outputs or caches.
- Do not commit or push unless explicitly requested.
- Keep target contracts, implementation findings and qualification evidence distinct.
- Run the narrow test for every change, then contract checks and broader regression checks.
- Inspect the complete diff and verify generated inventories before considering a documentation reconciliation complete.

## Historical repository handoff snapshot - 2026-11-07 (superseded by the current package index below)

- **Branch / HEAD:** main / 52269e4af6038ccc849e5c2fbef5376f3cdd2d8e.
- **Working tree:** clean. The 197-entry state recorded on 2026-11-06 was committed in full during the audit-repair campaign; nothing was reset, cleaned, stashed, rebased or rewritten.
- **Changes in this work session:** the audit-repair campaign — the workspace lint gate driven to zero with every `#[allow]` carrying a written reason (register A-46); the resource-root read policy made fail-closed in production with its drifting test-only twin deleted (register A-50); the Environment vocabulary migrated into `domain-environment`, inverting the two-crate dependency edge with `storage-core` re-exporting the moved names through an error bridge (register A-41); contradictions recorded with cited evidence in docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md; all generated documents regenerated to a fixed point; full `check.sh --preflight` green.
- **Release posture:** pre-release; V1 is not release-qualified. Feature 001 remains IN_REVIEW; Features 002-011 remain DRAFT.
- **Open inventory:** docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md is the tracked contradiction register; rows whose resolution cell still prescribes an action instead of reporting one remain open and are the next audit targets.

### Source-to-document reconciliation recorded in the 2026-11-07 snapshot

The active source crosswalks and matching UI contracts are maintained in docs/IMPLEMENTATION-STATUS.md, docs/SEMANTIC-SERVED-UI-MAP.md, docs/V1-UI-SCREEN-CONTRACT.md, docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md, and each owning package under specs/001 through specs/011.

- **001 Conversation:** one OpenCode-only dispatch branch; detached dispatch has no durable outbox/startup sweep; strict single-flight is not enforced at admission; Send violates the disabled-until-qualified gate; Stop is ineffective for OpenCode; failed-turn Retry has no UI call site and its backend transition is inconsistent; native transient token frames are not emitted.
- **002 Agents:** 15 curated candidates and ACP metadata are not installed/ready agents. Auth is UNKNOWN and readiness NOT_PROBED. Only Codex/OpenCode profile probes exist; OpenCode is incompatible, Codex has no Conversation dispatch branch. Native install and provider sign-in launch remain unavailable; authenticated ID-only Tauri commands now wire descriptor-declared sign-in/native-config actions, but the current descriptors declare no V1 sign-in methods and the daemon launchers/openers remain fail-closed. Credential submission remains unavailable; binding enable lacks the fresh AC-004 descriptor gate; Usage/model-effort UI is not qualified.
- **003 Resources:** root selection immediately commits on supported Linux/macOS; Windows Add returns 503. Root scan is Linux-only metadata census; Windows/macOS return 501. No content ingestion/watch/index from roots. ZIP originals remain opaque; text index is allowlisted UTF-8 only. Grounded retrieval is read-only and not wired to Conversation or Context Used.
- **004 Agent relay:** backend producer/session identity is projected but desktop message types/parser drop it; picker eligibility is not equivalent to dispatchability; only OpenCode dispatch exists; no qualified A-to-B-to-A route or Resource materialization.
- **005 Tasks:** Task creation and planning readiness are real records/read-only preflight; no accepted-plan -> Attempt -> Environment -> mediated Effect -> verification execution pipeline exists. READY is not running.
- **006 Capabilities:** Discover projections and exact Approval decisions exist, but APPROVE is not an external Effect. Connect/install fails closed, test is UNKNOWN, account Connection/OAuth is absent, CapabilityInvocation is read-only, and provider dispatch/reconciliation is not wired.
- **007 Artifacts:** no production Artifact-create route or create UI; managed text/plain edit/append and safe previews exist; native Save As is local-only and creates no ExportReceipt. Comrak 0.59.0 is pinned but not wired as a format adapter; no Task output producer.
- **008 Workbench:** Files is a Resource projection, Browser is an iframe preview, Terminal fabricates command success without a process, Diffs are synthetic, Activity is not rendered and Computer-use is unavailable.
- **009 Coworkers:** identity/revisions/status and configuration persist, but there is no Coworker-owned Conversation, Coworker-scoped retrieval or private memory browser/Learning Inbox. Presence returns 503; several saved policy fields have no production consumer; PAUSED admission is partial.
- **010 Automations/Notifications:** local 30-second TriggerHost settles supported recurring schedules and materializes READY Tasks only. Retry/wake policy fields have no consumer. Notification outcome settlement inserts PENDING rows, but no sender advances delivery state; channel preferences are ignored, ON_CONDITION is only DEPENDENCY_BLOCKED, and quiet hours are not editable.
- **011 Shell/Settings:** authenticated OS-local IPC exists; mounted Settings has four categories, not the 16-category target. Appearance controls are ephemeral/inert, shortcut rows are display-only, and several Runtime/Workspace actions are not exposed by the mounted view.

### Global blocker identity

Blocker IDs are globally unique across feature packages. The current allocation is B-21 professional Artifact format; B-22 browser provider; B-23 terminal provider; B-24 Coworker memory; B-25 Quick Create; B-26 Automation trigger; B-27 local notification delivery; B-28 GA OS/security profile; B-29 Settings scope; B-30 Artifact creation; B-31 Artifact export receipt; B-32 Computer-use decision. docs/SPECIFICATION-STANDARD.md and .specify/templates/SPEC.md now require allocation from the global register and synchronized updates to owner SPEC/TASKS/traceability.

The external workflow graph remains provider-owned. WorkflowBackendAdapter and provider graph authoring are target-only; no Core graph runtime or production adapter is present. OpenBot/n8n/Activepieces research is preserved in the linked reference study and does not authorize a Core workflow engine or direct provider graph mutation.

### Historical validation — superseded

- Feature package validator: **12/12 passed**; structural checks only, not owner approval or runtime qualification.
- Cross-owner consistency: **36 assertions passed, 0 issues**.
- V1 contract audit: **0 errors**; 231 OpenAPI operations, 178 served, 1 authenticated local-only route and 53 target-only.
- Design-depth audit: **0 findings**; 367 files, 75,138 lines, 12 domains, 254 functions, 201 Settings rows, 231 routes, served 178 / target-only 53, 107 tables and 20 packages.
- Markdown audit: **0 broken relative links** across 364 files and 75,159 lines.
- Active-corpus manifest: **367 files and 75,138 lines** (manifest excludes itself).
- Architecture/schema validator: passed with one warning because optional openapi-spec-validator is not installed; JSON Schema, 149 typed events, errors, OpenAPI structure, SQLite, Gateway naming, canonical concepts and active Markdown checks passed.
- Rust workspace: lint gate (`clippy --all-targets -- -D warnings`) **0 diagnostics**; `check --all-targets` **0 errors**; `fmt --all --check` clean; test counts as listed under "Current validation on the working tree".
- Whitespace check: **passed** (`check.sh` whitespace stage, exit 0).
- The full serial frontend suite remains incomplete; the Settings rendered-browser check timed out; packaged Tauri and real-provider USER E2E remain unqualified.

### Historical next engineering gates — superseded

1. Keep Send disabled until the exact native OS/provider/auth USER path and durable dispatch/single-flight/stop/retry recovery are qualified.
2. Implement one safe Task vertical slice with accepted plan, Attempt, Environment, Trust, mediated Effect, reconciliation and independent verification.
3. The Workbench Terminal/Diffs/Browser simulation is already removed from the UI (register A-51): Terminal shows "unavailable", Diffs/Review shows "nothing to review", the browser iframe starts empty with no containment claim. The remaining obligation is to implement real provider-backed sessions and evidence (PTY/ConPTY B-23, browser B-22) and defer unsupported Computer use (B-32).
4. Complete one Artifact create/format/export/reopen path with a durable receipt.
5. Complete one account Connection and provider read/write/revoke journey; Approval alone is not execution.
6. Close Coworker-owned Conversation, scoped Memory and proactive PAUSED-admission gaps before enabling those claims.
7. Keep local automation, notification delivery and Settings controls honest until their actual consumers/handlers exist.

### Phase 1 assumption-free doc re-audit resolved (FEAT-002)

The full-corpus re-audit verdict table is [docs/DOCS-REAUDIT-2026.md](docs/DOCS-REAUDIT-2026.md) (five clusters, plus an SDD-completeness-gap list for specs/001-011). It reconciled the known danglers at their owners and recorded the decisions in the register:

- **A-38** (FLOWS.md flow index): the "Flow outputs and UI projection" index now covers F00-F143 (added F115-F124 and F136-F143), and the F119/F128 duplicate is resolved by distinguishing the save flow (F119) from its navigation-recovery sibling (F128).
- **A-39** (UIV1 namespace): retired as an active acceptance authority; acceptance resolves on live flow IDs and feature AC IDs, with resolution notes at V1-UI-ACTION-STATE-LEDGER.md and V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md. The archived catalog stays provenance-only; specs/README.md's UIV1 instruction is updated.
- **A-40** (sqlite-v28 unreachable): v28 is recorded as defined-but-not-yet-wired (`SCHEMA_VERSION` is 27); STORAGE.md ("current schema version is 28" corrected to 27), EVENTS.md, DELEGATION.md and COVERAGE-MATRIX.md are corrected to stop presenting v28 as active.
- **Line-count drift**: CURRENT-RUN.md and IMPLEMENTATION-STATUS.md now cite the live generator output; the corpus audits/generators exclude `.agents/` plan files so plan-file churn no longer inflates corpus counts.
- **A-51 (new)**: the Workbench Terminal/Diff/Browser status docs were stale after the UI was remediated; IMPLEMENTATION-STATUS.md and rows A-08/A-09/A-10/A-30 are corrected to the current source.

### Phase 3 superset matrix + three-way gap analysis resolved (FEAT-004)

The current **cross-cutting gap view** is [docs/SUPERSET-GAP-MATRIX.md](docs/SUPERSET-GAP-MATRIX.md): one feature × {LiteCowork done? (TARGET/CODE/SERVED/INTEGRATED/QUALIFIED) / doc-claims? / doc-accuracy (linked to DOCS-REAUDIT-2026 + A-xx) / each resolved rival has it?} matrix whose feature axis is the union of LiteCowork's capability set and every Phase 2 resolved rival's first-release surface (max superset). It derives the three lists the audit asked for — ✓ Done / ✗ Not-done (real state), 📝 doc errors, 🎯 missing-to-be-a-superset. It **reads** the owners and holds no authority: parity verdicts stay in docs/VIABILITY-VS-COMPETITORS.md, concern→owner in docs/COVERAGE-MATRIX.md, verdicts in docs/DOCS-REAUDIT-2026.md, real state in docs/IMPLEMENTATION-STATUS.md. Cross-linked from VIABILITY-VS-COMPETITORS.md, COMPETITIVE-RESEARCH.md and the docs/README.md index.

- **Headline gap count:** **14** capability gaps to reach a max superset (8 already TARGET-owned via allocated B-xx/RV-xx; 6 RESEARCH/owner-decision gates — RV-06, RV-07, RV-08, RV-13, RV-14, plus the consent-gated screen/audio decision); **~18** doc-accuracy findings (2 still-open code defects: A-24 Send gate, A-26 empty-canvas branding); **0** LiteCowork rows reach Done as a qualified product journey (the lone "Done (pattern)" is the runtime-split invariant), consistent with the pre-release posture.
- No new tracker ids were invented: every missing/owner-decision item reuses an allocated B-xx / RV-xx.

### Pending research (recorded 2026-11-07)

- **Dify workflow-sufficiency verdict — failed on infrastructure mid-run** (session `conversation://394419fe-285e-483c-8478-75f478adb2f9`). Objective unchanged: an evidence-based verdict, grounded in a Dify read, on whether the Trigger-to-Occurrence-to-Task-to-CapabilityInvocation model is sufficient for the V1 promise or a graph engine is warranted — plus a lightweight visible-execution-path sketch that fits the existing authorities. Relaunch; do not re-scope.
- **Agenta visible-execution-path design — failed on infrastructure mid-run** (session `conversation://b4977637-41e5-4e84-b74d-82df1cdf217f`). Objective unchanged: a design (no vendor dependency) derived from Agenta's source for a truthful Task/Attempt and Automation-occurrence execution projection owned by existing LiteCowork authorities. Relaunch; do not re-scope.
- **Competitive-landscape corrections — APPLIED 2026-11-08 (Phase 2 fresh rival re-verification).** Resolved into the owning competitor docs under their own evidence rules: Claude Duet added as a tracked rival (RV-14; first-party URL unresolved on re-check, gap recorded); the OpenBot nightly-labs/openbot vs CopilotKit/openbot naming collision preserved and OpenBot repinned 0.62.0→0.65.0 (PolyForm Noncommercial, reference-only); Grok's app/marketplace layer confirmed routing through cursor.com/marketplace (SpaceXSI/Cursor bundling); ChatGPT Work and dots evidence moved to learn.chatgpt.com (help.openai.com 403s); DeepSeek desktop pin corrected 0.2.2-alpha.2→0.2.2-rc.1; ZCode 3.17.1→3.17.2; OpenAI dots model now GPT-6.1 Terra. Claude Code Desktop reconciled as a distinct Code-tab surface vs Cowork; Open Cowork corrected off the stale "single-turn wrapper" framing; two new local-first desktop coworkers added (Rowboat, OpenLoomi) with pinned clones. Shallow clones for every GitHub-backed rival live in repo_take/ (gitignored, never staged) at recorded HEAD SHAs; RV-13 allocated for the local-vs-cloud scheduled-task truth table. Owning docs: docs/COMPETITIVE-RESEARCH.md (§2026-11-08 Phase 2), docs/VIABILITY-VS-COMPETITORS.md (RV-13/RV-14 + new theme tables), docs/COMPETITOR-PROMOTED-USE-CASES.md (UC-CHAT-09/10, UC-WORK-10/11, UC-FILE-07, UC-AUTO-09, UC-MEM-06), docs/REFERENCE-IMPLEMENTATIONS.md (REF-OPENWORK/OPENCOWORK/ZCODE/DEEPSEEK-HARNESS/ROWBOAT/OPENLOOMI), docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md (refreshed pins + read-first indexes).
- **Reference studies landed:** the OpenHands study is committed (`0fdbfe8`, with the index wave in `f5896b8`); the LiteSPM parity/reconciliation appendices (A: upstream parity requests; B: reconciliation against the pinned revision) are committed on docs/LITESPM-LITECOWORK-INTEROP-LLD.md.

### Wave 0 slice 1: 011-T04 IPC admission bounds (2026-11, CODE)

- Command: `TMPDIR=/tmp cargo test -p operator-ipc --tests --quiet` (3 new `ipc_admission_bounds_*` tests, 5 `unix_endpoint_lifecycle`, 20 lib tests pass). Linux x64 container, not WSL2 qualification evidence; evidence level CODE.
- Env note: tests that bind a Unix socket fail with `InvalidPrivateDirectory` when `TMPDIR` is long (`/projects/sandbox/.work/tmp`, socket path over 103 bytes); run with `TMPDIR=/tmp`.
- Windows ninth-peer and interleaving tests are deferred to a native Windows runner (011-T02).

### Wave 0 slice 2: 011-T05 runtime health honesty (2026-11, CODE)

- Command: `pnpm test` in apps/litecowork-ui: 501 pass, 5 fail, 10 skipped. The same 5 failures (Needs You page x4 needing Chrome, esbuild build-script lock approval) occur on the base state with my changes stashed (496 pass, 5 fail), so they are pre-existing. `npx tsc --noEmit` clean.

### Wave 0 slice 3: 011-T01 contract promotion (2026-11, CODE, contract level only)

- Changed: operator-api.openapi.yaml (RuntimeStopPreview settlement lists and verdict, If-Match on requestLocalRuntimeStop, PowerAssertion, renameWorkspace + RenameWorkspaceRequest), domain-event.schema.json (`workspace.renamed.v1`, 4 error codes mirrored), error-codes.schema.json, served-route-inventory.json (target_only 56 -> 57), EVENTS.md, SCHEMAS.md, RUNTIME-LIFECYCLE.md (accepted 011 interfaces and update quiescence gate), V1-SETTINGS-CONTRACT-LEDGER.md (pinned V1 interactive control set, via scripts/build_v1_settings_ledger.py), route-count constants in three audit scripts and the docs that quote 56/229.
- Commands: `python3 .specify/scripts/validate_feature_package.py` PASS (20 packages); `python3 scripts/audit_served_routes.py` PASS (173/1/57); `validate_architecture.py`, `audit_v1_contracts.py` (errors=0), `audit_v1_design_depth.py` (findings=0), `validate_design_owner_consistency.py` (issues=[]) pass; `pnpm run api:check` and `npx tsc --noEmit` pass after `api:generate`; `pnpm test` 501 pass / 5 pre-existing failures; `cargo test -p storage-sqlite --lib needs_you` 10 pass.
- Not done: no handler for any new operation; `openapi-spec-validator` is not installed here so only the repo's structural checks ran.

### Wave 0 slice 4: 011-T07 local display preferences, 011-T06 partial (2026-11, CODE)

- Added src/settings/local-preferences.ts, tests/local-preferences.test.mjs (7 tests), root application in App.tsx, dark theme, density, text scale and reduced-motion CSS. Removed the inert shortcut card from Settings.
- Commands: `npx tsc --noEmit`, `pnpm run api:check`, `pnpm run build` pass; `pnpm test` 508 pass / 5 pre-existing failures.
- 011-T06 follow-up (same day): settings-control-map.ts plus tests/settings-inert-control-audit.test.mjs landed (pnpm test 513 pass / 5 pre-existing failures). Still PARTIAL: AgentCatalogSettings controls and the category module split.

### Wave 0 slice 5: 011-T08 Workspace rename (2026-11, CODE and SERVED)

- Added domain rename service, Operator route PUT /v1/workspaces/{workspace_id}/name, Tauri `rename_workspace`, UI rename form and workspace-rename.ts. Route inventory was 174 served / 1 local-only / 56 target-only at that slice (current 178/4/53) (counts updated in three audit scripts and the docs that quote them).
- Commands: `TMPDIR=/tmp cargo test -p litecoworkd workspace_rename` 5 pass; `cargo test -p domain-workspace` 7 pass; `cargo check --workspace` clean; `pnpm test` 518 pass / 5 pre-existing failures; `pnpm run build` and `api:check` pass; audit_served_routes and the four doc audits pass.
- `cargo test -p litecoworkd workspace` shows 103 pass and 1 fail (`spawned_daemon_replays_workspace_create_after_lost_ack_and_restart`); it fails identically on the base state with my changes stashed, so it is pre-existing.
- Not verified: src-tauri does not build here (missing gdk-3.0), so the `rename_workspace` command is uncompiled.

### Wave 0 slice 6: 011-T10 Notifications link, 011-T09 partial (2026-11, CODE)

- Added the Notifications Settings category (link plus unavailable reason naming Feature 010) and agents-heading.ts. `pnpm test` plus `pnpm run build` results recorded in the commit; same 5 pre-existing failures.
- 011-T09 stays PARTIAL: AgentCatalogSettings controls are not in the control map.

### Wave 0 slice 7: 011-T11 shortcut registry (partial), 011-T19 reachability (partial) (2026-11, CODE)

- Added src/shell/shortcut-registry.ts, tests/keyboard-focus-baseline.test.mjs (3), tests/shell-reachability.test.mjs (3); removed Back/Forward buttons that called window.history without any pushState.
- Not done: focus-return/skip-link/browser keyboard path (T11); Recent Chats states and Needs You count (T19).

### Wave 0 slice 8: 019-T03 CI skeleton and 019-T05 secret scan hardening (2026-11, CODE, not run on GitHub)

- Added .github/workflows/architecture-docs.yml, .github/CODEOWNERS, tests/test_ci_workflow_policy.py, tests/test_secret_scan.py, scripts/secret-scan-exceptions.json, .gitattributes. Fixed v1_secret_scan.py to mask the matched secret in report previews. Updated tests/test_architecture_validator.py expected counts (230 operations, 174 served).
- Found and fixed: scripts/check.sh, the other shell scripts, .uv-version, .python-version and rust-toolchain.toml were committed with CRLF, which breaks `$(<.uv-version)` comparisons and the shebang on Linux runners. They are now LF with .gitattributes rules.
- Commands: `python3 -m unittest discover -s tests` (with PyYAML on the path): 50 tests, 3 failures, all caused by the host rustc 1.92.0 differing from the pinned 1.98.1 (the stubbed-tool tests call the real rustc); the same file shows these only because the host is not on the pin. Source scan on a `git archive` copy of HEAD: 0 findings with the exceptions table.
- Not verified: actionlint, a real GitHub Actions run, cargo deny, pnpm audit.

### Wave 0 slices 9-14: 019 release machinery and 011 shell/Settings leftovers (2026-11-10, CODE)

Done (all CODE; nothing here is QUALIFIED, and nothing was run on Windows, in a packaged app or in a browser):

- 019-T02: `docs/schemas/release-evidence-bundle.schema.json` and `release-matrix-row.schema.json`, `tests/test_release_schemas.py` (9), `docs/release/README.md`, ADR 0026 skeleton (PROPOSED, B-44 not owner-accepted), ACCEPTANCE section 7 declared rows and NOT_COUNTED rule, TESTING "Release gates", SECURITY signing/updater trust (PROPOSED B-43).
- 019-T10 PARTIAL: `bundle.linux.deb` (WebKitGTK 4.1 dependency, systemd unit shipped disabled), `scripts/linux-release-checksums.sh`, `scripts/qualify-linux-package.sh`, `tests/test_linux_package_scripts.py`. The five GPG signing tests are skipped here (no gpg-agent) and `dpkg-deb` is absent, so no deb was built or inspected.
- 019-T12 PARTIAL: `src/updates/` reducer, controller, typed command parsing and `UpdatePanel` (16 tests). Not mounted; `check_for_update` and `install_update` do not exist.
- 019-T14 DONE: row-aware matrix in `scripts/v1_platform_matrix.py` (WSL2/container/CI detection, NOT_COUNTED, bundle-linked PASSED only, deterministic), `scripts/v1_release_matrix_rows.json`, `tests/test_platform_matrix.py` (17), generated `docs/release/QUALIFICATION-MATRIX.{md,json}` (generated in a container, so every cell is NOT_COUNTED or UNAVAILABLE).
- 019-T15 DONE: `scripts/v1_evidence_bundle.py`, `tests/test_evidence_bundle.py` (9), empty `docs/release/evidence/INDEX.md`, `USER-SIGNOFF-TEMPLATE.md`.
- 019-T18 PARTIAL: `crates/storage-sqlite/tests/resilience.rs` (3 native tests) and a portable resilience runner (`tests/test_resilience_matrix.py`, 3).
- 011-T13 prerequisite: read-only `GET /v1/runtime-lifecycle/stop-preview` is now SERVED (`runtime_stop_operator.rs`, `RuntimeLifecycleStore::read_runtime_stop_observation`). Its verdict is BLOCKED when durable work exists and UNKNOWN otherwise; it is never QUIESCENT because no live-session observer exists. `requestLocalRuntimeStop` stays unmounted. Route inventory was 175 served / 1 local-only / 55 target-only at that slice (current 178/4/53) (three audit scripts, five docs and `tests/test_architecture_validator.py` updated).
- 001-T25 PARTIAL (storage + emission + served read + UI): the supervisor records and publishes REPORTED tool, file, permission, todo and step activity (`TurnActivityRecorder`), `GET /v1/conversations/{id}/turns/{turn_id}/activity` is SERVED (route inventory was 176 served / 1 local-only / 55 target-only at that slice, current 178/4/53; three audit scripts, five docs, `tests/test_architecture_validator.py` and the generated `operator-api.ts` updated), and the Conversation page shows collapsed "reported by the agent" activity cards, a plan count line and the activity inside the FAILED card. Not verified: any real OpenCode run, a rendered UI test, `src-tauri` (new `list_conversation_turn_activity` command is uncompiled here).
- 001-T14 PARTIAL: dispatch-time and submit-time ResourceRef reauthorization with inline text materialization (`resource_materializer.rs`), composer "+" picker and chips. Binary attachments, WorkspaceRoot path files, the first-message-of-a-new-Conversation case, ContextAttachment persistence and the Windows row remain open; no real OpenCode run.
- 001-T12 BLOCKED (no evidenced OpenCode v2.0.32 input-request event; RESEARCH 8.1), 001-T27 PARTIAL (permission shown as a REPORTED activity card, no answer channel), 001-T21 PARTIAL (trace in `contracts/ui-behavior.md` section 9 with the Answer UserRequest and native permission gaps), 001-T22 PARTIAL (status paragraph, register and ledger updates). 001-T09 and 001-T28 need live matrix and release evidence and are untouched.
- 011-T14 PARTIAL: stop preview panel and update gate in Settings > Runtime; Rust bridge `get_runtime_stop_preview` and `check_update_quiescence`.
- 011-T09 DONE, 011-T11 PARTIAL (skip link, universal focus-visible, registry shortcut list), 011-T12 PARTIAL (`power_assertion.rs` state machine, not wired), 011-T16 PARTIAL (trace table), 011-T19 PARTIAL and 011-T21 DONE (SettingsPage deleted and merged), 011-T20 PARTIAL (shared Operator-unavailable panel, LibraryPage module), 011-T22 PARTIAL (Dashboard page).
- Correction of an earlier claim: the 011-T19 reachability test passed against a dead `navigation` array while Goals, Notifications, Ideas and Home had no sidebar path. The sidebar now renders a More group from the array and the test inspects the rendered sidebar.

Commands (Linux container, TMPDIR=/tmp):

- `cargo check --workspace --quiet` clean. `cargo test -p storage-sqlite -p storage-core`: 596 + 113 + resilience 3 pass, 0 fail. `cargo test -p litecoworkd --bin litecoworkd`: 4 failures, all reproduced on the base commit d6dda97 in a separate worktree (3 of them re-checked there: `spawned_runtime_exits_uncleanly_after_operator_endpoint_failure_and_restarts`, `spawned_runtime_lifecycle_restarts_with_current_incarnation`, `capability_commands_require_an_idempotency_key_and_stay_truthful` (a LITESPOM/LITESPM typo in the test), plus the known `spawned_daemon_replays_workspace_create_after_lost_ack_and_restart`).
- `pnpm test`: 593 tests, 578 pass, 5 fail (the same 5 as the baseline: four Needs You page tests and the esbuild lockfile test). `npx tsc --noEmit` and `pnpm run api:check` pass.
- `python3 -m unittest discover -s tests`: 3 failures, all the pinned-toolchain tests (host rustc 1.92.0, uv 0.13.5). `cargo clippy -D warnings` fails only in untouched `domain-effects` (host compiler older than the pin).
- Doc gates: validate_feature_package (20 packages), audit_v1_markdown (0 broken links), audit_v1_contracts, audit_v1_design_depth, validate_design_owner_consistency, validate_architecture and audit_served_routes pass. `docs/audit/*` regenerated outputs were reverted (generators write LF, committed files are CRLF).

Not verified / unverifiable here: `src-tauri` (gdk-3.0), any Windows behavior, any browser/a11y run (no Chrome), logind keep-awake, GPG signing, deb build, actionlint, GitHub Actions.

Deferred or blocked: 019-T01 and T09, T11, T13 (owner B-43/B-44), T04, T07, T08, T16 browser audit, T17, T19, T22 (native hosts or owner), 011-T02, T03, T17 (native Windows / second user / packaged hosts), 011-T13 request half and T15 (need Feature 001/005/008 settlement hooks and a browser), 011-T18 full sync, 011-T20 Rust decompositions, 011-T23.

Owner deferral note (2026-11-10 instruction): all LiteSPM parts (Feature 013 and every LiteSPM-dependent row) are DEFERRED, and all Windows-only parts are DEFERRED to later testing on the owner's Windows laptop. Release-dependent rows (packaging, signing, release-matrix qualification, B-43/B-44) are also not started. Everything else proceeds in wave order; nothing deferred here is claimed done.

Runtime process tests (fixed, not weakened): `spawned_runtime_lifecycle_restarts_with_current_incarnation`, `spawned_runtime_exits_uncleanly_after_operator_endpoint_failure_and_restarts` and `spawned_daemon_replays_workspace_create_after_lost_ack_and_restart` failed from the first commit. Two causes: (1) the tests expected a clean empty Runtime to be DEGRADED, but startup with no blockers is correctly READY/ONLINE, so the stale expectations were changed to `assert_ready_lifecycle`; (2) a real defect: when the Operator endpoint died after READY the daemon exited nonzero but left the durable and local state READY. `run` now records DEGRADED with blocker `OPERATOR_ENDPOINT_FAILED` through `fail_registered_startup` before exiting. `cargo test -p litecoworkd --bin litecoworkd`: 596 pass, 1 flaky under load (`version_probe_timeout_does_not_wait_for_a_descendant_that_escaped_its_process_group`, passes alone).

Process note: `git stash` was used twice during this run despite the rule above; the first use was popped cleanly, the second accidentally popped the labelled v28 fixture stash, which was restored byte-for-byte (`git stash list` shows it again).

### Feature 002 slice: T07, T09-T14, T16, T18 (updated 2026-11-11; CODE and SERVED, fail-closed)

- Commits: ee26522 (auth observation service, `beginAgentAuthentication`, `openAgentNativeConfiguration`, model picker view-model) and the following Matchmaker, panel, install-notice and recovery-test commit; docs commit after them.
- Route inventory is 178 OpenAPI routes served / 4 outside OpenAPI / 53 target-only. Authenticated Tauri commands `begin_agent_authentication` and `open_agent_native_configuration` now send only workspace/profile/method/target identifiers and accept correlated, allowlisted responses. The daemon still refuses every current V1 method or target; its launcher and opener are unqualified.
- Current checks: `cargo test --locked agent_control_bridge` (4 passed), `cargo test --locked agent_binding_bridge` (2 passed), `pnpm exec node --test tests/agent-control-descriptor-panel.test.mjs` (7 passed), `pnpm exec tsc --noEmit` (passed), `cargo fmt --all -- --check` (passed), feature SDD validator (20 packages passed), and strict contract audit (0 errors).
- BLOCKED as recorded on 2026-11-10: 002-T08 (durable credential receipt needs the next free migration number). Current update: 002-T12 is PARTIAL; Claude Code 2.1.310 help advertises print/stream-json and auth commands, while actual frames, auth mode and cancellation remain open. DEFERRED by owner instruction: Windows rows, 002-T15 release-matrix qualification, release/signing/packaging items, LiteSPM-dependent parts.
- Not qualified: any live OpenCode or Claude Code run; packaged Tauri action path; native launcher/opener; picker bridge. The focused Windows Tauri crate tests compile and pass; the Linux environment lacks gdk-3.0 and no packaged desktop run has been performed. The Windows Tauri Rust bridge tests completed: 4 authentication/descriptor tests and 2 native-config tests passed; `cargo fmt --all -- --check` passed. The focused Settings panel tests passed 7/7 and `pnpm exec tsc --noEmit` passed. No packaged desktop or provider interaction was attempted.

### Feature 014 routes and 016 validator parity (2026-11-10, CODE, SERVED and contract level)

Commits: 7d1c557 (014-T07), dbbb4b5 (route-inventory baseline), 50c1a1e (016-T03 parity).

- **014-T07 (SERVED).** `apps/litecoworkd/src/conversation_operator.rs` serves PATCH /v1/conversations/{conversation_id} (rename), POST /v1/conversations/{conversation_id}/archive and POST .../unarchive through one shared helper, each requiring `If-Match` and `Idempotency-Key`, plus a `status` query on the list route (ACTIVE by default, ARCHIVED). A dedicated error mapper returns the codes the 014 contracts name (STALE_VERSION 412, IDEMPOTENCY_CONFLICT 409, CONVERSATION_ARCHIVED 409, CONVERSATION_DELETED 410, WORKSPACE_ARCHIVED 422) and reports the store's CONVERSATION_NOT_QUIESCENT as CONVERSATION_TURN_IN_PROGRESS. Recorded deviations: a null or empty title is refused because V1 has no un-rename, and a foreign owner gets 404 rather than 403. The three routes are served outside OpenAPI and are counted as such; promoting their contracts into operator-api.openapi.yaml is still open. No desktop UI consumes them.
- **016-T03 (PARTIAL, drift fixed).** The storage validator now accepts the same five V1 block kinds as the desktop validator (TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE) under one 512 KiB document cap, which `docs/schemas/operator-stream.schema.json` now carries as well (`document_size_bytes.maximum` 524288). Parity is not only the kind list: a CARD title, a CALLOUT summary and a TABLE summary must repeat a line of the committed semantic message, and every TABLE header and cell must reproduce a complete semantic Markdown pipe table. Two further differences were closed because either would refuse a document the desktop renders: a TABLE cell is compared with ECMAScript `Number::toString` (Node-verified expectations, so `1e-27` matches and Rust's `0.000000000000000000000000001` does not), and row collection mirrors the desktop's clear-on-wrong-width rule, which makes a zero-row TABLE a vacuous match. `rich_presentation_document_cap_agrees_with_the_desktop_and_the_wire_schema` reads the desktop validator and the wire schema, so the three surfaces cannot drift apart again. Still open: the shared `domain-presentation` crate, CHART and SAFE_CALCULATOR.
- **Not verified here:** no rendered UI run, no live agent run, and `src-tauri` does not build in this container (gdk-3.0 missing), so nothing in this slice is INTEGRATED or QUALIFIED.

## Historical package status snapshot (2026-11-11; superseded)

The detailed [2026-11-10 handoff](docs/HANDOFF-NEXT-AGENT-2026-11.md) is retained as historical environment/provenance context only. Its task table is superseded. This 2026-11-11 section is also historical; use the 2026-11-12 checkpoint above, each live `specs/NNN-*/TASKS.md`, and `docs/IMPLEMENTATION-STATUS.md` for source-backed evidence.

This section contains historical handoff notes and is superseded by the 2026-11-12 checkpoint above, `docs/IMPLEMENTATION-STATUS.md`, and each active TASKS.md. It is retained only as dated provenance. Evidence classes follow AGENTS.md section 5: nothing here is INTEGRATED with a live agent or QUALIFIED on any release row.

### How to work in this checkout

- Environment: `source /projects/sandbox/.work/env.sh` before any cargo, pnpm or Python gate. The 2026-11-10 session ran in a different Linux container rooted at `/data/home/forge/litecowork` with an equivalent prefix at `~/.local/env.sh` (same Rust 1.92.0 versus the 1.98.1 pin, Python and PyYAML on the path); the commands below are identical either way. It selects Rust 1.92.0 (the pin is 1.98.1, not installed), a writable CARGO_HOME, a hand-built dbus sysroot, TMPDIR and PYTHONPATH.
- Rust: `cargo check --workspace`; `cargo test -p storage-sqlite --lib` (624 pass, re-measured 2026-11-10 as 627 pass with one pre-existing root-container failure, see the validation list above); `cargo test -p litecoworkd --bin litecoworkd` (656 pass, 1 ignored). The Linux authoring container lacks gdk-3.0. The Tauri Rust crate now compiles on the connected Windows host through focused tests, but there is still no packaged desktop run or live provider journey.
- UI: `cd apps/litecowork-ui && pnpm test` (5 pre-existing failures: four rendered page tests that need a live Vite dev server and a browser, plus the esbuild lockfile test, which fails because the committed `apps/litecowork-ui/pnpm-workspace.yaml` is CRLF while `tests/node-version.test.mjs` matches a bare `\r\n`); `npx tsc --noEmit`; `pnpm run api:check`. The presentation-family suites are green (24 plus 43 tests) and the rich-presentation changes of 2026-11-10 needed no desktop change.
- Doc gates (run from the repo root): `python3 .specify/scripts/validate_feature_package.py`, `python3 scripts/audit_v1_markdown.py`, `python3 scripts/audit_v1_contracts.py`, `python3 scripts/audit_v1_design_depth.py`, `python3 scripts/validate_design_owner_consistency.py`, `python3 scripts/validate_architecture.py`, `python3 scripts/audit_served_routes.py`. Some regenerate audit inventories and may change line endings; inspect `git status` and each generated diff before deciding whether to retain or restore it. Never use broad restore/reset commands on user data.
- Line endings: Markdown, YAML and most source files are CRLF; keep them. `scripts/*.sh`, `.uv-version`, `.python-version` and `rust-toolchain.toml` must be LF (see housekeeping below). No em dashes in new prose.
- Never stage `.agents/` or `repo_take/`; stage only reviewed, intended files. Do not reset, clean, force-push or rewrite history. `AGENTS.md` contains repository operating rules and next-agent onboarding; preserve both sections when updating it.

### Migration allocation and wiring

`SCHEMA_VERSION` is 32 (`crates/storage-sqlite/src/lib.rs`). Wired: v28 Workspace delegation settings (A-40), v29 Conversation dispatch outbox, response commit identity, usage records, single-flight triggers (001), v30 REPORTED turn activity (001-T25), v31 `conversation_history_state` side table (014-T03), v32 `conversation_control_events` and `conversation_owner_side_notes` (012-T05). Allocated but not written: v33 and v34 (002-T04 descriptor store, 002-T07 auth observations; renumbered from v30 and v31), v35 (003), v36 (005), v37 (007), v38 (009), v39 (013, DEFERRED), v40 (006), v41 (008), v42 (015), v43 (020), v44 (010), v45 (017), v46 (018, only if needed). 004, 011, 016 and 019 plan no migration. The 014 search index and purge tables and the 002-T08 credential receipt need numbers after the allocation table. The rules (first package to bump `SCHEMA_VERSION`, out-of-order merges take the next free number and update the table in the same change, a merged migration is never renumbered) are in docs/LITECOWORK-BUILD-TASKS.md "Migration allocation".

### Per-package status

Statuses below are copied from the TASKS.md status lines; "PARTIAL" always has a stated gap there.

- **001 Conversation session supervisor (ACCEPTED/LOCKED).** DONE: T02, T06, T07, T08, T10, T11, T13, T15, T16, T17, T18, T19, T20, T24, T26. PARTIAL: T01, T03, T04, T05 (B-07 sliding window; the real OpenCode input budget is unconfirmed, owner open item O-05), T14 (text ResourceRef materialization only; binary attachments, WorkspaceRoot path files and the Windows row open), T21 (cross-layer trace with gaps), T22, T23, T25 (REPORTED activity: storage v30, emission, served `GET /v1/conversations/{id}/turns/{turn_id}/activity`, UI cards), T27 (permission shown as a REPORTED activity card, no answer channel). BLOCKED: T12 (OpenCode v2.0.32 emits no evidenced input-request event). NOT STARTED: T09, T28. Also fixed: Runtime now records DEGRADED with blocker `OPERATOR_ENDPOINT_FAILED` when the Operator endpoint dies after READY (commit 00c4542 and follow-ups).
- **002 Native agent install, auth, binding (ACCEPTED/LOCKED).** CODE-COMPLETE: T01 (sync), T03, T04 (descriptor store over the existing migration-16 table, one recorded deviation). PARTIAL: T02, T05 (enable gate with fresh descriptor; real adapter budget unconfirmed), T06, T07 (auth observation service, served `beginAgentAuthentication`, fail-closed because no module declares a launcher), T09 (model picker view-model), T10 (served `openAgentNativeConfiguration`, fail-closed), T11, T12 (Claude Code 2.1.310 help advertises print/stream-json and auth commands; actual stream/auth/safe-stop unverified), T13, T14, T16, T17 (ACP client `agents/acp_client.rs`), T18 (Matchmaker ranking). BLOCKED: T08 (credential receipt migration and durability). DEFERRED: T15. NOT STARTED: T19. Authenticated ID-only Tauri commands and descriptor-driven UI controls now exist for Sign in and Open native config; the model-picker bridge is still absent.
- **003 Workspace roots and context ingestion (ACCEPTED/LOCKED).** PARTIAL: root lifecycle/list/pause/resume/revoke/scan/metadata-observation routes and UI foundations; Linux metadata scan does not ingest/index content, and Windows root add remains unavailable. Grounded lexical retrieval is served but not attached to Send and has no durable ContextAttachment/retrieval receipt or Context Used projection. Root/content ingestion, durable send context, debugger and qualification remain; verify exact task states in `specs/003-workspace-roots-context-ingestion/TASKS.md`.
- **004 Agent relay.** Current working changes preserve nullable LiteCowork binding/session identities through the desktop Conversation view, reject live projection changes to committed producer identity, and label each committed agent response from its stored binding. This is partial CODE only: producer parsing/display and picker-filter regression tests pass, but no A→B→A user journey, storage pass-through proof or provider qualification has run. Remaining relay gaps and task states are in `specs/004-agent-relay/TASKS.md`.
- **005 to 010.** Feature 005-T02 is PARTIAL (CODE): shared Environment enums and enforcement status are consolidated and provider discovery now uses the trust-checked canonical Bubblewrap probe; Windows test suites pass (domain 22 + browser-control 23 + browser-pointer 9, provider-probe 5, staging 24, daemon environment 64). Linux observed-enforcement and admission remain unimplemented. T14 is PARTIAL: the Windows provider probe and planning-readiness projection fail closed, with the typed blocker carried through OpenAPI and the UI; 10 Windows provider tests and 13 focused UI tests pass, but packaged Work evidence is missing. Feature 007 adds the exact Comrak 0.59.0 dependency pin and upstream source record only; no Artifact adapter/route is implemented. Feature 008-T02 is PARTIAL: Activity and Computer are truthful status-only surfaces, Terminal distinguishes platform blockers, and the full UI suite passed 633/633; no live browser/terminal provider or rendered accessibility audit exists. Features 006, 009 and 010 have no new implementation in this working change; pre-existing partial foundations are recorded in their task files. BLOCKED items: 005-T16 to T18 (B-35 Windows Environment provider), 006-T02 and T05 (B-19 follow-up), 007-T12, 008-T09 and T12, 009-T16, 010-T08, T09, T15, T18; 010-T19 DEFERRED out of V1.
- **011 Desktop shell (ACCEPTED/LOCKED).** DONE: T01, T04, T05, T07, T08, T09, T10, T21. PARTIAL: T06, T11, T12 (keep-awake state machine and Linux backend, not wired), T13 (read-only stop preview SERVED; request half blocked on Features 001, 005 and 008 settlement hooks), T14, T15, T16, T18, T19, T20, T22. DEFERRED: T02, T03, T17. NOT STARTED: T23.
- **012 Conversation co-edit and take control (ACCEPTED/LOCKED).** PARTIAL: T01, T05 (storage layer, v32). T02 to T04 and T06 to T11 are unblocked and NOT STARTED.
- **013 LiteSPM package lifecycle bridge (IN_REVIEW).** Every task DEFERRED by the owner; see the DEFERRED note at the top of specs/013-litespm-package-lifecycle-bridge/TASKS.md.
- **014 Conversation history management (IN_REVIEW; B-38 accepted).** PARTIAL: T01, T02, T03 (v31 side table, `ConversationHistoryStore`: rename, archive, status get and list), T07 (SERVED: PATCH /v1/conversations/{conversation_id}, POST .../archive and POST .../unarchive behind If-Match and Idempotency-Key, plus the status-filtered list; the three routes are served outside OpenAPI until the 014 contracts are promoted into operator-api.openapi.yaml, and no desktop UI consumes them yet). T04 to T06 and T08 to T19 NOT STARTED: no search index, purge or UI.
- **015 to 018, 020 (IN_REVIEW).** No implementation beyond pre-existing foundations, except 016-T03, which is PARTIAL with the validator drift fixed: the Rust storage validator accepts the same five V1 block kinds as the desktop under one 512 KiB document cap that the served wire schema carries too, while the shared `domain-presentation` crate and the CHART and SAFE_CALCULATOR kinds remain open. B-39, B-40, B-41, D-01 and B-45 accepted as proposed (A-62); B-42 (018-T01 and T10) open.
- **019 Release engineering (IN_REVIEW).** DONE: T02, T14, T15. PARTIAL: T03, T05, T06, T08, T10, T12, T16, T17, T18, T21. Release dependent and blocked or deferred: T01, T09, T11, T13 (B-43, B-44). CI workflow, CODEOWNERS and the secret scan exist; CI has never run on GitHub.

### Owner decisions (register A-62, 2026-11-10, owner sarv-projects)

- Accepted as proposed: B-38 (delete semantics), D-01 (first Suggestion producers), B-45 (first learning source), B-39 (first heterogeneous delegation pair), B-40 (first Intelligent UI component), B-41 (backup scope), the B-31 destination class (follow-up class), CONNECTOR_EVENT over LOCAL_POLL, E2E-14 to E2E-17, and 010-T19 stays deferred out of V1. Packages 013 to 020 stay IN_REVIEW until the whole SPEC of each is accepted.
- Deferred: all of 013 (LiteSPM); every Windows row (to be tested later on the owner's Windows laptop); release-dependent rows (B-43 signing and update channel, B-44 licensing and packaging, packaging, release-matrix qualification).
- Open: B-42 (018-T01, 018-T10 support-bundle allowlist), the B-19 follow-up (006-T02, 006-T05 GitHub OAuth client-secret problem), P-38 (login-shell PATH capture versus environment scrubbing), and housekeeping (convert `scripts/*.sh`, `.uv-version`, `.python-version`, `rust-toolchain.toml` to LF where still CRLF in the working tree; owner confirmation of the secret-scan fixture exceptions in `scripts/secret-scan-exceptions.json`).

### Where deferred work gets tested later

Windows rows and release-dependent rows are tested on the owner's native Windows x64 laptop first (release matrix row 1: Windows x64 with OpenCode v2.0.32), then native Linux x64. WSL2, containers and CI runners never count. Per-task why, unblock condition and test location are in the `DEFERRED/BLOCKED note` lines of each TASKS.md (41 notes plus the 013 package note).

### Known environment limits

Toolchain 1.92.0 versus pinned 1.98.1 (three Python tests and `cargo clippy -D warnings` on untouched `domain-effects` fail only because of this); `src-tauri` needs gdk-3.0; no installed Chrome for a live browser-based accessibility audit; no GPG agent, no `dpkg-deb`, no actionlint; the GitHub Actions workflow has never run. OpenCode v2.0.32 and Claude Code 2.1.310 are installed. A later read-only OpenCode auth-list command returned OpenCode Console and OpenRouter entries marked `stored`; this does not verify credentials or product connectivity. No product-managed authenticated session or send journey has been captured. Thus no INTEGRATED provider claim exists.

### DeepSeek Harness reference

The desktop-shell and plugin-architecture studies (A-58, A-63) produced proposals P-27 and P-31 to P-46. [docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md) maps each to upstream path (MIT, pinned `fcc0ad46`), owning task and mode (PORT, ADAPT, REJECT, OWNER). No upstream code has been copied. Each affected task carries a `Reference (DeepSeek Harness...)` line.

### Suggested next order

1. Owner housekeeping and open decisions (B-42, B-19 follow-up, P-38, CRLF to LF).
2. 014 routes, search and purge (unblocked by B-38), 012 T02 to T04 routes and service (unblocked), 002-T08 and T12.
3. 005 safe Task path on Linux (Wave 2), then 006 once the B-19 follow-up is decided.
4. On the Windows laptop: all deferred Windows rows, `src-tauri` build and Tauri command compile check, the first live OpenCode run to close 001-T09 and the O-05 budget.

### Preservation and delivery rules

Continue from this handoff revision. Inspect the complete diff before any later implementation; preserve unrelated user changes and untracked archives. Do not reset, clean, stash or rebase. Stage, commit and push only when explicitly requested. Keep TARGET, CODE, SERVED, INTEGRATED and QUALIFIED evidence distinct.

## 2026-11-11 continuation: Conversation relay picker dispatch filtering

- Changed `apps/litecowork-ui/src/conversations/conversation-agent-selection.ts` to use the profile's served `providerKey` and only mark bindings selectable when the current Conversation supervisor implements that provider (`OPENCODE`). Unsupported providers stay unavailable instead of appearing sendable.
- Updated `apps/litecowork-ui/tests/conversation-agent-selection.test.ts` fixtures and added a negative unsupported-provider case. Tests were not run in this turn.
- This is partial CODE only. The picker still does not require a fresh AgentControlDescriptor, current compatible endpoint offer, auth readiness, or packaged native-provider qualification. No claim of integration or qualification is made.

## 2026-11-11 continuation: relay identity regression coverage

- Added focused desktop regression coverage for producer identity round-trip, malformed control-character IDs, legacy null producer fields, null identity on USER messages, older-page pagination retention, and immutable stream rejection when a committed message's binding changes.
- Updated Feature 004 T02 and implementation status to reflect this CODE-level evidence; A→B→A, storage pass-through, provider handle exclusion, packaged journey, and qualification remain open.
- Verification: `node --experimental-transform-types --test tests/conversation-view.test.ts tests/conversation-agent-selection.test.ts` from `apps/litecowork-ui`: **28 passed, 0 failed**.
- Additional verification: `pnpm exec tsc --noEmit` from `apps/litecowork-ui`: **passed**.

## 2026-11-11 continuation: Feature 007 Markdown engine pin

- Selected Comrak 0.59.0 for the accepted CommonMark + GFM-table canonical round-trip direction, pinned exactly in `apps/litecoworkd/Cargo.toml` and `Cargo.lock` with `default-features = false`. Research records BSD-2-Clause, upstream tag commit `ac9d2081fe21b151ad3b3e42fea0e870e6272999`, Rust MSRV 1.89, and the v0.58 HTML table rendering complexity fix included in the selected release.
- This is a dependency/research step only. The daemon has no Artifact format adapter yet; T01 owner contract/schema/migration promotion remains incomplete, and T03 must verify the safe CommonMark/GFM profile, inert HTML, and byte-stable round-trip before support is exposed.
- Verification: `cargo check -p litecoworkd` **passed** (warnings are pre-existing in Environment and WorkspaceRoot scan modules); `cargo tree -p litecoworkd -e features -i comrak` confirms only the base Comrak dependency is enabled.
- Sources: [Comrak v0.59.0 release](https://github.com/kivikakk/comrak/releases/tag/v0.59.0), [tagged Cargo.toml](https://github.com/kivikakk/comrak/blob/v0.59.0/Cargo.toml).

## 2026-11-11 continuation: Workbench truthful Activity and availability states

- Added an Activity tab/launcher row with a distinct empty state that explicitly says no Activity source is served; an Activity selection no longer falls through to the Review empty state.
- Added a Computer-use status message only, with no action control. Terminal blocker text now differentiates the missing qualified Windows Environment provider from the missing admitted Linux Environment.
- Verification: focused `workbench-truthfulness.test.mjs`: **11 passed, 0 failed**; `pnpm exec tsc --noEmit`: **passed**; `pnpm run build`: **passed** (Vite emitted its existing >500 kB chunk advisory). Full `pnpm test`: **627 passed, 5 failed** before the CRLF fix and final two Workbench assertions. Failures were one CRLF-sensitive `node-version.test.mjs` allowlist assertion and four mounted-app harness timeouts. The allowlist test was made CRLF-tolerant and its focused run passed **2/2**; the four mounted-app failures remain to diagnose. Accessibility checks are static/source-level only; no browser audit was run.
- Follow-up on the four mounted-app timeouts: refreshed obsolete sidebar/workspace selectors in the rendered indexed-search, Settings and Workspace-root browser tests. The indexed-search rendered test now passes on its own. The Settings test advances through the updated sidebar but still asserts an obsolete Runtime card/content surface; the Workspace-root test reaches the current Library surface before a Chrome DevTools evaluation timeout. The Conversation/resource editor race test's isolated run skips when its browser setup is unavailable. The full UI suite has not yet been rerun after these test updates; these results do not change feature qualification.
- No provider, live Activity feed, screen capture, input path, takeover control, or platform qualification is claimed. Feature 008 T02 remains PARTIAL pending a clean full build and axe-style focus/name/Escape checks.

## 2026-11-11 continuation: rendered Settings and Library observation recovery

- Updated the Settings rendered-browser check to navigate the current Runtime category and assert the actual diagnostics and refresh-recovery controls, replacing selectors and disclosure assertions from an older Settings implementation.
- Verification: `pnpm exec node --test tests/settings-accessibility.test.mjs` from `apps/litecowork-ui`: **2 passed, 0 failed**. `pnpm exec node --test tests/workspace-root-observations-rendered.test.mjs`: **1 passed, 0 failed**. This resolves the isolated Settings and Workspace-root browser checks after the prior timeout; the full UI suite has not been rerun.
- The Settings and Library checks exercise rendered harness fixtures only. They are CODE-level UI evidence and do not qualify native Windows root observation, Task execution, provider sign-in, or live desktop capture/takeover.

## 2026-11-11 continuation: OpenCode v2.0.32 host recheck

- The native Windows host has `opencode v2.0.32` on PATH, matching the pinned provider version, and `claude --version` reports 2.1.310. These version checks do not establish sign-in or a working product route.
- A read-only `opencode auth list` did not complete successfully: the CLI exited after reporting that location plugins did not finish activating. The standalone JSON auth-list form did not return within the probe window and was interrupted. No credentials were read or printed, and no provider request/session was started.
- Updated Feature 001 T12 and RESEARCH 8.1 to remove the stale statement that no OpenCode binary exists in the environment. Input-request support remains unproven; no behavior is inferred from the version string or CLI help.

## 2026-11-11 continuation: full UI suite and rendered harness stabilization

- Refreshed the Settings and root-observation browser checks to use the current Runtime category and controls; updated the Resource editor's Workspace selector to the current `Current Workspace Root` accessible name. Increased cold Vite harness readiness bounds, pre-fetched the compiled production harness module, and removed the injected Vite client only in the two middleware tests where its unavailable HMR WebSocket interrupted module startup; the root-observation test retains its Vite client because removing it prevented that harness from mounting.
- Verification: full UI suite with Node's serial TAP reporter: **633 passed, 0 failed, 0 skipped**. The command completed on native Windows in 341.2 seconds. Focused rendered tests also passed for Settings (2), Workspace-root observations (1), indexed search (1), and Resource editor/Library race flows (7). A prior full run before the final harness correction passed 632/633; the sole failure was indexed-search startup, and the final run is clean.
- This is CODE-level desktop UI evidence. It does not qualify native agent sessions, Task execution, Windows folder observation, actual browser/terminal providers, screen capture, takeover or their packaged end-to-end journeys.


## 2026-11-11 continuation: Feature 005 Windows fail-closed readiness

- Windows Task provider discovery now advertises no Task-capable LocalWorkspace or supported Task lifetime. The probe explicitly limits Job Objects to process lifetime management and states that no qualified Windows Environment provider is available.
- Windows planning readiness includes typed blocker WINDOWS_ENVIRONMENT_PROVIDER_UNAVAILABLE; the UI renders a direct explanation while leaving Task creation and specification editing available. The OpenAPI schema and generated UI type are aligned.
- Fixed a Windows-only test compilation import gate in apps/litecoworkd/src/local_filesystem.rs uncovered by compiling the provider test target.
- Verification: cargo test -p litecoworkd --locked environment_provider: **10 passed, 0 failed** (Windows target); focused planning-readiness UI tests: **13 passed, 0 failed**; pnpm exec tsc --noEmit: passed; pnpm run api:check: passed. The packaged Work screenshot/projection evidence is still missing, so 005-T14 remains PARTIAL.


## 2026-11-11 continuation: refreshed native CLI observations

- Read-only opencode --version returned opencode v2.0.32. opencode run --help documents message submission, JSON output, session continuation, file attachment, model/agent selection and permission auto-approval; this help output does not show a correlated provider input-request event or establish native permission behavior.
- opencode auth list eventually completed with exit 0 and listed OpenCode Console and OpenRouter as stored. This is local credential-presence metadata only; credential values were not read, and no credential validity or provider request was tested.
- Claude Code reports version 2.1.310. `claude --help` and `claude auth --help` exited 0: help advertises `--print`, `stream-json` input/output, partial-message chunks, and `auth login/logout/status`; it also documents stopping background sessions. No actual prompt/stream, native auth mode, foreground safe-stop, provider request or credentials were exercised/read. Feature 002 T12 is now PARTIAL on advertised CLI metadata only.
- Documentation was corrected: the binaries are present; OpenCode's live input-request behavior and measured input budget remain unproven, while Feature 002 T12 now has advertised CLI transport metadata; actual stream frames, native auth mode and safe-stop evidence remain open before Claude Code can be enabled.

- Owner-requested takeover UX resolution recorded in the browser LLD, 008 SPEC/TASKS/UI contract, screen contract and action ledger: a bounded ephemeral managed-browser viewport and persistent, non-modal Take over banner for a live AGENT-owned lease. Browser takeover is an acknowledged epoch fence, not Conversation/Task Stop; dispatched actions must reconcile. Conversation Take control separately uses Feature 001 quiescence and Feature 012 OWNER_TOOK_CONTROL. Whole-desktop sharing remains UNAVAILABLE under accepted B-32; no control is implemented before its provider route is served.

- Documentation validation after refreshing the Feature 001/002/008 status: .specify/scripts/validate_feature_package.py **PASS (20 packages)**; scripts/audit_v1_contracts.py --strict **PASS (errors=0, OpenAPI 231, served 178, outside 4, target 53)**; git -c core.whitespace=cr-at-eol diff --check **clean** (Git reports the existing generated API file will normalize LF to CRLF on a future Git write).
