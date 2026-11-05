# Crate Reader E8 — run 89807517 participant 30

Digest written: [reading/digest-crates-08.md](reading/digest-crates-08.md)

**Bucket crates-08** — 25/29 files fully read, 4 partial (deadline): ~19,600 of 21,149 lines, ~760 of 818 KB.

Top 5 findings:
1. **Layering is real but fragile.** `storage-core → domain-effects`; five domain crates → `storage-core`; `storage-sqlite` → `storage-core` + 5 domain crates. No cycle exists *only* because `domain-effects` has zero internal deps — any dep added there creates a `storage-core ↔ domain-effects` cycle.
2. **Domain crates hold the write model.** `domain-workspace/lib.rs` computes next versions, CAS-checks `expected_version`, mutates records and builds `EventDraft`s itself; `change_replication_policy` uses unchecked `version += 1` while every other path uses `checked_add` — a real overflow hazard.
3. **Fail-closed defaults match the invariants** — `StepAttemptStore` defaults all methods to `Err(unsupported())`, admission proofs default `Unsupported`, empty drift observations ⇒ `UNKNOWN`, capability bridge demands all six mediation guarantees, Automation `Enabled` ⇒ `ReconciliationRequired`.
4. **SQLite adapters genuinely own atomicity** — version CAS in `WHERE`, `request_dedup` receipts, Immediate transactions, post-commit blob re-verification, canonical-JSON equality against stored rows; tests prove rollbacks via `RAISE(ABORT)` triggers and migration failpoints.
5. **Qualification gap:** the only real Bubblewrap namespace proofs are two `#[ignore]`d tests, so default `cargo test` never exercises that OS boundary.

Not read: `conversations.rs` tail (~530 lines), `context_document_purge_tests.rs` (90/3,091).