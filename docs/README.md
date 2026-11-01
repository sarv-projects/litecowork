# LiteCowork documentation map

**Cross-domain source:** the user-supplied *LiteCowork V1 Final Master Specification — Revision 9* (read in full for the 30 October 2026 reset). It composes the system; it does not replace the repository's root architecture, owner-domain contracts or machine schemas. The source file is not stored in this checkout.

## Authority order

1. Root [ARCHITECTURE.md](../ARCHITECTURE.md): system invariants and ownership.
2. [PRODUCT.md](PRODUCT.md) and [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md): product promise and V1 boundary.
3. Domain owner contracts and accepted ADRs: authoritative within their domain.
4. [schemas/](schemas/): canonical JSON Schema, OpenAPI, event and immutable SQLite migration contracts.
5. Function/settings/coverage/UI ledgers and generated types; these provide traceability but do not override owner contracts.
6. Revision 9 master specification: cross-domain composition, integrated product sequence and explicit resolutions; it cannot override domain owners or machine schemas.
7. [IMPLEMENTATION-MASTER-PLAN.md](IMPLEMENTATION-MASTER-PLAN.md) orders delivery only; it does not redefine domain semantics.
8. Recovered executable implementation establishes what is present/served but does not override normative contracts.
9. CODE / SYSTEM / USER qualification evidence establishes what is actually proven on an exact build/provider/OS.

**Do not create a second schema authority.** New machine contracts belong in `docs/schemas/`. Do not silently duplicate domain invariants in a new umbrella document.

## Start here

| Need | Current authority |
|---|---|
| Product promise and boundaries | [PRODUCT.md](PRODUCT.md), [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md) |
| Architecture and identity ownership | [../ARCHITECTURE.md](../ARCHITECTURE.md), [DATA-MODEL.md](DATA-MODEL.md), [STATE-MACHINES.md](STATE-MACHINES.md) |
| Native agents and switching | [AGENT-FABRIC.md](AGENT-FABRIC.md), [AGENT-CONTROL.md](AGENT-CONTROL.md), [PROTOCOLS.md](PROTOCOLS.md) |
| Conversation and context | [EXPERIENCE.md](EXPERIENCE.md), [CONTEXT.md](CONTEXT.md), [FLOWS.md](FLOWS.md) |
| Durable Tasks and isolation | [TASK-RUNTIME.md](TASK-RUNTIME.md), [ENVIRONMENTS.md](ENVIRONMENTS.md), [TRUST.md](TRUST.md) |
| UI and Workbench | [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md), [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md), [DESIGN-SYSTEM.md](DESIGN-SYSTEM.md) |
| LiteSPM integration | [LITESPM-LITECOWORK-INTEROP-LLD.md](LITESPM-LITECOWORK-INTEROP-LLD.md) and [LITESPM-INTEGRATION-PLAN.md](LITESPM-INTEGRATION-PLAN.md) |
| Connectors and workflow adapters | [CONNECTORS-WORKFLOWS.md](CONNECTORS-WORKFLOWS.md), [CONNECTOR-FACTORY-DESIGN.md](CONNECTOR-FACTORY-DESIGN.md) |
| Automation, routines and channels | [AUTOMATION.md](AUTOMATION.md), [ROUTINES.md](ROUTINES.md), [CHANNELS.md](CHANNELS.md) |
| Artifacts and intelligent rendering | [ARTIFACTS-EVIDENCE.md](ARTIFACTS-EVIDENCE.md), [PRESENTATION-RUNTIME.md](PRESENTATION-RUNTIME.md), [INTERACTIVE-ARTIFACT-RUNTIME.md](INTERACTIVE-ARTIFACT-RUNTIME.md) |
| Testing and recovery | [TESTING.md](TESTING.md), [FAILURE-RECOVERY.md](FAILURE-RECOVERY.md), [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md) |

## Complete owner and design indexes

The table above is a quick-start map, not an exhaustive list of all current documents. Use these indexes to navigate the full authority set without treating every file as an equal authority:

- [Product Contract Index](PRODUCT-CONTRACT-INDEX.md) — concise product-level owner map and rejected assumptions.
- [Architecture Coverage Matrix](COVERAGE-MATRIX.md) — full concern-to-owner mapping; the first linked document in each row is normative for that concern.
- [V1 Detailed Design Index](V1-DESIGN-INDEX.md) — target-level UI, workflow, function and integration design crosswalk; subordinate to domain owners and machine schemas.
- [ADR index](adr/README.md) — decisions and status metadata.
- [Machine contract index](SCHEMAS.md) and [schemas/](schemas/) — promoted JSON Schema, OpenAPI, event definitions and immutable SQL migrations.
## New reset documents

