# Architecture and contract coverage audit

This index connects the current architecture sources to implementation stories. The
primary story is a planning guardian; it does not claim that a story implements every
cross-cutting rule in a document. Linked flows, schemas, failure cases, and related
domain owners still apply. Story status tracks planning through owner acceptance; coverage is not implementation evidence.

## Coverage inventory

- 79 source Markdown documents; 1096 heading-level sections are digest-pinned.
- 35 implementation-plan documents and 212 plan sections are indexed; the mutable current-run handoff is intentionally excluded.
- 60 backlog stories with status recorded in backlog; each has CODE/SYSTEM/USER test case IDs.
- 137 flows and 90 benchmarks.
- 195 OpenAPI operations and 315 component schemas; 0 components are unreachable from every Operator operation.
- 99 error codes; 251 shared IDs/value definitions.
- 140 event types and 139 typed event payload schemas.
- 107 SQLite tables, 1172 columns, 475 foreign-key columns, 68 unique constraints, 348 checks, 1 view(s), 140 triggers, and 76 indexes.

Machine-object names and canonical digests are compared to current source contracts by
`validate_implementation_coverage.py`. This detects inventory drift; it does not prove
that a proposed implementation has correct behavior. Story-level CODE, SYSTEM, and
USER tests provide that evidence when executed.

## Architecture document owners

