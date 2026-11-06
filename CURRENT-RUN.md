# Current engineering handoff

- **Repository:** LiteCowork
- **Branch:** main
- **Inspected revision:** e94497f72bd5928e4abc86f5a5a8c7890cfdd54b
- **Working-tree state at this handoff:** non-clean; 197 status entries (186 modified tracked, 11 untracked). This includes substantial pre-existing code, UI, docs, audit and archive work; all were preserved.
- **Release posture:** pre-release; V1 is not release-qualified

This file is the current engineering handoff. It records the active repository state and the checks performed against it. It is not a chronological diary and must not preserve stale "previous/latest" status paragraphs. Update the current facts and validation results in place whenever the repository changes.

## Repository state

The active checkout contains the Tauri/React desktop application, the Rust daemon and workspace crates, first-party docs and JSON contracts, 11 feature SDD packages, scripts and pinned reference repositories. The repository is not documentation-only.

Primary application paths:

- Desktop UI: apps/litecowork-ui
- Daemon: apps/litecoworkd
- Shared domain/storage/runtime crates: crates
- Product and architecture contracts: docs
- Feature specifications: specs/001 through specs/011
- Specification templates and validators: .specify
- Reference implementation snapshots: repo_take (research only; never product source of truth)

The local ignored archive and historical documentation archive remain preservation material. They do not replace the active source tree.

## Product authority

Read these in order before material work:

1. Root ARCHITECTURE.md for ownership, invariants and system boundaries.
2. docs/PRODUCT.md and docs/V1-PRODUCT-RELEASE-CONTRACT.md for product promise and release scope.
3. The owning domain contracts and accepted ADRs.
4. docs/schemas for canonical OpenAPI, event, JSON Schema, migration and generated route inventory contracts.
5. docs/V1-UI-SCREEN-CONTRACT.md and docs/V1-UI-ACTION-STATE-LEDGER.md for the user-visible experience.
6. The relevant feature package under specs.
7. docs/IMPLEMENTATION-STATUS.md for what this checkout actually implements and the available evidence.
8. docs/ACCEPTANCE-AND-E2E-PLAN.md for required qualification.

Do not resolve a conflict by choosing whichever file has the newest date. Update the document that owns the semantic decision, then update its dependent contracts, UI maps, indexes and traceability.

## Current source-backed product status

The evidence details and implementation boundaries live in docs/IMPLEMENTATION-STATUS.md. The critical distinctions are:

- Runtime lifecycle health is not Task execution readiness.
- Operator API availability is not native-agent integration.
- A persisted Conversation turn is not proof that a provider completed it.
- Recent Chats are cursor-paginated; the sidebar loads additional pages only from the active Conversation page and preserves Workspace/list-epoch fencing. An unreadable pending-create journal pauses New Chat until its explicit retry succeeds.
- A persisted Task is not an admitted Plan or running Attempt.
- A source route registration is not a qualified user journey.
- A Workspace resource preview is not a live filesystem editor.
- A WebView iframe is not Chrome automation or a controlled browser.
- Terminal UI output is not a real process result unless it came from a real PTY/ConPTY process.
- A native action is not a Core-mediated Effect unless it passed through the authorized gateway.
- An Automation trigger host can materialize a READY Task without executing that Task.
- A Coworker configuration is not a continuously running agent process.
- A renderer or schema is not evidence that every artifact format or interactive app is usable.

The current Operator route inventory is docs/schemas/served-route-inventory.json. It records 173 OpenAPI route registrations, one authenticated local-only route outside OpenAPI and 56 OpenAPI operations that remain target-only. This inventory is generated from the composed daemon router. It is evidence of route mounting, not product qualification.

## Validation performed

The completed full Rust workspace run on the connected Windows development machine:

