# Daemon Reader C4 — run 89807517 participant 16

**Bucket daemon-04** — 15/22 files read fully, 10,264 lines (~380 KB of 783 KB).

**Top findings**
1. Replay/idempotency is durable via `request_dedup(principal_id, request_id)` + `response_digest=sha256:<hex>`; exact retries return byte-identical bodies and preserve `x-correlation-id`, survive process restart *and* store reopen; changed payload → 409 `CONFLICT`; stale `If-Match` writes **no** receipt.
2. `runtime_workspace_replay_process_tests.rs` spawns real daemon children (lost-ACK, unread response, TERM drain) across 7 scenarios: workspace/conversation create, root scan, 4 MiB chunked upload, instruction revision, ContextDocument purge reconciliation, `/v1/stream` task resume.
3. Fail-closed is deliberate and documented: control descriptor 503, Discover `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`, office providers all `NotEnforced`, planning dispatch always `Err`.
4. Cursors bind to the whole filter tuple (search 400s on changed `q`/`kind`; capability hosts 409 `STALE_VERSION`).
5. All restart proof is Unix-only; `attempt_process_scope` is `#![allow(dead_code)]` + systemd-only — Attempt containment is TARGET, not CODE.

Unread (deadline): `automation_operator`, `trust`, `codex_app_server`, `workspace_root_scan_operator`, `delegation_profiles_operator`, `rag_retrieval_operator`, `workspace_root_selection_operator_tests`.

Digest: [reading/digest-daemon-04.md](reading/digest-daemon-04.md)