| Document | Planning guardian | Classification |
|---|---|---|
| [`AGENTS.md`](../AGENTS.md) | E01-S01 | Repository development rules |
| [`ARCHITECTURE.md`](../ARCHITECTURE.md) | E01-S01 | Product architecture authority |
| [`CONTRIBUTING.md`](../CONTRIBUTING.md) | E01-S01 | Contribution workflow |
| [`GLOSSARY.md`](../GLOSSARY.md) | E01-S01 | Canonical vocabulary |
| [`README.md`](../README.md) | E02-S01 | Product overview |
| [`apps/litecowork-ui/README.md`](../apps/litecowork-ui/README.md) | E02-S01 | Architecture contract |
| [`apps/litecowork-ui/src/artifacts/README.md`](../apps/litecowork-ui/src/artifacts/README.md) | E08-S07 | Architecture contract |
| [`apps/litecowork-ui/src/presentation/README.md`](../apps/litecowork-ui/src/presentation/README.md) | E08-S06 | Architecture contract |
| [`assets/host-instructions/presentation-policy.md`](../assets/host-instructions/presentation-policy.md) | E08-S08 | Architecture contract |
| [`assets/host-skills/rich-response-design/SKILL.md`](../assets/host-skills/rich-response-design/SKILL.md) | E08-S08 | Architecture contract |
| [`capabilities/zip_intake/README.md`](../capabilities/zip_intake/README.md) | E06-S01 | Architecture contract |
| [`crates/domain-responsibility/README.md`](../crates/domain-responsibility/README.md) | E08-S02 | Architecture contract |
| [`docs/AGENT-FABRIC.md`](../docs/AGENT-FABRIC.md) | E03-S05 | Architecture contract |
| [`docs/API.md`](../docs/API.md) | E01-S04 | Architecture contract |
| [`docs/ARTIFACTS-EVIDENCE.md`](../docs/ARTIFACTS-EVIDENCE.md) | E04-S05 | Architecture contract |
| [`docs/AUTOMATION.md`](../docs/AUTOMATION.md) | E09-S02 | Architecture contract |
| [`docs/BENCHMARKS.md`](../docs/BENCHMARKS.md) | E13-S03 | Architecture contract |
| [`docs/CAPABILITY-FABRIC.md`](../docs/CAPABILITY-FABRIC.md) | E04-S02 | Architecture contract |
| [`docs/CAPABILITY-INVOCATIONS.md`](../docs/CAPABILITY-INVOCATIONS.md) | E04-S03 | Architecture contract |
| [`docs/CHANNELS.md`](../docs/CHANNELS.md) | E11-S04 | Architecture contract |
| [`docs/COMPETITIVE-RESEARCH.md`](../docs/COMPETITIVE-RESEARCH.md) | E08-S02 | Architecture contract |
| [`docs/CONTEXT.md`](../docs/CONTEXT.md) | E08-S03 | Architecture contract |
| [`docs/COVERAGE-MATRIX.md`](../docs/COVERAGE-MATRIX.md) | E01-S01 | Architecture contract |
| [`docs/DATA-MODEL.md`](../docs/DATA-MODEL.md) | E01-S02 | Architecture contract |
| [`docs/DELEGATION.md`](../docs/DELEGATION.md) | E05-S04 | Architecture contract |
| [`docs/DESIGN-SYSTEM.md`](../docs/DESIGN-SYSTEM.md) | E08-S05 | Architecture contract |
| [`docs/ENVIRONMENTS.md`](../docs/ENVIRONMENTS.md) | E07-S01 | Architecture contract |
| [`docs/EVENTS.md`](../docs/EVENTS.md) | E01-S02 | Architecture contract |
| [`docs/EXPERIENCE.md`](../docs/EXPERIENCE.md) | E08-S04 | Architecture contract |
| [`docs/FAILURE-RECOVERY.md`](../docs/FAILURE-RECOVERY.md) | E13-S02 | Architecture contract |
| [`docs/FLOWS.md`](../docs/FLOWS.md) | E13-S03 | Architecture contract |
| [`docs/HOST-GUIDANCE.md`](../docs/HOST-GUIDANCE.md) | E08-S08 | Architecture contract |
| [`docs/IMPLEMENTATION.md`](../docs/IMPLEMENTATION.md) | E01-S01 | Architecture contract |
| [`docs/LOCAL-OPERATOR-IPC.md`](../docs/LOCAL-OPERATOR-IPC.md) | E01-S04 | Architecture contract |
| [`docs/MOTION.md`](../docs/MOTION.md) | E08-S05 | Architecture contract |
| [`docs/NETWORK-SECURITY.md`](../docs/NETWORK-SECURITY.md) | E04-S02 | Architecture contract |
| [`docs/OBSERVABILITY.md`](../docs/OBSERVABILITY.md) | E13-S02 | Architecture contract |
| [`docs/PRESENTATION-RUNTIME.md`](../docs/PRESENTATION-RUNTIME.md) | E08-S06 | Architecture contract |
| [`docs/PRODUCT.md`](../docs/PRODUCT.md) | E08-S01 | Architecture contract |
| [`docs/PROTOCOLS.md`](../docs/PROTOCOLS.md) | E01-S04 | Architecture contract |
| [`docs/RESPONSIBILITIES.md`](../docs/RESPONSIBILITIES.md) | E08-S02 | Architecture contract |
| [`docs/RICH-RESPONSE.md`](../docs/RICH-RESPONSE.md) | E08-S08 | Architecture contract |
| [`docs/ROUTINES.md`](../docs/ROUTINES.md) | E09-S01 | Architecture contract |
| [`docs/RUNTIME-LIFECYCLE.md`](../docs/RUNTIME-LIFECYCLE.md) | E01-S03 | Architecture contract |
| [`docs/RUNTIME-MESH.md`](../docs/RUNTIME-MESH.md) | E11-S03 | Architecture contract |
| [`docs/SCHEMAS.md`](../docs/SCHEMAS.md) | E01-S04 | Architecture contract |
| [`docs/SECURITY.md`](../docs/SECURITY.md) | E04-S01 | Architecture contract |
| [`docs/SERVICES.md`](../docs/SERVICES.md) | E01-S04 | Architecture contract |
| [`docs/SOURCE-RECONCILIATION.md`](../docs/SOURCE-RECONCILIATION.md) | E01-S01 | Architecture contract |
| [`docs/STATE-MACHINES.md`](../docs/STATE-MACHINES.md) | E01-S02 | Architecture contract |
| [`docs/STORAGE.md`](../docs/STORAGE.md) | E01-S02 | Architecture contract |
| [`docs/TASK-RUNTIME.md`](../docs/TASK-RUNTIME.md) | E03-S04 | Architecture contract |
| [`docs/TESTING.md`](../docs/TESTING.md) | E13-S03 | Architecture contract |
| [`docs/TRUST.md`](../docs/TRUST.md) | E04-S01 | Architecture contract |
| [`docs/WORLD-RESOURCES.md`](../docs/WORLD-RESOURCES.md) | E02-S03 | Architecture contract |
| [`docs/adr/0001-durable-task-core.md`](../docs/adr/0001-durable-task-core.md) | E03-S04 | Architecture decision record |
| [`docs/adr/0002-task-and-attempt-identity.md`](../docs/adr/0002-task-and-attempt-identity.md) | E03-S04 | Architecture decision record |
| [`docs/adr/0003-litespm-is-independent.md`](../docs/adr/0003-litespm-is-independent.md) | E04-S04 | Architecture decision record |
| [`docs/adr/0004-runtime-and-environment-are-distinct.md`](../docs/adr/0004-runtime-and-environment-are-distinct.md) | E07-S01 | Architecture decision record |
| [`docs/adr/0005-replicate-domain-state-not-databases.md`](../docs/adr/0005-replicate-domain-state-not-databases.md) | E11-S02 | Architecture decision record |
| [`docs/adr/0006-conversation-task-separation.md`](../docs/adr/0006-conversation-task-separation.md) | E03-S02 | Architecture decision record |
| [`docs/adr/0007-reported-observed-verified.md`](../docs/adr/0007-reported-observed-verified.md) | E04-S05 | Architecture decision record |
| [`docs/adr/0008-no-generic-provider-layer.md`](../docs/adr/0008-no-generic-provider-layer.md) | E03-S01 | Architecture decision record |
| [`docs/adr/0009-attempt-free-lead-planning.md`](../docs/adr/0009-attempt-free-lead-planning.md) | E03-S03 | Architecture decision record |
| [`docs/adr/0010-revision-independent-occurrences.md`](../docs/adr/0010-revision-independent-occurrences.md) | E09-S02 | Architecture decision record |
| [`docs/adr/0011-atomic-plan-acceptance.md`](../docs/adr/0011-atomic-plan-acceptance.md) | E03-S03 | Architecture decision record |
| [`docs/adr/0012-native-harness-integrity.md`](../docs/adr/0012-native-harness-integrity.md) | E03-S05 | Architecture decision record |
| [`docs/adr/0013-host-delegation-uses-accepted-plan-steps.md`](../docs/adr/0013-host-delegation-uses-accepted-plan-steps.md) | E05-S02 | Architecture decision record |
| [`docs/adr/0014-versioned-worker-profiles-and-bounded-selection.md`](../docs/adr/0014-versioned-worker-profiles-and-bounded-selection.md) | E05-S01 | Architecture decision record |
| [`docs/adr/0015-warmth-is-operational-and-sharing-is-separate.md`](../docs/adr/0015-warmth-is-operational-and-sharing-is-separate.md) | E07-S03 | Architecture decision record |
| [`docs/adr/0016-coworker-goal-and-suggestion-authority-boundary.md`](../docs/adr/0016-coworker-goal-and-suggestion-authority-boundary.md) | E08-S02 | Architecture decision record |
| [`docs/adr/0017-deadline-sensitive-is-best-effort.md`](../docs/adr/0017-deadline-sensitive-is-best-effort.md) | E07-S03 | Architecture decision record |
| [`docs/adr/0018-context-content-is-resource-backed-and-provider-pluggable.md`](../docs/adr/0018-context-content-is-resource-backed-and-provider-pluggable.md) | E08-S03 | Architecture decision record |
| [`docs/adr/0019-credential-egress-and-audit-boundaries.md`](../docs/adr/0019-credential-egress-and-audit-boundaries.md) | E04-S01 | Architecture decision record |
| [`docs/adr/0020-runtime-identity-is-installation-scoped.md`](../docs/adr/0020-runtime-identity-is-installation-scoped.md) | E01-S03 | Architecture decision record |
| [`docs/adr/0021-device-signing-identity.md`](../docs/adr/0021-device-signing-identity.md) | E01-S03 | Architecture decision record |
| [`docs/adr/0022-rich-presentation-is-optional.md`](../docs/adr/0022-rich-presentation-is-optional.md) | E08-S08 | Architecture decision record |
| [`docs/adr/0023-host-guidance-is-not-capability-authority.md`](../docs/adr/0023-host-guidance-is-not-capability-authority.md) | E08-S08 | Architecture decision record |
| [`docs/adr/README.md`](../docs/adr/README.md) | E01-S01 | Architecture decision record |

