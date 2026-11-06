
# Documentation authority and navigation

The documentation set defines LiteCowork's product, architecture, owner-domain behavior, machine contracts, UI interactions, implementation boundaries and release evidence. The active executable implementation is in apps/ and crates/. Documentation, source and tests serve different purposes: normative contracts define intended behavior; source identifies what is implemented and wired; tests identify exercised cases; CODE/SYSTEM/USER evidence determines qualification.

## Authority order

1. Root ARCHITECTURE.md: global invariants, ownership, trust boundaries and execution model.
2. PRODUCT.md and V1-PRODUCT-RELEASE-CONTRACT.md: product promise, user model and V1/V2 boundary.
3. Domain owner contracts and accepted ADRs: authoritative semantics for their domains.
4. schemas/: canonical OpenAPI, JSON Schema, event and immutable SQLite migration contracts.
5. V1 UI contracts, function/settings ledgers, coverage maps and generated types: traceability, not independent semantic authority.
6. IMPLEMENTATION-STATUS.md: source-backed status for the active checkout; it does not rewrite target requirements.
7. CODE, SYSTEM and USER qualification evidence: proof for an exact build, platform, provider, agent, account route and behavior.
8. IMPLEMENTATION-MASTER-PLAN.md: delivery order only; it does not change owner semantics.

Do not create a second schema authority. Promote machine contracts in schemas/ and their owning domain document together. Do not silently resolve contradictions by choosing the most recently dated file.

## Start here

| Work | Canonical starting point |
|---|---|
| Product promise and scope | PRODUCT.md, V1-PRODUCT-RELEASE-CONTRACT.md |
| Current implementation and release status | IMPLEMENTATION-STATUS.md |
| Architecture and identity ownership | ../ARCHITECTURE.md, DATA-MODEL.md, STATE-MACHINES.md |
| Native agents and agent controls | AGENT-FABRIC.md, AGENT-CONTROL.md, AGENT-ACCOUNT-ROUTE-MATRIX.md |
| Conversation and context | EXPERIENCE.md, CONTEXT.md, FLOWS.md, NATIVE-COMPOSER-LLD.md |
| Durable Tasks and execution | TASK-RUNTIME.md, ENVIRONMENTS.md, TRUST.md, DELEGATION.md |
| Security and provider trust boundaries | SECURITY.md, TRUST.md, NETWORK-SECURITY.md, CONNECTORS-WORKFLOWS.md |
| UI and Workbench | V1-UI-SCREEN-CONTRACT.md, V1-UI-ACTION-STATE-LEDGER.md, SEMANTIC-SERVED-UI-MAP.md, SETTINGS-AND-SHELL-SURFACE-AUDIT.md, DESKTOP-SHELL-LLD.md, DESIGN-SYSTEM.md |
| LiteSPM | LITESPM-LITECOWORK-INTEROP-LLD.md, LITESPM-INTEGRATION-PLAN.md |
| Connectors and workflow adapters | CONNECTORS-WORKFLOWS.md, CONNECTOR-FACTORY-DESIGN.md, OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md |
| Automations and routines | AUTOMATION.md, ROUTINES.md, RESPONSIBILITIES.md |
| Artifacts and presentation | ARTIFACTS-EVIDENCE.md, PRESENTATION-RUNTIME.md, INTELLIGENT-UI-AND-IMAGE-CONTENT.md, INTERACTIVE-ARTIFACT-RUNTIME.md |
| Runtime, IPC and recovery | RUNTIME-LIFECYCLE.md, LOCAL-OPERATOR-IPC.md, FAILURE-RECOVERY.md |
| Testing and release acceptance | TESTING.md, ACCEPTANCE-AND-E2E-PLAN.md, V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md |

## Indexes and traceability

