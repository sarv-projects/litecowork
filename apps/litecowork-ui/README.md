# LiteCowork desktop shell

Implementation owners: E02-S01 (desktop shell) and E02-S03 (local Resource intake and
preview). This README describes current implementation state, not completed release scope.

This directory contains the Tauri 2 + React desktop shell. It can locate and start the
local `litecoworkd` process, display persisted lifecycle status, and list/create local
Workspaces through the daemon's OS-peer-authenticated Unix IPC Operator path on Linux and
macOS. Windows fails closed until named-pipe authentication is implemented. Workspace creation
uses a durable idempotency receipt in the Workspace commit transaction. The daemon opens
encrypted local storage and remains `DEGRADED` because Task recovery and execution are not
implemented. The shell supports a bounded quick Resource import/catalog and a text preview
for small current revisions. Resumable intake, ZIP extraction, Workspace roots, indexing,
search and Task execution remain unavailable; the shell shows no fabricated work.

## Local setup

Prerequisites: Node.js 22+, pnpm 10+, Rust 1.98.1, and the native Tauri prerequisites
for the host OS. The repository pins Rust in `rust-toolchain.toml`; Node and pnpm are
not pinned at repository root yet, so use a current compatible release and record the
exact versions when this app is qualified.

From this directory:

```sh
pnpm install
pnpm tauri dev
```

For development, build the daemon first and set its executable path:

```sh
cargo build -p litecoworkd
LITECOWORKD_PATH=../../target/debug/litecoworkd pnpm tauri dev
```

Build an installer with `pnpm tauri build`. These commands have not yet been qualified
on supported operating systems. Packaged executable lookup checks Tauri resources and
the desktop executable's directory. Development builds may use `LITECOWORKD_PATH` or
`PATH`. Status and startup are native Tauri commands. Workspace reads and creation use
the native Operator client; IPC endpoint details remain outside the WebView. This is still
a narrow Workspace/Resource API slice, not the complete Operator contract. The IPC source
is unbuilt and unqualified; do not treat it as production-ready.

## UI contract

The shell follows the calm neutral design tokens and truthful state/motion rules in
`docs/DESIGN-SYSTEM.md`, `docs/EXPERIENCE.md`, and `docs/MOTION.md`. It uses system fonts
and bundled CSS; runtime CDN assets are not used. This folder is isolated so it does not
change the root Cargo or JavaScript lockfiles.