## Not covered by this plan yet

1. **Product behavior is not implemented or qualified.** The backlog is planned work. No
   code, provider, cloud deployment, remote Runtime, platform installer, or real-user
   acceptance is claimed by this coverage audit.
2. **A digest is not a semantic test.** OpenAPI component digests catch any source change,
   but field-level required/optional/validation behavior must still be asserted in the
   owning story's executable contract tests. The same applies to narrative requirements
   and cross-domain policy interactions.
3. **Cross-cutting scenarios have one primary guardian in the CSV.** A flow can exercise
   several epics; the guardian is not its only implementation owner. Before coding, the
   story review must identify each affected domain, transition, event, authorization
   decision, failure path, projection, and test. The map does not encode a complete
   many-to-many requirement-to-story relation.
4. **External provider behavior remains conditional.** Claude/Codex/OpenCode/Cline, local
   model servers, browsers, OS integrations, cloud credentials, and package services
   require real qualification in the target environment. Mock results cannot close those
   gates. LiteSPM behavior is limited to its selected documented authority; unknown
   package API details are deliberately not invented.
5. **Not every machine-readable field is an independent CSV row.** SQLite fields and
   key constraints are enumerated; OpenAPI schema properties and nested constraints
   are digest-pinned as whole component schemas. Stories must expand relevant properties
   and negative cases into executable assertions rather than treating inventory as
   implementation evidence.
6. **Some architecture is intentionally outside v1.** Team/RBAC, native mobile/web clients,
   voice, general marketplace/template sharing, and future messaging gateways remain
   deferred. Their architecture docs are indexed, but they are not V1 acceptance claims.

## Explicit scope boundaries

The release order is desktop/local first, cloud after the desktop feature gate, then
remote Runtime. The owner is the only product reviewer; coding agents do not replace
owner acceptance. Messaging gateway integrations are architecture references for later
work and are not required for desktop/cloud/remote v1.

OpenClaw is a research reference for deployment patterns and possible future messaging
gateways. LiteCowork's contracts remain authoritative for Task state, permissions,
Effects, evidence, fencing, and channel identity.

## Using the map

Read [the plan](README.md), [the coverage CSV](coverage.csv), and the owning architecture
document before changing a domain. Refresh generated inventory only after reviewing the
source diff with `python3 scripts/validate_implementation_coverage.py --write`; default
validator mode is read-only and must pass in CI.
