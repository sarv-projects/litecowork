# Daemon Reader C2 — run 89807517 participant 14

Digest written.

**Bucket daemon-02** — 16/20 files read in full or substance (~11,344 of 21,117 lines, ~415 KB). Digest: [reading/digest-daemon-02.md](reading/digest-daemon-02.md)

Top findings:
1. **Conversation turns**: durable commit (message + OPEN turn + 2 events + receipt) then, without supervisor, a second txn settles `OPEN→FAILED` with `CONVERSATION_SESSION_ADMISSION_UNAVAILABLE` → 503, `no-store` — fail-closed. With supervisor it returns 202 *before* dispatch; dispatch failure is only `eprintln!`, no compensating settlement (fail-open risk).
2. `AgentSessionSupervisor` is constructed fresh per request via `FromRef<ApiState>`; OPEN-turn recovery/restart supervision is unverified in this bucket.
3. Trust is deliberately narrow: Task-planning read grant only, `workspace.resource.read`, `LOCAL_STRONG` hard-coded, ≤24 h expiry, no approvals/secrets; tests assert DB ledger counts.
4. Archive preview is gated on non-default Linux `source-zip-worker`; every other build returns `Unavailable`/503 rather than an empty manifest. Quota observation returns literal `null`; invocation failures are allowlisted (no `details`).
5. Source drift: `agent_credentials_operator.rs` contains literal `let encoded_secret=[redacted] (sanitized, non-compiling); `main.rs` dead-codes `trust`, yet trust routes are mounted.

Coverage gaps: `skill_operator.rs` 1–700/1962, `operator_ipc_tests.rs` ~900/8035 (tail read cancelled at deadline), two resource test files unread.