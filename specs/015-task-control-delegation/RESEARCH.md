# Research and Decision Log - Task Control and Heterogeneous Delegation

**Feature:** 015 | **Status:** IN_REVIEW - authored for owner review; does not authorize implementation.
**Checked:** 2026-11-12 against HEAD `64c435149a82ba952c6c3bf3049e26cf2d9344b1` plus shared working-tree changes on `main`; upstream pins remain sourced from reference indexes and were not re-fetched.

## 1. Repository sources inspected

| Source | Path | Finding |
|---|---|---|
| Task runtime | `docs/TASK-RUNTIME.md` | Pause/resume request shapes, ResumePacket fields, steering classes, cancellation order, host delegation admission rules |
| Delegation | `docs/DELEGATION.md` | DelegationProfile fields, depth 2 and 8 children platform limits, Coworker `enabled_delegation_profile_ids`, LeadFailoverPolicy, escalation, budgets, native harness integrity |
| State machines | `docs/STATE-MACHINES.md` | Task states including PAUSE_REQUESTED and PAUSED; Attempt; VerificationRun; RuntimeMesh Handoff (V2 scope) |
| Delegation schema | `docs/schemas/delegation.schema.json` | `escalation_policy.on_exhaustion` enum RETURN_TO_LEAD, NEEDS_YOU marked TARGET-not-yet-wired (register A-37) |
| Domain code | `crates/domain-task/src/` | Child admission, lead failover, lead handoff, packets, escalation and result service foundations exist; no production delegation producer. Result publishing validates terminal session/Attempt scope, runtime/incarnation and revision but does not fence against the current Step Attempt/live lease epoch. |
| Daemon | `apps/litecoworkd/src/task_lead_handoff_operator.rs`, `verification_runs_operator.rs`, `delegation_profiles_operator.rs` | Lead change CAS only; verification read-only; profile CRUD |
| Storage | `crates/storage-sqlite/src/execution.rs`, `lead_handoff_boundary.rs`, `verification_runs.rs`, `delegation_results.rs` | SQLite admission guard reads Coworker allowlist and budget; verification write paths unrouted. Feature 005 T08 specifies a bounded `admit_file_write` Attempt, and T09 a file verifier/reconciler; neither defines the generic native provider worker/verifier AgentSession lifecycle assumed by this feature. |
| UI | `apps/litecowork-ui/src/App.tsx` (`WorkPage`, `TaskDetailPanel`), `src/tasks/` | No timeline, Workers view or controls |
| Status | `docs/IMPLEMENTATION-STATUS.md` | Task pause/resume/cancel/steer target-only; allowlist read only by the guard |
| Release | `docs/V1-PRODUCT-RELEASE-CONTRACT.md` section 6 item 3, gate E; `docs/TASK-RUNTIME.md` "Lead-agent change" | Assigned lead change creates a fresh planning session without Attempt; existing Attempts retain original binding/lease; execution replacement is separately admitted after prior lease and Effects settle. Served lead-agent route only changes binding. |

## 2. Pinned external references

| Reference | Pin | License | Use |
|---|---|---|---|
| REF-OPENHANDS shell `OpenHands/OpenHands` | `22e3057c89b2e5245a5a36fd9c48e52c8a984ba4` (2026-11-06) | MIT | Study only |
| REF-OPENHANDS engine `OpenHands/software-agent-sdk` | `f19fb2cf6a7f38266c6be954e91a9ef2a360bf9f` (2026-11-06) | MIT | Study only |

Source rows: [docs/REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md) (REF-OPENHANDS) and [docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](../../docs/REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md) section "OpenHands". Those verdicts are a proposal for owner review in that index; this package adopts them as design input only.

### Copy / prototype / reject for this feature

