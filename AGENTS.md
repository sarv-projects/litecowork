# LiteCowork Repository Instructions

**Effective:** 31 October 2026 | **Posture:** documentation-first reset; source/build tree is preserved in a local ignored archive.

## 1. Authority and precedence

Resolve conflicts using this order:
1. Root `ARCHITECTURE.md` - global invariants, boundaries and ownership.
2. `docs/PRODUCT.md` and `docs/V1-PRODUCT-RELEASE-CONTRACT.md` - product promise and V1 boundary.
3. Domain owner contracts and accepted ADRs - semantics within their domain.
4. `docs/schemas/` - promoted JSON Schema, OpenAPI, event and immutable SQLite migration contracts.
5. Function/settings/coverage/UI ledgers - traceability; they do not override owner docs.
6. User-supplied Revision 9 master specification, referenced in `docs/RESET-AND-ARCHIVE-RECORD.md`, is a cross-domain composition source; its original file is not in this checkout and it cannot override domain owners/schemas.
7. `docs/IMPLEMENTATION-MASTER-PLAN.md` orders delivery; it does not redefine domain semantics.
8. Recovered executable source shows what exists/serves; dated CODE, SYSTEM and USER evidence shows what is proven for an exact build/platform/agent/provider.

Never silently choose the newest file. Record conflicts, identify the owning authority, resolve it there, then update dependent documents and links. Do not create a parallel schema or state-machine authority.

## 2. Product invariants

- LiteCowork is local-first, chat-first and BYOA. Ordinary Conversation does not require a Task or Coworker.
- Conversation and Task are separate; Task creation is explicit.
- Native agent harnesses own reasoning, authentication, model catalogs, private transcripts, tools and harness-native behavior.
- LiteCowork Core owns Workspace, canonical Conversation, Task, Trust, grants, mediated Effects, Artifacts, Evidence and recovery.
- LiteSPM owns package catalog, inventory, host registration, package mutation and lifecycle; LiteCowork owns scoped authorization for use.
- Workspace is the security/resource boundary. Runtime, RuntimeIncarnation, Environment, AgentHost and AgentSession are distinct identities.
- Installed != connected != assigned != granted != activated != invoked.
- Reported, observed, committed and verified are different. Unknown is not success.
- Provider events are untrusted until normalized/correlated. Native handles and credentials never enter public projections, replicated events or ordinary logs.
- V1 is local-first desktop. Cloud/remote Runtime, multi-device continuation and mobile are later scope unless the release contract changes.

## 3. Documentation-first feature process

No implementation starts for a material feature before its complete SDD package is reviewed and explicitly accepted by its owners.

Every new material feature has a folder under `specs/` containing:
- `README.md`: scope, status, owners, dependencies and navigation.
- `SPEC.md`: problem, goals/non-goals, locked/flexible decisions, HLD/LLD, interfaces, security, errors/recovery, config, EARS acceptance, command, out-of-bounds paths and open questions.
- `PLAN.md`: phases, dependency DAG, ownership, allowed files, migration/rollout/rollback, validation gates.
- `TASKS.md`: stable task IDs, one reviewable outcome each, dependencies, file scopes, tests and evidence.
- `DATA-MODEL.md`: identities, records, relations, lifecycle, persistence, events, retention and migration.
- `RESEARCH.md`: dated/versioned sources, findings, alternatives, assumptions and unresolved questions.
- `WIRING.yaml`: components, owners, calls, data/events, transactions and recovery.
- `contracts/`: feature-specific API/event/error/schema details or precise links to canonical owner definitions; **`contracts/ui-behavior.md`** is required (flows, states, typed UI commands — not pixels).

Follow `docs/SPECIFICATION-STANDARD.md` and `.specify/templates/`. Reuse global contracts via links; do not duplicate them. Promote a global API/event/schema in its actual owner and machine schema in the same reviewed change. Mark exact observed signatures CURRENT and unimplemented designs PROPOSED.

Lifecycle: DRAFT -> IN_REVIEW -> ACCEPTED/LOCKED -> IMPLEMENTING -> VERIFIED. BLOCKED and SUPERSEDED are explicit. Never self-approve or claim acceptance without owner decision/evidence. Blocking questions require named owners and resolution gates.

