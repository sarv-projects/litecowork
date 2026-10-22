# LiteCowork Desktop V1 — Detailed Design Authority Index

Status as of 2026-10-12: normative TARGET design and implementation-oriented contracts. **Not** evidence that an endpoint, provider, UI control, platform, installer, or behavior is implemented or qualified. Current built code and backlog truth stay distinct.

## Final 2026-10-22 product and integration LLDs

- [Ten-area V1 product/release contract](V1-PRODUCT-RELEASE-CONTRACT.md)
- [Chat-first shell and every V1 screen](V1-UI-SCREEN-CONTRACT.md)
- [LiteSPM inventory/adoption/plan/apply integration](LITESPM-LITECOWORK-INTEROP-LLD.md)
- [HorizonCode native chief/worker integration](V1-HORIZONCODE-NATIVE-AGENT-LLD.md)
- [Real product acceptance crosswalk](../implementation/V1-PRODUCT-ACCEPTANCE-MATRIX.md)

These are **TARGET** contracts. They do not supersede durable Task,
Trust, AgentSession, Effect, Evidence or Environment domain authority,
and they must not be interpreted as a working native provider.

## Source of truth and precedence

1. ARCHITECTURE.md and PRODUCT.md define product scope, invariants and owners. A new feature may never bypass Core Trust/Effect/Evidence/Task authority.
2. Domain contracts (DATA-MODEL, STATE-MACHINES, AGENT-CONTROL, AGENT-FABRIC, TASK-RUNTIME, DELEGATION, AUTOMATION, ENVIRONMENTS, CONNECTORS-WORKFLOWS, PRESENTATION-RUNTIME, TRUST, etc.) own durable semantics.
3. docs/schemas, docs/API.md, EVENTS.md, SERVICES.md and STORAGE.md own **promoted and served** wire/transaction contracts. No target LLD silently inserts an API/event or changes an existing canonical enum.
4. The detailed V1 LLD documents below own implementation-specific UI reducers, function responsibility, algorithms, browser/display distinctions and test expectations **within** domain authority.
5. implementation/backlog.json, epics and CURRENT-RUN are implementation status/progress. No implementation or qualified state can be inferred from this document.
6. All external references are version-sensitive. A URL is guidance to revalidate APIs against exact pinned harness/OS build; no speculative operation is permitted merely because vendor docs describe it.

## V1 LLD owners