- Command: `$env:CARGO_BUILD_JOBS='1'; cargo test --workspace --locked`
- **Exit code 0; runtime 1,383.28 seconds; no failed test targets.** All unit tests, integration tests and doc-tests completed successfully.
- `litecoworkd` unit target: **359 passed, 0 failed, 1 ignored**; `litecoworkd --test cli`: **4 passed**.
- `storage-core`: **110 passed**; `storage-sqlite`: **555 passed**; `storage-sqlite --test agent_registry_cache`: **2 passed**.
- `windows-local-security`: **36 passed**; `local-environment-staging`: **24 passed**; `operator-ipc`: **18 passed**.
- Domain targets passed, including Conversation (8), Effects (14), Environment (24), Invocations (14), RAG (38), Responsibility (99), Task (138), Workspace (7) and Linux process scope (5). Browser control (23) and browser pointer (9) integration targets also passed.
- The only ignored tests in the Windows run were the daemon's declared ignored test and the Windows Credential Manager integration test that writes uniquely scoped entries to the current user's credential store. Linux Secret Service integration is not run on this Windows target.
- The six Windows WorkspaceRoot route tests pass with platform-canonical fixture identities; production path validation remains fail-closed. Windows folder scanning remains unqualified.
- The deterministic cross-platform backup failure-path test passes as part of the 555-test `storage-sqlite` unit target.

Current validation on the working tree:

- Composed Operator inventory: **173 OpenAPI served / 1 authenticated local-only / 56 target-only**, generated and matching source router composition.
- Architecture/schema validator: passed active checks; one warning because the optional `openapi-spec-validator` package is not installed. JSON Schema, 148 event schemas, errors, OpenAPI structure, SQL, Gateway naming, concept and active Markdown checks passed.
- Cross-owner consistency guard: **36 assertions passed, 0 issues**.
- V1 contract/route/UI audit: **0 findings**.
- Design-depth audit: **0 structural findings**.
- Feature SDD validator: **11/11 packages passed**.
- Markdown audit: **0 broken relative links** across 260 files / 54,819 lines. The generated active-corpus manifest (docs/COMPLETE-ACTIVE-CORPUS-INVENTORY.md) excludes itself and records 259 files / 54,517 lines.
- Frontend production build: passed API type check, TypeScript and Vite. Main JS bundle is 1.05 MB minified (287 KB gzip), above Vite's 500 KB advisory.
- Focused Conversation + responsive tests: **24/24 passed**, including journal retry, cursor pagination, drawer semantics and mobile canvas sizing.
- The full serial frontend test command was attempted but did not emit a final TAP summary. It reached test 393 and stalled in the Settings Chrome DevTools check; observed failures included two production-App bootstrap timeouts and, before fixes, the Conversation pagination and responsive assertions. The latter three are now fixed and pass in isolation. Do not report an aggregate full-suite result.
- The standalone Settings accessibility test passed its static WCAG AA contrast check but failed its rendered-browser phase because Chrome DevTools timed out on `Page.navigate`. This does not establish the production Settings screen's keyboard behavior.
- Rendered UI, packaged Tauri smoke and real-provider desktop E2E remain unqualified.

The full Windows Rust workspace test suite is green, but that is not a LiteCowork V1 release pass. Do not describe frontend coverage as complete or imply packaged desktop/provider USER qualification.

## Release-critical work

1. Rust workspace tests are green on Windows with `CARGO_BUILD_JOBS=1`; retain the ignored credential-store test and unqualified Windows folder-scan provider as explicit platform gates. Next engineering priority is Conversation dispatch durability, strict single-flight admission, retry/cancel state-machine repair and restart recovery.
2. Make Conversation dispatch recoverable across the durable-submit/async-dispatch boundary; verify idempotency, timeout, crash and restart.
3. Qualify the real OpenCode turn path and implement/qualify additional native adapters before making their Send controls available.
4. Implement one safe Task vertical slice: planning, accepted plan revision, Attempt admission, Environment, Trust, one mediated Effect, reconciliation and verification.
5. Remove simulated terminal success and generated sample diffs; implement process-owned PTY/ConPTY and real change computation.
6. Keep iframe preview accurately labelled; implement actual browser/computer services before advertising automation or takeover.
7. Remove or wire Settings controls that currently affect only local UI state or notices.
8. Qualify LiteSPM service/version/protocol/receipt handling before package lifecycle mutations.
9. Qualify one professional Artifact format and one typed Intelligent UI schema path.
10. Verify each screen's UI state against its owner API and failure/recovery behavior.

