# LiteCowork V1 — Cross-Document Semantic Markdown Findings

Date: 2026-10-12. Scope: current first-party Markdown repository. Mechanical per-file report: V1-EVERY-MD-AUDIT-2026-10-12.md and CSV. Full-file/line ingestion does not mean every sentence received human semantic proof.

## Confirmed issues and correction status

| Priority | Finding | Action taken | Status |
|---|---|---|---|
| HIGH | New supplemental UI acceptance IDs U01–U158 collided with the existing, different U01–U20 workflow cases in implementation/TESTING.md and implementation/WORKFLOWS.md. Original U01 is folder organization; supplemental U01 was startup. | Rename supplemental catalog and its cross-references to UIV1-001–UIV1-158. Preserve original workflow IDs; add namespace rule. | FIXED |
| HIGH | docs/V1-WORKFLOW-EVENT-LLD.md used CREATED/TURN_RUNNING/COMMITTED and draft/review like machine states, conflicting with canonical docs/STATE-MACHINES.md. | Use canonical ConversationTurn, Task, Automation and Effect graphs with readable UI descriptions explicitly separated. | FIXED |
| HIGH | docs/V1-FUNCTION-FILE-CATALOG.md proposed an extra agent-binding composer-descriptor API despite existing agent-profile control-descriptor API, and a generic turn control endpoint despite separate retry/cancel routes. | Replace with canonical existing routes; keep proposed browser endpoints visibly unserved. | FIXED |
| HIGH | Target pseudocode confused stable domain RequestId with local IPC request_id. docs/API.md requires Idempotency-Key and separate If-Match/X-Workspace-ID context. | Correct scoped transport DTO, workflow sequencing, and native composer/browser clarifications. | FIXED |
| MEDIUM | Per-window layout and presentation-segment Operator endpoints duplicated or complicated existing local layout/presentation mechanisms. | Use non-authoritative local Tauri layout preference and existing rich-presentation routes; future segment optimization evidence-gated. | FIXED |
| MEDIUM | implementation/WORKFLOWS.md implied cloud laptop-close and remote execution within current release. Roadmap puts E11/E12 post-V1. | Explicitly label these workflows post-V1 and local V1 host-awake requirement. | FIXED |
| MEDIUM | Generated architecture coverage and markdown digest inventories became stale after edits. | Regenerate and validate against final source, without disturbing active WSL sessions. | PENDING; WSL command timeout |
| MEDIUM | Chrome CDP screencast side-panel path is detailed but not proven for actual Chrome controls, native dialogs, high DPI, IME and user takeover on target OS. | Keep explicit qualified external Chrome fallback, conformance and truthful view-mode requirements. | SYSTEM EVIDENCE PENDING |
| MEDIUM | Browser events/routes, Settings mutations, native renderer variants remain proposed, not all promoted to canonical OpenAPI, events and SQLite/handlers. | Implement atomic contract+handler+migration+bridge+tests under existing owners. | IMPLEMENTATION PENDING |
| MEDIUM | Supplemental 158 UI/integration scenarios do not yet have distinct automated fixtures and CODE/SYSTEM/USER execution evidence for all cases. | Assign and implement per-epic qualification; the catalog is not a release test result. | QUALIFICATION PENDING |

## Full Markdown file ingestion

The last completed scan before these additional corrections ingested 140 first-party Markdown files, 53,429 lines and 3,511,123 bytes; no missing relative Markdown file targets, unterminated fences or missing H1 headings were detected. The current file digests have changed since that scan and need a final refresh.

Includes root docs, docs, ADRs, implementation, epics, reviews, UI, skill and asset Markdown. Excludes .git, dependency installations, target/build outputs, .venv, ignored caches and temporary worktrees. Reading each byte/line and checking syntax/link candidates is not equivalent to manually verifying all semantic claims, anchors, external vendor links, actual source behavior or OS/browser fidelity.

## Remaining architecture and system gates

1. Reconcile newly proposed wire DTO/event names with actual docs/API.md, docs/SCHEMAS.md, docs/STATE-MACHINES.md, schemas/operator-api.openapi.yaml and storage migrations when implementing. Never turn a proposal into a falsely advertised served API.
2. Re-run all Markdown inventory, architecture coverage/plan validators and diff checks in the repository Python environment once WSL command execution is healthy. WSL previously returned WSAETIMEDOUT; do not restart active user sessions without authority.
3. Create per-story executable/source-qualified tests for UIV1-001–UIV1-158 and map them to canonical U01–U20 workflow tests where they overlap, not by replacing identifiers.
4. Qualify real native slash/@/model send/stop/resume, central output rendering, actual Chrome viewport and takeover, Settings, workflows and long-horizon recovery before release claims.
5. Review high-risk external vendor assumptions against pinned upstream source/build and actual OS acceptance. No mock-only proof.

## Evidence labels

MECHANICALLY READ means fully ingested and indexed. NORMATIVELY SPECIFIED means detailed proposed behavior exists. SOURCE IMPLEMENTED means a real handler/UI path exists. SYSTEM QUALIFIED means a tested provider/OS path works. OWNER VERIFIED means a real acceptance case passed. These labels are not interchangeable.

No repository source-code changes, destructive cleanup, WSL restart, or Git reset was performed by this documentation audit.


## Continued audit: machine validators and actual source reconciliation (2026-10-12)

