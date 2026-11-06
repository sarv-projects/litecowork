# Tasks - Agent Relay on One Conversation

**Feature:** 004 | **Status:** DRAFT - do not execute before SPEC is ACCEPTED/LOCKED.

**Spec revision:** 0.2

## 004-T01 - Lock portable context contract

- **Status:** BLOCKED
- **Depends on:** NONE
- **Owner:** Conversation and Agent Runtime
- **Allowed paths:** this package
- **Contract:** SPEC §5
- **Change intent:** Resolve B-15 and B-16.
- **Acceptance:** Conversation and Agent Runtime owners approve B-15's portable-context schema, message/resource/artifact reference policy and size/retention bounds; Feature 002 owner records the exact second AgentModule ID, OS/version/auth route and qualification evidence for B-16. No schema or provider identity is inferred from current implementation constants or probe code.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 004-T02 - Export portable context

- **Status:** NOT STARTED
- **Depends on:** T01
- **Owner:** Conversation
- **Allowed paths:** context export
- **Contract:** SPEC LLD
- **Change intent:** Build the export filter and tests for secret exclusion.
- **Acceptance:** AC-002. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked portable_context_export`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 004-T03 - Admit relay turns on one Conversation

- **Status:** NOT STARTED
- **Depends on:** T02
- **Owner:** Agent Runtime
- **Allowed paths:** supervisor turn admission
- **Contract:** 001 supervisor plus this SPEC
- **Change intent:** After T01 is accepted, execute A→B→A on one Conversation using a new durable Conversation AgentSession per turn and a distinct qualified native adapter for B; reconcile Feature 001 admission/settlement dependencies first.
- **Acceptance:** AC-001, AC-003, AC-004, AC-006. Criteria discussed include AC-001 AC-002.
- **Command:** `cargo test -p litecoworkd --locked conversation_agent_relay`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.

## 004-T04 - Show per-turn producer

- **Status:** NOT STARTED
- **Depends on:** T03
- **Owner:** Desktop UI
- **Allowed paths:** conversation transcript
- **Contract:** ui-behavior
- **Change intent:** Render producer without implying shared native memory.
- **Acceptance:** AC-005. Criteria discussed include AC-001 AC-002.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Stop:** stop if a blocker in SPEC section 5 is unresolved or a contract change lacks owner promotion.
