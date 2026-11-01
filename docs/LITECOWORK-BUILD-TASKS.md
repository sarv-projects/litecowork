# LiteCowork Build TASKS — From Docs-First to Qualified V1

**Status:** master delivery checklist (docs + engineering); not a git commit plan  
**Updated:** 2026-11-01  
**Authority order:** [AGENTS.md](../AGENTS.md), [IMPLEMENTATION-MASTER-PLAN.md](IMPLEMENTATION-MASTER-PLAN.md), [V1-FEATURE-SDD-REGISTER.md](V1-FEATURE-SDD-REGISTER.md), owner contracts.

Use this file as the **single follow list** from empty checkout to qualified desktop V1. Feature-level task IDs live in each `specs/NNN-*/TASKS.md` once packages exist.

**USER test catalog:** [COMPETITOR-PROMOTED-USE-CASES.md](COMPETITOR-PROMOTED-USE-CASES.md)  
**Viability gaps:** [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md)

---

## Phase 0 — Documentation and recovery truth

| ID | Task | Exit evidence |
|---|---|---|
| BT-0.1 | Confirm branch, SHA, ignored archives, preserved worktree boundary | [CURRENT-RUN.md](../CURRENT-RUN.md) |
| BT-0.2 | Recover implementation source into **clean worktree** (not docs-only main) | Record SHA; `apps/` `crates/` present |
| BT-0.3 | Re-run structural validators (feature package, links, OpenAPI name reconciliation) | Commands in CURRENT-RUN |
| BT-0.4 | Complete [SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md) for all shell surfaces | Map rows SERVED/TARGET/UNAVAILABLE |
| BT-0.5 | Pin and refresh rival repos (ZCode, DSH, AionUi, OpenDot, …) per [REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md](REFERENCE-REPOS-CORE-ARCHITECTURE-INDEX.md) | SHAs in REFERENCE-IMPLEMENTATIONS |
| BT-0.6 | Lock SDD **001** (resolve B-01–B-10) → IN_REVIEW → ACCEPTED/LOCKED | **COMPLETED** — SDD 001 is ACCEPTED/LOCKED (Rev 1.0) with B-01 through B-10 resolved |

**Do not start product coding on main while BT-0.2 and BT-0.6 are open.**

---

## Phase 1 — P1 Real native Conversation (SDD 001)

| ID | Task | Docs / code owners | Test UC-IDs |
|---|---|---|---|
| BT-1.1 | Implement supervisor + served Send path per locked 001 | NATIVE-COMPOSER, SERVICES, STORAGE | UC-CHAT-01–05 |
| BT-1.2 | Operator stream producers for turn delta/settled | PRESENTATION-RUNTIME | UC-CHAT-01 |
| BT-1.3 | Composer UI wired to typed IPC + OpenAPI | UI-CORE-TYPED-CONTRACTS, ui-behavior 001 | UIV1-016–022 |
| BT-1.4 | Stop/cancel/retry/idempotency USER tests | FAILURE-RECOVERY | FL01–FL05 |
| BT-1.5 | Restart mid-turn recovery | STORAGE, EVENTS | UC-CHAT-01 |

**Exit:** P1 gate in IMPLEMENTATION-MASTER-PLAN; CODE+SYSTEM+USER on one harness/OS tuple.

---

## Phase 2 — P2 Local files and context (SDD 003)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-2.1 | Author and lock SDD 003 | — |
| BT-2.2 | WorkspaceRoot select/revoke/scan | UC-FILE-01–02, UC-FILE-06 |
| BT-2.3 | Context attachment + citation USER | UC-CHAT-06 |

---

## Phase 3 — P3 Agent relay (SDD 004)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-3.1 | Lock SDD 004 | — |
| BT-3.2 | A→B→A same Conversation USER | IMPLEMENTATION-MASTER-PLAN P3 |

## Phase 4 — P4 Safe Task path (SDD 005)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-4.1 | Lock SDD 005 | — |
| BT-4.2 | Plan accept + one Environment + one bounded edit | UC-WORK-01–02, UC-FILE-01 |
| BT-4.3 | Verification independent of model text | TASK-RUNTIME |

---

