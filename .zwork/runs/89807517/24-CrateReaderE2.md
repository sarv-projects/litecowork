# Crate Reader E2 — run 89807517 participant 24

**Bucket crates-02** — 25/25 files opened, 14,287 lines read in full (+~690 sampled of the 7,231-line `resource_upload_gc_tests.rs`), ~818 KB. Digest: [reading/digest-crates-02.md](reading/digest-crates-02.md)

Top findings:
1. **Clean hexagonal core** — `domain-*` holds pure `decide_*`/`settle_*` rules; `storage-core` is trait ports only (`SecretStore` is write-only, `DISPATCHED` is deliberately not a command); `storage-sqlite` does all SQL in `Immediate` txns with version CAS, `request_dedup` receipts and verified `AggregateState` blobs.
2. **Boundary leak** — `domain-task` imports `storage_core::{TaskStore, EventDraft, StoreError}`, and `storage-core` re-exports `domain_responsibility`/`domain_invocations` types, so domain semantics are reachable through the port crate.
3. **Likely bug** — `decide_occurrence_transition` writes `claim_expires_at = Some(facts.now)` on entry to `Claimed`: the claim is born expired, defeating the live-claim guard it protects.
4. **Fail-closed is uneven** — strict `Integrity`/`CorruptSchema` in registry cache, resource index and occurrence reads, but `parse_instant(..).unwrap_or_default()` / `u64::try_from(..).unwrap_or_default()` in `automation_host` silently coerce malformed durable values to 0.
5. **Ownership self-declared off-spec** — `coworker_learning` (storage-core) owns learning policy while its doc says it belongs in a missing `domain-learning` crate; plus unreachable `EscalationBlocker::VerificationPassed`, a placeholder all-zero `cursor_digest`, duplicate DelegationProfile loaders, and a twice-evaluated (must-be-deterministic) profile commit closure.

All evidence here is **CODE** tier: no route, daemon, provider or user evidence.