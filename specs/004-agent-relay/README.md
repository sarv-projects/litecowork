# 004 - Agent Relay on One Conversation

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.2 | **Updated:** 2026-11-06
**Owners to approve:** Conversation, Agent Runtime, Product.

## User outcome

On one Conversation the owner runs a committed turn on agent A, a later turn on agent B that sees only portable context, then returns to A without copying private transcripts or credentials.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. This package remains DRAFT and does not authorize implementation until owner decisions and supporting contracts are accepted. Its current implementation boundary is mapped in docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

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
