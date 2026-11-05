# Crate Reader E6 — run 89807517 participant 28

Bucket **crates-06**: 29/29 files read, 21,392 lines (818 KB).

Top findings:
1. **Layering is real**: `domain-*` crates depend only on `serde`; `storage-core` declares ports/records with no rusqlite/tokio/keyring; `storage-sqlite` owns transactions, blobs, clock, keyring; `windows-local-security` is the only crate opting into `unsafe`.
2. **Fail-closed by design**: `admit_step_attempt`, lease renew/release, planning-session start, and `WORKSPACE_PERSISTENT` Environment creation return typed errors naming the missing proof (containment, Trust grant, budget reservation, Effect reconciliation). Only `expire_execution_lease` is a real writer.
3. **Test doubles are quarantined**: `MemoryCredentialStore` and `TestKeys` are `#[cfg(test)]`-only; no mock implements a production trait object, so doubles cannot reach runtime.
4. **One real divergence**: `discover.rs` `runtime_capability_entries()` uses `unwrap_or_default()`, so a failed capability-host read looks like "no hosts" — everywhere else errors propagate. Also three duplicate SHA-256 digest validators.
5. **Unknown ≠ success is typed**: near-empty `AttemptAdmissionChecks`, `ChildAttemptAdmissionProofs::default()`, `automatic_selection_qualified` hard-coded `false`, Goal progress `Partial` with named limitations, drift `Unknown` without observation.

Digest: [reading/digest-crates-06.md](reading/digest-crates-06.md)