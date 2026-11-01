# Feature and UI–Core Contract Coverage

**Status:** normative navigation and gap register (docs-first checkout)  
**Updated:** 2026-11-01  
**Purpose:** answer whether each engineering concern you listed already lives in `docs/`, `specs/`, or machine schemas; where it is partial; and what must be added per feature before implementation. This does not authorize implementation or claim QUALIFIED product behavior.

Research is **ongoing**. Treat competitor and reference-repo findings as dated evidence in [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md) and [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md), not as product requirements. Re-run research when a feature enters `IN_REVIEW`.

## 1. Global concern → authority map

| Your concern | Where it is defined today | Completeness | Notes |
|---|---|---|---|
| **Data schema and core domain model** | [DATA-MODEL.md](DATA-MODEL.md), [STATE-MACHINES.md](STATE-MACHINES.md), [SCHEMAS.md](SCHEMAS.md), `docs/schemas/*.json`, `docs/schemas/sqlite-v*.sql` | **Strong TARGET** | Entity ownership, fields, invariants and migrations are broad. Semantic alignment of every field to a served handler is **not** closed ([CURRENT-RUN.md](../CURRENT-RUN.md)). |
| **API contracts (OpenAPI)** | [API.md](API.md), `schemas/operator-api.openapi.yaml`, [PROTOCOLS.md](PROTOCOLS.md) | **Strong structural** | 229 operations reconciled to prose (audit §4.7). Many routes are **target-only** or unmounted; UI must not call them as live ([API.md](API.md) status tables). |
| **Events and integration boundaries** | [EVENTS.md](EVENTS.md), `schemas/domain-event.schema.json`, [SERVICES.md](SERVICES.md), [FAILURE-RECOVERY.md](FAILURE-RECOVERY.md), per-domain LLDs | **Strong names** | 148 versioned event types aligned to schema. UI reducer actions are **not** domain events ([EVENTS.md](EVENTS.md), [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md)). |
| **Key flows and edge cases** | [FLOWS.md](FLOWS.md), [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md), [FAILURE-RECOVERY.md](FAILURE-RECOVERY.md), domain owners | **Strong design** | FL01–FL25 and race classes are **acceptance targets**, not verified ([V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md)). |
| **HLD: components, boundaries, rationale** | [ARCHITECTURE.md](../ARCHITECTURE.md), [COVERAGE-MATRIX.md](COVERAGE-MATRIX.md), [adr/](adr/README.md), domain `*.md` owners | **Strong** | ADRs explain *why*; architecture explains *who owns what*. |
| **Requirements, acceptance, edge cases per feature** | [SPECIFICATION-STANDARD.md](SPECIFICATION-STANDARD.md), `specs/NNN-*/` packages, [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md) ten areas | **Partial** | DRAFT packages `specs/001` through `specs/011` exist. None are ACCEPTED/LOCKED. V1 release areas are product-level acceptance, not a substitute for those EARS. |
| **Market / competitor research** | [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md), [COWORKERS-TARGET.md](COWORKERS-TARGET.md) §12, [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md) | **Strong but dated** | Rivals you named are mostly covered; **AionUi** added 2026-11-01 (this pass). Re-check vendor pages and repos before copying UX. |
| **Prototype → main wiring discipline** | [IMPLEMENTATION-MASTER-PLAN.md](IMPLEMENTATION-MASTER-PLAN.md), [IMPLEMENTATION.md](IMPLEMENTATION.md) §Documentation gates, [SPECIFICATION-STANDARD.md](SPECIFICATION-STANDARD.md) | **Defined process** | Spikes belong in isolated worktrees; promote only through owner + schema + SDD lock. Historical `docs/superpowers/plans/` are **not** full packages. |
| **Tests per feature** | [TESTING.md](TESTING.md), [ACCEPTANCE-AND-E2E-PLAN.md](ACCEPTANCE-AND-E2E-PLAN.md), [BENCHMARKS.md](BENCHMARKS.md), spec `TASKS.md` / EARS | **Layered TARGET** | Global test architecture exists. Per-feature commands live in DRAFT packages 001–011. UI ledger cites `UIV1-*` IDs without implying tests exist in this checkout. |
| **UI behavior (not pixels)** | [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md), [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md), [EXPERIENCE.md](EXPERIENCE.md), [NATIVE-COMPOSER-LLD.md](NATIVE-COMPOSER-LLD.md) | **Strong TARGET** | Screen contract + action ledger + shell LLD cover flows, states, validation posture. Each package 001–011 has `contracts/ui-behavior.md`. |
| **Typed commands/queries (UI → core)** | [API.md](API.md) + OpenAPI, [LOCAL-OPERATOR-IPC.md](LOCAL-OPERATOR-IPC.md), Tauri native bridge (historical paths in [NATIVE-COMPOSER-LLD.md](NATIVE-COMPOSER-LLD.md)) | **Typed at wire** | Operator HTTP/OpenAPI is canonical for mutations; IPC is OS-authenticated transport to same handlers. UI must use **operationId-level** contracts, not ad hoc strings ([DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md) Settings pattern). |
| **Typed events (core → UI)** | [PRESENTATION-RUNTIME.md](PRESENTATION-RUNTIME.md) `OperatorStreamFrame`, domain events → projections, [EVENTS.md](EVENTS.md) | **Partial TARGET** | Stream frame types defined; several producers **not mounted** in historical baseline. UI reconciles projections + stream; must not parse free-text errors. |
| **Shared error shapes** | [SCHEMAS.md](SCHEMAS.md), `schemas/error-codes.schema.json`, OpenAPI error responses | **Strong** | 131 codes in schema and prose. UI renders `failure_code` / typed API error bodies ([V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) universal contract). |
| **UI renders state + sends commands** | [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md), [IMPLEMENTATION.md](IMPLEMENTATION.md) definition of done §7, [EXPERIENCE.md](EXPERIENCE.md) | **Normative intent** | Reducer/projections pattern documented; **no** shipped UI in active checkout to enforce it. |

