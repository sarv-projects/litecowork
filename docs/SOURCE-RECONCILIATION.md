# Architecture Source Reconciliation

## Input and review record

The supplied reference ZIP contained 46 entries, totaling 202,907 uncompressed bytes.
Every entry was inventoried and read in full, including the 4,187-line historical
source snapshot and all three machine-readable schema files. The archive SHA-256 was:

```text
b15029fc8082793cc61b853acea8dc079088b813becd05b4bbcdafc77e85cc5c
```

The current product identity is **LiteCowork**. The historical source snapshot was used
as design input but is not copied into this repository: its contents are represented by
the current authority and focused contracts, and a second full architecture text would
create duplicate authority and preserve obsolete naming. No source entry is silently
discarded; each entry's destination or disposition is listed below.

## Entry-by-entry disposition

| Reference entry | LiteCowork disposition |
|---|---|
| `ARCHITECTURE.md` | Reconciled into root `ARCHITECTURE.md`; current LiteCowork name and direct-capability safety condition added. |
| `MANIFEST.md` | Rebuilt as `docs/COVERAGE-MATRIX.md`, with the glossary and reconciliation record added. |
| `README.md` | Reconciled into root `README.md`; product phase, authority links, and LitePSM deferral stated. |
| `docs/AGENT-FABRIC.md` | Adopted as `docs/AGENT-FABRIC.md`; LiteCowork naming and capability boundary checked. |
| `docs/API.md` | Adopted and expanded as `docs/API.md` plus `docs/schemas/operator-api.openapi.yaml`. |
| `docs/ARTIFACTS-EVIDENCE.md` | Adopted as `docs/ARTIFACTS-EVIDENCE.md`; cross-linked to the data model and event rules. |
| `docs/AUTOMATION.md` | Adopted as `docs/AUTOMATION.md`; occurrence idempotency and trigger semantics retained. |
| `docs/BENCHMARKS.md` | Adopted as `docs/BENCHMARKS.md`; cases become the acceptance scenario catalog. |
| `docs/CAPABILITY-FABRIC.md` | Reworked in place; selected LitePSM endpoint recorded while its wire/package contract stays deferred. Direct mutation rules were tightened. |
| `docs/CHANNELS.md` | Adopted as `docs/CHANNELS.md`; channels remain human transports, not agent protocols or Task owners. |
| `docs/COVERAGE-MATRIX.md` | Adopted and expanded to include every current contract and this reconciliation. |
| `docs/DATA-MODEL.md` | Adopted and extended with missing connection, channel, handoff, audit, and secret-lease records and invariants. |
| `docs/DESIGN-SYSTEM.md` | Adopted and finalized as `docs/DESIGN-SYSTEM.md`; v1 visual tokens, component states, responsive layout, accessibility, and contrast requirements are fixed. |
| `docs/ENVIRONMENTS.md` | Adopted and extended as `docs/ENVIRONMENTS.md`; lifecycle, isolation, and cleanup ownership are explicit. |
| `docs/EVENTS.md` | Adopted and expanded as `docs/EVENTS.md` plus `docs/schemas/domain-event.schema.json`. |
| `docs/EXPERIENCE.md` | Adopted and expanded as `docs/EXPERIENCE.md`; LiteCowork UI naming and cross-device states are authoritative. |
| `docs/FAILURE-RECOVERY.md` | Reworked in place; ambiguity, stale fencing, unavailable Hub, and retry gates are explicit. |
| `docs/FLOWS.md` | Adopted and expanded as `docs/FLOWS.md`; flow coverage and state/UI outcomes are cross-checked. |
| `docs/IMPLEMENTATION.md` | Adopted as `docs/IMPLEMENTATION.md`; this remains a boundary/milestone plan, not a source tree to pre-create. |
| `docs/MOTION.md` | Adopted as `docs/MOTION.md`; timings and interrupted/reduced-motion behavior are specified. |
| `docs/OBSERVABILITY.md` | Adopted as `docs/OBSERVABILITY.md`; domain, audit, logs, metrics, and traces stay distinct. |
| `docs/PRODUCT.md` | Adopted and expanded as `docs/PRODUCT.md`; open user choices are separated from frozen product rules. |
| `docs/PROTOCOLS.md` | Adopted and expanded as `docs/PROTOCOLS.md`; LitePSM's API remains outside this contract. |
| `docs/RUNTIME-MESH.md` | Adopted and tightened as `docs/RUNTIME-MESH.md`; offline execution cannot bypass fencing assumptions. |
| `docs/SCHEMAS.md` | Adopted and expanded as `docs/SCHEMAS.md`; shared enums, errors, and versioning are centralized. |
| `docs/SECURITY.md` | Adopted and expanded as `docs/SECURITY.md`; threat controls and residual boundaries are explicit. |
| `docs/SERVICES.md` | Adopted and expanded as `docs/SERVICES.md`; owners, methods, dependencies, and forbidden edges are stated. |
| `docs/STATE-MACHINES.md` | Adopted and reconciled with `docs/SCHEMAS.md`, service owners, and UI projections. |
| `docs/STORAGE.md` | Adopted and expanded as `docs/STORAGE.md` plus `docs/schemas/sqlite-v1.sql`; missing durable records and uniqueness rules were added. |
| `docs/TASK-RUNTIME.md` | Adopted and tightened as `docs/TASK-RUNTIME.md`; plan/Attempt/recovery semantics are explicit. |
| `docs/TESTING.md` | Adopted as `docs/TESTING.md`; conformance, fault, cross-Runtime, security, and E2E layers are covered. |
| `docs/TRUST.md` | Adopted and extended as `docs/TRUST.md`; authorization order, assurance, secrets, and revocation are normative. |
| `docs/adr/0001-thin-core.md` | Reconciled into existing `docs/adr/0001-durable-task-core.md`. |
| `docs/adr/0002-litepsm-independent.md` | Reconciled into existing `docs/adr/0003-litepsm-is-independent.md`. |
| `docs/adr/0003-agents-own-reasoning.md` | Reconciled into existing `docs/adr/0001-durable-task-core.md` and `docs/AGENT-FABRIC.md`. |
| `docs/adr/0004-task-not-transcript-is-truth.md` | Reconciled into existing `docs/adr/0002-task-and-attempt-identity.md`. |
| `docs/adr/0005-one-runtime-anywhere.md` | Reconciled into existing `docs/adr/0004-runtime-and-environment-are-distinct.md`. |
| `docs/adr/0006-event-replication-not-db-sync.md` | Reconciled into existing `docs/adr/0005-replicate-domain-state-not-databases.md`. |
| `docs/adr/0007-leases-and-fencing.md` | Reconciled into `ARCHITECTURE.md`, `docs/RUNTIME-MESH.md`, and `docs/STATE-MACHINES.md`. |
| `docs/adr/0008-conversation-task-separation.md` | Reconciled into `docs/DATA-MODEL.md`, `docs/EXPERIENCE.md`, `docs/FLOWS.md`, and ADR-0006. |
| `docs/adr/0009-reported-observed-verified.md` | Reconciled into `docs/ARTIFACTS-EVIDENCE.md`, `docs/STATE-MACHINES.md`, and ADR-0007. |
| `docs/adr/0010-no-generic-provider-layer.md` | Reconciled into `docs/PROTOCOLS.md`, `ARCHITECTURE.md`, and ADR-0008. |
| `docs/archive/2026-10-01-vnext-source-snapshot.md` | Read in all 4,187 lines; not copied because its decisions are consolidated into current authority and it would duplicate a historical product-name authority. |
| `docs/schemas/domain-event.schema.json` | Adapted to `docs/schemas/domain-event.schema.json`; schema identifier uses a LiteCowork URN. |
| `docs/schemas/operator-api.openapi.yaml` | Adapted and expanded as `docs/schemas/operator-api.openapi.yaml`; it describes LiteCowork's Operator API only. |
| `docs/schemas/sqlite-v1.sql` | Adapted and expanded as `docs/schemas/sqlite-v1.sql`; it remains a candidate local schema until implementation migration review. |

