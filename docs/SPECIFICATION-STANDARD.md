# LiteCowork Software Specification Standard

**Status:** NORMATIVE REPOSITORY PROCESS  
**Owner:** Architecture / repository maintainers  
**Applies to:** Material features, cross-domain changes, new routes/events/schemas, runtime integrations and user-visible workflows.

## 1. Purpose

A discussion, issue, implementation plan, HLD, screen or code diff is not a complete design. A feature is ready to implement only when its contract is reviewable, internally consistent, traceable to system authorities, testable and explicitly accepted. The feature package is the implementation handoff. Code must not silently become a design authority.

## 2. Repository layout

This is a multi-crate Rust workspace. Its eventual source tree is `apps/`, `crates/` and `capabilities/`, not a generic root `src/`. Canonical product and domain documentation remains in `docs/`; feature packages live in `specs/`; workflow guidance, templates and documentation-only validation tools live in `.specify/`.

`ARCHITECTURE.md` is the system constitution. Do not create a duplicate `CONSTITUTION.md` that competes with it.

```text
litecowork/
  AGENTS.md ARCHITECTURE.md README.md CONTRIBUTING.md GLOSSARY.md CURRENT-RUN.md
  docs/                       # product, domain owners, ADRs, machine schemas, release plans
  .specify/
    README.md
    templates/{SPEC.md,PLAN.md,TASKS.md,DATA-MODEL.md,RESEARCH.md,WIRING.yaml}
    scripts/validate_feature_package.py
  specs/
    README.md
    NNN-feature-name/
      README.md SPEC.md PLAN.md TASKS.md DATA-MODEL.md RESEARCH.md WIRING.yaml
      contracts/
        README.md api-events.md errors-recovery.md ui-behavior.md
  apps/ crates/ capabilities/
```

Do not add placeholder source directories merely to satisfy a generic template.

## 3. Required feature package

Every material feature MUST have these artefacts. "Not applicable" requires a reason and owner agreement.

| File | Required coverage |
|---|---|
| `README.md` | Intent, scope, status, owners, dependencies, authoritative links, file map and acceptance entrypoint |
| `SPEC.md` | Problem, goals/non-goals, current evidence, LOCKED/FLEXIBLE decisions, HLD, LLD/interfaces, data, wiring, security, errors, recovery, config, observability, EARS acceptance, command, out-of-bounds paths, assumptions/questions and traceability |
| `PLAN.md` | Ordered phases, dependency DAG, owners, file scope, migration, rollout/rollback, risk, observability and exact verification gates |
| `TASKS.md` | Stable IDs, one reviewable outcome per task, dependencies, allowed files, acceptance, executable commands, evidence and stop criteria |
| `DATA-MODEL.md` | Records, identity, keys, relations, scope, fields, invariants, lifecycle, persistence, events, projections, retention and migration |
| `RESEARCH.md` | Version-pinned sources, observation dates, findings, alternatives, rationale, limitations and unresolved questions |
| `WIRING.yaml` | Components, owners, inputs/outputs, calls, events, transactions, side effects and recovery |
| `contracts/` | Feature request/response/event/error details or explicit links to canonical owner contracts; include `ui-behavior.md` (flows, UI states, validation, command and stream mapping) or cite equivalent [V1-UI-ACTION-STATE-LEDGER.md](../docs/V1-UI-ACTION-STATE-LEDGER.md) rows |

The package does not duplicate global definitions. A feature links to canonical owner files; any new promoted API/event/enum/schema is changed in its actual canonical owner and machine schema in the same reviewed change.

## 4. Required SPEC sections

1. Metadata: stable ID, status, owners, dates, revision, target release and dependencies.
2. Problem and observable user outcome.
3. Measurable goals, non-goals and scope.
4. Current state using TARGET/CODE/SERVED/INTEGRATED/QUALIFIED plus exact evidence and limitations.
5. LOCKED and FLEXIBLE decision register; identify blockers.
6. HLD: component diagram, trust boundaries, actor sequence and ownership.
7. LLD: exact existing signatures; proposed signatures explicitly marked PROPOSED; request/response, data transformations, ordering and transaction owner.
8. Data model and lifecycle.
9. Wiring and dependency graph, including requests, events and streams.
10. Security/trust: principals, scope, authority, secret custody, untrusted input, isolation and redaction.
11. Typed errors, retries, races, ambiguity, timeout-after-effect and partial failure.
12. Crash/restart/recovery, cleanup, reconciliation, cancellation and process shutdown.
13. Configuration, bounds, flags, OS/provider versions, migration and compatibility.
14. Observability, audit, provenance and privacy-safe diagnostics.
15. EARS acceptance for positive, negative, boundary, security, concurrency, recovery, UI/accessibility and performance.
16. One canonical executable acceptance command and additional qualification gates.
17. Explicit out-of-bounds paths and forbidden side effects.
18. Assumptions/open questions with owner and resolution gate.
19. Traceability: requirement -> spec section -> canonical contract -> task -> acceptance fixture.

