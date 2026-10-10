# Stack recommendations and qualification decisions

The development toolchain is pinned to Rust 1.98.1, Python 3.13.12, uv 0.12.23,
Node.js 24.21.0, and pnpm 12.10.1. The Rust workspace minimum supported version is
1.88, matching the minimum of the locked `time` release; it is distinct from the pinned
toolchain used for development.
`rust-toolchain.toml`, `.python-version`, `.uv-version`, `pyproject.toml`, and `uv.lock`
are the Rust/Python tooling authorities. Desktop pins are
`apps/litecowork-ui/.node-version`, its `package.json` `packageManager`/`engines` fields,
`apps/litecowork-ui/pnpm-workspace.yaml`, and `apps/litecowork-ui/pnpm-lock.yaml`.
The pnpm config pins the engine-validation target and turns strict dependency engine
checking on. `scripts/check.sh` remains the Rust/Python and
architecture-contract entry point; it does not install or validate the UI toolchain. The
desktop dependency graph was lockfile-resolved with lifecycle scripts disabled, but the
Node 24 UI build has not been run. The desktop supports pnpm only; npm is not a supported
installer because it has a separate lockfile/configuration contract, and pnpm 12 reads
project policy from `pnpm-workspace.yaml`, not npm's `.npmrc`. These pins qualify the
declared toolchain only; they do
not qualify provider behavior or production packaging. Other library/provider choices below remain
candidates until the named spikes produce evidence. Existing contracts remain stable when
a candidate fails qualification.

| Area | Recommended starting point | Reason and qualification |
|---|---|---|
| Runtime/domain | Rust stable, Tokio, serde, tracing; modular `litecoworkd` | Strong typed invariants/process control; no second TS runtime owning Tasks |
| Operator | Tauri 2, React 19, strict TypeScript 5.8, Vite 6, Node.js 24.21.0 LTS, pnpm 12.10.1 | Existing architecture fit; exact dependency graph is in the desktop pnpm lockfile; native-webview differences must be tested on each OS |
| UI state | Generated OpenAPI types; query cache for projections; component-local transient state | Server owns truth; reconnect rehydrates versioned projections |
| UI components | Accessible Radix primitives, CSS tokens/Tailwind where useful | Existing DESIGN-SYSTEM/MOTION drive appearance; no wholesale copied template |
| Local storage | SQLite WAL, FK on, one bounded write executor; rusqlite 0.40.2 is the provisional first adapter | Preserves the single-writer transaction boundary in the current slice; controlled system-SQLite SP02 samples show no decisive driver performance winner, so rusqlite remains provisional |
| Search | SQLite FTS5 for deterministic metadata/text search | Semantic retrieval remains a separate qualified provider |
| Operator transport | Typed HTTP/OpenAPI + resumable event stream through Axum; shared framed Unix IPC for the current Linux/macOS desktop source path | The daemon and Tauri source now use authenticated Unix IPC on Linux/macOS, with peer-UID checks and no bearer/loopback fallback. This source has not been built or OS-qualified. Windows fails closed pending named-pipe DACL/SID support; macOS sandbox bookmark handling remains a separate release gate (SP03). |
| Rust tests | cargo test, proptest, fault injection, fixture/contract suites | Test command/event transaction and legal transitions, not only methods |
| UI tests | Vitest + Testing Library; browser harness Playwright; native app driver where supported | Browser tests cannot prove native Tauri integration; OS-specific real-app test gate |
| Parsing/retrieval | Isolated optional Python provider; Docling candidate plus type-specific parsers | Parsing/OCR libraries are valuable, but untrusted CPU/memory-heavy input stays outside Core |
| Local inference | Qualified existing harness backed by Ollama/LM Studio/llama.cpp | Engine endpoint alone is not an agent; tools, context, cancel, resume need a real harness |
| Vector store | Provider-owned local index; embedded candidate first | Compare quality/delete durability and memory; Qdrant only if measured scale needs it |
| Cloud first | Same Rust daemon in Linux container/VM; Compose and restricted ingress | OS processes/browser/storage require long-lived compute; generic function-only hosting insufficient |
| Cloud persistence | SQLite for one authoritative writer initially; local durable volume + encrypted backup | No shared SQLite on network mounts; Postgres adapter only for proven multiwriter need |
| Portable blobs | Local immutable blobs; S3-compatible port when cloud copy is needed | Domain/event replication, never DB-file synchronization |
| Observability | tracing/OpenTelemetry-compatible export and redacted structured logs | No prompt/secret payload export by default |
| Packaging | Tauri installers + independent daemon service artifacts; locked builds/SBOM/signatures | Owner signing credentials are release dependencies, never stored in repo |

