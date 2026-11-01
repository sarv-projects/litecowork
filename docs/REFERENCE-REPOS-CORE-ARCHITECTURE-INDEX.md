# Reference Repositories — Core Architecture Reading Index

**Status:** research procedure + pinned paths (not normative product)  
**Updated:** 2026-11-01  
**Companion:** [DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md) (ZCode + DeepSeek Harness desktop deep dive)

Reading **every line** of large repos is not required. Before borrowing behavior or tests, read the **core architecture, IPC, persistence, agent loop, and lifecycle** paths below at the pinned revision.

Local clones (gitignored): `repo_take/` per [repo_take/README.md](../repo_take/README.md).

---

## ZCode — `zai-org/ZCode`

| Pin | `a60ba6a7e5a06742270ca605f5d2cc2766afa7e1` (2026-10-30) |
| App path | `packages/desktop` (not `apps/zcode-cli`) |
| License | Apache-2.0 + NOTICE |

### Read first (architecture)

| Path (under pin) | Why |
|---|---|
| `docs/architecture/zcode-code-architecture-overview.md` | Main / Renderer / Host / CLI ownership |
| `packages/desktop/src/main/index.ts` | Shell composition |
| `packages/desktop/src/main/desktopHostProcess.ts` | UtilityProcess Host |
| `packages/desktop/src/host/hostInitialization.ts`, `hostShutdownPhases.ts` | Partial start rollback, teardown |
| `packages/desktop/src/main/pluginSandbox/CONTRACT.md` | Guest sandbox rules |
| `packages/desktop/test/e2e/conversation-session/` | Session USER test themes |

### Dependencies / flows

- CLI owns conversation state; Host routes scopes; Renderer never calls CLI directly.  
- Compare to LiteCowork: **UI ≠ daemon**; Core SQLite truth.

---

## DeepSeek Harness — `deepseek-ai/deepseek-harness`

| Pin | `ce7073d51254c26cd63b22a60835b8f948deb447` (2026-10-30) |
| App path | `apps/desktop` |
| License | MIT |

### Read first

| Path | Why |
|---|---|
| `apps/desktop/src/main.ts` | Window + lifecycle |
| `apps/desktop/src/ipc.ts` | Typed channels + sender assert |
| `apps/desktop/src/preload-app.ts` | contextBridge surface |
| `apps/desktop/src/host-process.ts` | Host lifecycle, deadlines |
| `apps/desktop/src/quit-confirmation.ts` | Active work UNKNOWN UX |
| Desktop tests under `apps/desktop` (ipc, preload, quit) | USER prompts for LiteCowork |

### Gap vs LiteCowork

Task/Effect drain on quit **not integrated** in LiteCowork owners — see audit §4.3.

---

## AionUi — `iOfficeAI/AionUi`

| Pin | `11cd2a9a20d1e033157636efc570b41384c66d85` (2026-11-01) |

### Read first

| Path | Why |
|---|---|
| `docs/contributing/file-structure.md` | process/renderer/common split |
| `packages/desktop/src/common/platform/bridge.ts` | invoke/emit typed bridge |
| `tests/unit/common-platform/bridge.test.ts` | Transport tests |
| `src/process/bridge/`, `src/process/task/`, `src/process/agent/` | IPC surface inventory |

---

## OpenDot — `athulsreekumar/opendot`

| Pin | `ec75891b819713acf29cb659bd9c72d1ed34f76a` (2026-11-01) |

### Read first

| Path | Why |
|---|---|
| Root README architecture diagram | Watchers, Dot Links, SuperDot |
| `desktop/` Electron app | Session UI + Pi integration |
| `~/.opendot` data model (documented) | Local-first audit/masking patterns |

**Not** OpenAI dots; **not** `itsperini/opendot` (voice AGPL).

---

## Eigent — `eigent-ai/eigent`

| Pin | `2ffa2e4ef19356e87479845f2aa1600bc2fde671` |

### Read first

| Path | Why |
|---|---|
| `backend/app/utils/workforce.py` | Decomposition/retry (reference only) |
| `src/components/Trigger/*` | Automation UI patterns |

---

## OpenWork — `different-ai/openwork`

Read `DESIGN.md` + session/desktop layout; compare lazy capability gateway to CAPABILITY-FABRIC.

---

## Procedure for new rival repo

1. Pin SHA; record in [REFERENCE-IMPLEMENTATIONS.md](REFERENCE-IMPLEMENTATIONS.md).  
2. List **entry points**: main, preload/bridge, agent loop, storage, scheduler.  
3. Add **3–10 files** to this index with *why*.  
4. Document **copy / prototype / reject** in feature `RESEARCH.md`.  
5. Add promoted USER journeys to [COMPETITOR-PROMOTED-USE-CASES.md](COMPETITOR-PROMOTED-USE-CASES.md).  
6. Update [VIABILITY-VS-COMPETITORS.md](VIABILITY-VS-COMPETITORS.md).

---

## LiteCowork reference desktop audit status

ZCode + DSH: **targeted component audit complete** in DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT (2026-10-31). Re-read Host/IPC if pins change.

AionUi + OpenDot: **2026-11-01** shallow architecture pass; deepen when SDD 008/009/011 enters IN_REVIEW.
