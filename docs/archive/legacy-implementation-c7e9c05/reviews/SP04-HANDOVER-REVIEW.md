# SP04 handover review and production follow-through

Review date: 2026-10-07  
Status: **PoC reviewed to the extent retained evidence permits; product-safe switching is not implemented.**

## Scope and disposition

The five disposable Codex-to-OpenCode cases were reviewed against their recorded task,
checkpoint, receiver, test, and safety evidence in [`../spikes/SP04.md`](../spikes/SP04.md).
Cases HO-01 through HO-04 live under temporary `/tmp` paths. Those four working trees and
their source diffs are no longer present in this environment, so this review can validate
their recorded outcomes and limitations but cannot certify their exact final source diffs.
HO-05's source tree is still available at
`/tmp/litecowork-sp04-ho05-retry-after` and was reviewed and tested directly.

| Case | Evidence available | Review disposition |
|---|---|---|
| HO-01: money helpers | Recorded packet, manifest, reported combined 9-test pass; source tree absent | Reported sequential continuation pass. Exact diff unavailable for independent source review. |
| HO-02: duration helpers | Recorded packet and receiver comprehension; source tree absent; MiMo request was rate-limited and Ling was used | Reported sequential continuation pass with model fallback. Exact diff unavailable. |
| HO-03: interval helpers | Recorded packet, receiver comprehension, reported combined 10-test pass; source tree absent | Reported sequential continuation pass. Exact diff unavailable. |
| HO-04: interrupted slug implementation | Recorded live Codex App Server evidence, heartbeat, checkpoint digest, receiver and tests; source tree absent | Reported continuation pass only after sender host shutdown and observed quiescence. Turn-interrupted status alone failed to stop the writer. Exact final diff unavailable. |
| HO-05: retry policy | Source diff, packet, and fixture available; direct tests rerun | Pass after independent corrections below; sequential only, so it adds no live cancellation evidence. |

The first HO-05 review found signed delta-seconds being clamped to zero, contrary to the
HTTP grammar. A follow-up source review found two more edge cases: very long decimal values
could raise Python's integer-string limit exception, and fractional-millisecond date delays
were rounded down, allowing an early retry. The disposable fixture now parses bounded decimal
values in small groups, returns `None` above its documented 8,192-character resource limit,
and rounds positive date delays up. Regression tests cover each case. The fixture's full
suite passes **20 tests**. These corrections affect only the temporary retry-policy exercise;
they are not LiteCowork product code.

## Safe-switch PoC review

The repository PoC is useful for testing the shape of the protocol: manifest validation,
checkpoint snapshots, monotonic lease epochs in a fixture ledger, fail-closed process
identity recovery, and Linux read-only checkpoint mounts. Its `effects_reconciled` and
`lease_released` booleans are explicit test inputs. They are not calls to product
EffectReconciler or LeaseCoordinator and cannot establish production safety.

The process fixture and ledger are not product modules. Do not promote them directly. Product
switching must use the owning TaskService, AttemptRunner, LeaseCoordinator,
EffectReconciler, RuntimeLifecycleService, AgentHostSupervisor, EnvironmentSupervisor, and
storage transaction boundaries from their source contracts.

## Platform qualification

The host used for this pass is WSL2, not native Linux, macOS, or Windows:

| Target | Evidence | Status |
|---|---|---|
| Linux under WSL2 | bubblewrap 0.9.0; PoC process/mount/crash tests | Fixture-tested only; not native Linux qualification. |
| Native Linux | No non-WSL Linux host available | Not run. |
| macOS | No macOS host available | Not run; PoC must reject start. |
| Windows | No Windows host available | Not run; PoC must reject start. |

A new unit test asserts that Darwin and Windows platform identifiers fail closed before a
worker process is spawned. This tests refusal behavior; it is not OS containment qualification.
Production release requires a separately implemented and run native containment provider for
each supported desktop OS, with descendant termination, identity/reuse checks, crash recovery,
checkpoint isolation, and stale-write tests on that OS.

## Product integration plan and dependency gates

This repository currently has only Workspace persistence code. It has no product Task,
Attempt, AgentSession, Runtime service, agent adapter, ExecutionLease coordinator, or Effect
reconciler. Consequently no honest product integration test can be written against real
services yet. The order for this feature is:

1. Complete E01-S02 review/qualification, then implement E01-S03 Runtime lifecycle and
   E01-S04 authenticated Operator boundary; add the desktop shell according to its E02
   dependencies.
2. Implement E03-S01 through E03-S03: a real native agent adapter, durable Conversation
   turns, planning and atomic Plan/Step acceptance.
3. Implement E03-S04 Attempt admission/recovery with real ExecutionLease persistence and
   monotonically increasing fencing epochs. Add the pre-start and post-`RUNNING` Runtime
   crash recovery flows F80/F81.
4. Implement E04-S01 through E04-S03 so cancellation/replacement uses real Trust checks,
   durable CapabilityInvocation/Effect records, idempotency, reconciliation, and ambiguity
   blocking. An unresolved Effect must keep the Attempt and write boundary fenced.
5. Implement the qualified E07 Environment/provider boundary. Product containment must prove
   that the native harness control process and the writable worker Environment have the
   intended separation; the PoC's fake-worker bubblewrap wrapper is not a native-agent adapter.
6. Integrate F79: interrupt, stop or fence the sender's writable Environment, prove writer
   quiescence, reconcile Invocations/Effects, commit checkpoint and lease settlement, then
   admit a new Attempt at a higher epoch. If any proof fails, leave the sender unresolved and
   deny the replacement.
7. Run code-level state/property/race tests, real-daemon/provider crash-injection tests, and
   the owner real-user case with the configured Codex and OpenCode agents. Verify no stale
   sender writes, no duplicated external Effect, same durable Task/accepted work, correct
   checkpoint bytes, and the receiver's completion against an independent oracle.

The existing E03-S04/E04-S03/E07-S01 dependencies remain authoritative. This review does not
waive them or claim those stories complete. Safe-switch production acceptance is blocked until
their real services and native OS providers exist.

## Required release evidence

- Five source-level case reviews: recover archived HO-01..HO-04 fixture diffs if available;
  otherwise retain them as reported-only evidence and run replacement cases with committed,
  digest-pinned review artifacts.
- Native Linux, macOS, and Windows containment test packets for every OS LiteCowork chooses
  to support. WSL2 results do not satisfy a native Linux gate.
- Durable transaction evidence proving one active Attempt per Step, no reused lease epoch,
  and no replacement admission before sender quiescence and Effect reconciliation.
- Runtime crash/restart evidence both before and after replacement `RUNNING`, including
  missing/unreadable process identity and ambiguous Effect rejection.
- Real Codex-to-OpenCode user case with exact versions, bounded handoff manifest, changed
  paths, tests, stale-writer marker, Effect observation, and limitations.
- Owner review of the sanitized packet. Agent-run test results are not owner acceptance.
