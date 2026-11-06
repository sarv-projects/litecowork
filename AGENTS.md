# Repository operating instructions

- **Repository:** LiteCowork
- **Primary architecture:** Rust daemon and domain crates with a Tauri v2 / React desktop UI
- **Documentation authority:** docs/ and the accepted feature packages under specs/

## 1. Authority and precedence

Resolve conflicts using this order:

1. Root ARCHITECTURE.md defines global invariants, authority boundaries and ownership.
2. docs/PRODUCT.md and docs/V1-PRODUCT-RELEASE-CONTRACT.md define the product promise and V1 boundary.
3. The owning domain contracts and accepted ADRs define their domains.
4. docs/schemas contains canonical OpenAPI, JSON Schema, event, migration and generated route contracts.
5. UI contracts, function/settings ledgers, coverage maps and generated types provide traceability; they do not override owner contracts.
6. docs/IMPLEMENTATION-STATUS.md records what the current checkout implements and the available evidence; source evidence does not rewrite normative target behavior.
7. CODE, SYSTEM and USER qualification evidence applies only to the exact build, OS, agent/provider version, authentication route and tested behavior.
8. docs/IMPLEMENTATION-MASTER-PLAN.md orders work; it does not redefine domain semantics.

Never silently choose the newest file. Identify the owner of the disputed semantic decision, resolve it there, then update dependent contracts, UI mappings, indexes and traceability. Do not create a parallel schema or state-machine authority.

## 2. Product invariants

- LiteCowork is local-first, chat-first and BYOA-first. Ordinary Conversation does not require a Task or Coworker.
- Conversation and Task are separate; Task creation is explicit.
- Native agent harnesses own their reasoning, authentication, model catalogs, private transcripts, native tools and harness-specific behavior.
- LiteCowork Core owns Workspace, Conversation, Task, Trust, grants, mediated Effects, Artifacts, Evidence and recovery.
- LiteSPM owns cross-agent package catalog, package inventory, host registration, package mutation and package lifecycle; LiteCowork owns scoped authorization for use.
- Workspace is the security/resource boundary. Runtime, RuntimeIncarnation, Environment, AgentHost and AgentSession are distinct identities.
- Installed, connected, assigned, granted, activated, invoked and verified are separate facts.
- Reported, observed, committed and verified are different evidence states. Unknown is not success.
- Provider events are untrusted until normalized and correlated. Native handles and credentials never enter public projections, replicated events or ordinary logs.
- V1 is local desktop. Cloud/remote Runtime, laptop-off continuation, mobile dispatch and multi-user collaboration are later scope.
- Trust and Effect authority applies only to operations actually mediated by LiteCowork; do not claim interception of direct native tools.

## 3. Specification process

A material capability must have a complete feature SDD before its design is treated as ready for implementation.

Every feature package contains:

- README.md: scope, lifecycle, owners, dependencies and navigation.
- SPEC.md: problem, goals/non-goals, accepted and flexible decisions, HLD/LLD, interfaces, security, errors/recovery, config, EARS acceptance and open decisions.
- PLAN.md: phases, dependency graph, ownership, allowed paths, migrations, rollout and gates.
- TASKS.md: stable reviewable work items, dependencies, file scopes, tests and evidence.
- DATA-MODEL.md: identities, records, relations, lifecycle, persistence, events, retention and migration.
- RESEARCH.md: pinned sources, versions/SHAs, alternatives, decisions and unresolved questions.
- WIRING.yaml: components, owners, calls, events, transactions and recovery.
- contracts/: API/event/error and UI behavior mappings. The UI behavior contract is required and must specify screens, commands, state transitions, failure/empty/loading behavior and recovery.

Use docs/SPECIFICATION-STANDARD.md and .specify/templates. Reuse canonical global contracts by link; do not duplicate them. New machine contracts are promoted in the owning document and docs/schemas in the same reviewed change.

Feature lifecycle is DRAFT → IN_REVIEW → ACCEPTED/LOCKED → IMPLEMENTING → VERIFIED. BLOCKED and SUPERSEDED are explicit states. A passing structural validator is not owner approval. Do not claim acceptance without recorded owner review.

