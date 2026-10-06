# Stack recommendations and qualification decisions

The initial development toolchain is now pinned: Rust 1.98.1, Python 3.13.12, and uv
0.12.23. `rust-toolchain.toml`, `.python-version`, `.uv-version`, `pyproject.toml`, and
`uv.lock` are the authorities; `scripts/check.sh` is the local/CI entry point. Node,
TypeScript, and pnpm are intentionally deferred to E02-S01, when the actual desktop UI
exists. These pins qualify the build and architecture validators only; they do not qualify
provider behavior or production packaging. Other library/provider choices below remain
candidates until the named spikes produce evidence. Existing contracts remain stable when
a candidate fails qualification.

| Area | Recommended starting point | Reason and qualification |
|---|---|---|
| Runtime/domain | Rust stable, Tokio, serde, tracing; modular `litecoworkd` | Strong typed invariants/process control; no second TS runtime owning Tasks |
| Operator | Tauri 2, React, strict TypeScript, Vite, pnpm | Existing architecture fit; native-webview differences must be tested on each OS |
| UI state | Generated OpenAPI types; query cache for projections; component-local transient state | Server owns truth; reconnect rehydrates versioned projections |
| UI components | Accessible Radix primitives, CSS tokens/Tailwind where useful | Existing DESIGN-SYSTEM/MOTION drive appearance; no wholesale copied template |
| Local storage | SQLite WAL, FK on, one bounded write executor; rusqlite 0.40.2 is the provisional first adapter | Preserves the single-writer transaction boundary in the current slice; controlled system-SQLite SP02 samples show no decisive driver performance winner, so rusqlite remains provisional |
| Search | SQLite FTS5 for deterministic metadata/text search | Semantic retrieval remains a separate qualified provider |
| Operator transport | Typed HTTP/OpenAPI + resumable event stream candidate through Axum | Authenticate even loopback; origin checks; IPC bridge qualification SP03 |
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
validators; uv 0.12.23 is pinned for locked Python tooling. Update these only with a
reviewed toolchain change and regenerated lockfiles.

To bootstrap a Linux/macOS checkout, install rustup and the pinned uv version from their
official installers, then run:

```sh
rustup toolchain install 1.98.1 --profile minimal --component clippy --component rustfmt
curl -LsSf https://astral.sh/uv/0.12.23/install.sh | sh
scripts/check.sh
```

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
| SP03 | Authenticated Operator transport and event reconnect | Threat tests for malicious local page, stolen token, origin/peer identity; choose transport |
| SP04 | Codex App Server, Claude supported host interface, OpenCode server, Cline | Feature matrix from real runs; select first full-harness adapter and explicit unsupported features |
| SP05 | Local-model harness on two measured hardware profiles | Tool success, context/cancel, time-to-first-token, RSS/VRAM, no-network run; no brand-parity claim |
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
