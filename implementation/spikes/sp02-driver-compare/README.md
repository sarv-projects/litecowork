# SP02 isolated SQLite driver experiment

This experiment runs two separate processes because SQLx 0.9.0 and the current
rusqlite 0.40.2 resolve incompatible `libsqlite3-sys` link versions and cannot be
linked into one Cargo target. Each executable reads the same repository v1 SQLite DDL
and applies the same single-writer transaction shape: read Workspace version, update its
projection, increment the Workspace/origin sequence, insert a domain-event row, then
commit. Both use WAL, `synchronous=FULL`, an eight-producer workload, and a 32-entry
bounded writer queue. Each executable reopens the database and verifies the final version
and event count.

Run the checks and release samples from the repository root:

```sh
cargo test --locked --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml
cargo test --locked --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml
cargo run --release --locked --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml -- 1000
CARGO_TARGET_DIR=implementation/spikes/sp02-driver-compare/target cargo run --release --locked --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml -- 1000
```

This is a directional single-host experiment. It does not implement the product adapter
ports, encrypted aggregate-state BlobStore work, mixed read/write load, memory sampling,
process cancellation, or storage-full injection. The bundled SQLite patch versions also
differ because the dependency link constraints require isolated binaries. These samples
must not decide the production driver without controlling those differences and comparing
the complete adapter workload.