## Pinned development baseline

Rust 1.98.1 and Python 3.13.12 are pinned for the current executable and contract
validators; uv 0.12.23 is pinned for locked Python tooling. The Tauri frontend separately
pins Node.js 24.21.0 in `.node-version`, requires Node `>=24.21.0 <25` through
`package.json` `engines`, and selects pnpm 12.10.1 through `packageManager`.
`pnpm-workspace.yaml` sets `nodeVersion: 24.21.0` and `engineStrict: true`: pnpm rejects
an incompatible project Node engine and fails on incompatible required dependency engines.
Its `pnpm-lock.yaml` records the resolved frontend dependency graph. Update pins only with
a reviewed toolchain change and regenerate the relevant lockfile.

The selected versions follow the official Node.js release table, which identifies
24.21.0 as LTS, and pnpm's 12.10.1 release from 2026-10-06. pnpm 12 supports Node.js 22
and newer; LiteCowork chooses Node 24 LTS for the desktop frontend. Sources:
[Node.js 24.21.0](https://nodejs.org/en/download/archive/v24.21.0),
[Node.js release status](https://nodejs.org/en/about/previous-releases),
[pnpm 12.10.1 release](https://pnpm.io/blog/releases),
[pnpm installation and compatibility](https://pnpm.io/installation), and
[pnpm engineStrict/nodeVersion settings](https://pnpm.io/settings/cli),
[pnpm project configuration](https://pnpm.io/settings),
[npm engines behavior](https://docs.npmjs.com/cli/v11/configuring-npm/package-json#engines),
and [pnpm CI/frozen-lockfile behavior](https://pnpm.io/continuous-integration).

To bootstrap a Linux/macOS checkout, install rustup and the pinned uv version from their
official installers, then run:

```sh
rustup toolchain install 1.98.1 --profile minimal --component clippy --component rustfmt
curl -LsSf https://astral.sh/uv/0.12.23/install.sh | sh
scripts/check.sh
```

For the desktop UI, install Node.js 24.21.0 and pnpm 12.10.1, then from
`apps/litecowork-ui` use `pnpm install --frozen-lockfile`. This is the dependency
bootstrap command, not a claim that the current unverified source builds.

`uv` reads `.python-version` and provisions Python 3.13.12 as needed. Windows setup uses
the official PowerShell installers linked in [SOURCES](SOURCES.md), followed by the same
repository check commands in a supported Bash environment (for example, Git Bash).

Tauri uses the OS webview and supports web frontends; its own documentation also describes
security scoping and distribution. Small framework examples do not predict LiteCowork's
actual package size. [Tauri](https://v2.tauri.app/start/)
SQLite provides full-text search via FTS5; retrieval quality and indexing costs still need
our corpus benchmarks. [SQLite](https://www.sqlite.org/fts5.html)
Docling is a parsing candidate, not a guarantee of lossless office fidelity.
[Docling](https://docling-project.github.io/docling/)

## Spikes with pass/fail outputs

| ID | Experiment | Required output / decision |
|---|---|---|
| SP01 | Tauri+React native shell on Linux; Windows/macOS qualification matrix | Daemon survives UI close; tray/reconnect; install prerequisites; supported driver limitations |
| SP02 | rusqlite vs SQLx, actual contract DDL, encrypted aggregate-state writes, event+projection transactions and reopen/replay | Product rusqlite adapter and recovery checks are implemented; controlled samples use the same system SQLite and production encrypted FileBlobStore, while SQLx remains a less-complete prototype; mixed-load correctness and initial writer-pressure telemetry pass, while longer capacity/backpressure, memory, cancellation/shutdown, disk-full injection, owner-host repeats and production driver choice remain open |
| SP03 | Qualify the integrated authenticated Operator IPC and event reconnect across supported desktop OSes | Build and exercise the Linux/macOS Unix IPC path; prove browser origins cannot invoke privileged commands; validate peer UID, endpoint ownership/mode, stale endpoint cleanup, cancellation and Runtime shutdown. Add Windows named-pipe current-user/logon-session DACL, remote-client rejection and client-token identity before enabling Windows. Qualify macOS sandbox security-scoped bookmark transfer. Until the platform gates pass, do not claim production-ready desktop IPC or safe switching. |
| SP04 | Codex App Server, Claude supported host interface, OpenCode server, Cline | Feature matrix from real runs; select first full-harness adapter and explicit unsupported features |
| [SP05](spikes/SP05.md) | Local-model provider + native-harness qualification on two measured hardware profiles | Exact protocol/security boundary, tool success, context/cancel, time-to-first-token, RSS/VRAM, no-network/no-fallback run; no brand-parity claim |
| SP06 | Parsing/hybrid RAG over gold PDF/office/code/scans/ZIP corpus | Extraction accuracy, citations, deletion receipts, quality/latency vs FTS; choose provider and index |
| SP07 | Linux cloud deployment and disposable second Runtime | Persistence, sandbox isolation, auth/egress, restart, backup; price and limits documented |

Spikes are bounded to one focused experiment and one report. If evidence is insufficient,
record BLOCKED/next experiment rather than endlessly expanding architecture. Linux-first
coding does not eliminate cross-platform provider, accessibility or packaging tests.

## Optimizations worth implementing

Batch journal writes within documented transactions; index stable query predicates;
bound stream queues and context; cache metadata by revision/digest; cancel stale retrieval;
virtualize long lists; use on-demand workers and TTL WarmHolds; separate compute pools for
OCR/embedding; structured browser actions before vision; measured chunk/rerank policies.
Warm wrappers do not preserve cloud-model cache. No premature microservices, Kubernetes,
Postgres, universal model router, or heavy always-on Python sidecar by default.

Native-harness App Server integration should follow its actual protocol instead of a
lowest-common-denominator chat API. [Codex App Server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)
Ollama and LM Studio expose local inference surfaces; qualification still determines the
agent behavior above them. [Ollama](https://docs.ollama.com/api),
[LM Studio](https://lmstudio.ai/docs/developer).


## Agent-module stack boundary

| Area | Starting point | Qualification rule |
|---|---|---|
| Agent registry | Official ACP Registry through bounded HTTPS cache/client | Registry metadata is untrusted discovery only; verify schema/cache/integrity semantics before implementation |
| Agent module registry | Rust trait/object registry inside litecoworkd | No Core/UI branch on Codex/Claude/OpenCode names; ambiguous claims fail closed |
| Agent lifecycle | Per-agent Rust AgentLifecycleAdapter | Install/update/auth/native-config behavior must be versioned and qualified against upstream agent |
| Agent session runtime | Existing AgentAdapter/ACP/native protocol work | AgentControlDescriptor is separate from live session handles |
| Agent secrets | Native harness store or OS-backed LiteCowork SecretStore slot | Raw bytes never enter AgentBinding JSON/events/logs/WebView caches |
| Agent UI | React Agent Registry master-detail + descriptor-driven composer controls | No global provider/model catalog; control presence/values come from selected descriptor |
| Cross-agent packages | LiteSPM + AgentCapabilityBridge | Package lifecycle remains LiteSPM; each agent attachment route is separately qualified |

The concrete current-source refactor map and target function signatures are maintained in
[AGENT-MODULE-AUDIT.md](AGENT-MODULE-AUDIT.md).
