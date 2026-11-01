# LiteCowork Viability vs Competitors (Local Desktop V1)

**Status:** product viability assessment from **design authority** and **dated research** — not implementation proof  
**Updated:** 2026-11-01  
**Scope:** compare **local desktop** capability intent to named rivals. **Cloud execution** (laptop-off continuation, hosted agent computers, multi-device sync) is **explicitly out of scope** for this parity table unless noted as V2.

**Related:** [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md), [COMPETITIVE-RESEARCH.md](COMPETITIVE-RESEARCH.md), [COMPETITOR-PROMOTED-USE-CASES.md](COMPETITOR-PROMOTED-USE-CASES.md), [IMPLEMENTATION-MASTER-PLAN.md](IMPLEMENTATION-MASTER-PLAN.md).

Legend:

| Verdict | Meaning |
|---|---|
| **TARGET** | Documented in LiteCowork owners; viable if implemented and qualified per gates |
| **PARTIAL** | Subset or stricter model; gap called out |
| **DEFER-V2** | Post-V1 or cloud/mobile lane |
| **RESEARCH** | Promotion understood; LiteCowork path not locked — needs spike/owner decision |
| **NO** | Conflicts with architecture/product non-goals |

## Summary counts (honest)

| Verdict | Approx. promoted rival themes |
|---|---|
| TARGET | Ordinary chat, BYO native agents, optional Coworkers, Tasks, Workbench, LiteSPM packages, Connections, automations, memory policy, library artifacts, settings/daemon |
| PARTIAL | Organisation-style multi-agent projects, teach-to-skill, some doc formats, channel ingress |
| DEFER-V2 | Cloud computer while asleep, cross-device chat sync, mobile/WhatsApp agents |
| RESEARCH | LiteSPM wire, first harness tuple, browser/computer OS matrix, voice, OpenDot-style Dot Links |
| NO | Universal model router, hidden second planner, duplicate package manager, fake served UI |

---

## By competitor theme

### Grok Bot

| Promoted capability | LiteCowork verdict | Authority / gap |
|---|---|---|
| Named Bots + optional primary | **TARGET** (optional Coworker) | COWORKERS-TARGET, ADR-0024 pending impl |
| Shared cloud computer across Bots | **NO** for local V1 | Local Environment per scope |
| Local computer separate permission | **TARGET** | ENVIRONMENTS, Trust |
| Routines + Test run + history | **TARGET** | AUTOMATION, ROUTINES |
| Teach workflow → skill | **RESEARCH** | No locked SDD; see UC-SKL-01 |
| Library tab per Bot | **PARTIAL** | Library + Coworker scope |
| Marketplace shortcut | **PARTIAL** | LiteSPM Discover |
| Connect-in-chat resume | **TARGET** | P5 + FLOWS |

### OpenAI dots / ChatGPT Plus / Work

| Promoted capability | Verdict | Notes |
|---|---|---|
| Plus tier models in ChatGPT app | **PARTIAL** | BYO agent routes; not Plus billing emulation |
| Persistent dot + cloud computer | **DEFER-V2** cloud; **TARGET** local Coworker analogue |
| Local computer bridge (desktop open) | **TARGET** | P7; separate from cloud dot |
| Scheduled tasks | **TARGET** | P9 |
| Custom rules (act/ask/hand off) | **TARGET** | Trust + Automation policy |
| Work mode + team tasks | **PARTIAL** | Team/RBAC **DEFER-V2**; local Task **TARGET** |
| Work sync web/mobile | **DEFER-V2** | RUNTIME-MESH V2 |

### Claude Cowork / Claude chat

| Promoted capability | Verdict | Notes |
|---|---|---|
| Unified chat + cowork experience | **TARGET** | Conversation + optional Task |
| VM folder mounts RO/RW | **TARGET** | Environment roots (modes **RESEARCH** detail) |
| Long-running delegated projects | **TARGET** | Task runtime |
| Scheduled Cowork | **TARGET** | P9 |
| Cloud continue away from machine | **DEFER-V2** | Local-first V1 |

### Kimi Work

| Promoted capability | Verdict | Notes |
|---|---|---|
| Desktop agent + dashboard | **TARGET** | BYO Kimi Code route when qualified |
| Scheduled tasks | **TARGET** | P9; vendor miss-on-sleep **honest local daemon** |
| Parallel agents | **PARTIAL** | Delegation profiles; bounded |

### Muse

| Promoted capability | Verdict | Notes |
|---|---|---|
| Goals / Ideas / approvals | **TARGET** | RESPONSIBILITIES, non-authoritative |
| Secure VM cloud surface | **DEFER-V2** | Not local LiteCowork Runtime |
| Mac local files + tabs | **TARGET** | P2/P7 |
| Generated tools self-install | **NO** | LiteSPM review path |

### OpenWork / OpenCowork / opencowork variants

