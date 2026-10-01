# Test Architecture

## Test layers

### Unit
Pure domain invariants, schema validation, state transitions, conflict rules, policy evaluation helpers.

### State-machine/property tests
Generate legal/illegal sequences for Task, Attempt, Effect, Approval, Lease and AutomationOccurrence. Invariants must hold under arbitrary command ordering.

### Contract tests
Every AgentAdapter, EnvironmentProvider, Capability/LitePSM adapter, StateStore, BlobStore and ChannelAdapter implementation runs a common conformance suite.

### Integration
Task Runtime + storage + one agent adapter + one capability + one environment.

### Fault injection
Kill processes, sever network, expire leases, corrupt provider responses, duplicate events, reorder event batches, expire secrets, fail blob uploads.

### Cross-runtime
At least local + cloud runtime with real event/artifact replication and lease handoff.

### Security
Permission bypass attempts, stale fencing token, spoofed channel events, malicious capability metadata, secret leakage checks, prompt-injection scenarios.

### E2E
Real user workflows from BENCHMARKS.md.

### Performance/soak
Long Tasks, many events, large artifacts, large file/data corpora, repeated reconnects, hours-long automation/channel operation.

## Mandatory vertical slice gates

### Gate 1 — durable local Task
Desktop -> local runtime -> external ACP agent -> LiteCowork Gateway -> LitePSM -> MCP -> Artifact/Effect -> Verifier.

Kill worker mid-Task; replacement must continue from ResumePacket and complete.

### Gate 2 — heterogeneous delegation
Lead agent delegates bounded child to different external agent; no shared full transcript; child result integrates and verifies.

### Gate 3 — cloud
Local Task replicates; explicit handoff creates higher-epoch cloud Attempt; reconnect shows same final state. Kill local runtime mid-effect and prove no duplicate external action.

### Gate 4 — channels
Telegram/email creates/steers same Conversation/Task; sensitive approval is blocked on weak channel.

### Gate 5 — automation
Recurring trigger creates ordinary Task and deduplicates duplicate trigger delivery.

## Non-negotiable invariant tests

- worker cannot set Task COMPLETED directly
- old fence rejected after new epoch
- ArtifactVersion references committed blob digest
- duplicate external channel event creates no duplicate message/task
- duplicate automation trigger creates no duplicate occurrence task
- package update does not alter in-flight CapabilityLock
- native agent reported action never becomes VERIFIED without verifier evidence
- SQLite databases are never synchronized across runtimes
