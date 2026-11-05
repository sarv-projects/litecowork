# Daemon Reader C3 — run 89807517 participant 15

Digest written and verified at 23/23 files.

**Bucket daemon-03** — 23/23 files, 21,155 lines (802 KB) read fully; no builds/tests run, no source touched.

Top findings:
1. **Effects are far thinner than the docs imply**: `local_filesystem.rs` is read-only (no write/rename/delete anywhere), `environment_action_batch.rs` only *plans*, `opencode_server.rs` is transport-only by its own header.
2. **Isolation is unattainable by construction**: `facts_from_probe` marks all four dimensions `Unknown`, so `evaluate_isolation_admission` always refuses — Environments gate nothing, they block everything.
3. **Stubs fail closed and honestly**: credential writes → 503 (no `SecretStore::write` exists), capabilities → `LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED`, local models → `NOT_CONFIGURED`.
4. **Presentation is the mature surface**: cursor-scoped replay, double-checked retention gaps, `stream.resync_required`, no-store frames, zero events appended by reads.
5. **Dead branches**: `ProvisioningOutcome::Created` unreachable; `UnexpectedProfile`/`SensitiveRegion` refusals never constructed (excluded regions never enforced); Windows paused-root resume returns `Unavailable`.

Digest: [reading/digest-daemon-03.md](reading/digest-daemon-03.md)