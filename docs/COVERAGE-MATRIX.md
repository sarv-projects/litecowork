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
| Workspace identity, instructions, roots, owner, lifecycle, and replication policy | [`DATA-MODEL.md`](DATA-MODEL.md), [`CONTEXT.md`](CONTEXT.md), [`WORLD-RESOURCES.md`](WORLD-RESOURCES.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md) |
| One-time folder attachment path provenance and upload-session pinning | [`DATA-MODEL.md`](DATA-MODEL.md), [`WORLD-RESOURCES.md`](WORLD-RESOURCES.md), [`API.md`](API.md), [`EVENTS.md`](EVENTS.md), `schemas/operator-api.openapi.yaml`, `schemas/domain-event.schema.json`, `schemas/sqlite-v2.sql`, [`FLOWS.md`](FLOWS.md) |
| Entity meaning, fields, relationships, invariants | [`DATA-MODEL.md`](DATA-MODEL.md) |
| Shared enums, value types, error taxonomy | [`SCHEMAS.md`](SCHEMAS.md) |
| Legal lifecycle transitions and transition owners | [`STATE-MACHINES.md`](STATE-MACHINES.md) |
| Service methods, dependencies, forbidden edges | [`SERVICES.md`](SERVICES.md) and owning domain contracts |
| Task, plan, Attempt, checkpoint semantics | [`TASK-RUNTIME.md`](TASK-RUNTIME.md) |
| Truthful Task progress, active workstreams, activity/evidence freshness | [`TASK-RUNTIME.md`](TASK-RUNTIME.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml` |
| Step placement preview, candidate selection, and recovery version checks | [`TASK-RUNTIME.md`](TASK-RUNTIME.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml` |
| Agent sessions, adapters, delegation | [`AGENT-FABRIC.md`](AGENT-FABRIC.md) |
| Native harness integrity, DelegationProfiles, host/native/capability worker classes, selection, escalation, cost, quotas, warmth, and handoff | [`DELEGATION.md`](DELEGATION.md), [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`API.md`](API.md), `schemas/delegation.schema.json`, `schemas/operator-api.openapi.yaml` |
| Lead failover policy, Attempt profile pinning, Coworker-owned Automation revisions, ActionBatch membership, execution-method provenance, and ContextDocument revision/deletion controls | [`DELEGATION.md`](DELEGATION.md), [`TASK-RUNTIME.md`](TASK-RUNTIME.md), [`AUTOMATION.md`](AUTOMATION.md), [`CAPABILITY-INVOCATIONS.md`](CAPABILITY-INVOCATIONS.md), [`CONTEXT.md`](CONTEXT.md), [`EVENTS.md`](EVENTS.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml`, `schemas/domain-event.schema.json`, `schemas/sqlite-v1.sql` |
| Coworker identity/presence, evidence-linked Goals and immutable Task/Routine/Artifact links, Suggestions, personal context, and user-authored ContextDocuments | [`RESPONSIBILITIES.md`](RESPONSIBILITIES.md), [`CONTEXT.md`](CONTEXT.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml`, `schemas/sqlite-v10.sql`, [`STORAGE.md`](STORAGE.md), [`FLOWS.md`](FLOWS.md), [`EXPERIENCE.md`](EXPERIENCE.md) |
| Durable AgentSession provenance, operation-scoped lifetime, replacement, and Runtime-local native/session handles | [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STATE-MACHINES.md`](STATE-MACHINES.md), [`STORAGE.md`](STORAGE.md), [`EVENTS.md`](EVENTS.md) |
| Discovered AgentProfiles, local installation-vs-profile probe, Runtime enrollment precondition, Workspace bindings, enablement | [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md), [`FLOWS.md`](FLOWS.md), `schemas/operator-api.openapi.yaml`, [`implementation/epics/E03.md`](../implementation/epics/E03.md) |
| Workspace default lead selection and standalone Task persistence/admission boundaries | [`TASK-RUNTIME.md`](TASK-RUNTIME.md), [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md), [`EVENTS.md`](EVENTS.md), [`FLOWS.md`](FLOWS.md), `schemas/operator-api.openapi.yaml`, `schemas/domain-event.schema.json`, [`implementation/CURRENT-RUN.md`](../implementation/CURRENT-RUN.md) |
| Capability use and LiteSPM boundary | [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md) |
| Runtime-local provider instance readiness, health freshness, sharing isolation, and derived Activation-use count | [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml` |
| Durable capability operations and MCP Tasks mapping | [`CAPABILITY-INVOCATIONS.md`](CAPABILITY-INVOCATIONS.md), [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md), [`DATA-MODEL.md`](DATA-MODEL.md) |
| Provider input outbox, pause/resume authority, exact-key retry, non-migrating provider handles, non-secret forms, and URL-mode credential handoff | [`CAPABILITY-INVOCATIONS.md`](CAPABILITY-INVOCATIONS.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STATE-MACHINES.md`](STATE-MACHINES.md), [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml`, `schemas/sqlite-v1.sql` |
| UserRequest response eligibility, immutable response provenance, and one-way resolution | [`STATE-MACHINES.md`](STATE-MACHINES.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`SERVICES.md`](SERVICES.md), [`STORAGE.md`](STORAGE.md), [`TESTING.md`](TESTING.md), `schemas/sqlite-v1.sql` |
| Runtime-private MCP task handles, provider cursors, and input-request keys | [`CAPABILITY-INVOCATIONS.md`](CAPABILITY-INVOCATIONS.md), [`AUTOMATION.md`](AUTOMATION.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`EVENTS.md`](EVENTS.md), [`STORAGE.md`](STORAGE.md), [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md) |
| Resource identity, locations, roots, revision-scoped encrypted lexical indexing, key-version freshness, deletion boundary, and deterministic search | [`WORLD-RESOURCES.md`](WORLD-RESOURCES.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STORAGE.md`](STORAGE.md), [`CONTEXT.md`](CONTEXT.md), [`EVENTS.md`](EVENTS.md), [`API.md`](API.md), [`TESTING.md`](TESTING.md), [`BENCHMARKS.md`](BENCHMARKS.md), `schemas/sqlite-v7.sql`, `schemas/operator-api.openapi.yaml` |
| Workspace instructions and bounded context projection | [`CONTEXT.md`](CONTEXT.md), [`DATA-MODEL.md`](DATA-MODEL.md) |
| Operator/daemon/worker lifecycles, startup, incarnations, lazy hosts, sleep/resume, drain | [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`STATE-MACHINES.md`](STATE-MACHINES.md), [`SERVICES.md`](SERVICES.md) |
| Authenticated local Operator-serving handshake, bootstrap-incarnation fencing, and separation from Runtime execution readiness | [`API.md`](API.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md), [`FLOWS.md`](FLOWS.md), `schemas/operator-api.openapi.yaml` |
| Desktop local Operator IPC framing, OS-peer authentication, endpoint lifecycle, and cross-platform qualification | [`LOCAL-OPERATOR-IPC.md`](LOCAL-OPERATOR-IPC.md), [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`API.md`](API.md), [`SERVICES.md`](SERVICES.md), [`FLOWS.md`](FLOWS.md), [`TESTING.md`](TESTING.md) |
| Lightweight application inventory, launch/attach ownership, and local process identity | [`WORLD-RESOURCES.md`](WORLD-RESOURCES.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`TESTING.md`](TESTING.md) |
| Workspace-persistent Environment provision, budget, retention and reuse | [`ENVIRONMENTS.md`](ENVIRONMENTS.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`API.md`](API.md), `schemas/operator-api.openapi.yaml` |
| Environment lifetime, sharing scope, owner binding, worktree isolation, browser control lease | [`ENVIRONMENTS.md`](ENVIRONMENTS.md), [`DELEGATION.md`](DELEGATION.md), [`DATA-MODEL.md`](DATA-MODEL.md), `schemas/sqlite-v1.sql` |
| Reusable work and revision pinning | [`ROUTINES.md`](ROUTINES.md), [`DATA-MODEL.md`](DATA-MODEL.md) |
| Dated competitor evidence and adoption decisions | [`COMPETITIVE-RESEARCH.md`](COMPETITIVE-RESEARCH.md), [`SOURCE-RECONCILIATION.md`](SOURCE-RECONCILIATION.md) |
| Runtime identity, replication, conflict, leases, handoff | [`RUNTIME-MESH.md`](RUNTIME-MESH.md) |
| Device signing algorithm, public-key/DeviceId encoding, OS-keystore scope, and rotation gate | [`RUNTIME-MESH.md`](RUNTIME-MESH.md), [`SCHEMAS.md`](SCHEMAS.md), [`STORAGE.md`](STORAGE.md), ADR [0021](adr/0021-device-signing-identity.md) |
| Installation-scoped Runtime identity, per-Workspace enrollment/pairing/revocation, and readiness separation | [`DATA-MODEL.md`](DATA-MODEL.md), [`RUNTIME-MESH.md`](RUNTIME-MESH.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md), [`STATE-MACHINES.md`](STATE-MACHINES.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md), [`FLOWS.md`](FLOWS.md), [`STORAGE.md`](STORAGE.md), `schemas/operator-api.openapi.yaml`, ADR [0020](adr/0020-runtime-identity-is-installation-scoped.md) |
| Authenticated RuntimeIncarnation registration/order and local-only OS boot observations | [`RUNTIME-MESH.md`](RUNTIME-MESH.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STORAGE.md`](STORAGE.md) |
| Execution substrates and cleanup | [`ENVIRONMENTS.md`](ENVIRONMENTS.md) |
| Local Task Environment containment, exact Resource pinning, OS isolation attestations, and writer quiescence | [`ENVIRONMENTS.md`](ENVIRONMENTS.md), [`ARCHITECTURE.md`](../ARCHITECTURE.md), [`TRUST.md`](TRUST.md), [`FLOWS.md`](FLOWS.md), [`TESTING.md`](TESTING.md), [`BENCHMARKS.md`](BENCHMARKS.md), [`implementation/epics/E07.md`](../implementation/epics/E07.md) |
| Provider-private Environment locators/checkpoint handles and portable checkpoint policy | [`ENVIRONMENTS.md`](ENVIRONMENTS.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STORAGE.md`](STORAGE.md), [`EVENTS.md`](EVENTS.md) |
| Resource location indirection and local pseudonymous file identity | [`WORLD-RESOURCES.md`](WORLD-RESOURCES.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STORAGE.md`](STORAGE.md), [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md) |
| Agent endpoint identity versus Runtime-local locator/readiness | [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STORAGE.md`](STORAGE.md), [`API.md`](API.md) |
| Principals, policy, approval, secrets, grants | [`TRUST.md`](TRUST.md), [`SECURITY.md`](SECURITY.md) |
| Artifact, Effect, Evidence, verification | [`ARTIFACTS-EVIDENCE.md`](ARTIFACTS-EVIDENCE.md) |
| Trigger, occurrence, duplicate and overlap semantics | [`AUTOMATION.md`](AUTOMATION.md) |
| Routine health, recent outcomes, dependency freshness, and reviewed drift repair | [`ROUTINES.md`](ROUTINES.md), [`EXPERIENCE.md`](EXPERIENCE.md), [`SERVICES.md`](SERVICES.md) |
| Human-facing messaging transports | [`CHANNELS.md`](CHANNELS.md) |
| Channel host ownership, receipt claims, cursor continuity/replication barriers, ingress gaps, and Runtime-local reply targets | [`RUNTIME-MESH.md`](RUNTIME-MESH.md), [`RUNTIME-LIFECYCLE.md`](RUNTIME-LIFECYCLE.md), [`CHANNELS.md`](CHANNELS.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`STATE-MACHINES.md`](STATE-MACHINES.md), [`FLOWS.md`](FLOWS.md), [`API.md`](API.md), [`STORAGE.md`](STORAGE.md), [`TESTING.md`](TESTING.md), `schemas/sqlite-v1.sql`, `schemas/domain-event.schema.json` |
| Durable event envelope, registry, evolution | [`EVENTS.md`](EVENTS.md), `schemas/domain-event.schema.json` |
| Operator, Mesh, Agent, Capability, Environment boundaries | [`PROTOCOLS.md`](PROTOCOLS.md) |
| User-facing request/response API | [`API.md`](API.md), `schemas/operator-api.openapi.yaml` |
| Transactions, tables, migrations, blobs | [`STORAGE.md`](STORAGE.md), `schemas/sqlite-v1.sql`, `schemas/sqlite-v2.sql`, `schemas/sqlite-v3.sql`, `schemas/sqlite-v4.sql` |
| Workspace backup manifests, encryption references, and restore behavior | [`DATA-MODEL.md`](DATA-MODEL.md), [`STORAGE.md`](STORAGE.md), [`RUNTIME-MESH.md`](RUNTIME-MESH.md), [`FLOWS.md`](FLOWS.md), [`API.md`](API.md) |
| Actor-by-actor end-to-end sequences | [`FLOWS.md`](FLOWS.md) |
| Failure matrix, retry, reconcile, failover | [`FAILURE-RECOVERY.md`](FAILURE-RECOVERY.md) |
| Navigation, screens, interaction and UI states | [`EXPERIENCE.md`](EXPERIENCE.md) |
| Typed Operator presentation items, renderers, transient stream/reconnect behavior, context-use disclosure, Artifact workbench/version interactions | [`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md), [`EXPERIENCE.md`](EXPERIENCE.md), [`API.md`](API.md), [`SERVICES.md`](SERVICES.md), [`ARTIFACTS-EVIDENCE.md`](ARTIFACTS-EVIDENCE.md) |
| Semantic-message authority, optional RichPresentation schema/compiler/bindings/fallbacks, citations and exact Artifact deliverables | [`RICH-RESPONSE.md`](RICH-RESPONSE.md), [`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md), [`DATA-MODEL.md`](DATA-MODEL.md), [`SCHEMAS.md`](SCHEMAS.md), [`EVENTS.md`](EVENTS.md), [`API.md`](API.md), [`STORAGE.md`](STORAGE.md), ADR [0022](adr/0022-rich-presentation-is-optional.md) |
| Built-in zero-authority HostSkills and negotiated instruction delivery | [`HOST-GUIDANCE.md`](HOST-GUIDANCE.md), [`AGENT-FABRIC.md`](AGENT-FABRIC.md), [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md), ADR [0023](adr/0023-host-guidance-is-not-capability-authority.md) |
| Coworker/Home/Work/Subagents onboarding and cost/quota/error/mobile projection | [`EXPERIENCE.md`](EXPERIENCE.md), [`RESPONSIBILITIES.md`](RESPONSIBILITIES.md) |
| Deduplicated Needs You inbox projection and actions | [`EXPERIENCE.md`](EXPERIENCE.md), [`SERVICES.md`](SERVICES.md), [`API.md`](API.md) |
| Component anatomy, tokens and accessibility | [`DESIGN-SYSTEM.md`](DESIGN-SYSTEM.md) |
| Semantic motion and timing | [`MOTION.md`](MOTION.md) |
| Presentation Runtime item model, RendererRegistry, rich snapshot/stream/publication lifecycle, artifact/context rendering | [`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md), [`RICH-RESPONSE.md`](RICH-RESPONSE.md), [`HOST-GUIDANCE.md`](HOST-GUIDANCE.md), [`API.md`](API.md), [`SERVICES.md`](SERVICES.md), [`DESIGN-SYSTEM.md`](DESIGN-SYSTEM.md), [`MOTION.md`](MOTION.md) |
| Worker/profile/Goal/Suggestion/takeover motion triggers | [`MOTION.md`](MOTION.md), [`EVENTS.md`](EVENTS.md), [`STATE-MACHINES.md`](STATE-MACHINES.md) |
| Logs, metrics, traces, audit and SLO categories | [`OBSERVABILITY.md`](OBSERVABILITY.md) |
| Threat model and mandatory controls | [`SECURITY.md`](SECURITY.md) |
| Egress/SSRF, Gateway authentication, IPC, file identity, archive and MCP App isolation | [`NETWORK-SECURITY.md`](NETWORK-SECURITY.md), [`SECURITY.md`](SECURITY.md) |
| Test layers and conformance gates | [`TESTING.md`](TESTING.md) |
| Real-world acceptance scenarios | [`BENCHMARKS.md`](BENCHMARKS.md) |
| Module layout, dependency direction, build stages | [`IMPLEMENTATION.md`](IMPLEMENTATION.md) |
| Source-by-source comparison/disposition | [`SOURCE-RECONCILIATION.md`](SOURCE-RECONCILIATION.md) |
| Decision rationale | [`adr/`](adr/) |
| Implementation stories, Agile delivery, product research, RAG, real-user tests, sources, release gates | [`../implementation/README.md`](../implementation/README.md), [`../implementation/COVERAGE.md`](../implementation/COVERAGE.md) |

## Deliberately deferred decisions

These are visible dependencies, not blanks to fill by guesswork:

- LiteSPM API version, authentication, package/plugin schema, MCP discovery, and
  install/activation lifecycle. The selected base URL is recorded in
  [`CAPABILITY-FABRIC.md`](CAPABILITY-FABRIC.md); LiteSPM owns the contract.
- UI component-library choice and implementation-specific asset tooling. The v1 color,
  type, spacing, shape, layout, component-state, responsive, and accessibility contracts
  are fixed in [`DESIGN-SYSTEM.md`](DESIGN-SYSTEM.md); an implementation may change them
  only through a reviewed design-system update that preserves documented contrast and
  interaction guarantees.
- Exact numeric SLO targets until hardware and representative workloads are measured.
- Concrete managed-cloud vendor and storage provider.
- Final Operator transport implementation (the logical API contract is transport
  independent; HTTP/OpenAPI and local IPC map to the same commands).
- Concrete database access library and Rust crate boundaries before the first vertical
  slice.

Changing entity meaning, transition ownership, protocol boundaries, Core ownership,
event/effect guarantees, lease semantics, or the threat model requires an architecture
change and ADR. Choosing an implementation that preserves those contracts does not.