## Preservation and delivery rules

- Inspect the worktree and ignored/untracked paths before every change.
- Preserve unrelated user work; never clean, reset, stash, rebase or delete archives.
- Do not alter repo_take clones as part of LiteCowork implementation.
- Do not stage secrets, private agent profiles, local provider credentials, build outputs or caches.
- Do not commit or push unless explicitly requested.
- Keep target contracts, implementation findings and qualification evidence distinct.
- Run the narrow test for every change, then contract checks and broader regression checks.
- Inspect the complete diff and verify generated inventories before considering a documentation reconciliation complete.

## Current repository handoff - 2026-11-06

- **Branch / HEAD:** main / e94497f72bd5928e4abc86f5a5a8c7890cfdd54b.
- **Working tree:** 197 status entries: 186 modified tracked entries and 11 untracked entries. This is a non-clean user worktree with substantial pre-existing source, UI, documentation, audit and archive changes. Preserve all of them.
- **Changes in this work session:** documentation and generated audit/inventory updates, plus a SettingsView source-comment correction. Existing application/UI/runtime/test modifications in the worktree were preserved and reviewed; no reset, clean, stash, rebase or history rewrite was performed.
- **Release posture:** pre-release; V1 is not release-qualified. Feature 001 remains IN_REVIEW; Features 002-011 remain DRAFT.

### Source-to-document reconciliation completed

The active source crosswalks and matching UI contracts are maintained in docs/IMPLEMENTATION-STATUS.md, docs/SEMANTIC-SERVED-UI-MAP.md, docs/V1-UI-SCREEN-CONTRACT.md, docs/FEATURE-AND-UI-CONTRACT-COVERAGE.md, and each owning package under specs/001 through specs/011.

- **001 Conversation:** one OpenCode-only dispatch branch; detached dispatch has no durable outbox/startup sweep; strict single-flight is not enforced at admission; Send violates the disabled-until-qualified gate; Stop is ineffective for OpenCode; failed-turn Retry has no UI call site and its backend transition is inconsistent; native transient token frames are not emitted.
- **002 Agents:** 15 curated candidates and ACP metadata are not installed/ready agents. Auth is UNKNOWN and readiness NOT_PROBED. Only Codex/OpenCode profile probes exist; OpenCode is incompatible, Codex has no Conversation dispatch branch. Install/native sign-in are absent; descriptor and credential routes fail closed; binding enable lacks the fresh AC-004 descriptor gate; Usage/model-effort UI is not qualified.
- **003 Resources:** root selection immediately commits on supported Linux/macOS; Windows Add returns 503. Root scan is Linux-only metadata census; Windows/macOS return 501. No content ingestion/watch/index from roots. ZIP originals remain opaque; text index is allowlisted UTF-8 only. Grounded retrieval is read-only and not wired to Conversation or Context Used.
- **004 Agent relay:** backend producer/session identity is projected but desktop message types/parser drop it; picker eligibility is not equivalent to dispatchability; only OpenCode dispatch exists; no qualified A-to-B-to-A route or Resource materialization.
- **005 Tasks:** Task creation and planning readiness are real records/read-only preflight; no accepted-plan -> Attempt -> Environment -> mediated Effect -> verification execution pipeline exists. READY is not running.
- **006 Capabilities:** Discover projections and exact Approval decisions exist, but APPROVE is not an external Effect. Connect/install fails closed, test is UNKNOWN, account Connection/OAuth is absent, CapabilityInvocation is read-only, and provider dispatch/reconciliation is not wired.
- **007 Artifacts:** no production Artifact-create route or create UI; managed text/plain edit/append and safe previews exist; native Save As is local-only and creates no ExportReceipt. No professional format engine or Task output producer.
- **008 Workbench:** Files is a Resource projection, Browser is an iframe preview, Terminal fabricates command success without a process, Diffs are synthetic, Activity is not rendered and Computer-use is unavailable.
- **009 Coworkers:** identity/revisions/status and configuration persist, but there is no Coworker-owned Conversation, Coworker-scoped retrieval or private memory browser/Learning Inbox. Presence returns 503; several saved policy fields have no production consumer; PAUSED admission is partial.
- **010 Automations/Notifications:** local 30-second TriggerHost settles supported recurring schedules and materializes READY Tasks only. Retry/wake policy fields have no consumer. Notification outcome settlement inserts PENDING rows, but no sender advances delivery state; channel preferences are ignored, ON_CONDITION is only DEPENDENCY_BLOCKED, and quiet hours are not editable.
- **011 Shell/Settings:** authenticated OS-local IPC exists; mounted Settings has four categories, not the 16-category target. Appearance controls are ephemeral/inert, shortcut rows are display-only, and several Runtime/Workspace actions are not exposed by the mounted view.