## 5. LOCKED versus FLEXIBLE

**LOCKED** means every implementation must preserve the invariant. Typical locked decisions: domain ownership, trust boundary, durable identity, authorization, event semantics, idempotency, transaction atomicity, secret/handle exclusion, lifecycle, failure semantics and user-visible guarantees.

**FLEXIBLE** means multiple internal implementations are acceptable if they satisfy all owner contracts. Typical flexible decisions: helper/module naming, private decomposition, non-semantic cache strategy and equivalent presentation details.

A choice is not flexible when it changes a public interface, schema, security authority, data lifetime, state transition or observable failure behavior.

Blocking decision IDs (`B-01`, `B-02`, …) are globally unique across the repository, not local to a feature package. Before assigning one, consult `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md` and allocate the next unused ID there. The owning SPEC, README/current-truth summary, TASKS, `docs/SPEC-TASK-TRACEABILITY.md`, coverage/index ledgers and global register must use the same ID and decision. Never reuse a blocker number for a different question in another feature. When correcting a collision, update every reference in the owning packages and global ledgers in one change; do not renumber unrelated feature-local Task IDs.

## 6. EARS syntax

- Ubiquitous: THE SYSTEM SHALL ...
- Event-driven: WHEN [event], THE SYSTEM SHALL ...
- State-driven: WHILE [state], THE SYSTEM SHALL ...
- Unwanted behavior: IF [condition], THEN THE SYSTEM SHALL ...
- Optional: WHERE [feature is enabled], THE SYSTEM SHALL ...
- Complex: WHEN [trigger] AND [condition], THE SYSTEM SHALL ...

Every stable AC ID has a verification method. Avoid vague words such as "works", "fast" or "secure". Persisted state and asynchronous integrations require explicit denial, concurrency and recovery acceptance.

## 7. Status and locking

- `DRAFT`: authoring; decisions can change.
- `IN_REVIEW`: complete package submitted to product, architecture, domain and security owners.
- `ACCEPTED/LOCKED`: authorized owners approved the contract and blocking questions are zero. Only this status authorizes implementation.
- `IMPLEMENTING`: code follows the accepted revision.
- `VERIFIED`: acceptance/qualification evidence meets the spec.
- `BLOCKED`: named blocker and owner are mandatory.
- `SUPERSEDED`: replaced by a linked revision; preserve history.

An author may prepare a DRAFT without asking the user to repeat known context. The author may not self-approve or mark the spec LOCKED. A change to a locked contract creates a new revision and renewed approval.

## 8. Plan and task discipline

The plan orders work but never overrides the spec. Include dependencies/critical path, implementation owner, file scope, migration/upgrade, rollback, observability, risks and exact exit checks.

Each task records ID, outcome, status, dependencies, component owner, permitted paths, contract sections, change intent, positive and negative acceptance, exact command, expected result, evidence location and stop/rollback condition. Tasks should be independently reviewable. If coding reveals a contract gap, stop and revise/reapprove the spec rather than patching around it.

## 9. Research and contradiction resolution

Record source, exact version/commit, date checked, observed fact, interpretation and limitation. A mutable URL alone is insufficient proof of exact protocol behavior.

For every contradiction: list all conflicting paths; identify authority from `docs/README.md` and `ARCHITECTURE.md`; distinguish intentional TARGET/current differences from inconsistency; correct the owner; update dependent docs/schemas/plans/ledgers; record the resolution and deferred questions; rerun all relevant gates.

Historical documents are evidence, not current authority. Never rewrite their date/status to make a capability appear earlier than it was.

## 10. Validation

Validate required files/sections, EARS IDs, package-wide local links, WIRING node/edge/transaction references, task-dependency cycles, and spec-task-acceptance traceability. Cross-check canonical API/OpenAPI/JSON/event/error/SQL alignment, unique ownership of durable mutations, failure/recovery for each asynchronous boundary and absence of unintended code/archive mutations. Do not require the WIRING interaction graph itself to be acyclic: request/response, stream and recovery interactions may legitimately be cyclic. Then perform a complete manual diff review and update `CURRENT-RUN.md`.

A green validator is necessary but not sufficient; it neither approves a DRAFT nor proves runtime behavior.

## 11. Implementation handoff

After ACCEPTED/LOCKED, select a clean isolated worktree, confirm the accepted spec revision/hash and owner approval, execute tasks in dependency order, synchronize docs when contracts evolve, run unit/contract/integration/system/USER/recovery gates and obtain explicit commit approval. Give the implementation owner the accepted package, relevant canonical docs, repository instructions and a narrow task ID. Do not ask the implementer to invent architecture.

## 12. Existing-plan exception

Older `docs/superpowers/plans/` documents are historical working plans and lack complete sibling feature packages. New features MUST use this standard. Migrate a historical plan only when that feature is resumed; do not generate empty packages for every old plan. The preserved dirty Conversation activation slice is not accepted merely because this process now exists.