| Mechanism (upstream file) | Verdict | How it applies here |
|---|---|---|
| Host-mediated child conversation with a real observation, declared isolation and an ambiguity-honest timeout (`openhands-tools/openhands/tools/child_conversation/impl.py`) | copy (pattern) | Child admission returns a real committed record; a worker timeout is AMBIGUOUS and reconciled, never assumed success |
| Sub-agent budget and iteration inheritance, metrics replace-not-merge across resume (`openhands-tools/openhands/tools/task/manager.py`, `delegate/impl.py`) | copy (pattern) | Budget reservation per child Attempt; resume uses a fresh Attempt so metrics do not merge |
| Sub-agent capability subset and MCP inheritance (`openhands-sdk/openhands/sdk/subagent/scope.py`) | prototype | Worker `capability_grant_refs` must be a subset of the parent's; native MCP stays native (`NATIVE_UNMODIFIED`) |
| In-process sub-agents over the shared parent workspace, blocking parent, stop/stuck/paused collapsed into ERROR (`delegate/impl.py`, `task/manager.py`) | reject | Children run in `ATTEMPT_PRIVATE` Environments; paused, cancelled and failed are distinct typed states |
| `declared_resources -> declared=True` on a tool that spawns writers (`task/definition.py`) | reject | Authority is the lease and fence, not a self-declaration |
| Acknowledgment-only client tools | reject | A control acknowledgement is intent only; settlement needs observed quiescence |
| Replay recorded actions as recovery | reject | Recovery reconciles Effects; it never replays |

No upstream code is imported. Upstream repositories are not cloned into `repo_take/` for this feature.

## 3. Alternatives considered

| Topic | Option | Decision |
|---|---|---|
| Typed Task status | Keep strings | Rejected: a 13-state machine with pause and cancel interplay needs a compile-checked transition table (Rust enum with exhaustive match) |
| Control routes | Single `/control` endpoint | Allowed (F-01); default is one route per command for clearer error mapping |
| Worker Environment | Shared Task worktree with locks | Rejected for V1 (B-39): merge and lock semantics are not qualified; private Environments avoid them |
| Verification | Same agent self-checks | Rejected: violates independence; verifier binding must differ |
| Depth | Platform maximum 2 | Deferred: V1 uses 1 to keep the first gate-E journey testable |
| Timeline | Client-side reconstruction from many reads | Rejected: daemon projection with cursor paging keeps the UI from owning state |

## 4. Optimization notes (language and module placement)

- Some policy decisions already live in Rust (`delegation_admission.rs`, `lead_failover.rs`), but provider-session admission, quiescence, result fencing, verification, dispatch and recovery are not mere composition. Keep pure decisions at the domain layer and obtain an owner decision for the generic execution/session seam (SPEC O-06) before wiring provider work.
- Replace string Task statuses with a Rust `TaskStatus` enum in `storage-core` (T03); this removes duplicated string matches in `lead_failover_service.rs` and storage guards.
- Map `ChildAdmissionDenial` to wire codes in one exhaustive Rust function so a new variant cannot silently map to a generic error.
- Keep new Work-screen state logic in pure TypeScript modules (`task-control-state.ts`, `task-activity.ts`) instead of expanding `App.tsx`; the UI package's current test command is Node's built-in `--test` runner through `pnpm test`.
- Paginate the Activity projection in the daemon (limit 1..200, cursor) rather than streaming the full journal to the WebView.

## 5. Open research and decision register

| ID | Question | Status |
|---|---|---|
| R-01 | Claude Code CLI result and interrupt fixture for a delegated Step | Open, Feature 004 |
| R-02 | OpenCode bridge path for issuing a delegate request without native config edits | Open |
| R-03 | Reference hardware for AC-020 | Open, Feature 019 |
| B-39 | First pair, Environment sharing rule, V1 bounds, A-37 default | ACCEPTED as proposed 2026-11-10; whole SPEC still IN_REVIEW |
| O-06 | Generic Attempt/session admission seam for child, verifier and control paths | OPEN/BLOCKED: Feature 005's file-write admission contract is not a generic provider-session contract |
| O-07 | Lead binding change versus execution replacement operation | OPEN/BLOCKED: canonical Task Runtime requires distinct operations; served route performs binding CAS only |
| O-08 | A-37 exhaustion producer ownership and persisted location | OPEN/BLOCKED: keep owner-accepted TARGET-not-yet-wired field until Feature 004/005 dispatcher owner implements it |
