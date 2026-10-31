# LiteCowork V1 — Contract Reconciliation Implementation Handoff (2026-10-13)

**Status:** implementation handoff; design reconciliation is not CODE, SYSTEM, USER or release acceptance. This file tracks obligations against **existing** stories and authoritative contracts. It does not add routes, events, states, a planner, a memory store, or a second backlog.

## Use this record

Before editing a related feature: read `AGENTS.md`, `implementation/CURRENT-RUN.md`, the exact backlog story, its epic, this record, and the linked canonical source. Inspect current source and migrations; target-only LLD prose is not evidence of served APIs. Preserve user/uncommitted changes and verify before accepting. Stories retain existing IDs, statuses, dependencies and separate CODE/SYSTEM/USER gates.

## Critical-path implementation blockers

| Track / existing owner | Current evidence and remaining change | Proof needed for acceptance |
|---|---|---|
| **P0 Planner isolation and admission — E03-S03, E07-S00, E03-S01** | SQLite currently rejects planner reservation and activation with internal `TASK_PLANNING_ISOLATION_UNAVAILABLE`; no qualified `IsolationAttestation` producer, scoped native tool controls, or end-to-end dispatch. `docs/TASK-RUNTIME.md` records a **conditional**, not approved/implemented, Runtime-controlled containment alternative to waiting for Codex restricted-roots support. Preserve storage fail-closed gate until external filesystem/process/network containment, scoped Resources, native MCP/app/plugin/hook mediation, current-incarnation binding, Trust, lease and settlement are proven. No `readOnly` prompt or directory-only workaround. | CODE: unqualified callers leave zero durable session/event/receipt/Task mutation; adversarial escape and negative protocol tests. SYSTEM: actual agent/OS containment, restart, cancel, permission, lease/fence tests. USER: useful Task proceeds via authenticated planner, persisted Plan/Steps, eventual independent verification. |
| **P0 Plan-submit provenance — E03-S03, E01-S04** | `POST /v1/tasks/{id}/plan-revisions` is not yet producer-scoped from the authenticated active planning or lead Attempt session. Owner-supplied session/Attempt IDs must not impersonate the producer. Replay of an accepted plan must survive closed session/lost response without new Steps. | Test authenticated subject + Workspace + active producer + spec/head/version/lease, parallel stale/cancel race, idempotency digest conflict (`409 CONFLICT`), stale Task aggregate (`409 STALE_TASK_VERSION`), same-key same-result replay and one atomic PlanRevision/Step/event/receipt commit. No new owner-authored plan path without separate reviewed contracts. |
| **P0 Native Conversation Send — E03-S01, E03-S02** | Saved Conversation list and read-only/disabled composer are not provider-backed turns. Qualify native harness input/options, restricted context and tools, turn admission, stream commit, cancellation/retry, restart/reconnect, failure and native capability parity. | CODE + actual provider SYSTEM + USER: send answer; stop; lost ACK; late stream; crash/restart; no Task/Attempt from ordinary chat; false or unsupported agent capabilities never appear enabled. |
| **P0 Windows WorkspaceRoot revalidation — E02-S03, E07-S00, E13-S03** | Current Linux/macOS startup root identity/rebind logic is not a qualified Windows implementation. Windows named-pipe IPC and ConPTY are separate capabilities and do not prove root safety. | Before Windows mutable/indexed root support: handle-based identity and reparse/junction/symlink-safe traversal; exact binding across restart; volume/case/rename/swap tests; Windows+WSL boundaries; revoked/missing roots UNAVAILABLE, never permissively rebound; real OS SYSTEM and owner USER. An unqualified Windows root remains UNAVAILABLE, and that release-matrix cell cannot PASS. |
| **P1 Coworker-private learning — E08-S03** | Current user-authored Resource-backed ContextDocuments are distinct from accepted V1 **target** automatic private MemoryRecords. `docs/LEARNING.md` owns candidate eligibility, provenance, policy/consent, conflicts, private scope, revocation and commit; legacy `require_user_confirmation_for_memory` is not the new policy. | Promote one canonical LearningCandidate/MemoryRecord typed store/API/event path only with implementation and migration, filtering of temporary chat/external instructions, deterministic privacy and secret rejection, concurrent revision/revocation fences, bounded latency and agent-controlled recall, USER visibility/opt-out; never overwrite native agent memory. |
| **P1 Browser, terminal, and OS qualification — E07-S02, E07-S05, E08-S05, E13-S03** | A target LLD/placeholder Workbench is not Chrome side-panel or an executing PTY. Native agent private shell is not automatically controlled by LiteCowork. | Real Chrome/Chromium distribution and consent; Chrome 136+ default-profile restriction; qualified Chrome 144+ opt-in attach; epochs, takeover, reconnect, geometry/IME/multiple display; PTY/ConPTY profiles and shell process liveness; exact supported OS/provider/version, keyboard/accessible USER tests. Show truthful fallback/unavailable states. |
| **P1 Real end-to-end outcome and system fault suite — E03-S04, E04, E05, E09, E13-S03** | Green design/schema validators do not constitute a running Task. | Run UI → authenticated Operator → service → CAS/storage/event/receipt → native session/Attempt → invoked Effect → recovery → independent Evidence/Verification → UI. Inject crash, stale epoch, partial effect, provider failure/quota, memory revoke, pause/cancel, sleep/trigger misfire, repeated notifications; show UNKNOWN/AMBIGUOUS when unproven. |

