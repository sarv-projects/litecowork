# V1 Feature SDD Register

**Status:** planning register for docs-first checkout  
**Updated:** 2026-11-01  
**Rule:** no material implementation until the row’s SDD is **ACCEPTED/LOCKED** ([SPECIFICATION-STANDARD.md](SPECIFICATION-STANDARD.md)). Until then, global owner docs below are **interim design authority** only.

Engineering checklist (domain, API, events, flows, UI wiring, tests, research): [FEATURE-AND-UI-CONTRACT-COVERAGE.md](FEATURE-AND-UI-CONTRACT-COVERAGE.md). Typed UI↔Core wire index: [UI-CORE-TYPED-CONTRACTS.md](UI-CORE-TYPED-CONTRACTS.md).

## Register

| ID | Working name | V1 release area ([contract](V1-PRODUCT-RELEASE-CONTRACT.md) §3) | Master plan gate | SDD status | Interim global docs (until SDD) | Planned `contracts/ui-behavior.md` |
|---|---|---|---|---|---|---|
| **001** | Conversation Session Supervisor | 1 Universal Chat | P1 Real native Conversation | **ACCEPTED/LOCKED** (Rev 1.0) — [specs/001](../specs/001-conversation-session-supervisor/README.md) | API, NATIVE-COMPOSER-LLD, SERVICES, STORAGE, EVENTS | **Present** |
| **002** | Native agent install, auth, binding | 2 BYO Agent | P1 / agent matrix | **DRAFT** — [specs/002](../specs/002-native-agent-install-auth-binding/README.md) | AGENT-CONTROL, AGENT-FABRIC, AGENT-ACCOUNT-ROUTE-MATRIX | **Present** |
| **003** | Workspace roots and context ingestion | 2–3 files/context | P2 | **DRAFT** — [specs/003](../specs/003-workspace-roots-context-ingestion/README.md) | WORLD-RESOURCES, CONTEXT, STORAGE, API | **Present** |
| **004** | Agent relay (A→B→A) | 2–3 continuity | P3 | **DRAFT** — [specs/004](../specs/004-agent-relay/README.md) | DELEGATION, NATIVE-COMPOSER-LLD, FLOWS | **Present** |
| **005** | Safe Task path (plan, env, one edit) | 6 Tasks | P4 | **DRAFT** — [specs/005](../specs/005-safe-task-path/README.md) | TASK-RUNTIME, ENVIRONMENTS, TRUST, DELEGATION | **Present** |
| **006** | Golden Connection + one capability write | 5 Capabilities | P5 | **DRAFT** — [specs/006](../specs/006-golden-connection-capability-write/README.md) | CONNECTORS-WORKFLOWS, CAPABILITY-FABRIC, NETWORK-SECURITY | **Present** |
| **007** | Professional Artifacts (one format) | 9 Library | P6 | **DRAFT** — [specs/007](../specs/007-professional-artifacts/README.md) | ARTIFACTS-EVIDENCE, PRESENTATION-RUNTIME | **Present** |
| **008** | Workbench browser/terminal/computer | 4 Workbench | P7 | **DRAFT** — [specs/008](../specs/008-workbench-browser-terminal-computer/README.md) | DESKTOP-SHELL-LLD, ENVIRONMENTS, CHROME-BROWSER-LLD, V1-INTEGRATED-TERMINAL-LLD | **Present** |
| **009** | Coworkers + scoped memory (minimal) | 3, 8 | P8 | **DRAFT** — [specs/009](../specs/009-coworkers-scoped-memory/README.md) | COWORKERS-TARGET, RESPONSIBILITIES, LEARNING, ADR-0024 | **Present** |
| **010** | Automations (recurring host) | 7 | P9 | **DRAFT** — [specs/010](../specs/010-automations-recurring-host/README.md) | AUTOMATION, ROUTINES | **Present** |
| **011** | Desktop shell, IPC, settings, runtime health | 10 | P0–P11 | **DRAFT** — [specs/011](../specs/011-desktop-shell-ipc-settings-runtime/README.md) | RUNTIME-LIFECYCLE, LOCAL-OPERATOR-IPC, DESKTOP-SHELL-LLD, V1-SETTINGS-CONTRACT-LEDGER | **Present** |

## What “complete documentation” means here

| Layer | Complete for V1 design? | Complete for implementation handoff? |
|---|---|---|
| Global domain + OpenAPI + events + flows | **Yes** (TARGET; semantic handler map open) | Partial |
| UI ledger + screen contract + typed wire index | **Yes** (TARGET) | Partial |
| Competitor research (rivals list) | **Yes** (dated; refresh before lock) | N/A |
| **Per-row SDD package** | **DRAFT** — 001–011 exist and structurally validate | **No** — none are ACCEPTED/LOCKED; blockers remain in each SPEC §5 |

Packages **001–011** are DRAFT and structurally valid. They are not accepted. Creating them did not lock owner decisions; each SPEC §5 lists blockers. Copy [.specify/templates](../.specify/templates/) only when a new ID is added.

## Research obligation per new SDD

Each new package `RESEARCH.md` must include:

1. Pinned SHAs for any studied repo ([REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md)).  
2. Dated vendor doc URLs for named rivals relevant to that slice.  
3. Explicit **copy / prototype / reject** decision vs LiteCowork invariants ([ARCHITECTURE.md](../ARCHITECTURE.md)).  
4. Open questions with named owner and gate (not silent TBD in code).

## Tests per feature

Global layers: [TESTING.md](TESTING.md). Each SDD `TASKS.md` must name executable commands and evidence paths. Each `contracts/ui-behavior.md` must cite `UIV1-*` and feature AC IDs. USER/E2E: [ACCEPTANCE-AND-E2E-PLAN.md](ACCEPTANCE-AND-E2E-PLAN.md).