- PRODUCT-CONTRACT-INDEX.md: product-level owner map and rejected assumptions.
- COVERAGE-MATRIX.md: concern-to-owner mapping; the first owner in a row is normative.
- V1-DESIGN-INDEX.md: V1 target, source and UI crosswalk.
- adr/README.md: decision records and status.
- SCHEMAS.md and schemas/: machine-readable contract index and definitions.
- V1-FEATURE-SDD-REGISTER.md and ../specs/README.md: feature-package lifecycle.
- FEATURE-AND-UI-CONTRACT-COVERAGE.md: screen, API, owner, test and SDD coverage.
- SEMANTIC-SERVED-UI-MAP.md: user-visible controls mapped to their actual owner operations and evidence state.
- SETTINGS-AND-SHELL-SURFACE-AUDIT.md: source-level inventory of mounted Settings controls, unused props, unmounted alternate page and shell-surface boundaries.
- V1-FUNCTION-CONTRACT-LEDGER.md: user-facing function contracts.
- V1-SETTINGS-CONTRACT-LEDGER.md: individual settings behavior and persistence.
- V1-UI-ACTION-STATE-LEDGER.md: UI actions, eligibility, loading, empty, failure and recovery states.
- SPEC-TASK-TRACEABILITY.md: requirements to SDD tasks, implementation work and acceptance tests.
- COMPLETE-ACTIVE-CORPUS-INVENTORY.md: active first-party document manifest.
- CONTRADICTION-AND-ALIGNMENT-REGISTER.md: adjudicated conflicts and unresolved owner decisions.
- REPOSITORY-DOCUMENTATION-AUDIT.md: documentation consistency and validation status.
- OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md: pinned comparison of OpenBot's Tasks, model picker, Usage and workflow graph; includes LiteCowork's non-adoption boundary.
- IMPLEMENTATION-STATUS.md: source-backed implementation status and release blockers.
- IMPLEMENTATION-MASTER-PLAN.md: dependency-aware delivery sequence.
- ACCEPTANCE-AND-E2E-PLAN.md: release evidence gates.

## Feature specification workflow

A material change requires a complete feature package under ../specs/ conforming to SPECIFICATION-STANDARD.md and ../.specify/templates/. A package validator checks structure; it does not approve design or implementation.

Each package records scope, owner decisions, API/event/error references, data model, wiring, tasks, research, security, failure/recovery and UI behavior. Update the owning global contract and machine schemas where needed, then update the SDD, UI/action ledgers, coverage and traceability.

The feature lifecycle is DRAFT → IN_REVIEW → ACCEPTED/LOCKED → IMPLEMENTING → VERIFIED, with BLOCKED and SUPERSEDED as explicit states. Do not mark a package accepted without owner review.

## Reference implementations and derived reading notes

repo_take/ contains pinned external source repositories for engineering research. They are not product dependencies or source of truth. Record exact repository, commit, relevant files, adopted/rejected idea and LiteCowork invariant affected in REFERENCE-IMPLEMENTATIONS.md or the relevant SDD RESEARCH.md.

reading/ contains derived source-reading digests and audit snapshots. It is a navigation aid only, not an authority or current implementation record. Recheck every claim against the active owner document and source. The active corpus manifest and Markdown audit deliberately exclude reading/ so stale digests cannot affect current product/release counts.

## Historical records

docs/archive/ preserves historical research, old implementation plans and evidence snapshots. They are useful for provenance but do not establish the active checkout's implementation or release status. The active source is apps/ and crates/. Use IMPLEMENTATION-STATUS.md for current implementation evidence.

## Evidence vocabulary

Use TARGET, CODE, SERVED, INTEGRATED and QUALIFIED. TARGET is normative design; CODE is compiled or unit-exercised; SERVED is an authenticated mounted operation connected to its owning service/storage path; INTEGRATED is a real provider/harness and persisted product path; QUALIFIED is exact OS/build/provider/auth-route evidence with CODE, SYSTEM and owner USER acceptance, including negative and recovery cases. No route count, catalog, UI tile or unit test alone qualifies a product journey.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]