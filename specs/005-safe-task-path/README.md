# 005 - Safe Task Path

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-12
**Implementation state (2026-11-18):** Partial foundations are served for Task create/read, plan acceptance storage, planning readiness and presentation. T02 has consolidated shared Environment types and trust-checked Bubblewrap discovery (CODE only; it does not provision or attest a Task Environment). T06 now has a bounded, handle-confined B-18 file replacement primitive with focused native Windows and WSL Linux CODE tests; its Linux POSIX ACL-copy branch still lacks capable-filesystem evidence, and no Task dispatch calls the primitive. A complete admitted Task execution path is not implemented. T14 has a partial Windows UNAVAILABLE implementation; T16 (B-35), T17 and T18 remain owner-blocked/deferred. T01 is IN PROGRESS under the owner's direction to implement the revision 1.1 resolutions for A-70/A-87/A-88/A-90; cross-owner review remains pending. Migration v33 is wired in source for the durable AgentSession Environment binding and scope guards; the latest focused `v33` filter passes 3/3, including a valid legacy-row upgrade case. The full storage suite passed 592 unit tests plus 5 integration tests (1 native Credential Manager test ignored) before the final test-only fixture refinement; production code was unchanged. The Feature 005 lane still has no attestation-producing provider or wired Task admission. See TASKS.md and docs/IMPLEMENTATION-STATUS.md.
**Owners:** Task Runtime, Environments, Trust, Storage, Desktop UI. Accepted by the repository owner via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md).

## User outcome

The owner promotes one Conversation outcome into a Task, accepts an immutable plan, and one isolated Attempt edits one approved file with a receipt that survives cancel and restart.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. Feature 005 revision 1.0 is ACCEPTED/LOCKED (B-17 and B-18 accepted 2026-11-09); owner-directed revision 1.1 is IN_REVIEW pending cross-owner review, with implementation authorized while that review proceeds. See [TASKS.md](TASKS.md) for per-task evidence. Task execution qualifies first on the Linux x64 row and stays visibly UNAVAILABLE on the Windows x64 row until the PROPOSED Windows Environment provider B-35 is owner-accepted and qualifies. Its current implementation boundary is mapped in docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

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
