# 004 - Agent Relay on One Conversation

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-13 (current-evidence crosswalk only; target revision remains 1.0)
**Implementation state (source cross-check 2026-11-13):** Partial CODE/SERVED only. Desktop parsing/rendering preserves producer binding/session identity; the picker filters out providers with no dispatch arm but does not gate on fresh descriptor/auth state. Portable context carries producer identity, excludes SYSTEM_NOTICE/native handles and has an A→B→A builder test. The storage loader fetches at most 1,024 message rows, but first scans the Conversation for orphan messages and runs COUNT/SUM queries over the full committed prefix, so bounded-history AC-011 is not yet proven. Feature 001 materializes eligible text ResourceRefs at dispatch; Workspace instruction preamble and canonical ArtifactVersionRef inputs are absent from the current Conversation prompt path. Feature 004 still lacks the relay refusal/truncation receipt and adapter-budget integration. No qualified second provider, SQLite-to-desktop A→B→A journey, or recovery qualification exists. See TASKS.md, docs/IMPLEMENTATION-STATUS.md and the [2026-11-13 source cross-check](../../docs/audit/FEATURE-004-CURRENT-CROSSCHECK-2026-11-13.md).
**Owners:** Conversation, Agent Runtime, Product. Accepted by the repository owner via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md).

## User outcome

On one Conversation the owner runs a committed turn on agent A, a later turn on agent B that sees only portable context, then returns to A without copying private transcripts or credentials.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. B-15 and B-16 are owner-accepted (2026-11-09) and implementation is authorized. Today the supervisor has one dispatch arm (OpenCode), no Claude Code arm, and the desktop retains producer identity in `parseConversationSnapshot` and renders it on committed AGENT messages; a portable-context unit test now proves A→B→A producer attribution and excludes native session handles. There is still no selectable, qualified A→B→A provider pair (SPEC §4). The implementation boundary is mapped in docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

## Lifecycle, scope and dependencies

- Lifecycle: ACCEPTED/LOCKED (2026-11-09). IMPLEMENTING when the first TASKS.md task starts; VERIFIED only through the final task with owner sign-off.
- Relay pair: agent A OpenCode v2.0.32, agent B Claude Code CLI. Release matrix: Windows x64 first, Linux x64 second.
- Depends on [Feature 001](../001-conversation-session-supervisor/SPEC.md) (turn admission, settlement, recovery), [Feature 002](../002-native-agent-install-auth-binding/SPEC.md) (OpenCode binding fix, descriptor, Claude Code module) and [Feature 003](../003-workspace-roots-context-ingestion/SPEC.md) (Resource materialization). Delivery wave 4 (gate E).

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Specification standard](../../docs/SPECIFICATION-STANDARD.md). Domain narrative: [RESEARCH.md](RESEARCH.md).