## Phase 5 — P5 Connections + capability (SDD 006)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-5.1 | LiteSPM protocol probe (RV-01) | — |
| BT-5.2 | Lock SDD 006 | — |
| BT-5.3 | One Connection read + one approved write | UC-CONN-01–04, UC-MCP-01 |
| BT-5.4 | Connect-in-chat resume | UC-CONN-01 |

---

## Phase 6 — P6 Artifacts (SDD 007)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-6.1 | Lock SDD 007 | — |
| BT-6.2 | One professional format round-trip | UC-FILE-03 |
| BT-6.3 | DSH/ZCode file handoff patterns if adopted | UC-FILE-05 |

---

## Phase 7 — P7 Workbench browser/terminal/computer (SDD 008)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-7.1 | Lock SDD 008 | — |
| BT-7.2 | Takeover/return epochs | UC-BRW-01, UC-CMP-01 |
| BT-7.3 | Terminal in admitted Environment | UC-TRM-01 |
| BT-7.4 | Optional Inspector trace | UC-WORK-07 |

---

## Phase 8 — P8 Coworkers + memory (SDD 009)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-8.1 | ADR-0024 schema/API promotion | RV-12 |
| BT-8.2 | Lock SDD 009 | — |
| BT-8.3 | Quick Create → chat; revoke scope | UC-CHAT-04, UC-MEM-01–03 |
| BT-8.4 | Organisation-like project (**partial**) | UC-WORK-05 |

---

## Phase 9 — P9 Automations (SDD 010)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-9.1 | Lock SDD 010 | — |
| BT-9.2 | Schedule + misfire + one Task per occurrence | UC-AUTO-01–08 |
| BT-9.3 | Routine test-run semantics | UC-AUTO-01 |

---

## Phase 10 — P2 BYO Agent depth (SDD 002)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-10.1 | Lock SDD 002 | — |
| BT-10.2 | Second qualified AgentModule | IMPLEMENTATION-MASTER-PLAN P3 precursor |
| BT-10.3 | Settings agents/auth flows | UC-SET-04, ledger rows |

---

## Phase 11 — P11 Shell, settings, runtime (SDD 011)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-11.1 | Lock SDD 011 | — |
| BT-11.2 | All **187** settings controls: served or honest unavailable | SETTINGS-AND-SHELL-SURFACE-AUDIT |
| BT-11.3 | Daemon start/stop/recovery UX | UC-SET-04 |
| BT-11.4 | Accessibility/keyboard USER on shell | V1-UI-SCREEN-CONTRACT |

---

## Phase 12 — Release qualification (P11 exit)

| ID | Task | Test UC-IDs |
|---|---|---|
| BT-12.1 | Run full **UC-*** catalog applicable to declared matrix row | COMPETITOR-PROMOTED-USE-CASES |
| BT-12.2 | V1 release matrix green per OS/agent/provider | ACCEPTANCE-AND-E2E-PLAN |
| BT-12.3 | Update evidence ledgers; no TARGET labeled QUALIFIED without USER | BENCHMARKS policy |

---

## Ongoing documentation tasks (parallel)

| ID | Task |
|---|---|
| BT-DOC-1 | On each SDD lock, add `contracts/ui-behavior.md` + RESEARCH rival SHAs |
| BT-DOC-2 | On each new OpenAPI route, update SEMANTIC-SERVED-UI-MAP + V1-UI-ACTION-STATE-LEDGER |
| BT-DOC-3 | On LiteSPM probe, update CAPABILITY-FABRIC + RV-01 |
| BT-DOC-4 | Quarterly refresh COMPETITIVE-RESEARCH primary rivals index |

---

## SDD package creation order (recommended)

1. **001** (in progress) → 2. **011** (shell/IPC/settings) → 3. **002** (BYO) → 4. **003** (context) → 5. **005** (Task) → 6. **006** (connections) → 7. **008** (Workbench) → 8. **009** (Coworkers) → 9. **010** (automations) → 10. **004** (relay) → 11. **007** (artifacts)

Adjust only with owner approval; register changes in [V1-FEATURE-SDD-REGISTER.md](V1-FEATURE-SDD-REGISTER.md).