## Other reconciled contract obligations

| Contract seam and owner | Implementation action / negative check |
|---|---|
| Error schema, idempotency and stale versions — E01-S04, E03-S03, E04-S05 | Use canonical `CONFLICT`, not nonexistent `IDEMPOTENCY_CONFLICT`; `STALE_TASK_VERSION` denotes Task aggregate CAS, `STALE_VERSION` the independent recovery-placement snapshot. Verify actual API/IPC codes and replay against strict JSON schema/OpenAPI. |
| Internal planner diagnostics — E03-S03 | Storage `TASK_PLANNING_ISOLATION_UNAVAILABLE` never serializes as a public ErrorCode: read-only readiness exposes only sanitized dispatch blockers; a future failed command uses `DEPENDENCY_UNAVAILABLE`. Negative responses must not expose internal store strings; no session/receipt on denial. |
| Agent auth descriptor — E03-S08, E03-S09 | `AgentAuthDescriptor.credential_slots` is required in OpenAPI and now the HLD pseudotype; zero slots is allowed. Resolve `credential_slot_id` uniquely (do not index a list by ID), honor `storage_owner`, never echo secret values or accept untrusted registry-proposed secret execution. |
| Skill drift blocker — E10-S02, E09-S01 | `SKILL_DRIFT_DETECTED` is a canonical typed blocker; verify strict Blocker serialization, pinned Skill/Routine revision, evidence, Needs You and reviewed repair; never rewrite published revisions or restart an effectful action blindly. |
| Delegation policy and evidence — E05-S03, E13-S02 | `ExistingPolicyClass` is prose, not new DTO. Do not turn B83 into invented numeric thresholds: automatic benefit-based selection stays with current lead unless qualified B83 thresholds/confidence exist; pinned choices/unknown cost remain honest. |
| Upstream sources and links — E03/E07 qualification owners | OmniRoute `ourines/omniroute` citations point to a fork of `diegosouzapw/OmniRoute`; pin exact fork commit/file before source reuse and check license/ToS. Chrome/MCP links are research inputs, not proof of installed browser/protocol support. Historical ADR names in SOURCE-RECONCILIATION are reference ZIP inputs, not current paths. |
| Recovery documentation — E03-S04, E13-S03 | Correct four-column Failure/Recovery matrix and move native-unfenced effects to split-brain ownership. Validate actual mesh/hub detection, no falsely fenced native side effect, no duplicate external write, proper user-visible status. |

## Gate and evidence recording

1. **CODE:** link exact source/contract versions, test command, assertion and synthetic fixture digest. Positive schema/lint alone is inadequate for admission or security.
2. **SYSTEM:** prove the *actual* Runtime, agent and platform build, including negative authorization, restart, ambiguous acknowledgment, scoped secrets and OS permission bypass attempts.
3. **USER:** owner repeats an installed-build practical task and approves expected versus observed result; classify each platform/provider cell PASS, FAIL, BLOCKED or qualified NOT_APPLICABLE. Do not mark BLOCKED as PASS.
4. **Release:** E13-S03/S04 must explicitly reject any unsupported Windows root, unqualified plan/turn, untested learning, browser or terminal, unresolved external Effect, or missing per-OS tests as a supported capability. No status is upgraded merely by this handoff.
5. **Reconciliation:** machine/API or storage promotions require owning HLD/LLD + `docs/SCHEMAS.md`, OpenAPI, domain-event schema, SQLite migration, implementation tests and generated coverage together. New candidate route names remain target-only until served.

## Reproducible documentation checks (not system qualification)

```sh
cd /home/sarvesh/business_Dev/litecowork
.venv/bin/python scripts/validate_architecture.py
.venv/bin/python scripts/audit_v1_contracts.py
.venv/bin/python scripts/audit_v1_design_depth.py --strict
.venv/bin/python scripts/validate_design_owner_consistency.py
.venv/bin/python scripts/validate_implementation_coverage.py --write # only after reviewing changed sources
.venv/bin/python scripts/validate_implementation_plan.py
git diff --check
```

Source of truth for baseline/status remains `implementation/backlog.json`, the respective epic/review packet and CURRENT-RUN. This handoff is a dated navigation/evidence obligation, not a competing architectural authority. Earlier positive design checks do not authorize removing security gates or claiming acceptance.
## Documentation reconciliation validation — 2026-10-13

After updating existing implementation handoffs and 12 backlog stories, `.venv/bin/python scripts/validate_architecture.py` passed (142 typed events; JSON Schema, OpenAPI, SQLite, Gateway/product names and links). `scripts/audit_v1_contracts.py` passed (206 OpenAPI routes, 158 UIV1 cases, 67 stories, zero findings). `scripts/audit_v1_design_depth.py --strict` passed (20 domains, 226 functions, 187 Settings controls, zero unmapped/duplicate signatures/structural blockers). `scripts/validate_design_owner_consistency.py` passed (34 checks, zero issues). Generated implementation coverage passed at 103 architecture documents, 1,668 sections and 6,282 rows. Scoped `git diff --check` on files edited in this handoff passed. A broad `git diff --check` flagged CRLF/trailing-whitespace lines in the existing `implementation/audit-inventory.csv`; that unrelated inventory was preserved.

All results above are **documentation/contract evidence only**. They do not change existing backlog statuses or satisfy CODE/SYSTEM/USER/release gates. No application source was modified by this handoff.
