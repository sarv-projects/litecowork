# V1 Competitive Reference Audit: OmniRoute, OmniRush and VS Code Terminal

Status: source/UX comparison for LiteCowork V1 HLD/LLD, not a feature/implementation claim. Source URLs below were inspected as public GitHub repository documents and selected source files on 2026-10-12. They may change; pin exact upstream commit before porting any algorithm or asset and separately audit license, source security and provider terms.

## Distinguish the three products

OmniRoute (reference inspected: https://github.com/ourines/omniroute, a public fork of https://github.com/diegosouzapw/OmniRoute) is a model/provider routing gateway with a dashboard, compatibility API, quotas, pricing/cost accounting, caching, provider breakers, connection/model failure layers, ACP tool catalog and token telemetry. It is not the LiteCowork durable Work/Task/Effect authority. The cited `ourines` paths reference that fork, not automatically the upstream project. Verify cited files at a pinned fork commit before source-level reuse and record the source repo and commit in the review packet; do not silently substitute upstream paths. Provider/free-tier counts differ between snapshots. Do not hardcode screenshot/README marketing numbers in LiteCowork architecture.

OmniRush (https://github.com/omnirush-ai/omnirush-gui) is a desktop agent/coding interface. The two supplied OmniRush screenshots show a useful calm split chat + destination Workbench with a dedicated right rail and starter prompts. They are layout references, not evidence of its internal implementation, reliability or entitlement. LiteCowork is broader: ordinary noncoding Coworkers, Tasks, Connections, memories, receipts and long-horizon evidence alongside native coding agents. Do not transform LiteCowork's home into six coding-only cards.

VS Code is the reference for user-facing terminal profiles, PTY tabs, shell integration command marks, CWD and persistent sessions—not a reason to adopt Electron internals in Tauri Rust. Detailed docs:
https://code.visualstudio.com/docs/terminal/profiles
https://code.visualstudio.com/docs/terminal/shell-integration
https://code.visualstudio.com/docs/terminal/advanced

## OmniRoute source-level subsystem map and transfer decisions

| OmniRoute source inspected | Concrete verified pattern | LiteCowork adaptation | Must NOT copy |
|---|---|---|---|
| docs/architecture/ARCHITECTURE.md | request logging, usage history, local DB, route/proxy separation; named source file map | Correlate durable Tasks/Turns with qualified native/gateway leaf usage | duplicate gateway/provider OAuth layer |
| docs/architecture/RESILIENCE_GUIDE.md and docs/diagrams/resilience-3layers.mmd | provider breaker vs account cooldown vs model lockout, distinct repair scope | HealthObserver distinguishes agent host, connection and model failures when source reports them | blindly swapping native agent/account after denied operation |
| src/shared/utils/requestTelemetry.ts | distinct parse/validate/policy/resolve/connect/stream/finalize timing phases | timings for Operator/IPC/AgentStart/memory/Provider/Effect/UI paint | use one total model latency as LiteCowork overhead |
| open-sse/utils/usageTracking.ts | usage extraction from provider response; token safety buffer config | preserve provider-reported counts and cache token subsets with provenance | treat safety buffer as billable usage; invent exact native subscription tokens |
| src/domain/costRules.ts and src/lib/db/domainState.ts | budgets, reset windows and persistent cost/state history | price snapshots and reservations scoped to Task/Attempt/budget | assume all agents bill USD per token or plans cost $0 |
| src/shared/utils/circuitBreaker.ts | CLOSED/DEGRADED/OPEN/HALF_OPEN with transition history | scoped Operator-owned transport health; observe upstream reported states | run model/provider breakers inside Core without source authority |
| docs/reference/CLI-TOOLS.md | CLI Code/CLI Agent/ACP agent are distinct catalogs; batch status separates installed/runnable/config | separate native AgentModule discovery/readiness state axes, batch probes and stale TTL | equate CLI configured for gateway with native ACP support or agent installed |
| docs/guides/CLI-INTEGRATIONS.md | dry-run native config writers versus environment-only launchers | reviewed native setup/repair with before/after diff and no surprise config edits | edit global vendor settings without user confirmation |
| src/shared/constants/providers.ts | provider catalog metadata with labels/logo, capability tags, risk notice | approved logo asset identity and manifest metadata | dynamic remote icon/SVG execution or provider spoofing |
| open-sse/services/signatureCache.ts | detection of model thinking tags, not general content memoization | no direct transplant; native semantic event adapter remains owner | confuse signature-pattern cache with safe replay of LLM results |

The 3-layer OmniRoute breaker is specific to provider routing. LiteCowork's source-of-truth recovery is Task/Attempt/Effect and native agent session control; provider-level breakers only make sense if its configured gateway or native provider emits real status. The gateway could be an optional user-chosen Route via AgentModule configuration, never LiteCowork's mandatory agent provider.

The OmniRoute documentation refers to in-memory/DB usage, batch detection and price rules. It does NOT prove those signals cover native agents run outside OmniRoute. LiteCowork must display UNKNOWN appropriately.

## Screenshot-to-LiteCowork UI corrections

| Reference screenshot element | Adopt | Additional LiteCowork truth |
|---|---|---|
| Memory empty state/search/Add Memory | simple, discoverable | scope Personal/Coworker/Workspace; source/provenance/revoke; native memory separate |
| Overview/Evals/Search/Utilization tabs | compact dashboard tabs | diagnostics linked to Task/Attempt/Evidence and owner scope |
| Usage totals with 1D/7D/30D/90D/YTD/All/Custom | same range control principle, density | unknown vs zero, actual vs estimated costs, subscription coverage |
| 2-column agent cards with logo/version/detection/config | offline trusted icon, status and repair CTA | native AgentModule authority, OS and WSL discovery, auth/readiness separate |
| OmniRush center composer | calm broad objective input, Workspace, Agent + Model controls | model options follow selected native Agent; not a hard-coded one-provider model |
| OmniRush right-side destination panel | single resizable Workbench with Browser/Files/Library | same host also offers Terminal, Computer, Inspector, Artifacts/MCP; real qualified providers |
| Prompt-start task cards | useful examples | default varied: organize files, research, planning, reports, automations, code when appropriate |

Distinct brand/logo, light/dark/contrast, responsive 900px split policy, keyboard/focus and motion semantics remain owned by DESIGN-SYSTEM and DESKTOP-SHELL-LLD. Do not copy logos or pixel layouts from screenshots without rights/asset validation.

## Source citations for implementers

- https://github.com/ourines/omniroute/blob/main/docs/architecture/ARCHITECTURE.md
- https://github.com/ourines/omniroute/blob/main/docs/architecture/RESILIENCE_GUIDE.md
- https://github.com/ourines/omniroute/blob/main/src/shared/utils/requestTelemetry.ts
- https://github.com/ourines/omniroute/blob/main/open-sse/utils/usageTracking.ts
- https://github.com/ourines/omniroute/blob/main/src/shared/utils/circuitBreaker.ts
- https://github.com/ourines/omniroute/blob/main/src/domain/costRules.ts
- https://github.com/ourines/omniroute/blob/main/src/lib/db/domainState.ts
- https://github.com/ourines/omniroute/blob/main/docs/reference/CLI-TOOLS.md
- https://github.com/ourines/omniroute/blob/main/docs/guides/CLI-INTEGRATIONS.md
- https://github.com/ourines/omniroute/blob/main/src/shared/constants/providers.ts
- https://github.com/omnirush-ai/omnirush-gui/blob/main/AGENTS.md

Design owners: V1-TELEMETRY-USAGE-RESILIENCE-LLD.md, V1-AGENT-MEMORY-FAST-PATH-LLD.md, V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md, V1-INTEGRATED-TERMINAL-LLD.md.