| Coverage | Detailed normative document | Working implementer |
|---|---|---|
| Agent selection, model/options, /, @, custom native commands, chat send/stop/resume, drafts and parity | [NATIVE-COMPOSER-LLD.md](NATIVE-COMPOSER-LLD.md) | E03-S01/S02/S06–S11, E08 |
| Managed real Chrome, user profiles/import, connect existing Chrome, side browser, takeover and control epochs | [CHROME-BROWSER-LLD.md](CHROME-BROWSER-LLD.md) | E07-S02/S05, E08 |
| Settings, left and right sidebars, responsive geometry, Workbench, focus, all controls and animations | [DESKTOP-SHELL-LLD.md](DESKTOP-SHELL-LLD.md) | E02/E08/E09/E10 |
| Normalization and reliable rendering of arbitrary native-agent outputs, rich fallback and interactive Workbench | [RENDERING-CONFORMANCE-LLD.md](RENDERING-CONFORMANCE-LLD.md) | E03-S11, E08-S06/S07/S08 |
| Conversation/Task/Plan/Step/Attempt/Effect, delegation, workflows, automation, events and recovery | [V1-WORKFLOW-EVENT-LLD.md](V1-WORKFLOW-EVENT-LLD.md) | E03/E04/E05/E09/E10 |
| Exhaustive function ownership, file landing zones and machine contract promotion | [V1-FUNCTION-FILE-CATALOG.md](V1-FUNCTION-FILE-CATALOG.md) | all V1 implementers |
| User flow acceptance/negative testing and real-code gap classification | [implementation/V1-DESIGN-COVERAGE.md](../implementation/V1-DESIGN-COVERAGE.md) | all implementation gates |
| Mechanical all-Markdown-file line/hash/link inventory | [implementation/V1-AUDIT-INVENTORY.md](../implementation/V1-AUDIT-INVENTORY.md) | scripts/audit_v1_markdown.py |
| Settings: each named control, authoritative scope/owner, result and failure/repair state | [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md) | E02/E03/E04/E08/E09/E10 |
| Every shell/central/right-pane/overlay interaction and UI state transitions | [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md) | E02/E03/E07/E08 |
| Every proposed function signature and minimum pre/post/error/race gate | [V1-FUNCTION-CONTRACT-LEDGER.md](V1-FUNCTION-CONTRACT-LEDGER.md) | E01–E10/E13; scripts/build_v1_function_contract_ledger.py |
| Full end-to-end admission/event/recovery handoffs and adverse interleavings | [V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md](V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md) | E03/E04/E05/E07/E08/E09/E10 |
| Cross-agent usage, token/cost, logging, cache, health, transport resilience, diagnostics | [V1-TELEMETRY-USAGE-RESILIENCE-LLD.md](V1-TELEMETRY-USAGE-RESILIENCE-LLD.md) | E04/E05/E08/E13 |
| Native-agent-invoked scoped memory, caching, retrieval latency and injection policy | [V1-AGENT-MEMORY-FAST-PATH-LLD.md](V1-AGENT-MEMORY-FAST-PATH-LLD.md) | E04/E08/E10 |
| Agent auto-discovery, bundled logos, install/auth/readiness status cards and manual-path fallback | [V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md](V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md) | E03/E08 |
| Right Workbench PTY terminal, shell profiles, WSL, tab sessions and OS process ownership | [V1-INTEGRATED-TERMINAL-LLD.md](V1-INTEGRATED-TERMINAL-LLD.md) | E07/E08/E13 |
| Source-linked OmniRoute/OmniRush/VS Code reference evaluation | [V1-REFERENCE-OMNIROUTE-OMNIRUSH-TERMINAL.md](V1-REFERENCE-OMNIROUTE-OMNIRUSH-TERMINAL.md) | E02/E03/E08 |
| Additional test ledger for usage, native detection, memory and terminal | [implementation/V1-OBSERVABILITY-AGENT-MEMORY-TERMINAL-UAT.md](../implementation/V1-OBSERVABILITY-AGENT-MEMORY-TERMINAL-UAT.md) | CODE/SYSTEM/USER gates |
| Cross-owner semantic reconciliation and simulated actual request paths | [implementation/V1-DESIGN-DRYRUN-AND-RECONCILIATION-2026-10-13.md](../implementation/V1-DESIGN-DRYRUN-AND-RECONCILIATION-2026-10-13.md) | Coworker/Memory/Task/Native/Effect/Recovery, scripts/validate_design_owner_consistency.py |

## Specific owning reference documentation (do not replace)

- Product/IA: [EXPERIENCE](EXPERIENCE.md), [DESIGN-SYSTEM](DESIGN-SYSTEM.md), [MOTION](MOTION.md), [implementation/UI](../implementation/UI.md).
- Agent native control: [AGENT-CONTROL](AGENT-CONTROL.md), [AGENT-FABRIC](AGENT-FABRIC.md), [DELEGATION](DELEGATION.md), ADR-0025.
- Durable work: [TASK-RUNTIME](TASK-RUNTIME.md), [DATA-MODEL](DATA-MODEL.md), [SERVICES](SERVICES.md), [STATE-MACHINES](STATE-MACHINES.md), [FAILURE-RECOVERY](FAILURE-RECOVERY.md).
- Output: [PRESENTATION-RUNTIME](PRESENTATION-RUNTIME.md), [RICH-RESPONSE](RICH-RESPONSE.md), [ARTIFACTS-EVIDENCE](ARTIFACTS-EVIDENCE.md).
- Browser OS and policy: [ENVIRONMENTS](ENVIRONMENTS.md), [TRUST](TRUST.md), [NETWORK-SECURITY](NETWORK-SECURITY.md), [SECURITY](SECURITY.md).
- Workflow and integration: [AUTOMATION](AUTOMATION.md), [ROUTINES](ROUTINES.md), [CONNECTORS-WORKFLOWS](CONNECTORS-WORKFLOWS.md), [CAPABILITY-FABRIC](CAPABILITY-FABRIC.md), [CAPABILITY-INVOCATIONS](CAPABILITY-INVOCATIONS.md), [LEARNING](LEARNING.md).
- Promoted machine definitions: [API](API.md), [EVENTS](EVENTS.md), [SCHEMAS](SCHEMAS.md), [STORAGE](STORAGE.md), [Operator OpenAPI](schemas/operator-api.openapi.yaml), [domain event schema](schemas/domain-event.schema.json). These remain canonical at runtime after implementing the LLD.