## Reconciliation decisions

1. The current HLD and this focused LLD suite are normative. Reference wording is not
   copied blindly where it conflicts with explicit LiteCowork boundaries.
2. LitePSM's selected base URL is recorded, but no LitePSM API, package, plugin, MCP,
   authentication, or installation contract is invented here.
3. LiteCowork-mediated consequential calls require an Effect record and current fence
   before dispatch. Direct attachment is not an exception to authorization or recovery.
4. The historical source snapshot is not a current requirement document. Its retained
   ideas are linked through current authorities; its unrelated external-product claims
   are not treated as facts to implement.
5. Items called “deferred” in `COVERAGE-MATRIX.md` remain visible and cannot be inferred
   from example schemas or implementation conveniences.

## Runtime, Routine and UX proposal disposition (45 numbered items)

The duplicated proposal is one input, not two independent confirmations. Item numbers
below refer to that proposal; adoption is constrained by current domain contracts.

| Items | Disposition and current authority |
|---|---|
| 1 | Dated primary-source research in COMPETITIVE-RESEARCH; rollout dates and uncertain sources are qualified. OpenHands' GitHub/event integrations are distinguished from an unverified generic cron/polling service. Competitor internals and exclusivity claims are not inferred. |
| 2–7 | Adopted independent Operator/Runtime/worker lifetimes, startup policy, boot recovery, incarnation identity and sleep semantics in RUNTIME-LIFECYCLE. OS startup is not an Automation. |
| 8 | WakeProvider remains a later, best-effort Mesh provider; availability cannot assume successful wake or closed-lid support. |
| 9 | Resume coordinator validates watchers, stale observations, Effects and deferred work before admission. |
| 10–16 | AgentHostInstance, lazy single-flight admission, negotiated sharing/model changes, operation-scoped session close/replacement, and drain-safe lead replacement in AGENT-FABRIC and RUNTIME-LIFECYCLE. Durable AgentSession provenance is separated from Runtime-local host/native handles; prewarming is optional and existing Attempts remain pinned. |
| 17–18 | Capability/app demand activation adopted; LitePSM owns provider process supervision. Only verified owned app instances may be automatically closed. |
| 19 | ExecutionDependencyPlan is an infrastructure readiness DAG, with cycle detection/idempotent prepare/reverse owned cleanup; no reasoning or general workflow engine. |
| 20–25 | Broader typed triggers, separate placement, stable cursor identity, misfires and WAITING_DEPENDENCY adopted in AUTOMATION. Provider-specific triggers are enabled only after qualification. |
| 26–28 | Routine/RoutineRevision added in ROUTINES; Automation pins a reviewed reusable work revision. Routines/Automations/Runs are separate views. |
| 29–33 | Persistent conversation recents, Needs You, one composer and explicit save/schedule actions adopted in EXPERIENCE. Agent Auto means explicit binding/default, not a hidden model router. |
| 34 | Quick Entry remains an optional Operator feature; capture and send require user action. |
| 35–37 | Two desktop executables, explicit stop preview/drain and advance dependency availability adopted in API, RUNTIME-LIFECYCLE and EXPERIENCE. Closing UI is distinct from stopping daemon. |
| 38–39 | Offer readiness, lightweight authorized structural resource observation, and small local application inventory adopted; semantic indexing/LLM/OCR stays demand-driven external work. |
| 40 | Human takeover uses separate EnvironmentControlLease epochs, fresh observation and rejected queued stale actions in ENVIRONMENTS and F31. |
| 41 | LiteCowork owns a normalized per-Runtime ProviderHost view and derives its active Activation references; LitePSM owns package/process lifecycle and global process counts. Sharing, health freshness, and isolation are explicit in CAPABILITY-FABRIC, DATA-MODEL, SERVICES, RUNTIME-LIFECYCLE, and the Operator API. No second package/process supervisor is introduced. |
| 42 | Native abstractions stop at Routine/Automation/Task; deterministic workflow engines remain external capabilities. |
| 43–44 | Control-plane framing adopted in ARCHITECTURE; “no competitor combines all” rejected as unproven. Supervisor ownership is refined to preserve LitePSM independence. |
| 45 | Cross-contract coverage tracked in COVERAGE-MATRIX, typed schema candidates, flows and conformance scenarios; coverage is not a claim of implemented runtime behavior. |

LitePSM integration is intentionally limited to its recorded selected service. Its MCP,
plugin, package, authentication and supervision wire contracts remain deferred until the
actual service contract is provided or independently verified with user authorization.
