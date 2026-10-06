# Current run and agent handoff

Update this file when the active story, implementation state, or handoff changes. It is
operational context; architecture and owning domain contracts remain authoritative.

## Active story

**E01-S02 — Transactional persistence** (`IN_PROGRESS`, owner review pending).

E01-S01 remains `IN_REVIEW`: owner clean-clone acceptance on the reference machine and
hosted GitHub Actions are still pending. This S02 work continues at the owner's direction;
it does not waive the dependency or claim either story accepted.

## Repository state

- Repository: `litecowork`
- Branch: `main`
- S02 implementation base: `61b4376c12127664faf7dceca4825569bfdb8cc6`
- Most recent completed qualification commit: `test(storage): qualify mixed SQLite read-write load`.
- At the start of this run, `main` was seven commits ahead of `origin/main`; this current
  writer-pressure telemetry slice is uncommitted.
- No push is authorized or performed in this run.
- V1 delivery order remains desktop/local, cloud continuation, then remote Runtime.
- Coding agents implement and report evidence; the owner reviews and accepts product
  behavior. Local tests do not establish hosted CI, owner-machine acceptance, provider
  qualification, or production readiness.

## Completed implementation scope

- Added Rust `storage-core` ports for `StateStore`, `EventStore`, `BlobStore`, and their
  Workspace persistence composition.
- Added `domain-workspace::WorkspaceService` create and replication-policy update commands
  with expected-version checks and closed, versioned Workspace events.
- Added `storage-sqlite`: bounded writer actor; FK/WAL/FULL durability configuration;
  canonical v1 DDL migration; source checksum and resulting-schema fingerprint; bounded
  busy timeout; immediate transactions; atomic Workspace projection/event/origin-sequence
  commits; replay from immutable state blobs; and error mapping.
- Added encrypted content-addressed `FileBlobStore` with Workspace/purpose-scoped paths,
  XChaCha20-Poly1305 authenticated data, versioned injected keys, digest/size validation,
  atomic no-clobber writes, fsync, and fail-closed permissions/key errors. Tests use a
  fixed test-only key. There is no production key provider.
- Added adversarial tests for JCS ordering/number formatting, unsafe integer rejection,
  migration receipts/rollback, schema drift, concurrent stale writes, transaction rollback,
  busy timeout, replay, corrupt/wrong-scope blobs, unavailable keys, broad directory
  permissions, private SQLite state-directory/file permissions, symlink database paths,
  and failed BlobStore commits.
- Updated storage/event/schema docs, the E01-S02 story/backlog, research references and
  generated implementation coverage.

## Active SP02 qualification update

- Separate SQLx 0.9.0 and rusqlite 0.40.2 executables use the same `WorkspaceService`,
  production encrypted `FileBlobStore`, eight concurrent producers, 32-slot bounded writer,
  WAL/FULL settings, and host SQLite 3.45.1. SQLx is configured with `sqlite-unbundled`
  alone; the SQLx `sqlite` convenience feature also enables bundled SQLite. Product
  `storage-sqlite` remains bundled by default; only the rusqlite spike disables that default.
- Both executables close/reopen the database, verify the Workspace projection, decrypt
  aggregate-state blobs, replay events, and compare the replayed state. The rusqlite side
  uses the product adapter. SQLx is a prototype and lacks the product migration receipts,
  schema-drift checks, full path hardening/error mapping, and root-scope reads.
- Three sequential optimized 200-update runs per driver were alternated on Ubuntu 24.04.4
  LTS under WSL2, x86_64, Linux 6.18.40.1, Rust/Cargo 1.98.1, system SQLite 3.45.1.
  Median throughput was 327.4 updates/s for rusqlite and 322.9 for SQLx; observed ranges
  overlap, so this experiment does not select a driver. Exact samples and method are in
  `implementation/spikes/SP02.md`.
- `storage-sqlite` retains bundled SQLite and rusqlite's default cache/wasm-compatible
  features for production. The benchmark disables only SQLite bundling while explicitly
  preserving those rusqlite defaults. The storage runtime test wording now applies to both
  bundled and system SQLite builds.
- Added a rusqlite product-adapter mixed workload: 40 Workspace writers, four readers, a
  32-slot writer queue, and 1,200 updates per measured run. Readers check monotonic,
  policy-consistent snapshots; close/reopen verifies replay for all 40 Workspaces. Three
  sequential runs passed integrity checks but showed highly variable elapsed and tail
  latency. Exact queue occupancy is not exposed; longer capacity qualification remains open.
- Added process-local `SqliteWriterMetricsSnapshot` observations for outstanding command
  submissions and bounded-channel send wait. A held external writer lock test confirms
  blocked callers raise the high-water mark and settle without a partial Workspace write.
  This is runtime telemetry only; it is not durable state, exact queue occupancy, or an
  Operator metrics export.

## Verification status

Focused checks passed during this run:

- `cargo test --no-default-features --features sqlite-rusqlite-defaults -p storage-sqlite --offline`:
  19 passed using system SQLite while retaining rusqlite's non-bundle default features.
- `cargo test --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml --offline`:
  6 passed, including the encrypted Workspace restart/replay run and production blob tests.
- `cargo test --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml --offline`:
  2 passed using the product adapter, including mixed-load snapshot/replay correctness.
- Three release samples per driver completed; every run reported SQLite 3.45.1, eight
  replayed Workspaces and `replay_matches=true`.

The full `scripts/check.sh` passed after refreshing generated coverage. It ran Rust formatting,
Clippy, build and 25 workspace Rust tests; six Python tests; architecture validation; plan
and coverage validation (64 documents, 864 sections, 4,798 traceability rows); and
`git diff --check`. Locked SQLx and rusqlite spike test commands also passed after the
feature correction. Default production bundled SQLite remains SQLite 3.53.2 as previously
recorded. Owner-machine acceptance and hosted GitHub Actions remain pending.

Exact review instructions and remaining product limitations are in
[`reviews/E01-S02.md`](reviews/E01-S02.md) and [`spikes/SP02.md`](spikes/SP02.md). SP02 and
E01-S02 remain partial/in progress. The mixed-load correctness and initial pressure sample
is complete, but longer supported-hardware capacity qualification, memory,
cancellation/shutdown, disk-full injection, key-service qualification, key rotation and
cryptographic review are still open.
The storage library is not yet wired into
`litecoworkd`, the desktop, Operator transport, or a real external provider.

## Next actions

1. Inspect all changed paths and review the complete diff before staging.
2. Keep E01-S02 `IN_PROGRESS` and rusqlite provisional pending owner review and remaining
   qualification. Do not infer owner acceptance from local evidence.
3. Owner completes the separate E01-S01 reference-machine and hosted Actions gates; neither
   E01-S01 nor E01-S02 becomes `ACCEPTED` before its stated dependencies and owner review.
4. Do not push without an explicit owner instruction.

## Next implementation scope

After review, continue the next dependency-ready story. Do not add UI, Operator transport,
agent adapters, cloud Runtime, RAG, or production key-service behavior to E01-S02 without a
contract/story change. Re-read `AGENTS.md`, the story, `docs/IMPLEMENTATION.md`,
`docs/STORAGE.md`, `docs/EVENTS.md`, `docs/SERVICES.md`, `docs/PROTOCOLS.md`,
`docs/NETWORK-SECURITY.md`, `docs/schemas/sqlite-v1.sql`, and relevant event schemas before
the next storage domain slice. Preserve the Task/Attempt/Effect/Evidence/Trust/fencing
invariants.