## Design completion matrix

A human can find a named contract, proposed file, algorithm, errors and test case for each V1 area through the crosswalk. This is **design-level coverage**, not mathematical proof of literally all future functions/OS edge cases. Previously accepted contract details not repeated by an LLD remain in their original owners. Unexpected cases discovered in implementation must be added with code, tests and documentation in one change.

The following gaps are known TODAY: normal provider-backed Conversation Send absent; no fully qualified desktop native adaptive composer; unified right Workbench shell incomplete; true Chrome side browser/profile import/takeover absent; production browser Environment qualification absent; rich renderer coverage partial; no proven complete long-horizon delegation/automations across all supported agents/OS. Neither source probing nor documentation erases these gaps.

## Implementer handoff: canonical contract first, then concurrent work

Read AGENTS.md, current story/backlog, owning existing domain contracts, this index and the matching LLD. If new endpoints/events are required, first land typed service and OpenAPI/SQLite/event contracts under a single contract-owning agent, with compatibility fixtures. Delegate **disjoint** UI rendering/reducer, Rust backend/provider, test fixture, and adversarial review modules to separate agents. Merge when producer/consumer fixtures and actual provider pass; do not run the full suite after every trivial patch. Narrow tests on each module, cross-module contract tests at integration points, full regression/security/OS USER gates at milestone/release. No stopping after a single story when more dependency-ready work is available.

## External documentation — versioned references

| Interface | URL | Use/constraint |
|---|---|---|
| OpenCode command/native slash | https://docs.opencode.ai/docs/commands/ | built-in/custom collision and args; fetch current exact harness |
| OpenCode @ agents | https://docs.opencode.ai/docs/agents/ | primary/subagent semantics |
| OpenCode server/API | https://docs.opencode.ai/docs/server/ | sessions, events, health, auth |
| OpenCode permissions | https://docs.opencode.ai/docs/permissions/ | native permission rules |
| Codex | https://developers.openai.com/codex/ | locate exact App Server versioned endpoints |
| ACP | https://agentclientprotocol.com/ | only use where faithful to native |
| MCP | https://modelcontextprotocol.io/specification/ | Tasks, elicitation, permissions versioning |
| Tauri webview | https://v2.tauri.app/reference/webview-versions/ | platform-specific engine, not Chrome |
| Chrome debugging security | https://developer.chrome.com/blog/remote-debugging-port | Chrome 136+ default profile restriction |
| Chrome existing profile attach | https://developer.chrome.com/docs/devtools/agents/use-cases/auto-connect | Chrome 144+ consent-required path |
| Playwright browser profiles | https://playwright.dev/docs/api/class-browsertype | persistent isolated contexts; default Chrome profile unsupported |
| Chrome CDP Target | https://chromedevtools.github.io/devtools-protocol/tot/Target/ | versions + process/session ownership |

This is documentation only; no API key, paid backend, or tool installation is required to read it.

## Audit interpretation

Mechanical all-file audit inventory records what was read and hashed from the current working tree; contextual owner assessment lives in the dedicated LLDs plus implementation/V1-DESIGN-COVERAGE.md. A heading/link/regex hit is not an end-to-end proof; a green plan validator is not production behavior. Do not claim zero risk or no missing functions by mere doc volume.

## Source-grounded mechanical contract gate (2026-10-12)

Run `scripts/audit_v1_contracts.py --strict --write-report` to check proposal-versus-existing route ownership against the actual OpenAPI, validate canonical story references and all 158 individual UIV1 acceptance case definitions. Results are in `../implementation/V1-CONTRACT-CHECK.md` and latest concrete semantic corrections in `../implementation/V1-SEMANTIC-MD-FINDINGS-2026-10-12.md`. This checker is not a runtime/OS qualification tool and does not certify all prose line by line.
