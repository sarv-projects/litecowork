# Crate Reader E5 — run 89807517 participant 27

Bucket **crates-05**: 29/29 files read, 21,951 lines, 818 KB. Digest: [reading/digest-crates-05.md](reading/digest-crates-05.md)

Top 5 findings:
1. **No wall clock in any domain decision.** `now` is always a parameter (`WorkerPerformanceQuery.now`), a store callback (`RoutineTransaction::now()`), or a caller event field (`recorded_at`, `now_epoch_ms`). `linux-process-scope` uses `Instant` only; `hlc_timestamp` is carried but never generated or compared here.
2. **Ordering is enforced by strict timestamp advance:** context-document purge requires `recorded_at > resources.updated_at`; failover needs `observed <= recorded < expires`; cursors never rewind on a backwards clock; the 90-day performance window is boundary-inclusive.
3. **Migration handling is fail-closed and event-versioned:** `check_schema_compatibility` refuses both too-new and too-old schemas; `schema_version: 1` envelopes carry `.v2` payloads (`verification.completed.v2`); `SQLITE_V20/V22_DDL` triggers prove SQLite itself rejects caller-supplied `PASSED` and terminal rewrites. No migration runner in this bucket.
4. **Uniform idempotency:** `request_dedup(principal_id, request_id)` + canonical-JSON SHA-256; replay resolves *before* mutable-state checks, so a paused Automation still replays its committed Task receipt.
5. **Encryption and transport posture are real, not claimed:** every blob/purge snapshot is an `LCB1` XChaCha20-Poly1305 envelope whose AAD binds workspace+purpose+digest+size+media type (transplant tests fail authentication); IPC binds peer trust to a caller callback (Unix) or a kernel DACL (Windows). Divergences noted: `WRITE_DEADLINE` 30 s vs 3 s, and `domain-effects` validates timestamps by shape only.

No builds, tests, or writes outside the digest were performed.