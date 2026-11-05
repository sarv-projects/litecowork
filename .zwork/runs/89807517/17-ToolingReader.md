# Tooling Reader — run 89807517 participant 17

Bucket **tooling-01** — 51/51 files, 13,658 lines (~670 KB) read in full; digest at [reading/digest-tooling-01.md](reading/digest-tooling-01.md).

Top 5 findings:
1. `scripts/check.sh` is the sole gate (pins uv 0.12.23 / Python 3.13.12 / Rust 1.98.1, freshness-checks generated docs, restores stale output before failing) — and it never runs `.specify/scripts/validate_feature_package.py` or `capabilities/zip_intake/tests`, so SDD packages and the capability suite are ungated.
2. `validate_architecture.py` (3,498 lines) enforces 23 rule families by machine-contract parity (EVENTS.md ↔ domain-event schema, 4-way error-code parity incl. `operator_error*` literals in `apps/litecoworkd/src`, SQLite/OpenAPI/SCHEMAS) plus live SQLite constraint fixtures with exact error codes.
3. Real contradiction: `packaging/systemd/litecoworkd.service` has `TimeoutStopSec=60` and lacks `# Managed by LiteCowork;`, twice failing `qualify-linux-systemd-runtime.sh` (requires ≥90 s + marker); `v1_packaging_check.py` only greps strings, so CI misses it.
4. Coverage tooling requires every tracked Markdown doc, OpenAPI op, event, table/column/FK/CHECK/trigger/index and every FLOW/BENCHMARK heading to have an explicit owning story — mechanically enforcing "one normative definition".
5. `zip_intake` is the best-scoped component: hand-parsed central directory → `ZipInfo` cross-check → parent re-validation → provider path re-derivation, sentinel-byte budgets, `ISOLATED_WORKER_NOT_QUALIFIED`, Linux/Bubblewrap only. Windows leaks noted: `/tmp` report paths, `fcntl` in `v1_resilience_matrix.py`.

Note: `tests/` is stdlib `unittest` with `bash` shell-outs, not pytest — POSIX-host-bound.