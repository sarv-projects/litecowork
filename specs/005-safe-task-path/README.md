# 005 - Safe Task Path

**Status:** Revision 1.0 ACCEPTED/LOCKED; revision 1.1 IN_REVIEW with user-delegated implementation direction; neither status is whole-package verification or qualification.
**Revision:** 1.1 proposal | **Updated:** 2026-11-23
**Implementation state (2026-11-23):** Task create/read, spec revision, planning readiness and presentation are served. Feature 005-T12 adds a CODE/SERVED Conversation-to-Task path through the existing create command/route; it saves a READY Task with exact existing source-message provenance and no Attempt. Feature 001-T31 supplies same-snapshot owning-turn status and is DONE for its declared CODE/SERVED scope. Focused native-Windows storage, daemon parsing, Tauri request-payload, UI eligibility/cache, Conversation projection/parser, TypeScript, API and structural documentation checks pass (exact commands and counts in TASKS.md and CURRENT-RUN.md). The ambiguous-save cache remains process-local, so authenticated packaged UI, restart recovery and full acceptance remain open. T02 has consolidated shared Environment types and trust-checked Bubblewrap discovery (CODE only; it does not provision or attest a Task Environment). T06 has a bounded, handle-confined B-18 file replacement primitive with focused native Windows and WSL Linux CODE tests; its Linux POSIX ACL-copy branch still lacks capable-filesystem evidence, and no Task dispatch calls the primitive. A complete admitted Task execution path is not implemented. T14 has a partial Windows UNAVAILABLE implementation; T16 (B-35), T17 and T18 remain owner-blocked/deferred. T01 is IN PROGRESS under owner-directed A-70/A-87/A-88/A-90 implementation; cross-owner review remains pending. Migration v33 is wired for the durable AgentSession Environment binding and scope guards. The Feature 005 lane still has no attestation-producing provider or wired Task execution admission. See TASKS.md and docs/IMPLEMENTATION-STATUS.md.
**Owners:** Task Runtime, Environments, Trust, Storage, Desktop UI. Accepted by the repository owner via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md).

## User outcome

The owner may explicitly promote a completed Conversation exchange into a Task, accepts an immutable plan, and one isolated Attempt edits one approved file with a receipt that survives cancel and restart.

## Current truth

The active checkout contains the Rust daemon, Tauri desktop application and shared crates. Feature 005 revision 1.0 is ACCEPTED/LOCKED (B-17 and B-18 accepted 2026-11-09); owner-directed revision 1.1 is IN_REVIEW pending cross-owner review, with implementation authorized while that review proceeds. The A-115 promotion direction likewise remains pending cross-owner review. See [TASKS.md](TASKS.md) for per-task evidence. Task execution qualifies first on the Linux x64 row and stays visibly UNAVAILABLE on the Windows x64 row until the PROPOSED Windows Environment provider B-35 is owner-accepted and qualifies. Its current implementation boundary is mapped in docs/IMPLEMENTATION-STATUS.md; do not infer qualification from a matching route, type or screen.

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