- Running the repository Python environment successfully completed scripts/validate_architecture.py: **PASSED**. The validator reports JSON Schema, 142 typed events, canonical error codes, OpenAPI, SQLite, Gateway names, product names and relative Markdown links validated. This is machine consistency, not live UX proof.
- A new auditable guard, scripts/audit_v1_contracts.py, compares docs/V1-FUNCTION-FILE-CATALOG.md to the actual OpenAPI operation map, verifies existing named operator endpoints, guards the canonical ConversationTurn lifecycle owner, checks actual backlog story IDs and enforces 158 **distinct** UIV1-001–UIV1-158 definitions.
- First checker run discovered missing canonical route matches due to harmless differing parameter names (prose {id} versus OpenAPI {conversationId}), plus duplicate definition matches on several range-summary lines. **FIXED** by path-shape normalization and by labeling range rows as summaries, preserving all 158 individual case definitions.
- New guard strict invocation **PASSED**: 0 errors, 206 actual OpenAPI route/method pairs, 158 individual supplemental acceptance definitions, 67 actual backlog stories. No new browser route is misrepresented as already served.
- Checked official upstream pages on this date: https://docs.opencode.ai/docs/commands/ (custom commands override built-ins), https://docs.opencode.ai/docs/server/, https://playwright.dev/docs/api/class-browsertype (automating default Chrome profile unsupported), https://developer.chrome.com/blog/remote-debugging-port (Chrome 136+ non-default data directory requirement), https://chromedevtools.github.io/devtools-protocol/tot/Page/ (current CDP Page documentation redirect). These are reference checks, not qualification of our own adapters.
- Existing UI source remains the concrete blocker: apps/litecowork-ui/src/conversations/ConversationsPage.tsx includes a disabled/read-only composer and text saying native Send is unavailable. Neither schema validator certifies E03 native turns, E07 real Chrome, E08 rich Workbench, or E09 automation.
- The architecture coverage generator refreshed 94 architecture documents and 6,132 traceability rows. Run final coverage/plan checks after the last corpus edits and record exact outcomes; earlier stale coverage error is not evidence of broken API/schema.
- Git diff --check on the then-current tracked working tree returned no output/error. Pre-existing changes from other sessions remain uncommitted and must be preserved.

### Explicit limitations still open

(1) A full human semantic adjudication of every line in the corpus remains unproven; machine ingestion and owner-targeted cross-checks are not that. (2) No executable per-user owner acceptance pass has been recorded for all UIV1 cases. (3) Native provider/Chrome/browser GUI end-to-end and per-OS security/fidelity verification remain implementation gates. (4) No migration, authenticated Browser Operator endpoint or adapter was created merely from an LLD candidate. (5) The architecture check must be rerun if machine schemas change after this record.

## Follow-up design-depth pass — HLD/LLD/UI/flows/functions/contracts

Findings were resolved in the target documentation without modifying application source or canonical machine schemas:

1. **Authority boundary (confirmed and corrected).** DESKTOP-SHELL-LLD previously put a client-chosen principalId into SettingsMutation. It now derives actor identity only at authenticated Operator/IPC, distinguishes durable Idempotency-Key from per-exchange request_id and uses If-Match and X-Workspace-ID correctly.
2. **Settings owner gap (documented).** All 187 named Settings operations from 30 groups are now individually mapped in docs/V1-SETTINGS-CONTRACT-LEDGER.md to a proposed owning service, identity/scope, operation class and failure path. A subsequent semantic review corrected action classes for selectWorkspace, selectFolder, selectChromeChannel, openNativeConfig, pairCompatibleAgent, testNativeAuth, testServer, testSkill, verifyManifest, runProviderDiagnostic and openRequest. This mapping is an implementation target, NOT proof each control has an existing endpoint.
3. **UI behavior gap (documented).** docs/V1-UI-ACTION-STATE-LEDGER.md covers 35 titlebar/left rail/chat/composer/right Workbench/browser/computer/overlay interaction groups with reducer actions, owning authority, loading/permission/stale states, keyboard/motion/focus and acceptance families.
4. **Function error and concurrency gap (documented).** The first pass enumerated 175 V1-FUNCTION-FILE-CATALOG F01–F10 function signatures. The catalog has since grown to 226 signatures across F01–F14, and docs/V1-FUNCTION-CONTRACT-LEDGER.md maps those current signatures to input/output, source owner, side-effect class, minimum pre/postcondition, failure and race fixture requirements. Generated classification is a review aid; verify actual provider side effects before coding.
5. **Cross-owner workflow/race gap (documented).** docs/V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md enumerates 25 explicit user/system flows, source authority, transaction/event/recovery/UI outcome, and 24 concurrency/partial-failure categories. It explicitly distinguishes canonical existing routes from unpromoted browser candidates.
6. **Coverage proof and indexing (mechanical).** scripts/audit_v1_design_depth.py generated implementation/V1-DESIGN-DEPTH-AUDIT.md: 20 subsystem ownership chains, no duplicate incompatible function signatures, 206 canonical OpenAPI methods, 107 baseline SQLite tables, 187 controls with exact ledger matches, 0 structural failures on the audited source snapshot. This is static evidence only, not a line-by-line human semantic certification.
7. **Audit tooling (confirmed and corrected).** scripts/audit_v1_markdown.py formerly recursively traversed excluded node_modules/build trees before filtering; now prunes excluded directories upfront. The updated mechanical inventory read 148 first-party Markdown files and 54,489 lines with 0 broken local relative link targets.

### Still not proven or served

- None of the 187 Settings or 226 function ledger entries is a direct claim of served endpoint. The exact operationId/typed handler/event/table for each new mutation must be assigned when promoted; the 9+ browser routes remain explicit candidates.
- End-to-end executions, OS browser screencast/controller fidelity, privileged process and plugin integration, provider native slash/@ parity, automation host wake/sleep and all 158 CODE/SYSTEM/USER acceptance cases are not qualified by documentation.
- The source-ingestion/ownership program does not reason through every possible 54k-line prose contradiction. It provides a much narrower and reproducible set of concrete checks. Release quality still depends on focused contract-owning review and real tests.

