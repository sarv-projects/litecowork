# 008 - Workbench Browser Terminal and Computer

> **Revision 1.1 scope revision (2026-11-12): IN_REVIEW / TARGET.** The user explicitly requested whole-laptop Computer observation and immediate manual takeover, rejecting the earlier browser-only substitution. Revision 1.0 owner acceptance (2026-11-09) and B-32 deferral below are historical authority for that revision; they are not acceptance of this extension. Implementation remains unchanged and Computer remains UNAVAILABLE. The revised owner design is [Computer observation and control LLD](../../docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md). No new provider, transport, route or qualification is approved by this documentation pass.

**Status:** IN_REVIEW (revision 1.1); revision 1.0 ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.1 IN_REVIEW (1.0 accepted history below) | **Updated:** 2026-11-12 | **Owner acceptance:** repository owner, 2026-11-09, via [OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)
**Implementation state (2026-11-12 review):** Partial CODE only. T02 truthfulness slice is partial: Activity has an explicit empty state, Computer is status-only UNAVAILABLE, and Terminal names the Windows/Linux Environment blocker; the task's full UI suite passed 633/633 at its recorded revision. No BrowserSession, browser stream/provider, PTY, real Activity source, or Diff provider is served. T09 is BLOCKED on a Feature 005 Linux Environment; T12 is BLOCKED on B-35 (Windows); migration v41 is allocated subject to rechecking the next free number at promotion. A-83 also blocks terminal receipt promotion/Task evidence pending an owner decision. The 2026-11-11 browser-only refinement is historical and superseded as requested scope by revision 1.1; actual laptop Computer observation/control is specified for review. Browser takeover remains separate from Conversation Take control. No Computer route/provider/UI control exists; this design is not implementation or qualification evidence.
**Component owners:** Desktop shell, Environments, Browser provider, Trust, Desktop UI.

## User outcome

The owner uses the right Workbench to finish real browser and terminal work, takes over and returns control, and a stale handle cannot act.

## Current truth

The active Workbench Files pane is a Resource projection/preview, not a live filesystem. Browser is an empty-by-default embedded iframe preview labelled as such, not Chrome or a browser-control provider. After register A-51 the Terminal pane (`TerminalPane`) and Review pane (`ReviewPane`) in `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx` render honest unavailable notices with no fabricated output, exit codes or containment labels. Activity now has a dedicated empty state and tab; Computer capture/input remains unavailable and the Workbench presents status text only. No BrowserSession, PTY, Activity, Diff or Computer route/provider exists.

Accepted decisions (2026-11-09): B-22 `MANAGED_CHROMIUM` with an isolated profile, ambient-CDP discovery and default-profile copy disabled; B-23 one daemon-owned PTY/ConPTY per tab under an admitted Environment with ordered receipts and timeout-as-receipt, qualifying on the Linux x64 row first and visibly UNAVAILABLE on Windows until Feature 005 B-35; B-32 Computer use deferred and explicitly UNAVAILABLE. Revision 1.0 adds the Workbench Activity view and exact-revision/VCS Diffs to scope. The V1 plan is in [TASKS.md](TASKS.md).

By owner approval on 2026-11-09 (see [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)), this package folds proposed tasks P-06 (008-T19, remove `allow-same-origin` from the Browser preview iframe and add a guard test; audit finding U-02) and P-13 (008-T20, Git worktree view, VCS diff and user-invoked GitHub PR ops behind [Feature 006](../006-golden-connection-capability-write/SPEC.md)). Sources: [docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](../../docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md) section 9 and [docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8.

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

## Requested revision 1.1 target

The Computer tab must show the real selected laptop display/window and native apps. Browser remains a separate managed-tab view. Local preview, disclosure to an exact agent and input each have separate authority; manual takeover does not by itself stop local observation. Persistent Take over remains reachable outside captured content and while Canvas/Terminal is selected. Compact object tabs and **+** open authorized views without automatic capture/process spawn. Terminal defaults resolve against actual OS/Runtime and never lend a manual user's host shell to an agent. See SPEC section 20 and new stable tasks T22..T27. This revision is IN_REVIEW; no provider or new acceptance is claimed.