## 2. Per-feature documentation checklist (required before IMPLEMENTING)

Every material feature under `specs/NNN-name/` must include the [Specification Standard](SPECIFICATION-STANDARD.md) package **plus** the UI column below. Use [IMPLEMENTATION.md](IMPLEMENTATION.md) definition-of-done as the engineering gate.

| Artifact | Must contain |
|---|---|
| `SPEC.md` | Problem, goals, HLD/LLD, security, errors/recovery, **EARS** acceptance, traceability |
| `DATA-MODEL.md` | Records touched, lifecycle, persistence, events (link global owners) |
| `contracts/api-events.md` | Exact OpenAPI operationIds, request/response, domain events emitted/consumed |
| `contracts/errors-recovery.md` | Typed failures, retries, races, restart behavior |
| **`contracts/ui-behavior.md`** | **Flow, screen transitions, all UI states, validation messages, command + stream mapping** (behavior only) |
| `TASKS.md` | Tests and evidence paths per task |
| `RESEARCH.md` | Dated competitor/reference repo findings; prototype outcome |
| `WIRING.yaml` | Components, transactions, external boundaries |

**UI spec rules (behavior, not pixels):**

1. UI **displays** authoritative projections and **sends** typed commands; it does not own durable truth.
2. Every user action maps to **one** OpenAPI operation (or explicit local-only reducer action that does not mutate domain state).
3. Every async surface lists: `empty`, `loading`, `error`, `partial`, `offline`, `long-running` (+ domain-specific: `STALE`, `AMBIGUOUS`, `NEEDS_AUTH`, etc.).
4. Failures use **shared error codes**; no regex on English strings.
5. Cross-link [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) rows or add feature-specific `UIV1-*` extensions in the feature spec.

## 3. V1 product areas vs documentation depth

From [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md) §3. “Doc depth” means owner contracts + UI ledger + tests **described**; not shipped.

