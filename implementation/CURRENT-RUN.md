# Current run and agent handoff

Update this file when the active story, implementation state, or handoff changes. It is
operational context; architecture and owning domain contracts remain authoritative.

## Active story

**E01-S01 — Contract/toolchain baseline** (`IN_REVIEW`)

The goal is one reproducible local/CI gate and a real, minimal `litecoworkd` executable
that supports help/version only. This is a build foundation: no listener, database,
Runtime lifecycle, UI, or agent integration is implemented yet.

## Repository state

- Repository: `litecowork`
- Branch: `main`
- Starting HEAD for this run: `a1fb178` (implementation coverage audit)
- Implementation review commit: current local `HEAD` (do not assume it has been pushed).
- V1 delivery order: desktop/local first, cloud continuation second, remote Runtime third.
- The owner reviews and accepts product behavior. Coding agents do not claim owner
  acceptance, provider qualification, or GitHub Actions success from local checks.
- Two read-only subagent reviews were used for bounded E01-S01 scope and toolchain/CI
  review, at the owner's request. Their shared finding was to move storage, generated
  client, and native-provider tests to the stories that implement those systems.

## Implemented in this run

- Pinned Rust 1.98.1 in `rust-toolchain.toml`, Python 3.13.12 in `.python-version`, and
  uv 0.12.23 in `.uv-version`; added reproducible Python validator dependencies in
  `pyproject.toml`/`uv.lock` and a Rust workspace lockfile.
- Added `apps/litecoworkd`, with process-level help, version, and unsupported-argument
  behavior tests, including that secret-shaped argument values are not echoed. It does not
  open a listener, storage, or an agent.
- Added `scripts/check.sh` for exact tool-version checks, Rust format/Clippy/tests/build,
  Python fixture tests, both required contract validators, and whitespace validation.
- Updated the existing GitHub Actions workflow to install the pinned toolchains and run
  the same local check command.
- Added valid/malformed-schema fixtures and wrong-tool-version checks.
- Updated the E01-S01 epic/backlog scope, deferred Node/TypeScript/pnpm pinning to E02-S01,
  and aligned its states with `implementation/PROCESS.md`: `PLANNED`, `READY`,
  `IN_PROGRESS`, `IN_REVIEW`, `BLOCKED`, and owner-approved `ACCEPTED`. Accepted stories
  also require accepted dependencies.
- Regenerated architecture coverage outputs after the implementation-plan changes.

## Verification run

Run from repository root:

```sh
scripts/check.sh
```

Expected local evidence includes:

- Rust 1.98.1, Python 3.13.12, uv 0.12.23.
- 4 Rust CLI integration tests pass.
- 6 Python tests pass, covering valid/malformed schemas, toolchain mismatch rejection, and
  CI/local command version agreement.
- Architecture validator passes all machine/document checks.
- Implementation-plan and generated coverage validation pass.
- `git diff --check` passes.

A disposable local clone of the implementation commit passed `scripts/check.sh` on Linux.
GitHub Actions has not run for this change, and the owner has not yet run the clean-clone
acceptance case on the reference development machine. This packet does not claim either
result. Do not mark the story `ACCEPTED` until the owner has reviewed the change and run
the owner acceptance case in `implementation/epics/E01.md`.

## Owner review still required

1. Review the local commit and confirm the pinned actions/toolchain setup is acceptable.
2. Run `scripts/check.sh`, `cargo run --locked -p litecoworkd -- --help`, and
   `cargo run --locked -p litecoworkd -- --version` in a clean checkout on the reference
   development machine; record OS/tool versions and output. GitHub Actions remains a CI
   check after the branch is pushed through the owner's workflow.
3. Request any corrections, then set E01-S01 to `ACCEPTED` only after the owner evidence
   and CI result are satisfactory. E01-S02 is dependency-gated on that acceptance.

## Next story after E01-S01

E01-S02 implements the storage ports and SQLite write executor. Before coding, reread the
full current contracts in `docs/IMPLEMENTATION.md`, `docs/STORAGE.md`, `docs/EVENTS.md`,
`docs/PROTOCOLS.md`, and the exact `docs/schemas/sqlite-v1.sql` plus event schema sections.
Implement the transaction/recovery slice and its adversarial tests before touching UI,
agent adapters, or cloud work. Run SP02 in that story. Never infer product behavior from
passing architecture validators.
