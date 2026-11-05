# Doc Reader A4 — run 89807517 participant 3

**Bucket docs-04** — 19/19 files read, 5,485 lines, 455,700 bytes.

Top findings:
1. One shared admission gap unites the bucket: ARTIFACTS-EVIDENCE, CAPABILITY-INVOCATIONS and the DESKTOP audit all state dispatch is unavailable — `STARTED` rejected by `TrustDecisionUnavailable`, `InvocationTransitionWriterUnavailable`, `ApprovalUseConsumptionUnavailable`; no Invocation create/dispatch; no ApprovalUse consumer. Fail-closed TARGET, not CODE.
2. Identities are fenced consistently (Runtime/RuntimeIncarnation, AgentSession never revived, Environment lifetime ≠ sharing scope, `EnvironmentControlLease` single owner), but `AgentHostInstance` is never named as a record here — only `AgentHostSupervisor`. Must come from AGENT-FABRIC.md.
3. Readiness is decomposed, never boolean: 5 detection axes + `AgentReadiness`; 8 delegation predicates; warm/prowarm never implies session, grant or lease.
4. Delegation ceilings agree across DELEGATION/catalog/ledger: depth 2, ≤8 children, concurrency 1..8, failover ≤3, 1 MiB envelopes; profile is ceiling-only, no credential crossing.
5. Drift logged: schema v22 vs v16; `crates/capabilities` vs root `capabilities/`; PRODUCT #11 "safe drain" is currently unsatisfiable.

Digest: [reading/digest-docs-04.md](reading/digest-docs-04.md)