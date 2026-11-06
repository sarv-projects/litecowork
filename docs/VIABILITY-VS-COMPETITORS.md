# LiteCowork Viability vs Competitors (Local Desktop V1)

**Status:** product viability assessment from **design authority** and **dated research** — not implementation proof  
**Updated:** 2026-11-06
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

### OpenBot (nightly-labs/openbot)

Reference pin: `6759fa3ee857f4f4966865e12de99ae84ab6de07` (package 0.62.0, checked 2026-11-06). Its package manifest declares PolyForm Noncommercial 1.0.0; its README describes the product as a development preview and documents a broad danger-full-access / approvalPolicy-never agent mode. Treat it as a source/UI comparison, not an assumption of production security or reusable code. Full details: [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).

| Promoted capability | LiteCowork verdict | Notes |
|---|---|---|
| Compact local-first multi-agent desktop and persistent agent profiles | **PARTIAL / TARGET UX** | Learn profile identity, slim rail, provider-scoped settings; LiteCowork retains its own daemon/Runtime and BYOA model |
| Agent-specific model picker and effort controls | **TARGET** | Compact searchable picker only from exact fresh AgentControlDescriptor; no universal model router; Feature 002 currently descriptor reads return 503 and exact send route remains unqualified |
| Tasks composer, grouped work list and side panel | **TARGET UX** | Adopt list + side detail pattern; map Needs You/Working/Waiting/Ready/Done from LiteCowork owner projections, not OpenBot's enums |
| Provider-specific historical usage and quota readers | **TARGET** | Study per-provider counter semantics, host report and quota separation; LiteCowork keeps its stricter inference/cost-basis and quota ledgers; dashboard remains target-only |
| In-Core visual workflow graph and graph runtime | **NO for V1 Core** | Conflicts with PRODUCT-STRATEGY and CONNECTORS-WORKFLOWS. User authors graphs in external provider UI; LiteCowork invokes existing graphs as a bounded CapabilityInvocation inside Task |
| Agent-edited workflow graph | **EXTERNAL PROVIDER NOW / LITE COWORK DEFERRED** | n8n and Activepieces document provider-native AI/MCP tools to build/edit/test/publish provider-owned graphs. LiteCowork has no production WorkflowBackendAdapter in the source tree; direct LiteCowork Coworker graph mutation remains a future provider-specific authoring capability with separate Trust/approval |
| JavaScript/Python/TypeScript workflow code | **EXTERNAL PROVIDER ONLY** | OpenBot graph schema has no Code node; n8n supports JS/Python, Activepieces CODE steps accept JS/TS and Node-RED Function nodes execute JS. These run under the external provider's own host/credential/network/isolation boundary, not LiteCowork Environment or automatic Core Trust mediation. Check the n8n Python Task Runner advisory GHSA-9pq8-m8gp-4p53 when applicable |
| OpenBot permission model / direct full-access agent execution | **NO** | Development-preview danger-full-access / approvalPolicy-never behavior is incompatible with LiteCowork's Trust, Grant and Environment boundary |

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

## Research / Engineering Gates & Architectural Resolutions

This table is a research/design register. **LOCKED DESIGN** means an owner has chosen an architectural contract; it does not mean code is wired or release-qualified. See [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) for current source evidence and the CODE/SERVED/INTEGRATED/QUALIFIED vocabulary.