| # | V1 area | Domain + flow docs | UI behavior docs | SDD package | Test / E2E doc hooks |
|---|---|---|---|---|---|
| 1 | Universal Chat | API, SERVICES, NATIVE-COMPOSER-LLD, FLOWS | Ledger composer/send/stop; screen contract | **`specs/001-*` (DRAFT)** | AC-001–019 in 001; FL01–FL05; UIV1-016–022 |
| 2 | BYO Agent | AGENT-CONTROL, AGENT-FABRIC, route matrix | Ledger agents/settings | None (use P1 slice + future SDD) | TESTING contract adapters |
| 3 | Coworkers | COWORKERS-TARGET, RESPONSIBILITIES, ADR-0024 | Ledger coworker rows | None (pending schema/API) | COWORKERS-TARGET §12 research |
| 4 | Workbench / computer | DESKTOP-SHELL-LLD, ENVIRONMENTS, INTERACTIVE-ARTIFACT-RUNTIME | Ledger Workbench/browser/terminal | None | V1-END-TO-END flows |
| 5 | Capabilities / LiteSPM | CAPABILITY-FABRIC, LITESPM-* | Discover ledger rows | LiteSPM plan scenarios LSI-* | Integration plan gates |
| 6 | Tasks / multi-agent | TASK-RUNTIME, DELEGATION | Work/Task ledger | None | Property tests TASK/Attempt |
| 7 | Automations | AUTOMATION, ROUTINES | Automations ledger | superpowers plan only (legacy) | AUTOMATION owner tests |
| 8 | Memory / knowledge | LEARNING, CONTEXT | Settings memory rows | None | LEARNING policy tests |
| 9 | Library / artifacts | ARTIFACTS-EVIDENCE, PRESENTATION-RUNTIME | Library/artifact ledger | None | Format-specific USER gates |
| 10 | Shell / settings / runtime | RUNTIME-LIFECYCLE, LOCAL-OPERATOR-IPC | 187 settings controls in shell LLD | None | IPC + readiness qualification |

**Gap summary:** global docs cover all ten areas at **TARGET** level; **only Conversation Send** has a feature SDD; **per-feature `ui-behavior.md`** is mandatory for new packages (template: `specs/001-*/contracts/ui-behavior.md`).

## 4. Research and “copy vs prototype vs extend” workflow

Use this order for **every** material feature ([AGENTS.md](../AGENTS.md), [SPECIFICATION-STANDARD.md](SPECIFICATION-STANDARD.md)):

1. **Research (dated)** — vendor docs + pinned git SHAs; record in `RESEARCH.md`. For repos, inventory structure and read boundary files (IPC, state store, task loop); do not treat README marketing as behavior.
2. **Copy (behavior only)** — when a rival solves the same user job and fits LiteCowork invariants (chat-first, local truth, no duplicate orchestration). Document *what* is borrowed in COMPETITIVE-RESEARCH or REFERENCE-IMPLEMENTATIONS; do not copy code without license review.
3. **Prototype** — when behavior is novel or rival patterns conflict with Trust/Task boundaries. Spike in isolated worktree; no promotion without SDD lock.
4. **Extend main wiring** — promote API/event/SQL in **owner** docs + schemas in the same change; update WIRING.yaml and UI ledger rows; add TASKS with exact test commands.

**Primary rivals (your list) — doc coverage:**

| Rival | In repo docs? | Primary authority |
|---|---|---|
| Grok Bot | Yes — deep UX benchmark | COMPETITIVE-RESEARCH § Grok Bot |
| Claude Cowork / Claude chat | Yes | COMPETITIVE-RESEARCH § Claude Cowork; Anthropic sources |
| ChatGPT Plus / Work / Codex / Dots | Yes | COMPETITIVE-RESEARCH § ChatGPT Work, OpenAI dots, **Plus vs Work** (2026-11-01) |
| Kimi Work | Yes | COMPETITIVE-RESEARCH Kimi Work; COWORKERS-TARGET |
| Muse assistant | Yes | COMPETITIVE-RESEARCH § Meta Muse |
| OpenAI dots | Yes | COMPETITIVE-RESEARCH § OpenAI dots |
| **OpenDot (opendot.live)** | **Yes (2026-11-01)** | COMPETITIVE-RESEARCH § OpenDot (`athulsreekumar/opendot`); not OpenAI Dots |
| OpenWork | Yes | COMPETITIVE-RESEARCH § OpenWork |
| OpenCowork / opencowork | Yes (two repos distinguished) | COMPETITIVE-RESEARCH, REFERENCE-IMPLEMENTATIONS |
| Eigent | Yes + pinned files | REFERENCE-IMPLEMENTATIONS REF-EIGENT |
| **AionUi** | **Added 2026-11-01** | COMPETITIVE-RESEARCH § AionUi; REFERENCE-IMPLEMENTATIONS REF-AIONUI |

