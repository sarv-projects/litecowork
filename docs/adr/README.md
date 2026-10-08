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