### Global blocker identity

Blocker IDs are globally unique across feature packages. The current allocation is B-21 professional Artifact format; B-22 browser provider; B-23 terminal provider; B-24 Coworker memory; B-25 Quick Create; B-26 Automation trigger; B-27 local notification delivery; B-28 GA OS/security profile; B-29 Settings scope; B-30 Artifact creation; B-31 Artifact export receipt; B-32 Computer-use decision. docs/SPECIFICATION-STANDARD.md and .specify/templates/SPEC.md now require allocation from the global register and synchronized updates to owner SPEC/TASKS/traceability.

The external workflow graph remains provider-owned. WorkflowBackendAdapter and provider graph authoring are target-only; no Core graph runtime or production adapter is present. OpenBot/n8n/Activepieces research is preserved in the linked reference study and does not authorize a Core workflow engine or direct provider graph mutation.

### Current validation

- Feature package validator: **11/11 passed**; structural checks only, not owner approval or runtime qualification.
- Cross-owner consistency: **36 assertions passed, 0 issues**.
- V1 contract audit: **0 errors**; 229 OpenAPI operations, 173 served, 1 authenticated local-only route and 56 target-only.
- Design-depth audit: **0 findings**; 257 files, 54,497 lines, 12 domains, 254 functions, 187 Settings rows, 229 routes, 173 served, 56 target-only, 107 tables and 11 packages.
- Markdown audit: **0 broken relative links** across 258 files and 54,797 lines.
- Active-corpus manifest: **257 files and 54,497 lines** (manifest excludes itself).
- Architecture/schema validator: passed with one warning because optional openapi-spec-validator is not installed; JSON Schema, 148 typed events, errors, OpenAPI structure, SQLite, Gateway naming, canonical concepts and active Markdown checks passed.
- git diff --check: **exit 0**. Git emitted LF-to-CRLF advisory warnings in the existing dirty Windows worktree; line endings were not normalized.
- The full serial frontend suite remains incomplete; the Settings rendered-browser check timed out; packaged Tauri and real-provider USER E2E remain unqualified.

### Next engineering gates

1. Keep Send disabled until the exact native OS/provider/auth USER path and durable dispatch/single-flight/stop/retry recovery are qualified.
2. Implement one safe Task vertical slice with accepted plan, Attempt, Environment, Trust, mediated Effect, reconciliation and independent verification.
3. Replace simulated Terminal/Browser/Diffs claims with real provider-backed sessions and evidence; defer unsupported Computer use.
4. Complete one Artifact create/format/export/reopen path with a durable receipt.
5. Complete one account Connection and provider read/write/revoke journey; Approval alone is not execution.
6. Close Coworker-owned Conversation, scoped Memory and proactive PAUSED-admission gaps before enabling those claims.
7. Keep local automation, notification delivery and Settings controls honest until their actual consumers/handlers exist.

### Preservation and delivery rules

Continue from this exact dirty worktree. Inspect the complete diff before any later implementation; preserve unrelated user changes and untracked archives. Do not reset, clean, stash or rebase. Stage, commit and push only when explicitly requested. Keep TARGET, CODE, SERVED, INTEGRATED and QUALIFIED evidence distinct.
