# Architecture Coverage Matrix

## Authority order

1. [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — product definition, system HLD,
   invariants, ownership, and architectural authority.
2. [`PRODUCT.md`](PRODUCT.md) — product behavior and non-goals.
3. Domain contracts — canonical definitions for each owner below.
4. [`PROTOCOLS.md`](PROTOCOLS.md), [`API.md`](API.md), and `schemas/` — wire and
   persistence representations of those contracts.
5. [`FLOWS.md`](FLOWS.md), [`FAILURE-RECOVERY.md`](FAILURE-RECOVERY.md), and
   [`EXPERIENCE.md`](EXPERIENCE.md) — end-to-end behavior and its UI projection.
6. [`SECURITY.md`](SECURITY.md), [`OBSERVABILITY.md`](OBSERVABILITY.md),
   [`TESTING.md`](TESTING.md), and [`BENCHMARKS.md`](BENCHMARKS.md) — operation,
   assurance, and acceptance.
7. ADRs explain rationale and never override the current authority.

When two documents conflict, the higher authority wins and the lower document must be
corrected in the same change. A schema or example cannot silently create a new product
decision.

## Concern-to-authority map

| Concern | Normative authority |
|---|---|
| Domain language | [`../GLOSSARY.md`](../GLOSSARY.md) |
| Product identity, HLD, ownership, invariants | [`../ARCHITECTURE.md`](../ARCHITECTURE.md) |
| Product promise, user-visible outcomes, Core non-goals | [`PRODUCT.md`](PRODUCT.md) |
| Workspace identity, owner, lifecycle, and replication policy | [`DATA-MODEL.md`](DATA-MODEL.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md) |
| Entity meaning, fields, relationships, invariants | [`DATA-MODEL.md`](DATA-MODEL.md) |
| Shared enums, value types, error taxonomy | [`SCHEMAS.md`](SCHEMAS.md) |
| Legal lifecycle transitions and transition owners | [`STATE-MACHINES.md`](STATE-MACHINES.md) |
| Service methods, dependencies, forbidden edges | [`SERVICES.md`](SERVICES.md) and owning domain contracts |
| Task, plan, Attempt, checkpoint semantics | [`TASK-RUNTIME.md`](TASK-RUNTIME.md) |
| Agent sessions, adapters, delegation | [`AGENT-FABRIC.md`](AGENT-FABRIC.md) |
| Discovered AgentProfiles, Workspace bindings, enablement | [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md) |
| Capability use and LitePSM boundary | [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md) |
| Runtime identity, replication, conflict, leases, handoff | [`RUNTIME-MESH.md`](RUNTIME-MESH.md) |
| Execution substrates and cleanup | [`ENVIRONMENTS.md`](ENVIRONMENTS.md) |
| Principals, policy, approval, secrets, grants | [`TRUST.md`](TRUST.md), [`SECURITY.md`](SECURITY.md) |
| Artifact, Effect, Evidence, verification | [`ARTIFACTS-EVIDENCE.md`](ARTIFACTS-EVIDENCE.md) |
| Trigger, occurrence, duplicate and overlap semantics | [`AUTOMATION.md`](AUTOMATION.md) |
| Human-facing messaging transports | [`CHANNELS.md`](CHANNELS.md) |
| Durable event envelope, registry, evolution | [`EVENTS.md`](EVENTS.md), `schemas/domain-event.schema.json` |
| Operator, Mesh, Agent, Capability, Environment boundaries | [`PROTOCOLS.md`](PROTOCOLS.md) |
| User-facing request/response API | [`API.md`](API.md), `schemas/operator-api.openapi.yaml` |
| Transactions, tables, migrations, blobs | [`STORAGE.md`](STORAGE.md), `schemas/sqlite-v1.sql` |
| Actor-by-actor end-to-end sequences | [`FLOWS.md`](FLOWS.md) |
| Failure matrix, retry, reconcile, failover | [`FAILURE-RECOVERY.md`](FAILURE-RECOVERY.md) |
| Navigation, screens, interaction and UI states | [`EXPERIENCE.md`](EXPERIENCE.md) |
| Component anatomy, tokens and accessibility | [`DESIGN-SYSTEM.md`](DESIGN-SYSTEM.md) |
| Semantic motion and timing | [`MOTION.md`](MOTION.md) |
| Logs, metrics, traces, audit and SLO categories | [`OBSERVABILITY.md`](OBSERVABILITY.md) |
| Threat model and mandatory controls | [`SECURITY.md`](SECURITY.md) |
| Test layers and conformance gates | [`TESTING.md`](TESTING.md) |
| Real-world acceptance scenarios | [`BENCHMARKS.md`](BENCHMARKS.md) |
| Module layout, dependency direction, build stages | [`IMPLEMENTATION.md`](IMPLEMENTATION.md) |
| Source-by-source comparison/disposition | [`SOURCE-RECONCILIATION.md`](SOURCE-RECONCILIATION.md) |
| Decision rationale | [`adr/`](adr/) |

## Deliberately deferred decisions

These are visible dependencies, not blanks to fill by guesswork:

- LitePSM API version, authentication, package/plugin schema, MCP discovery, and
  install/activation lifecycle. The selected base URL is recorded in
  [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md); LitePSM owns the contract.
- Brand color, typeface, final visual style, and UI component library.
- Exact numeric SLO targets until hardware and representative workloads are measured.
- Concrete managed-cloud vendor and storage provider.
- Final Operator transport implementation (the logical API contract is transport
  independent; HTTP/OpenAPI and local IPC map to the same commands).
- Concrete database access library and Rust crate boundaries before the first vertical
  slice.

Changing entity meaning, transition ownership, protocol boundaries, Core ownership,
event/effect guarantees, lease semantics, or the threat model requires an architecture
change and ADR. Choosing an implementation that preserves those contracts does not.
