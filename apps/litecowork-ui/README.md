# LiteCowork desktop shell

Implementation owners: E02-S01 (desktop shell) and E02-S03 (local Resource intake and
preview). This README describes current implementation state, not completed release scope.

This directory contains the Tauri 2 + React desktop Operator. Current source can locate
and start local `litecoworkd`, show lifecycle/readiness state, and use the authenticated
local Operator transport on Linux/macOS; Windows still fails closed until named-pipe
peer authentication is implemented. The UI now contains partial source slices for
Workspace setup/instructions, resumable Resource upload and revision editing, persistent
folder-scope registration, metadata/on-demand/indexed plain-text search, Goals,
Suggestions, Coworker settings, Routines, Automation definitions plus one-shot Manual
runs, Needs You, Conversation catalog/read views, agent catalog settings, Artifact
Library/Workbench, and typed Presentation rendering. These source surfaces are not a
release-complete product and many remain unverified or deliberately read-only.

Important limits remain explicit: provider-backed Conversation send and native Task/Attempt
execution are not qualified; Coworker-owned Conversation association, automatic memory,
StandingResponsibilities, recurring/provider trigger hosting, unified Browser/Computer/
Terminal Workbench surfaces, ZIP extraction, and folder watching/crawling are not complete.
A registered persistent folder is therefore a saved scope, not proof that its contents are
being watched or indexed. The shell shows blockers/empty states rather than fabricated work.

## Local setup

Prerequisites: Node.js 24.21.x (`>=24.21.0 <25`), pnpm 12.10.1, Rust 1.98.1, and the
native Tauri prerequisites for the host OS. The desktop `package.json`, `.node-version`,
`pnpm-workspace.yaml`, and `rust-toolchain.toml` are the version authorities. pnpm is the
supported package manager; do not treat successful source-only checks on an older local
Node installation as release qualification.

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
the native Operator client; IPC endpoint details remain outside the WebView. This remains a partial desktop integration rather than the complete Operator contract.
Linux/macOS IPC and multiple UI slices have compiled/test evidence recorded in
`implementation/CURRENT-RUN.md`, but supported-OS system/user qualification is incomplete;
Windows IPC remains unavailable. Do not infer production readiness from source presence or
a successful frontend build.

## UI contract

The shell follows the calm neutral design tokens and truthful state/motion rules in
`docs/DESIGN-SYSTEM.md`, `docs/EXPERIENCE.md`, and `docs/MOTION.md`. It uses system fonts
and bundled CSS; runtime CDN assets are not used. This folder is isolated so it does not
change the root Cargo or JavaScript lockfiles.
