# Daemon Gap Reader — run d1a060f5 participant 0

Digest: [reading/digest-daemon-gap.md](reading/digest-daemon-gap.md)

Top findings:

1. **`trust.rs` is compiled, never wired.** `main.rs:36` is `#[allow(dead_code)] mod trust;` ("not wired until the atomic CapabilityBroker admission transaction and audit persistence exist"). No `trust::` reference exists outside the file; no caller of `request_approval`. Its two-rule policy engine (`workspace.resource.read` ALLOW, `external.message.send` REQUIRE_APPROVAL) governs nothing.
2. **No mutation path consults Trust.** `automation …/run` and `…/test-runs` admit READY Tasks with zero policy evaluation; `approval_operator` works independently of `trust.rs`.
3. **Mounted:** automation (13 routes + 30 s `AutomationTriggerHostWorker`, start failure aborts startup), rag_retrieval (2), workspace_root_scan (2), delegation_profiles (7), user_request (4) — all behind `authenticate`/`AuthenticatedLocalPeer`.
4. **Hard gates:** `POST …/resume` → 503 `DEPENDENCY_UNAVAILABLE` via `ReconciliationRequired`; delegation enable → `EnablementUnavailable`; off-Linux scan → 501 `SCAN_UNSUPPORTED_PLATFORM`.
5. **Fixed answers:** `occurrence_version: 3`, `occurrence_status: "STARTED"`, empty `failure_categories`/`observed_costs`, `confidence:"LOW"`, `writer_quiescence_proven:false`.

Caveat: 18 KB, not the 8–12 KB target — exact identifier inventories dominate.