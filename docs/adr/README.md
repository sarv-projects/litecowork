# Architecture Decision Records

Architecture decisions explain the reason and tradeoffs behind the current contract.
`ARCHITECTURE.md` remains the single current authority: an ADR does not silently amend
it. When an accepted decision changes the architecture, update the authority and the
relevant detailed contract in the same change.

Use stable, never-reused numbers. State, context, decision, consequences, and status.
Superseded decisions remain available with an explicit successor reference.

## Accepted vNext decisions

| ADR | Decision |
|---|---|
| [0001](0001-durable-task-core.md) | Durable Task Core Around External Agents |
| [0002](0002-task-and-attempt-identity.md) | A Task Continues Through New Attempts |
| [0003](0003-litespm-is-independent.md) | Keep LiteSPM Independent |
| [0004](0004-runtime-and-environment-are-distinct.md) | Runtime and Environment Are Separate |
| [0005](0005-replicate-domain-state-not-databases.md) | Replicate Domain State, Not Database Files |
| [0006](0006-conversation-task-separation.md) | Conversation and Task Are Separate |
| [0007](0007-reported-observed-verified.md) | Preserve Evidence Levels |
| [0008](0008-no-generic-provider-layer.md) | Keep Protocol Boundaries Explicit |
| [0009](0009-attempt-free-lead-planning.md) | Initial Planning Does Not Require an Attempt |
| [0010](0010-revision-independent-occurrences.md) | Deduplicate Automation Occurrences Across Revisions |
| [0011](0011-atomic-plan-acceptance.md) | Accept Plans and Materialize Steps Atomically |
| [0012](0012-native-harness-integrity.md) | Preserve native agent harnesses |
| [0013](0013-host-delegation-uses-accepted-plan-steps.md) | Host delegation uses accepted Plan Steps and child-scoped authority |
| [0014](0014-versioned-worker-profiles-and-bounded-selection.md) | Version worker profiles and bound selection |
| [0015](0015-warmth-is-operational-and-sharing-is-separate.md) | Warmth is operational; Environment sharing is a separate scope |
| [0016](0016-coworker-goal-and-suggestion-authority-boundary.md) | Coworkers, Goals, and Suggestions do not own execution authority |
| [0017](0017-deadline-sensitive-is-best-effort.md) | Deadline-sensitive work is best-effort |
| [0018](0018-context-content-is-resource-backed-and-provider-pluggable.md) | Context is Resource-backed and provider-pluggable |
| [0019](0019-credential-egress-and-audit-boundaries.md) | Separate policy, approval, credentials, egress, and audit boundaries |
| [0020](0020-runtime-identity-is-installation-scoped.md) | Runtime identity is installation-scoped; Workspace access uses explicit bindings |
| [0021](0021-device-signing-identity.md) | Device identity uses OS-keystore-backed Ed25519 signing keys |
| [0022](0022-rich-presentation-is-optional.md) | Rich presentation is optional; ConversationMessage remains semantic authority |
| [0023](0023-host-guidance-is-not-capability-authority.md) | Built-in Host guidance is zero-authority context, not a capability |
| [0024](0024-optional-chat-first-coworkers.md) | Coworkers are optional chat-first identities; standing work and automatic memory remain scoped and bounded |
| [0025](0025-agent-module-registry-and-native-configuration.md) | Agent-specific lifecycle/configuration is modular; native auth/options/surface stay agent-owned and LiteSPM bridges cross-agent packages |