- [Product strategy](PRODUCT-STRATEGY.md) — what LiteCowork is, whom it serves, product moat and V1/V2 boundaries.
- [Implementation master plan](IMPLEMENTATION-MASTER-PLAN.md) — dependency-aware vertical slices, not a replacement for owner contracts.
- [Acceptance and E2E plan](ACCEPTANCE-AND-E2E-PLAN.md) — CODE/SYSTEM/USER evidence, real UI and recovery gates.
- [Coding cockpit UI specification](CODING-COCKPIT-UI-SPEC.md) — one-shell coding and cowork experience.
- [LiteSPM integration plan](LITESPM-INTEGRATION-PLAN.md) — first-class shared local service, ACSR and marketplace.
- [Connector factory design](CONNECTOR-FACTORY-DESIGN.md) — Activepieces/Nango/OpenAPI import strategy and brokered operation contract.
- [Reset and archive record](RESET-AND-ARCHIVE-RECORD.md) — exact baseline and preserved history.
- [Repository documentation audit](REPOSITORY-DOCUMENTATION-AUDIT.md) — inventory, stale-reference remediation and remaining limitations.

## Feature engineering and UI–core contracts

- [Feature and UI–core contract coverage](FEATURE-AND-UI-CONTRACT-COVERAGE.md) — engineering checklist map and gaps.
- [UI and Core typed contracts](UI-CORE-TYPED-CONTRACTS.md) — commands, stream frames, errors.
- [V1 Feature SDD register](V1-FEATURE-SDD-REGISTER.md) — planned packages 001–011.
- [LiteCowork build TASKS](LITECOWORK-BUILD-TASKS.md) — master path from recovery to qualified V1.
- [Competitor promoted use cases](COMPETITOR-PROMOTED-USE-CASES.md) — UC-* USER test catalog.
- [Viability vs competitors](VIABILITY-VS-COMPETITORS.md) — local desktop parity and RV-* research gates.
- [Settings and shell surface audit](SETTINGS-AND-SHELL-SURFACE-AUDIT.md) — 187 controls + shell crosswalk.
- [Semantic served-handler ↔ UI map](SEMANTIC-SERVED-UI-MAP.md) — TARGET baseline for 187 settings and shell controls; SERVED operationIds still open.
- [Contradiction and alignment register](CONTRADICTION-AND-ALIGNMENT-REGISTER.md) — known resolutions and remaining owner conflicts.
- [Reference repos core architecture index](REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md) — what to read in ZCode, DSH, AionUi, OpenDot.
- [Complete active corpus inventory](COMPLETE-ACTIVE-CORPUS-INVENTORY.md) — **every** active root/docs/specs/.specify file with line counts; nothing omitted from the index.
- [Spec / build TASK / USER test traceability](SPEC-TASK-TRACEABILITY.md) — BT-* ↔ 001-T* ↔ UC-* ↔ AC-*.
- [Desktop reference implementation audit](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md) — ZCode + DeepSeek Harness.

## Feature specification workflow

- [Specification Standard](SPECIFICATION-STANDARD.md) is the normative SDD process for material features and cross-domain changes.
- [Current run / handoff](../CURRENT-RUN.md) records active reconciliation status, preservation boundaries, validation results and the next safe action.
- [Specification tooling](../.specify/README.md) contains the templates and documentation-only package validator.
- [Feature package index](../specs/README.md) lists current feature specifications, their lifecycle state and review boundary.

A passing package validator is not owner approval, implementation authorization or runtime qualification. Only an explicitly accepted/locked package may enter implementation.

## Historical documentation

Older implementation plans, review packets, audits and evidence snapshots are preserved under [archive/legacy-implementation-c7e9c05/](archive/legacy-implementation-c7e9c05/). These are historical records, not current release status. The old executable source snapshot is now in the local ignored archives/legacy-codebase-c7e9c05/ directory, not in the active checkout. Until a specific source slice is recovered into a working tree, this repository has no current executable implementation from which to claim CODE, SERVED, INTEGRATED or QUALIFIED status.

## Evidence vocabulary

Use the repository's evidence vocabulary consistently. **TARGET** means specified; **CODE** means compiled or unit-exercised; **SERVED** means an authenticated route and storage path are mounted; **INTEGRATED** means a real provider and persisted path work together; **QUALIFIED** means the exact OS/build/provider/agent route has CODE, SYSTEM and owner USER evidence, including negative and recovery behavior. A unit test, mock, catalog entry, UI tile or route does not by itself qualify a product journey.

## Desktop implementation research

- [Desktop Reference Implementation Audit: ZCode and DeepSeek Harness](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md) compares the actual Electron desktop applications, Host/renderer/IPC/plugin boundaries, lifecycle and test suites. It is non-normative research; LiteCowork's Tauri/daemon and domain-owner contracts remain authoritative.
