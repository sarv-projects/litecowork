# V1 Detailed Design Handoff — 2026-10-12

This handoff describes the documentation work performed in the existing LiteCowork WSL repository without deleting the user's pre-existing uncommitted changes. It is intentionally stricter than a "fully done" claim: target design, schema promotion, implementation and runtime qualification are separate milestones.

## New authored target LLDs

- docs/V1-DESIGN-INDEX.md — precedence, owner map, official upstream URLs, critical gap summary.
- docs/NATIVE-COMPOSER-LLD.md — versioned native /, @, model/effort, editor, attachment, agent switching, send/stream/stop and native parity.
- docs/CHROME-BROWSER-LLD.md — qualified managed Google Chrome, profile import restrictions, explicit existing Chrome connection, user take-over, CDP screencast bridge, frame/epoch identity and OS test requirements.
- docs/DESKTOP-SHELL-LLD.md — Settings control tree and full control table, left/right rails, Workbench, responsive layout, focus, keyboard and animations.
- docs/RENDERING-CONFORMANCE-LLD.md — semantic messages, provider-native stream normalization, safe block fallback, media/office renderer, exact ArtifactVersion and golden corpus.
- docs/V1-WORKFLOW-EVENT-LLD.md — native Conversation->Task, plan/step/attempt, cross-agent delegation, Effects, automations, notifications, recovery and stop criteria.
- docs/V1-FUNCTION-FILE-CATALOG.md — proposed file landing zones and function contracts with boundaries, errors and machine promotion rules.
- implementation/V1-DESIGN-COVERAGE.md — subsystem/epic crosswalk and individually named UIV1-001–UIV1-158 acceptance scenarios.
- scripts/audit_v1_markdown.py + generated implementation/V1-MARKDOWN-INVENTORY.csv and V1-AUDIT-INVENTORY.md — mechanical all-Markdown read, line hash and relative-link check; **not** semantic certification.

## Existing documents reconciled in place

AGENTS.md gained a throughput-first parallel-agent implementation/testing protocol and cleanup discipline. Architecture, product, coverage map, native-agent, environment, trust, security, UI, motion, presentation, workflow, API/events/schema/data/storage/state, testing and epics received scoped V1 integration notes linking new specifications to existing canonical owners. A total of 55 existing documents were changed by the idempotent cross-reference pass, in addition to AGENTS.md. Old history and machine registries were not destructively rewritten merely to make unimplemented routes appear operational.

## Contract rules that prevent implementation thrash

1. Never duplicate existing Core domain models when implementing target frontend projections. Promote typed schema/OpenAPI/service/storage/event changes together with actual handlers.
2. Put native slash and @ semantics in each faithful harness adapter; do not execute arbitrary slash text in the host.
3. Browser automation uses a real Chrome provider and separate view bridge, not Tauri's ordinary WebView branded as Chrome. Existing profile reuse requires explicit permission; protected default profile cookies are not copied.
4. Browser control epochs permit exactly one human or agent input actor. Reconnect invalidates stale DOM and frame geometry. External effect ambiguity blocks blind resend.
5. The central display preserves complete committed semantic answers, safely falls back on unknown native output and never marks Task verified because something rendered.
6. A right Workbench is user-controlled, single-shell, permission revalidated, dirty-draft protected; no background tool result may steal chat focus.
7. Settings rows are not decorative toggles. Every operative control must have a typed owner command, saved receipt, optimistic version, error/repair and keyboard/ARIA state.
8. Native agent/provider/OS feature claims need direct-harness parity and actual system-level evidence.

## Verification performed and current blockers

- First mechanical script run: 138 first-party Markdown files, 52,516 lines, 0 broken local Markdown links.
- Second mechanical scan after 55 cross-links: 138 files, 52,897 lines, 0 broken links. **This scan predates the later detailed Settings and UAT addenda**, so run it again.
- The architecture coverage generator successfully regenerated coverage for 94 architecture documents and 6,095 rows. This occurred before the later addenda; rerun the validator to ensure the generated coverage digest is current.
- Initial system-Python architecture validator failed due to missing openapi_spec_validator; the project .venv Python successfully imported this library. The .venv architecture validator later ran without producing a completion result and was explicitly terminated after WSL service became unresponsive. No PASS can be asserted.
- A subsequent WSL smoke command returned Wsl/Service/WSAETIMEDOUT. Do not run wsl --shutdown or restart services without considering unrelated user sessions. The repository files were written through authorized Windows UNC while Desktop Commander remained connected.
- No native browser/UI runtime demo, actual Chrome control, real harness conformance, tests, release package, backend execution or visual snapshot is claimed in this documentation pass.

## Mandatory follow-up verification in existing virtual environment

~~~sh
cd /home/sarvesh/business_Dev/litecowork
.venv/bin/python scripts/audit_v1_markdown.py
.venv/bin/python scripts/validate_implementation_coverage.py --write
.venv/bin/python scripts/validate_implementation_plan.py
.venv/bin/python scripts/validate_architecture.py
git diff --check
git status --short
~~~

If the WSL service is unresponsive, diagnose that availability separately without killing the user's WSL sessions. If generated rows change, inspect before regenerating, especially with concurrent external coding sessions. More deeply review any existing narrative contradictions than a mechanical link check can detect. No mass search/replace of existing domain definitions without exact authority approval.

## Architectural incompleteness that only implementation resolves

Typed proposed operation names are not yet fully realized route handlers, schema migrations or V1-tested platform adapters. The browser's actual live side panel uses an explicit proposed CDP screencast bridge; its performance, DPI, IME, Chrome native dialogs and platform support remain evidence-gated. The exact source-level replacement/addition of all proposed files and functions is therefore still an implementation task. The existing E03, E07, E08, E09 and E13 stories own that work, preserving the official backlog and accepting a feature only after CODE/SYSTEM/USER proof.

## Launch-critical implementation order

1. E03: qualified provider-backed Conversation Send/Stop/stream + native adapter and / @ model/effort parity.
2. E08: unified central chat renderer and right Workbench shell with exact artifact/stream provenance, settings and visual tests.
3. E07: real Chrome process/provider/session, managed profile and user takeover, qualified visible side browser (or explicit external-window fallback with disclosed gap).
4. E04/E05/E09/E10: trust/connections, delegation, responsible automation/notifications and Skills across agents.
5. E13: OS matrix, real user journeys UIV1-001–UIV1-158, performance, security, accessibility, packaging and release.

Do not defer critical trust tests; do not rerun the entire stack after every cosmetic tweak. Use AGENTS.md parallel role partitioning with one integrator over shared contracts and clean up only agent-owned temporary worktrees/build artifacts.

## Continued design verification handoff

Additional normative ledgers: docs/V1-SETTINGS-CONTRACT-LEDGER.md (187 controls), docs/V1-UI-ACTION-STATE-LEDGER.md (35 UI interaction groups), docs/V1-FUNCTION-CONTRACT-LEDGER.md (226 function pre/post/failure gates), docs/V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md (25 end-to-end chains and 24 race cases). All are TARGET documentation, not endpoint/system acceptance. scripts/audit_v1_design_depth.py emits implementation/V1-DESIGN-DEPTH-AUDIT.md with the per-file source ingest and ownership crosswalk. When writing code, implement the exact canonical service/API/events/storage contract and link real fixture evidence to each ledger group.
