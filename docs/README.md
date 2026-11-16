
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
| Product promise and scope | PRODUCT.md, V1-PRODUCT-RELEASE-CONTRACT.md, V1-PRODUCT-FEATURE-COVERAGE.md |
| Current implementation and release status | IMPLEMENTATION-STATUS.md |
| Architecture and identity ownership | ../ARCHITECTURE.md, DATA-MODEL.md, STATE-MACHINES.md |
| Native agents and agent controls | AGENT-FABRIC.md, AGENT-CONTROL.md, AGENT-ACCOUNT-ROUTE-MATRIX.md |
| Conversation and context | EXPERIENCE.md, CONTEXT.md, FLOWS.md, NATIVE-COMPOSER-LLD.md |
| Durable Tasks and execution | TASK-RUNTIME.md, ENVIRONMENTS.md, TRUST.md, DELEGATION.md |
| Security and provider trust boundaries | SECURITY.md, TRUST.md, NETWORK-SECURITY.md, CONNECTORS-WORKFLOWS.md |
| UI and Workbench | V1-UI-SCREEN-CONTRACT.md, V1-UI-ACTION-STATE-LEDGER.md, SEMANTIC-SERVED-UI-MAP.md, SETTINGS-AND-SHELL-SURFACE-AUDIT.md, UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md, DESKTOP-SHELL-LLD.md, DESIGN-SYSTEM.md |
| LiteSPM | LITESPM-LITECOWORK-INTEROP-LLD.md, LITESPM-INTEGRATION-PLAN.md, LITESPM-INTEGRATION-STUDY-2026-11.md |
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
- [V1-PRODUCT-FEATURE-COVERAGE.md](V1-PRODUCT-FEATURE-COVERAGE.md): the capability-by-capability COVERED/PARTIAL/DEFERRED verdict for all 17 Final Product Feature List capabilities, the UI shell, the deferred V1.1/V1.x/V2 boundaries and the 12 qualification journeys.
- FEATURE-AND-UI-CONTRACT-COVERAGE.md: screen, API, owner, test and SDD coverage.
- SEMANTIC-SERVED-UI-MAP.md: user-visible controls mapped to their actual owner operations and evidence state.
- SETTINGS-AND-SHELL-SURFACE-AUDIT.md: source-level inventory of mounted Settings controls, unused props, unmounted alternate page and shell-surface boundaries.
- [UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md): 2026-11-09 audit of agent-output rendering, Intelligent UI, every screen, every Settings control and architecture, with cowork-style recommendations and proposed tasks.
- [REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md): 2026-11-09 synthesis of nine reference repos, the LiteSPM service model, ACP, and alignment of all 17 Final Product Feature List capabilities to the 20 packages. Its proposed tasks P-11..P-19 (with the UI audit's P-00..P-09) were owner-accepted on 2026-11-09 (register A-55) and folded into their packages; the capability verdict is in [V1-PRODUCT-FEATURE-COVERAGE.md](V1-PRODUCT-FEATURE-COVERAGE.md).
- [LITESPM-INTEGRATION-STUDY-2026-11.md](LITESPM-INTEGRATION-STUDY-2026-11.md): historical, source-cited LiteSPM snapshot (HEAD 84d5fab, v0.6.0). Its daemon IPC and tool counts are upstream research only; the 12-tool MCP snapshot is not the approved LiteCowork pin. Root Architecture requires the published, pinned local MCP stdio bridge. Feature 013 remains owner-deferred; verify exact MCP schemas, approval and recovery behavior before integration.
- [RIVAL-DEEP-DIVE-PLUGIN-AND-CAPABILITY-PATTERNS-2026-11.md](RIVAL-DEEP-DIVE-PLUGIN-AND-CAPABILITY-PATTERNS-2026-11.md): architecture-level study of eight rival plugin/capability systems (DeepSeek Harness capability seams and plugin manager, open-design plugin registry, ZCode MCP-Apps UI, paradev worktree coding), with proposed tasks P-20..P-26.
- [DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md](DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md): line-level read of the DeepSeek Harness desktop shell (register A-63, proposals P-31 to P-46).
- [HANDOFF-NEXT-AGENT-2026-11.md](HANDOFF-NEXT-AGENT-2026-11.md): full next-agent handoff: environment (env.sh), every command and baseline, git and PR state, commit list, per-package per-task status, next tasks, pitfalls, open decisions, deferrals and study pointers.
- [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md): per-pattern map from DeepSeek Harness source (MIT, pinned `fcc0ad46`) to LiteCowork tasks, with ADOPT, ADAPT, REJECT or OWNER mode and where to test.
- [DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md](DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md): line-level read of the DeepSeek Harness plugin subsystem (profile/bundle composition, crash-safe pnpm operations, capability seam, client plugin loading/HMR), with proposed task P-27.
- [OPEN-DESIGN-CAPABILITY-DEEP-DIVE-2026-11.md](OPEN-DESIGN-CAPABILITY-DEEP-DIVE-2026-11.md): line-level read of nexu-io/open-design (Apache-2.0) - capability-grant model, manifest adapters, pure apply pipeline, a second ACP client, live artifacts and GenUI; proposed task P-28.
- [ZCODE-PARADEV-CODING-COCKPIT-DEEP-DIVE-2026-11.md](ZCODE-PARADEV-CODING-COCKPIT-DEEP-DIVE-2026-11.md): line-level read of the paradev persistent terminal daemon and worktree model and the ZCode atomic install and MCP-Apps UI; proposed tasks P-29, P-30.
- V1-FUNCTION-CONTRACT-LEDGER.md: user-facing function contracts.
- V1-SETTINGS-CONTRACT-LEDGER.md: individual settings behavior and persistence.
- V1-UI-ACTION-STATE-LEDGER.md: UI actions, eligibility, loading, empty, failure and recovery states.
- SPEC-TASK-TRACEABILITY.md: requirements to SDD tasks, implementation work and acceptance tests.
- COMPLETE-ACTIVE-CORPUS-INVENTORY.md: active first-party document manifest.
- CONTRADICTION-AND-ALIGNMENT-REGISTER.md: adjudicated conflicts and unresolved owner decisions.
- DOCS-REAUDIT-2026.md: assumption-free doc-claim → code-reality → verdict table across the five doc clusters, with the SDD-completeness gap list.
- SUPERSET-GAP-MATRIX.md: cross-cutting feature × {LiteCowork done? / doc-accuracy / each resolved rival has it?} matrix and the three derived gap lists (real state, doc errors, missing-to-be-a-superset); it links to VIABILITY-VS-COMPETITORS.md (parity authority) and COVERAGE-MATRIX.md rather than duplicating them.
- REPOSITORY-DOCUMENTATION-AUDIT.md: documentation consistency and validation status.
- [T3 Code source deep-dive](audit/T3CODE-DEEP-DIVE-2026-11-16.md): pinned comparative source review with exact full/partial/unread coverage; research only, not LiteCowork authority.
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

## Workspace experience design revision (2026-11-12)

- [Cross-screen scope, navigation, dependencies and sequencing](WORKSPACE-EXPERIENCE-REVISION-2026-11.md).
- [Computer observation, input ownership and takeover](COMPUTER-OBSERVATION-AND-CONTROL-LLD.md) (Feature 008).
- [ACP structured output and intelligent presentation pipeline](ACP-PRESENTATION-PIPELINE-LLD.md) (Feature 016).

These distinguish user-requested scope from proposed provider/schema details and actual CODE/qualification. They retain the existing domain and canonical machine-contract owners.