| ID | Topic | Architectural Resolution & Design Contract | Current evidence / owner docs |
|---|---|---|---|
| RV-01 | LiteSPM IPC/auth/schema | Upstream current-main 13-tool local MCP stdio bridge; host-specific prompt/tool entry points; Windows owner-only Named Pipe DACL with `peerAuth.verified=false`; LiteCowork still needs pinned schema, identity, approval, receipt and UNKNOWN-recovery qualification. | **UPSTREAM WIRED / LITE COWORK TARGET NOT QUALIFIED** — [LITESPM-LITECOWORK-INTEROP-LLD.md](LITESPM-LITECOWORK-INTEROP-LLD.md), [LITESPM-INTEGRATION-PLAN.md](LITESPM-INTEGRATION-PLAN.md) |
| RV-02 | First native harness/OS/auth route and readiness handshake | Current supervisor dispatches OpenCode through the v2.0.32 server/session path. The earlier Claude Code/Codex first-GA tuple remains a design choice, not current execution evidence. Session reservation/activation/final settlement must be exercised through the real supervisor. | **OpenCode CODE path only; not QUALIFIED** — [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md), [specs/001-conversation-session-supervisor/SPEC.md](../specs/001-conversation-session-supervisor/SPEC.md) |
| RV-03 | Atomic turn completion write | The OpenCode supervisor has a final response/turn-settlement write path. It must atomically preserve the committed semantic message, terminal turn, session and idempotency provenance; crash/restart and ambiguous dispatch still need end-to-end proof. | **CODE / recovery qualification open** — [STORAGE.md](STORAGE.md), [specs/001-conversation-session-supervisor/SPEC.md](../specs/001-conversation-session-supervisor/SPEC.md) |
| RV-04 | Chrome/browser computer-use per OS | Target: isolated Chrome profile/CDP, policy and takeover lease with stale-command fencing. Current Workbench Browser is an iframe/WebView preview, not Chrome automation. | **TARGET; provider not integrated** — [CHROME-BROWSER-LLD.md](CHROME-BROWSER-LLD.md), [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) |
| RV-05 | Environment isolation attestation | Target: platform-specific process/filesystem/network isolation before Attempt admission. A Job Object constrains process lifetime but is not, by itself, a complete filesystem/network/credential sandbox. Current environment/isolation modules are not wired to a qualified Task execution path. | **CODE/TARGET; not qualified** — [ENVIRONMENTS.md](ENVIRONMENTS.md), [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) |
| RV-06 | Teach-to-skill from recording | Target: opt-in action recorder produces a reviewable declarative Routine/Skill proposal; never silently enables it or records secrets/PII. | **DESIGNED SPEC / later slice** — [ROUTINES.md](ROUTINES.md), Feature 010 |
| RV-07 | Dot Links / inter-Coworker message policy | Target: scoped Agent Relay via committed ConversationMessage envelopes and provenance; no private harness transcript/session transfer and no authority inheritance down a delegation tree. | **TARGET / not qualified** — [DELEGATION.md](DELEGATION.md), Feature 004 |
| RV-08 | Voice realtime agent | Optional local audio pipeline or WebRTC media channel, behind its own privacy/permission/provider contract. | **DEFERRED** — [CHANNELS.md](CHANNELS.md), V1 Release Contract |
| RV-09 | Semantic served-handler ↔ UI map | The current semantic map covers the principal user journeys and identifies exact routes/commands where verified; it does not claim that every one of the 229 OpenAPI operations has a UI control. The 173/1/56 route registration counts are separate from UI coverage. | **Documentation traceability, not qualification** — [SEMANTIC-SERVED-UI-MAP.md](SEMANTIC-SERVED-UI-MAP.md), [served-route-inventory.json](schemas/served-route-inventory.json) |
| RV-10 | Stop/update drain with Effect reconciliation | Target: bounded shutdown drain, admission stop, effect reconciliation and explicit UNKNOWN outcomes. Confirm the exact handler/process behavior for each daemon shutdown path before claiming a 60-second guarantee. | **TARGET; not fully qualified** — [RUNTIME-LIFECYCLE.md](RUNTIME-LIFECYCLE.md), [FAILURE-RECOVERY.md](FAILURE-RECOVERY.md) |
| RV-11 | Interactive Artifact OS isolation profile | Target: dedicated isolated execution context with bounded IPC and no host filesystem/network authority. CSP alone is not an isolation boundary; current full interactive Artifact runtime is not release-qualified. | **TARGET; not qualified** — [INTERACTIVE-ARTIFACT-RUNTIME.md](INTERACTIVE-ARTIFACT-RUNTIME.md), [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) |
| RV-12 | Coworker ADR-0024 schema/API | Optional Coworker aggregate with scoped instructions, pinned default AgentBinding, dedicated ContextAttachments, and per-Coworker audit ledger. Current Coworker CRUD/revision/primary records exist, but scoped learning, Coworker-owned Conversation and responsibility execution are not fully integrated. | **TARGET / PARTIAL CODE; NOT QUALIFIED** — [COWORKERS-TARGET.md](COWORKERS-TARGET.md), ADR-0024, [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) |

---

## “Can LiteCowork do everything they can?” (local, viability-only)

**Design intent:** yes for the **ten V1 product areas** in [V1-PRODUCT-RELEASE-CONTRACT.md](V1-PRODUCT-RELEASE-CONTRACT.md) §3, with **qualified** combinations — not every rival feature on day one.

**Not pursuing parity:** cloud-only differentiators, mobile-first channels, shared cloud computers, subscription tier emulation, and orchestration stacks that violate LiteCowork invariants.

**Implementation today:** the active checkout contains the Rust daemon, Tauri/React desktop UI and shared crates, with partial Conversation, Workspace/Resource, Coworker, Automation, Artifact and Operator route implementations. None of the broad competitor-parity journeys is release-qualified as a complete V1 product path. See [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) for the source-backed boundary; no source recovery from archives is required to begin work.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]