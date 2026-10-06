# Current run and agent handoff

Update this file when the active story, implementation state, or handoff changes. It is
operational context; architecture and owning domain contracts remain authoritative.

## Active story

**E01-S02 — Transactional persistence** (`IN_PROGRESS`, owner review pending).

E01-S01 remains `IN_REVIEW`: the owner has not yet completed the clean-clone acceptance on
the reference machine, and hosted GitHub Actions has not run for that work. This S02 work
continues at the owner's direction but does not waive the dependency or claim either story
accepted.

## Repository state

- Repository: `litecowork`
- Branch: `main`
- Base for this S02 working tree: `61b4376c12127664faf7dceca4825569bfdb8cc6`
- This storage slice is committed locally for owner review; the branch was already two commits
  ahead of `origin/main` at the start of the run.
- No push is authorized or performed in this run.
- V1 order remains desktop/local first, cloud continuation second, remote Runtime third.
- Coding agents implement and report evidence; the owner reviews and accepts product
  behavior. Agents do not claim hosted CI, owner-machine acceptance, provider qualification,
  or production readiness from local tests.

## Completed in this run

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
- Updated the storage/event/schema docs, E01-S02 story/backlog, provisional SP02 report,
  research references and generated implementation coverage.
- Added a separate-process SQLx 0.9.0 and rusqlite 0.40.2 SP02 harness. Both apply the
  canonical DDL and exercise an eight-producer, 32-slot bounded writer with WAL/FULL and
  Workspace version/sequence/event transactions. SQLx includes a mid-transaction rollback
  test. Three optimized 1,000-write samples per driver are recorded in `SP02.md`; because
  their bundled SQLite versions differ and the harness excludes encrypted aggregate-state
  blobs, it is explicitly exploratory and does not select the production driver.

## Verification status

The focused replay test has passed on Ubuntu 24.04.4 LTS x86_64 and printed:

```text
storage replay: sqlite=3.53.2, workspace=workspace-persist, version=2, policy=METADATA_ONLY, events=2, replay_matches=true
```

Toolchain observed: Rust 1.98.1, Cargo 1.98.1, uv 0.12.23, Python 3.13.12; rusqlite
0.40.2 with bundled SQLite 3.53.2. The complete `scripts/check.sh` passed after the implementation and coverage inventory
were refreshed: Rust format, Clippy, 24 Rust tests, build, 6 Python tests, architecture
validation, implementation-plan/coverage validation (64 documents, 4,796 rows), and
`git diff --check`. This is local Ubuntu evidence only; owner-machine acceptance and hosted
GitHub Actions remain pending.

Exact review instructions and limitations are in the [E01-S02 review packet](reviews/E01-S02.md).
SP02 remains partial: the separate-process driver sample is not a controlled full-adapter
comparison, and mixed reader/writer behavior, adapter-level backpressure, memory,
cancellation/shutdown, and real disk-full injection remain unqualified. No production
keychain/cloud key-service, key rotation/recovery, or cryptographic review exists. This
library is not yet wired into `litecoworkd`, the desktop, Operator transport, or a real
external provider.

## Next actions

1. Review the local implementation commit and its S02 review packet; do not push without an
   explicit owner instruction.
2. Owner runs the S02 replay command and `scripts/check.sh`, records OS/output/decision in
   `implementation/reviews/E01-S02.md`, and reviews the limitations.
4. Owner separately completes E01-S01 reference-machine acceptance and hosted Actions.
   Neither E01-S01 nor E01-S02 may become `ACCEPTED` before its explicit owner gates and
   dependency conditions are met.
5. If continuing S02 qualification, use the pinned spike commands in
   `implementation/spikes/sp02-driver-compare/README.md`; first control SQLite version and
   include the encrypted aggregate-state blob path before using timings to choose a driver.

## Next implementation scope

After review, continue the next dependency-ready story. Do not add UI, Operator transport,
agent adapters, cloud Runtime, RAG, or production key-service behavior to E01-S02 without a
contract/story change. Re-read `AGENTS.md`, the story, `docs/IMPLEMENTATION.md`,
`docs/STORAGE.md`, `docs/EVENTS.md`, `docs/SERVICES.md`, `docs/PROTOCOLS.md`,
`docs/NETWORK-SECURITY.md`, `docs/schemas/sqlite-v1.sql`, and relevant event schemas before
the next storage domain slice. Preserve the Task/Attempt/Effect/Evidence/Trust/fencing
invariants.
