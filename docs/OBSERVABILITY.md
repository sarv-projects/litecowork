# Observability Architecture

Domain events, audit records, debug logs, metrics and traces are separate data classes.

## Structured logs

Every service log entry should include when applicable:

```text
correlation_id
causation_id
workspace_id
task_id
step_id
attempt_id
runtime_id
agent_binding_id
capability_id
environment_id
```

Never log raw secrets, tokens, cookies or unredacted provider credentials.

## Metrics

### Task
- tasks_created_total
- task_completion_rate
- task_time_to_first_attempt
- task_end_to_end_duration
- tasks_needing_user
- tasks_failed

### Attempts/agents
- attempts_started/completed/failed
- agent_session_loss_total
- delegation_children_total
- recovery_attempts_total
- repeated_failure_stop_total

### Capabilities
- capability_search_latency
- activation_latency
- invocation_latency
- invocation_failure_rate
- approval_required_rate

### Runtime mesh
- runtime_online_count
- heartbeat_lag
- replication_lag_events
- replication_lag_bytes
- lease_conflict_total
- handoff_duration
- failover_total

### Effects/verification
- ambiguous_effect_total
- reconciliation_duration
- duplicate_effect_prevented_total
- verification_pass/fail/inconclusive

### Storage/artifacts
- blob_transfer_latency
- blob_digest_failures
- event_append_latency
- projection_rebuild_time

## Tracing

One Task operation should propagate `correlation_id`; nested calls use trace/span context where available. External provider calls are spans with redacted request metadata and outcome class.

## Audit log

Security/audit records include:
- approvals
- grant issue/revoke
- secret lease
- runtime pairing/revoke
- sensitive effect
- policy denial
- channel identity/assurance changes

Audit retention may exceed debug-log retention.

## SLO categories

Exact targets may be tuned after profiling, but tests/monitoring must cover:
- daemon startup
- Task create latency
- UI projection propagation
- capability search/activation
- lease renew latency
- runtime offline detection
- explicit handoff duration
- event replication lag
- artifact transfer throughput
- idle CPU/RAM footprint
- max concurrent Attempts under supported hardware tiers
