# Tasks - Native Agent Install, Auth and Binding

**Feature:** 002 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.1

## 002-T01 - Lock first agent route and descriptor TTL

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Product and Agent Runtime
- **Allowed paths:** this package and AGENT-CONTROL after owner decision
- **Contract:** SPEC §5
- **Change intent:** Resolve B-11 and B-12 and the shared first-route decision.
- **Acceptance:** Written owner resolution for route, auth method and descriptor TTL. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 002-T02 - Serve registry and lifecycle observations

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Agent Runtime
- **Allowed paths:** apps/litecoworkd agent registry/lifecycle and tests
- **Contract:** AGENT-CONTROL §5, §9
- **Change intent:** Mount observation reads that cannot create bindings.
- **Acceptance:** AC-001 and AC-002 covered by contract tests; secrets absent from payloads. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked agent_registry_observation`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 002-T03 - Commit AgentBinding separately from auth

- **Status:** NOT STARTED
- **Depends on:** T01-T02
- **Owner:** Agent Runtime and Storage
- **Allowed paths:** binding service and sqlite tests
- **Contract:** AGENT-CONTROL §7–§8
- **Change intent:** Enable binding only with a fresh descriptor; auth failure does not wipe it.
- **Acceptance:** AC-003, AC-004 and AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p storage-sqlite --locked agent_binding_auth_split`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 002-T04 - Project registry UI without collapsing state

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** apps desktop agent registry settings
- **Contract:** contracts/ui-behavior.md
- **Change intent:** Render install, auth, binding and readiness as separate chips.
- **Acceptance:** AC-005 and AC-007 visible in UI states. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