## 4. Required cross-layer trace

For every user-visible operation, trace:

User outcome → screen and control → typed UI command → authenticated IPC/Operator route → authorization → service owner → transaction/version/fence → aggregate/event/receipt → projection/stream → restart/recovery → acceptance evidence.

Record the exact UI control. A backend route with no UI path is not a user journey; a UI button with no owner route is not an implemented operation.

Before a material change:

1. Inspect branch, revision, dirty paths, ignored data and worktrees.
2. Read the product/domain owners, accepted ADRs, schemas, plans and evidence.
3. Trace the whole user journey and affected API/event/error/schema/UI/test contracts.
4. Keep one normative definition per entity, state machine, API and event.
5. Update owner prose and promoted machine contracts together.
6. Update UI screen/action ledgers, SDD wiring, traceability and indexes.
7. Run narrow tests, contract validators, security/negative/recovery tests and platform tests.
8. Inspect the entire diff and record exact commands/results.

## 5. Evidence vocabulary

- TARGET: normative design only.
- CODE: compiled or unit-exercised at an identified revision.
- SERVED: authenticated route and owning storage/service path are mounted.
- INTEGRATED: a real native provider/harness and persisted product path work together.
- QUALIFIED: exact OS/build/provider/auth route has CODE, SYSTEM and owner USER evidence, including denial and recovery.

A mock, schema, catalog entry, UI tile, probe, transport helper, unit test or mounted route alone is not product qualification. Runtime readiness and Task execution readiness are separate.

Use docs/IMPLEMENTATION-STATUS.md and docs/ACCEPTANCE-AND-E2E-PLAN.md. Full E2E must exercise the packaged desktop UI, daemon, IPC/Operator, storage, real native session, filesystem effects and recovery. Test denial, duplicate replay, timeout-after-dispatch, cancellation races, crash, restart and reconnect. WSL2 is not proof of native Windows or native Linux desktop behavior.

## 6. LiteSPM and connectors

- LiteSPM owns package discovery and lifecycle. Do not duplicate its database, resolver or configuration writer inside LiteCowork.
- ACSR and package mutations require explicit consent and an exact approved plan.
- Package approval is bound to the plan digest; an agent can request but cannot self-approve.
- Installation is not Connection authentication, Coworker assignment, Task grant or invocation authority.
- Direct-native attachment is reported/observed unless a qualified hook or OS boundary mediates the operation.
- Connectors use opaque Connection IDs and brokered HTTP. Tokens and arbitrary URLs do not reach an agent.
- TLS verification, SSRF controls, redirect policy, timeout, byte limits, redaction, idempotency and UNKNOWN reconciliation are mandatory.
- LiteSPM wire names, version and authentication are governed by its published pinned bridge contract. Never infer RPC names from CLI output or marketplace listings.

## 7. Repository layout and preservation

- docs/: canonical product, architecture, owner, schema, ADR, plan and evidence documents.
- specs/: feature SDD packages.
- apps/: desktop UI and local daemon applications.
- crates/: domain, storage, IPC, runtime and shared libraries.
- .specify/: specification workflow, templates and documentation validators.
- repo_take/: pinned external reference clones for research only; never import their code as LiteCowork source of truth.
- docs/archive/: historical documentation snapshots, not current implementation status.
- archives/ and local worktrees: preserve as user data; do not clean or rewrite without explicit authorization.

The active Rust and Tauri source trees are apps/ and crates/. Do not describe the active checkout as documentation-only or say those trees are absent.

## 8. Security and delivery

- Never log or expose provider credentials, tokens, cookies, native session handles or secret-bearing request bodies.
- Do not weaken TLS, IPC peer identity, workspace boundaries, idempotency, fencing or recovery for convenience.
- Do not claim a Job Object is a complete filesystem/network/credential sandbox.
- Never convert direct native actions into fictitious LiteCowork Effect receipts.
- Do not execute arbitrary model-supplied HTML/JavaScript in the chat WebView.
- No force-push or history rewrite. No commit or push unless explicitly requested.
- Preserve unrelated user work and stage only explicitly intended files.