| Promoted capability | Verdict | Notes |
|---|---|---|
| OpenCode/session desktop | **TARGET** | Native harness BYOA |
| Capability search/execute gateway | **TARGET** | CAPABILITY-FABRIC discovery |
| Trace panel default | **NO** as default | Inspector optional |
| OpenCowork doc generation | **PARTIAL** | P6 format-by-format |
| VM/WSL isolation (OpenCoworkAI) | **PARTIAL** | Environment providers **RESEARCH** per OS |

### OpenDot (opendot.live) / Pi-based Dots

| Promoted capability | Verdict | Notes |
|---|---|---|
| 24/7 watchers (email/calendar/files) | **TARGET** | Channels + Automation **RESEARCH** per provider |
| SuperDot + Organisation departments | **PARTIAL** | Coworker + Task graph; not 13 fixed departments |
| Dot Links inter-agent policy | **RESEARCH** | No owner contract yet |
| ~/.opendot plaintext + masking | **PARTIAL** | Local-first **TARGET**; masking policy **RESEARCH** |
| Ollama/local model per Dot | **PARTIAL** | Local model via qualified agent, not global router |

### AionUi / Eigent / CopilotKit OpenDots

| Promoted capability | Verdict | Notes |
|---|---|---|
| Multi-provider desktop shell | **TARGET** | Agent fabric |
| Typed Electron bridge | **TARGET** (pattern) | Tauri + Operator IPC |
| CAMEL workforce orchestration | **NO** as authority | DELEGATION native lead |
| OpenDots Slack/voice template | **RESEARCH** | CHANNELS post-V1 |

### ZCode / DeepSeek Harness (reference desktops)

| Pattern | Verdict | Notes |
|---|---|---|
| Thin shell / Host / CLI authority split | **TARGET** | ARCHITECTURE Operator vs daemon |
| Plugin guest sandbox | **TARGET** | INTERACTIVE-ARTIFACT-RUNTIME |
| Quit drain with Task awareness | **RESEARCH** | RUNTIME-LIFECYCLE gap documented |
| Drop-in React reuse | **NO** | Port patterns only |

---

## LiteCowork cannot match (documented non-goals or V2)

1. **Cloud agent computer runs while the user's laptop is off** (Grok, Dots, Muse VM, Manus cloud).  
2. **Cross-device conversation/task sync** as primary UX (ChatGPT Work sync, Claude cloud).  
3. **Mobile/WhatsApp as first-class agent surface** (Muse).  
4. **Shared cloud filesystem/login between Coworkers** (Grok shared computer).  
5. **Universal subscription-agnostic “all models” picker** bypassing harness routes.  
6. **Second hidden reasoning/planner** replacing native agents.  
7. **Embedded Activepieces/Nango runtime** as Core engine.  
8. **Qualification claims without OS/harness/provider USER evidence**.

---

## Research / engineering gates (viability unproven until closed)

| ID | Topic | Blocker | Owner docs |
|---|---|---|---|
| RV-01 | LiteSPM IPC/auth/schema | No version-pinned protocol probe | LITESPM-*, CAPABILITY-FABRIC |
| RV-02 | First GA harness/OS/auth tuple and readiness handshake | SDD 001 blockers B-02 and B-03. B-01 is only the concurrent-send policy | specs/001 |
| RV-03 | Atomic turn completion write | B-04 | STORAGE, 001 |
| RV-04 | Chrome/browser computer-use per OS | Provider qualification | CHROME-BROWSER-LLD, P7 |
| RV-05 | Environment isolation attestation | Real sandbox proof | ENVIRONMENTS, P4 |
| RV-06 | Teach-to-skill from recording | No SDD | **RESEARCH** |
| RV-07 | Dot Links / inter-Coworker message policy | No SDD | **RESEARCH** |
| RV-08 | Voice realtime agent | Not V1 contract row | **RESEARCH** |
| RV-09 | Semantic served-handler ↔ UI map | Open | [SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md) |
| RV-10 | Stop/update drain with Effect reconciliation | Open vs DSH pattern | RUNTIME-LIFECYCLE |
| RV-11 | Interactive Artifact OS isolation profile | E08 qualification | INTERACTIVE-ARTIFACT-RUNTIME |
| RV-12 | Coworker ADR-0024 schema/API | Pending | COWORKERS-TARGET |

Add rows to this table when a doc section marks **RESEARCH** or **target-only** without a qualification path.

---

## “Can LiteCowork do everything they can?” (local, viability-only)

**Design intent:** yes for the **ten V1 product areas** in [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md) §3, with **qualified** combinations — not every rival feature on day one.

**Not pursuing parity:** cloud-only differentiators, mobile-first channels, shared cloud computers, subscription tier emulation, and orchestration stacks that violate LiteCowork invariants.

**Implementation today:** **none** of the above is QUALIFIED in the active checkout; viability here means **architecture + contracts allow** building and testing UC-* journeys when SDDs complete and source is recovered.
