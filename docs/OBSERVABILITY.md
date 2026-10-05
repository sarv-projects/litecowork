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
runtime_incarnation_id
host_instance_id
routine_id
automation_id
trigger_id
agent_binding_id
capability_id
environment_id
```

Never log raw secrets, tokens, cookies or unredacted provider credentials.

## Metrics

### Task
- tasks_created_total
- task_completion_rate
- task_create_latency
- task_time_to_first_attempt
- task_end_to_end_duration
- tasks_needing_user
- tasks_failed

### Attempts/agents
- attempts_started/completed/failed
- agent_session_loss_total
- attempt_resume_duration
- process_crash_recovery_duration
- delegation_children_total
- recovery_attempts_total
- repeated_failure_stop_total

### Capabilities
- capability_search_latency
- capability_activation_latency
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
- event_to_projection_latency
- event_to_operator_update_latency
- projection_rebuild_time

### World Index
- local_resource_search_latency by root-count and indexed-item-count tier
- index_update_lag after filesystem/provider observation
- watcher_gap_reconciliation_duration
- root_scan_duration and bytes inspected
- stale/unknown-location result rate

### Runtime and automation lifecycle
- daemon recovery/drain duration and unresolved recovery blockers
- cold AgentHost start duration, active use refs, idle-stop duration, orphan reconciliation
- dependency preparation duration and failures by prerequisite kind
- trigger lag, misfire/coalesced/skipped occurrence count, cursor gaps and host-epoch rejection
- due occurrences waiting for Runtime/resources and wait duration
- Routine materialization failures and pinned-revision conflicts

### Delegation, budget, and prewarm

- delegation_admission_total by bounded outcome/error class and selection mode
- delegation_candidates_count and admission rejection reasons
- delegated_worker_start_latency and child_attempt_duration by bounded TaskCategory
- worker_verification_pass/fail/inconclusive and escalation_attempt_count
- delegation_budget_threshold_total by action; reservation/observed-usage mismatch
- cost_observation_coverage by source/confidence/currency; cost_per_verified_task only
  when units are comparable
- quota_observation_total by NORMAL/LOW/EXHAUSTED/UNKNOWN and observation age bucket
- host/session/environment prewarm attempts, hit rate, failures, and eviction reason
- lead_change_total and handoff duration by bounded reason class
- deadline_preflight_failure_total and actual execution-method fallback counts
- suggestion proposed/accepted/dismissed/expired and duplicate-suppression counts
- Coworker-paused admission blocks and ContextDocument conflict resolutions

Use no Task title, objective, prompt, Artifact name, ContextDocument text, profile name,
worker message, raw provider model string, or arbitrary Resource ID as a metric label.
Binding/profile IDs belong in access-controlled trace/log fields with bounded retention.
Worker quality/cost projections must suppress or mark insufficient samples; aggregate
monetary metrics are partitioned by currency and never combine by implicit FX.

Process existence is measured separately from installed inventory. Provider process
restart/idle policy remains LitePSM-owned; LiteCowork observes activation/invocation health.
Identifiers belong in logs/traces rather than unbounded metric labels.

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
- local deterministic Resource search by Workspace size tier
- Task create latency
- event-to-projection and event-to-UI propagation
- capability search/activation
- lease renew latency
- runtime offline detection
- explicit handoff duration
- event replication lag
- artifact transfer throughput
- idle CPU/RAM footprint
- max concurrent Attempts under supported hardware tiers
- Attempt/process crash recovery and Task continuation duration

Before Stage 1 exits, record a reproducible baseline for daemon startup, idle CPU/RAM,
Task creation, deterministic Resource search, event-to-UI propagation, local crash
recovery, and CapabilityHost activation/binding overhead through a deterministic LitePSM
adapter fixture. The activation measurement must report LiteCowork control-plane time
separately from the fixture's provider-start time; it must not be presented as a real
provider startup result. Freeze numeric Stage 1 targets from those measurements. Stage 3
adds real LitePSM/MCP activation and provider recovery measurements, retaining the Stage 1
control-plane budget and publishing provider-specific latency separately. Cross-Runtime
recovery receives measured targets when Mesh exists; none are invented in advance.
