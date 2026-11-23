# Plan - Workbench Browser Terminal and Computer

> **Revision 1.1 scope revision (2026-11-12): IN_REVIEW / TARGET.** The user explicitly requested whole-laptop Computer observation and immediate manual takeover, rejecting the earlier browser-only substitution. Revision 1.0 owner acceptance (2026-11-09) and B-32 deferral below are historical authority for that revision; they are not acceptance of this extension. Implementation remains unchanged and Computer remains UNAVAILABLE. The revised owner design is [Computer observation and control LLD](../../docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md). No new provider, transport, route or qualification is approved by this documentation pass.

**Feature:** 008 | **Status:** IN_REVIEW (revision 1.1); revision 1.0 ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Spec revision:** 1.1 IN_REVIEW (1.0 historical baseline)

## Phases

1. SPEC section 5 blockers B-22, B-23 and B-32 are ACCEPTED (2026-11-09). Workbench truthfulness baseline (A-51) is verified in source; finish Activity fallthrough and the Computer UNAVAILABLE surface.
2. Promote accepted browser, terminal, Activity and Diff interfaces in the canonical owner docs and `docs/schemas/` (OpenAPI, events, error codes, allocated immutable `sqlite-v42.sql`) in one reviewed change.
3. Browser lane (Windows x64 row first): storage, managed Chromium provider, service/routes/bridge, UI, recovery.
4. Terminal lane (Linux x64 row first, after Feature 005 admits a bubblewrap Environment): storage, discovery and PTY, service/routes/bridge, UI, recovery. Windows ConPTY waits on Feature 005 B-35.
5. Activity and Diff lanes (independent of providers): read projection and exact-revision diff.
6. Cross-layer traces, packaged E2E-08 qualification on each row, documentation sync, VERIFIED transition.

The ordered task list is [TASKS.md](TASKS.md).

## Dependency DAG

- T01 (contract promotion) blocks every implementation task except T02 (UI truthfulness) and T19 (the source-only iframe sandbox correction); the product owner delegated T19's pre-provider sequencing on 2026-11-19 because the unsafe preview already exists and T19 does not mount or authorize Browser operations.
- Browser T03-T07, terminal T08-T13, Activity T14 and Diff T15 are independent lanes; T16-T18 close the package, T19 is the independent pre-provider iframe safety task, and T20 adds the user-invoked Git worktree/GitHub PR surface.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Desktop shell, Environments, Browser provider, Trust, Desktop UI.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

Browser session/lease/receipt and terminal session/receipt records need one new immutable SQLite migration (`docs/schemas/sqlite-v42.sql` per the Migration allocation in docs/LITECOWORK-BUILD-TASKS.md; v34 was wired in the 2026-11-20 checkout and v35 is now wired; take the next free number if another package merges first), promoted by T01 and implemented by T03/T08. Activity and Diff add no tables. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Implementing a provider before its contract is promoted (T01).
- Claiming terminal qualification on Windows before Feature 005 B-35, or from WSL2.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner acceptance recorded 2026-11-09
- TASKS.md commands and denial, replay, restart and cancel cases
- Packaged E2E-08 on each release-matrix row

## DeepSeek Harness proposals (PROPOSED, owner review required)

PROPOSED only; see the matching section in [TASKS.md](TASKS.md). Nothing here changes the accepted design of this package. Owner review is required first.

- **P-45 Default-deny webview and session permissions, including the main one, plus a guest network filter** (touches this package; row owned by package 019). Class **ADAPT**. Design impact if accepted: T06 of package 019; T04 and T06 of package 008. Deferral: Needs a packaged webview run on the Windows laptop and native Linux; DEFERRED with B-43. Upstream: [src/browser-guests.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/browser-guests.ts), [src/microphone-permissions.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/microphone-permissions.ts), [src/policy-test-auth.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/policy-test-auth.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 9 and 14 (row P-45).


## Revision 1.1 phases and gates (IN_REVIEW)

T22 (review attachment, permits, B-32 revision and terminal scope) -> T23 (same-change owner prose + API/event/error/migration promotion) -> T24 (observation-only real laptop capture) -> T25 (separately qualified mediated input). T23 -> T26 (compact object tabs, +, Canvas and terminal defaults/input); terminal execution still depends on T10/T11 and existing T09/T12 platform blockers. T24/T25/T26 -> T27 (full trace and packaged native qualification). No new migration allocation is assumed; check next free number at promotion. Capture/input provider and transport choices in the Computer LLD are recommendations requiring measured spikes, not owner-approved dependencies.

The 2026-11-14 universal-workspace refinement stays in the T22/T26 review lane: compact left navigation, contextual object tabs, searchable object discovery, truthful per-owner readiness and reauthorization on reopen. It creates no domain entity or public API by itself. External-app handoff, ACP subagent exposure and additional provider families require their own owner decision/qualification; none is inferred from this UI shell.

The 2026-11-17 Browser Settings refinement also stays in T22 review. The Browser & Computer category is target navigation, not a mounted screen. Before promotion, T22 must resolve its cross-feature dependency on Features 001/002 for provider-specific proof of LiteCowork-mediated Browser-tool provisioning; ACP alone is insufficient. Browser permission display must report the owning service's effective outcome and contributing readiness/policy/OS reasons without UI-invented precedence. Existing-Chrome attachment remains a separate per-session consent path, and staged profile import/cleanup must cross a durable owner intent boundary before external provider effects. No route, schema, migration, or Settings control is promoted by this plan update.

Roll out observation-only before agent input; feature gates distinguish local-view READY, agent-disclosure readiness and input readiness. Rollback revokes permits/fences input and stops capture, preserving durable ambiguous Effect/receipt records; never replay commands or revive old streams. Desktop observation is independent of B-35, but does not remove any Task/agent terminal isolation gate. Exact Windows laptop qualification comes first for Computer investigation, while existing Linux-first terminal qualification is unchanged. Do not count WSL as either native desktop row.

Required evidence includes picker cancel/denial, source removal, privacy pause, USER-owner local view, expired disclosure permit, slow consumer/over-bound memory, physical mouse/key interference, immediate takeover under load, takeover while the target app is foreground and LiteCowork is backgrounded, shortcut conflict/registration loss, stale geometry, partial injection, two-window/Workspace input conflict, timeout-after-dispatch, restart/reconnect and explicit Return. If no out-of-app takeover trigger is qualified on an OS/provider row, that row remains observation-only. Reconcile the old Computer-unavailable release row only with recorded owner review.
