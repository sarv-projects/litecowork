# SP02 SQLite driver comparison

This directory contains separate SQLx and rusqlite executables. They cannot be linked
together because their `libsqlite3-sys` version requirements conflict. Each runs in its
own Cargo workspace, lockfile, binary, and target directory.

Both executables use the same Workspace domain service, eight-producer update workload,
32-slot writer queue, WAL/FULL durability settings, and production encrypted
`FileBlobStore` implementation. The rusqlite executable calls the actual product
`SqliteWorkspaceStore`; the SQLx executable uses an experimental adapter around SQLx and
the same WorkspaceStore ports. Both initialize from `docs/schemas/sqlite-v1.sql`, close
and reopen the database, decrypt/replay aggregate-state blobs and compare the replayed
projections. SQLx migration receipts/schema-drift checks and some other product adapter
hardening are not implemented, so this experiment does not establish full adapter parity.

The production `storage-sqlite` default remains bundled SQLite. Only these comparison
executables disable bundling and use the host library. SQLx must enable `sqlite-unbundled`
without the SQLx `sqlite` convenience feature because that convenience feature also enables
bundled SQLite. Before measuring, verify both executables report the same result from
`SELECT sqlite_version()` and dynamically load the same system library.

## Commands

Run focused tests from the repository root:

```sh
cargo test --no-default-features --features sqlite-rusqlite-defaults -p storage-sqlite
cargo test --locked --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml
cargo test --locked --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml
```

Build optimized executables:

```sh
cargo build --release --locked --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml
cargo build --release --locked --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml
```

Run a sample (200 updates is the recorded comparison workload):

```sh
implementation/spikes/sp02-driver-compare/target/release/sp02-driver-compare 200
implementation/spikes/sp02-driver-compare/rusqlite/target/release/sp02-rusqlite-qualification 200
```

Repeat three times per driver, alternating driver order. Each reported end-to-end update
latency includes the domain service, canonical aggregate serialization, encrypted
content-addressed blob write and readback verification, and database transaction. It does
not include workspace setup or reopen/replay verification. See [`../SP02.md`](../SP02.md)
for the recorded environment, results, limitations and provisional decision.