## 4. Before any material change

1. Inspect branch, SHA, dirty paths, ignored local data and worktrees; preserve all pre-existing changes.
2. Read root guidance, relevant product/domain owners, accepted ADRs, schemas, plans and evidence.
3. Trace user outcome -> UI -> authenticated request -> authorization -> service owner -> transaction/fence -> aggregate/event/receipt -> projection/stream -> restart/recovery -> acceptance.
4. Identify affected canonical docs, schemas, migrations, errors, functions, UI controls and tests.
5. Keep one normative definition per entity/state machine/API/event; link everywhere else.
6. Update owner prose and promoted machine contracts together. Keep target-only proposals visibly marked.
7. Update indexes, ledgers, traceability and local links.
8. Inspect the full diff and record exact validation commands/results in `CURRENT-RUN.md`.

## 5. Evidence vocabulary

- **TARGET:** normative design only.
- **CODE:** compiled or unit-exercised at an identified revision.
- **SERVED:** authenticated route and durable owner/storage path are mounted.
- **INTEGRATED:** real native harness/provider and persisted product path work together.
- **QUALIFIED:** exact OS/build/provider/version/auth route has CODE, SYSTEM and owner USER evidence, including denial and recovery.

A mock, schema, catalog entry, UI tile, transport helper, unit test or mounted route alone is not product qualification. Record date, revision, platform and limits. Never carry historical evidence forward as current without fresh verification.

Use `docs/ACCEPTANCE-AND-E2E-PLAN.md`. Full E2E uses actual desktop UI, daemon, IPC/Operator, storage, native session, filesystem effects and recovery. Test denial, duplicate replay, timeout-after-dispatch, cancellation races, crash, restart and reconnect. WSL2 is not proof of native Windows or native Linux desktop behavior.

## 6. LiteSPM and connectors

- One shared LiteSPM service/database/CAS per OS user; thin bridge registrations per agent.
- LiteSPM owns package discovery/lifecycle; do not duplicate its database, resolver or writer in LiteCowork.
- ACSR requires explicit standing consent; never silently mutate host agent config.
- Package approval is bound to exact plan digest; agents can request but cannot self-approve.
- Connectors use opaque Connection IDs and brokered HTTP; no token bytes or arbitrary URLs reach an agent.
- Activepieces provides operation examples; Nango provides provider/auth/scope metadata; OpenAPI provides endpoint candidates. Do not embed their runtimes or enable generated candidates without review.
- Keep TLS verification enabled; enforce SSRF, redirects, timeout, byte limits, redaction, idempotency and UNKNOWN reconciliation.
- Exact LiteSPM wire names/schema/auth are governed by a published, version-pinned contract; never infer method names from CLI output or catalogs.

## 7. Layout and preservation boundary

- `docs/`: canonical product, architecture, owner, schema, ADR, plan and evidence docs.
- `specs/`: complete feature SDD packages.
- `.specify/`: workflow, templates and documentation-only validators.
- Multi-crate source layout: `apps/`, `crates/`, `capabilities/`; do not introduce generic root `src/`.
- `archives/legacy-codebase-c7e9c05/`: ignored local source archive. Never stage or rewrite it in docs work.
- `docs/archive/legacy-implementation-c7e9c05/`: versioned historical docs, not current release truth.
- Dirty `feature/conversation-session-supervisor` worktree is preserved under the local archive. Do not reset, stash, rebase, commit, merge, prune or mutate it without explicit authorization.
- `CURRENT-RUN.md` is the durable handoff. Keep preservation boundary, progress, decisions, changed paths, checks and next safe action current.

## 8. Git, security and delivery

- Never discard or normalize unrelated dirty work. Stage intended paths explicitly.
- Never stage archives, caches, credentials, native profiles, provider sessions, secrets or unreviewed generated output.
- No force-push/history rewrite. No commit, push, release, production mutation or spend without explicit authorization.
- Keep secrets out of docs, fixtures, logs and reports; use fake values only.
- Do not add dependencies without documenting why existing options do not fit.
- If interrupted, leave a resumable state and update `CURRENT-RUN.md`.