## 5. UI ↔ core wiring reference (canonical)

```text
WebView UI reducer
  → Tauri native command (narrow, no secrets)
  → LOCAL-OPERATOR-IPC (OS-authenticated)
  → Operator route (OpenAPI operationId)
  → domain service + transaction
  → domain event + projection
  → Operator stream (OperatorStreamFrame) + REST read models
  → UI reducer reconciles snapshot
```

| Direction | Canonical types | Owner doc |
|---|---|---|
| UI → core | OpenAPI operations, `RequestId`, `If-Match`, idempotency keys | [API.md](API.md), [LOCAL-OPERATOR-IPC.md](LOCAL-OPERATOR-IPC.md) |
| Core → UI (durable) | Projections from reads; domain events are not sent raw to WebView | [SERVICES.md](SERVICES.md), [EVENTS.md](EVENTS.md) |
| Core → UI (live) | `OperatorStreamFrame`: STREAM_READY, PROJECTION, TURN_DELTA, TURN_SETTLED, RICH_DRAFT, RESYNC_REQUIRED | [PRESENTATION-RUNTIME.md](PRESENTATION-RUNTIME.md) |
| Errors | `failure_code`, OpenAPI error schema, admission unavailable | [SCHEMAS.md](SCHEMAS.md) |

## 6. Known gaps to close (documentation work)

1. **Semantic OpenAPI/prose/handler alignment** — structural pass done; field-level and UI state mapping open ([CURRENT-RUN.md](../CURRENT-RUN.md)).
2. **SDD packages** — 001–011 exist as DRAFT. None are ACCEPTED/LOCKED, so none authorize coding.
3. **`contracts/ui-behavior.md` per feature** — present on 001–011. Owner lock is still open.
4. **Competitor re-validation** — snapshots are 2026-10-04–2026-10-31; refresh before ACCEPTED/LOCKED on UX-heavy features.
5. **Executable tests in this checkout** — none; test *descriptions* exist in TESTING.md and spec TASKS.

## 7. Related indexes

- [Product Contract Index](PRODUCT-CONTRACT-INDEX.md)
- [V1 Design Index](V1-DESIGN-INDEX.md)
- [V1 Feature SDD Register](V1-FEATURE-SDD-REGISTER.md) — planned packages 001–011 vs V1 areas
- [UI and Core Typed Contracts](UI-CORE-TYPED-CONTRACTS.md)
- [Repository Documentation Audit](REPOSITORY-DOCUMENTATION-AUDIT.md)
- [Feature packages](../specs/README.md)

## 8. Finalization note (2026-11-01)

This document plus [UI-CORE-TYPED-CONTRACTS.md](UI-CORE-TYPED-CONTRACTS.md), [V1-FEATURE-SDD-REGISTER.md](V1-FEATURE-SDD-REGISTER.md), [COMPLETE-ACTIVE-CORPUS-INVENTORY.md](COMPLETE-ACTIVE-CORPUS-INVENTORY.md) (active **and** archive file manifest), and [SPEC-TASK-TRACEABILITY.md](SPEC-TASK-TRACEABILITY.md) close the **documentation mapping** for the engineering checklist. Remaining honest limits:

- **Per-feature SDD:** `001`–`011` are DRAFT packages under `specs/`. None are ACCEPTED/LOCKED. Blockers remain in each SPEC §5.
- **Semantic handler/UI served map:** started in [SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md); row-level completion is BT-0.4 in [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md).
- **Research:** rival sections dated; refresh SHAs before each SDD lock.
- **Runnable product:** docs-first checkout; no CODE/SERVED/QUALIFIED claims.
