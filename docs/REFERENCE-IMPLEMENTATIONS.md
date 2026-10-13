# Reference Implementations and Upstream Design Sources

**Checked:** 2026-10-10

This file is a pinned research map for LiteCowork architecture work. It records the exact
upstream repositories, revisions, files, and the narrow design ideas we intentionally study.
It is **not** permission to copy code blindly, does not make an upstream project a runtime
dependency, and does not override its license. Before copying or adapting implementation
code, re-check the exact file's current license, dependencies, security assumptions, and
whether the pinned behavior still exists.

The default decision is **reference first, adapter second, vendoring last**. LiteCowork keeps
its own Task/Plan/Attempt/Effect/Evidence, Runtime, Trust, Connection, Automation, and
Conversation authority. External systems may supply implementation patterns, connector
packages, workflow execution, authentication, or tools without becoming the LiteCowork
control plane.

## Pinned repository matrix

| ID | Repository | Revision checked | License note at checked revision | LiteCowork use |
|---|---|---|---|---|
| REF-AGENTA | https://github.com/Agenta-AI/agenta | `4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137` | MIT outside separately licensed enterprise paths | Learn configuration revision, automation/run UX, per-agent durable files, trigger/session continuity. Do not adopt its whole platform model. |
| REF-EIGENT | https://github.com/eigent-ai/eigent | `2ffa2e4ef19356e87479845f2aa1600bc2fde671` | Apache-2.0 | Learn visual cowork UX, CAMEL Workforce decomposition/retry/replan, browser/terminal toolkit assembly and activity timeline. Do not make CAMEL LiteCowork's orchestration authority. |
| REF-ACTIVEPIECES | https://github.com/activepieces/activepieces | `65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7` | MIT outside enterprise paths listed in its LICENSE | Strong candidate for a compatibility/provider spike around community Pieces, OAuth/action/trigger descriptors, and connection lifecycle. Do not import enterprise paths. |
| REF-N8N | https://github.com/n8n-io/n8n | `7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3` | Sustainable Use License; enterprise files separately licensed | External/user-owned WorkflowBackend only by default. Study run/trigger semantics; do not vendor or hide n8n as LiteCowork's engine. |
| REF-NANGO | https://github.com/NangoHQ/nango | `b309bc1716f9a868e7cf3af0a403a7b0da093df2` | Elastic License 2.0 | Architecture reference and optional external auth backend. Study token refresh/encryption/connection state; no unreviewed source reuse. |
| REF-ARCADE-PLUGIN | https://github.com/arcadeai-labs/arcade | `41ff1e9aea3a410c85a6b0f61147c073ef606924` | Apache-2.0 | Study portable MCP/skill packaging, browser sign-in handoff and agent-facing discovery. Optional provider only. |
| REF-ARCADE-MCP | https://github.com/ArcadeAI/arcade-mcp | `79a2dc90d347e4cd1a96366a2d94c8ddc356f1c3` | MIT | Study authorized MCP tool declarations and secret injection boundaries. Optional provider only. |
| REF-CAMEL | https://github.com/camel-ai/camel | `24fde60ad739b4be8f8f533fea9ba54256fbc4a2` | Apache-2.0 | Reference for decomposition, worker channels, task events, retry/replan. LiteCowork keeps its own accepted Plan/Step/Attempt model. |
| REF-COMPOSIO | https://github.com/ComposioHQ/composio | `f9ce49953598e13371c406ac346846696a4d8385` on `next` | MIT at checked root LICENSE | Optional integration/tool provider and catalog reference; never a mandatory dependency or secret owner. |
| REF-OPENINTELLIGENTUI | [CopilotKit/OpenIntelligentUI](https://github.com/CopilotKit/OpenIntelligentUI) | [`f6e4388b26a64b9a0714943b08a1ce622b924eec`](https://github.com/CopilotKit/OpenIntelligentUI/tree/f6e4388b26a64b9a0714943b08a1ce622b924eec) | MIT at checked root LICENSE | E08-S08 UX, sandbox lifecycle, explicit bridge, and renderer-test reference. Do not adopt its arbitrary generated HTML/CSS/JavaScript renderer or CSP policy as LiteCowork's RichPresentation implementation. |

## OpenIntelligentUI exact files

Repository: [CopilotKit/OpenIntelligentUI](https://github.com/CopilotKit/OpenIntelligentUI)

Reviewed at [`f6e4388b26a64b9a0714943b08a1ce622b924eec`](https://github.com/CopilotKit/OpenIntelligentUI/tree/f6e4388b26a64b9a0714943b08a1ce622b924eec) (`main` at research time).
The root `LICENSE` at this revision is MIT.

| File | Exact link | What to study | LiteCowork decision |
|---|---|---|---|
| `README.md` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/README.md | Prose/native/custom interactive answer selection, conversation presentation, and streaming user experience | UX reference for E08-S08; keep semantic ConversationMessage complete without the enhancement |
| `docs/architecture.md` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/docs/architecture.md | Stream flow, placeholder/preview/final renderer lifecycle, local iframe control state, and follow-up behavior | Study lifecycle and state-ownership test cases; LiteCowork's stream schema and immutable presentation remain authoritative |
| `apps/app/src/components/generative-ui/open-generative-ui/schema.ts` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/components/generative-ui/open-generative-ui/schema.ts | Runtime validation of streamed activity fields | Test-shape reference only; LiteCowork uses its closed versioned RichPresentation and stream schemas |
| `apps/app/src/components/generative-ui/open-generative-ui/renderer.tsx` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/components/generative-ui/open-generative-ui/renderer.tsx | Throttled streaming updates, preview-to-final sandbox transition, resize source checks and height clamping | Adapt only as behavioral test ideas after LiteCowork's renderer boundary is implemented; no source reuse |
| `apps/app/src/components/generative-ui/open-generative-ui/frame-content.ts` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/components/generative-ui/open-generative-ui/frame-content.ts | Frame composition and content-height measurement | Inspect for lifecycle edge cases only. The CSP permits `unsafe-inline`/`unsafe-eval` and selected CDN script origins to support generated code, so it is not a policy template for LiteCowork |
| `apps/app/src/lib/sandbox/sandbox-functions.ts` and `prompt-bridge.tsx` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/lib/sandbox/sandbox-functions.ts · https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/lib/sandbox/prompt-bridge.tsx | Narrow `sendPrompt` and HTTPS `openLink` host actions, validation, busy handling, and settlement | Reference for explicit, bounded host-action and failure tests; LiteCowork still binds actions to Core-owned references and authorization |
| `apps/app/src/components/generative-ui/open-generative-ui/__tests__/open-generative-ui-renderer.test.tsx` and `apps/app/src/lib/sandbox/__tests__/prompt-bridge.test.tsx` | https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/components/generative-ui/open-generative-ui/__tests__/open-generative-ui-renderer.test.tsx · https://github.com/CopilotKit/OpenIntelligentUI/blob/f6e4388b26a64b9a0714943b08a1ce622b924eec/apps/app/src/lib/sandbox/__tests__/prompt-bridge.test.tsx | Renderer lifecycle, foreign resize-message rejection, bridge contention and teardown tests | Use as candidate behavioral cases for E08-S08; implement against LiteCowork's actual renderer and contracts |

**Decision:** This repository is useful for interaction and test design when E08-S08 reaches
renderer integration. Study its text/native/interactive selection, streamed preview/final
states, locally scoped controls, explicit follow-up action, iframe resize validation, and
bridge failure cases. Do not import its dynamic HTML/JavaScript execution model, Websandbox
runtime, CDN import map, or CSP. At this revision the renderer streams generated HTML,
CSS, and JavaScript into an iframe; its CSP allows inline/eval scripts and selected CDN
origins. LiteCowork's [Rich Response contract](RICH-RESPONSE.md) instead requires closed,
bounded blocks and deterministic rendering, with no arbitrary HTML, JavaScript, CSS, or
model-owned host actions. No upstream code or dependency was copied or added.

**Speed and viability comparison (planned for E08-S08; no comparative result is claimed):**

| Question | Current expectation | Why / evidence still needed |
|---|---|---|
| Which is faster for a standalone generated-UI prototype? | OpenIntelligentUI | It already supplies the iframe sandbox, streaming preview/final lifecycle, and bridge. This is a qualitative estimate from the pinned implementation, not a timed result. |
| Which is faster to ship as LiteCowork local V1? | LiteCowork's native RichPresentation path | Its semantic-first response, closed presentation blocks, authenticated local transport, and React renderer fit the Tauri plus Rust `litecoworkd` product. Reusing OpenIntelligentUI would require qualifying and operating a separate Next.js, FastAPI/LangGraph, CopilotKit, and Websandbox stack. |
| Which has lower runtime/rendering cost? | Unknown; native bounded rendering may have less overhead | Avoiding an iframe and generated code may reduce client work, but actual CPU, memory, latency, and frame behavior depend on the scenarios and WebView. Measure both under matched conditions. |
| Which is viable as LiteCowork's production renderer? | LiteCowork's accepted native design | OpenIntelligentUI is valuable as UX and test reference, but its generated HTML/CSS/JavaScript execution and CSP are outside LiteCowork's closed, deterministic RichPresentation boundary. |

The benchmark must compare implementations of the same useful interaction, not framework
reputation or a generated-UI demo against a different LiteCowork feature. The estimates
above distinguish prototype and integration speed from runtime performance; only the
matched E08-S08 measurements can settle the latter.

When implementing E08-S08, run the same representative prose, simple-card, and richer
interactive-answer scenarios on the same device/OS and display webview. Hold content,
model/provider, prompt/request, network, and cache state constant; record their exact
versions and separate provider/model latency from renderer time. Measure time to first
semantic text, first useful visual, and completed render; client CPU, peak memory, dropped
frames, transferred bytes, and external network/CDN requests; and cold-start versus warm
render behavior for small and large outputs. Include offline/local operation and renderer,
bridge, and reconnect failure behavior. Record raw timings and environment details with
sanitized evidence. Do not claim one is faster unless repeated matched runs support that
claim; if OpenIntelligentUI cannot run under the same local conditions, report that as a
viability constraint and compare only the measurements that remain equivalent.

**Implementation commitment:** E08-S08 will include a viability run of the pinned
[OpenIntelligentUI repository](https://github.com/CopilotKit/OpenIntelligentUI/tree/f6e4388b26a64b9a0714943b08a1ce622b924eec)
and the matched renderer comparison above where both systems can run under equivalent
conditions. Record commands, versions, raw measurements, failures, and limitations in the
E08-S08 review packet and `implementation/CURRENT-RUN.md`; update this note with the
measured conclusion then. Until that evidence exists, the speed assessment above remains
a hypothesis and the reference remains a UX/test input rather than an implementation
dependency.

**Owner direction (2026-10-11):** Defer the viability and matched-speed tests until E08-S08
implementation. Test the upstream [CopilotKit/OpenIntelligentUI GitHub repository](https://github.com/CopilotKit/OpenIntelligentUI)
at the pinned [reviewed commit](https://github.com/CopilotKit/OpenIntelligentUI/tree/f6e4388b26a64b9a0714943b08a1ce622b924eec)
and run the scenarios and metrics above against LiteCowork under matched conditions where
possible. Finalize this comparison only after recording the measured results, commands,
versions, failures, and limitations in the E08-S08 review packet and
`implementation/CURRENT-RUN.md`. Until then, the assessment remains a planning hypothesis:
OpenIntelligentUI is likely quicker for a standalone generated-UI prototype, while
LiteCowork's native bounded renderer is likely quicker to ship inside the existing product.

## Agenta exact files

Repository: https://github.com/Agenta-AI/agenta

Checked at `4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137`.

| File | Exact link | What to study | LiteCowork decision |
|---|---|---|---|
| `README.md` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/README.md | Product framing: learning agents, schedules/events, run history, per-agent computer, integrations, workspaces, agent teams | Product reference only |
| `sdks/python/agenta/sdk/agents/platform_instructions.py` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/sdks/python/agenta/sdk/agents/platform_instructions.py | Distinguishes task vs persistent configuration change; memory vs skills vs triggers; `read_config`/`commit_revision`; durable `agent-files/` guidance | Borrow the separation, but route persistent changes through LiteCowork revision/learning authority |
| `sdks/python/agenta/sdk/agents/platform/op_catalog.py` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/sdks/python/agenta/sdk/agents/platform/op_catalog.py | Self-targeted platform operations, schedule/subscription tools, session-bound context | Reference for narrowly bound host operations |
| `web/packages/agenta-automation-ui/src/automationModel.ts` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/web/packages/agenta-automation-ui/src/automationModel.ts | One user-facing Automation projection over schedule and event subscription entities; status/run labels | Adopt the UX abstraction, not the storage model |
| `web/packages/agenta-automation-ui/src/automationEdit.ts` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/web/packages/agenta-automation-ui/src/automationEdit.ts | Explicit create/edit payload construction; duplicate-off behavior; preserving fields on full update | Reference for safe edit semantics |
| `web/packages/agenta-automation-ui/src/runModel.ts` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/web/packages/agenta-automation-ui/src/runModel.ts | Delivery/run projection and session linkage | Adopt run-to-conversation navigation in LiteCowork |
| `api/oss/src/core/triggers/service.py` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/api/oss/src/core/triggers/service.py | Trigger discovery, connection state, schedule/subscription orchestration | Reference only; LiteCowork TriggerCoordinator remains authoritative |
| `api/oss/src/tasks/asyncio/triggers/dispatcher.py` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/api/oss/src/tasks/asyncio/triggers/dispatcher.py | One session ID per delivery/run and dispatch behavior | Reference for run/session attribution |
| `services/runner/src/engines/sandbox_agent/run-turn.ts` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/services/runner/src/engines/sandbox_agent/run-turn.ts | Pause/cancel/continuity, durable resume points, sandbox failure handling | Reference for continuity edge cases |
| `docs/blog/entries/automation-runs.mdx` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/docs/blog/entries/automation-runs.mdx | User-facing history and continuing a run's session | Product/UX reference |
| `openspec/changes/agenta-tools/proposal.md` | https://github.com/Agenta-AI/agenta/blob/4bc51903a1597d3ef8f1ac17f2d6f472ba7c8137/openspec/changes/agenta-tools/proposal.md | Which self-edit/trigger tools are opt-in vs default | Reference for least-authority self-configuration |

**Decision:** LiteCowork adopts the useful separation
`memory != instruction != skill != automation`, a simple schedule/event Automation UI,
and explicit run/session continuation. LiteCowork does **not** let a model mutate arbitrary
persistent configuration directly. Only a narrowly authorized LearningService or normal
versioned Operator command may commit persistent state.

## Eigent and CAMEL exact files

Eigent repository: https://github.com/eigent-ai/eigent

Checked at `2ffa2e4ef19356e87479845f2aa1600bc2fde671`.

| File | Exact link | What to study | LiteCowork decision |
|---|---|---|---|
| `README.md` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/README.md | Single Agent / Workforce product split, local deployment, MCP/skills/browser/terminal | Borrow visual simplicity; do not create a separate LiteCowork control plane for Workforce |
| `backend/app/utils/workforce.py` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/backend/app/utils/workforce.py | CAMEL task decomposition, dependency updates, worker events, retry/replan, stall handling | Use as algorithm/reference input to LiteCowork delegation benchmarking |
| `backend/app/workspace_runtime/workforce_adapter.py` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/backend/app/workspace_runtime/workforce_adapter.py | Runtime adapter between workspace/session state and CAMEL Workforce | Reference only |
| `backend/app/agent/factory/toolkit_assembler.py` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/backend/app/agent/factory/toolkit_assembler.py | Conditional browser, terminal, MCP, skills, files, screenshots, delegation toolkit assembly and cleanup | Borrow lifecycle/rollback test ideas; LiteSPM/CapabilityBroker stays owner |
| `src/components/Trigger/AutomationDashboard.tsx` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/src/components/Trigger/AutomationDashboard.tsx | Visual automation status, next run, schedule label | UI reference |
| `src/components/Trigger/TriggerDialog.tsx` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/src/components/Trigger/TriggerDialog.tsx | Automation creation/edit flow | UI reference |
| `src/components/Trigger/SchedulePicker.tsx` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/src/components/Trigger/SchedulePicker.tsx | Schedule configuration and preview | UI reference |
| `src/store/spaceStore.ts` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/src/store/spaceStore.ts | Space/session information architecture | Reference only |
| `src/components/Session/index.tsx` | https://github.com/eigent-ai/eigent/blob/2ffa2e4ef19356e87479845f2aa1600bc2fde671/src/components/Session/index.tsx | Session workspace shell | Visual reference |

CAMEL repository: https://github.com/camel-ai/camel

Checked at `24fde60ad739b4be8f8f533fea9ba54256fbc4a2`.

- `camel/societies/workforce/workforce.py`:
  https://github.com/camel-ai/camel/blob/24fde60ad739b4be8f8f533fea9ba54256fbc4a2/camel/societies/workforce/workforce.py
- `camel/societies/workforce/task_channel.py`:
  https://github.com/camel-ai/camel/blob/24fde60ad739b4be8f8f533fea9ba54256fbc4a2/camel/societies/workforce/task_channel.py
- `camel/societies/workforce/events.py`:
  https://github.com/camel-ai/camel/blob/24fde60ad739b4be8f8f533fea9ba54256fbc4a2/camel/societies/workforce/events.py
- `camel/tasks/task.py`:
  https://github.com/camel-ai/camel/blob/24fde60ad739b4be8f8f533fea9ba54256fbc4a2/camel/tasks/task.py

**Decision:** CAMEL/Eigent validate decomposition, bounded worker execution, retry/replan, and
coherent cross-agent activity timelines. They do not replace LiteCowork's immutable
PlanRevision, Step, Attempt, lease, Effect, Evidence, verifier, and native AgentBinding
contracts.

## Activepieces exact files

Repository: https://github.com/activepieces/activepieces

Checked at `65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7`.

| File | Exact link | Use |
|---|---|---|
| `LICENSE` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/LICENSE | Defines MIT core and separately licensed enterprise paths; re-check every reused path |
| `packages/pieces/community/gmail/src/lib/auth.ts` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/packages/pieces/community/gmail/src/lib/auth.ts | Provider auth descriptor, Google clients, account connection behavior |
| `packages/pieces/community/gmail/src/lib/actions/get-mail-action.ts` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/packages/pieces/community/gmail/src/lib/actions/get-mail-action.ts | Action schema/execution shape |
| `packages/pieces/community/gmail/src/lib/triggers/new-label.ts` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/packages/pieces/community/gmail/src/lib/triggers/new-label.ts | Trigger descriptor/provider pattern |
| `packages/pieces/community/google-calendar/src/lib/auth.ts` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/packages/pieces/community/google-calendar/src/lib/auth.ts | Google Calendar OAuth/auth variants |
| `brain/knowledge/connections-auth/app-connections.md` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/brain/knowledge/connections-auth/app-connections.md | Connection identity, encrypted credentials, refresh and replacement semantics |
| `packages/server/api/src/app/app-connection/app-connection-service/app-connection-service.ts` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/packages/server/api/src/app/app-connection/app-connection-service/app-connection-service.ts | Server-side connection lifecycle |
| `packages/server/api/src/app/app-connection/app-connection-service/app-connection.handler.ts` | https://github.com/activepieces/activepieces/blob/65bfba72b855ed6c8c3b4c24ad5d12a8960b1aa7/packages/server/api/src/app/app-connection/app-connection-service/app-connection.handler.ts | Expiry/refresh handling |

**Decision:** implement a spike for an `ActivepiecesPieceProvider` behind LiteSPM. The
provider may translate qualified community Piece metadata/auth/actions/triggers into
LiteCowork capability/connector descriptors. It must not expose enterprise code, silently
install arbitrary npm dependencies, bypass LiteSPM signatures/provenance, or give a Piece
direct Trust authority. A second mode may connect to a user-owned Activepieces instance as
a WorkflowBackend instead of loading Piece code locally.

## n8n exact files

Repository: https://github.com/n8n-io/n8n

Checked at `7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3`.

- `LICENSE.md`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/LICENSE.md
- `packages/cli/src/workflows/workflow-execution.service.ts`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/packages/cli/src/workflows/workflow-execution.service.ts
- `packages/cli/src/workflow-runner.ts`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/packages/cli/src/workflow-runner.ts
- `packages/core/src/execution-engine/active-workflow-triggers.ts`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/packages/core/src/execution-engine/active-workflow-triggers.ts
- `packages/cli/src/modules/mcp/tools/execute-workflow.tool.ts`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/packages/cli/src/modules/mcp/tools/execute-workflow.tool.ts
- `packages/nodes-base/nodes/Webhook/Webhook.node.ts`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/packages/nodes-base/nodes/Webhook/Webhook.node.ts
- `packages/cli/src/workflows/triggers/non-webhook-trigger-registrar.ts`:
  https://github.com/n8n-io/n8n/blob/7424f0ca0452f9dc9fe4cf6627ecbf0b315ed1e3/packages/cli/src/workflows/triggers/non-webhook-trigger-registrar.ts

**Decision:** support a user-owned n8n deployment through a narrow
`WorkflowBackendAdapter`. Do not vendor n8n, expose its internal database as LiteCowork
state, or market embedded n8n functionality without a separate license review.

## Nango exact files

Repository: https://github.com/NangoHQ/nango

Checked at `b309bc1716f9a868e7cf3af0a403a7b0da093df2`.

- `LICENSE`:
  https://github.com/NangoHQ/nango/blob/b309bc1716f9a868e7cf3af0a403a7b0da093df2/LICENSE
- `packages/shared/lib/services/connection.service.ts`:
  https://github.com/NangoHQ/nango/blob/b309bc1716f9a868e7cf3af0a403a7b0da093df2/packages/shared/lib/services/connection.service.ts
- `packages/shared/lib/services/connections/credentials/refresh.ts`:
  https://github.com/NangoHQ/nango/blob/b309bc1716f9a868e7cf3af0a403a7b0da093df2/packages/shared/lib/services/connections/credentials/refresh.ts
- `packages/shared/lib/clients/oauth2.client.ts`:
  https://github.com/NangoHQ/nango/blob/b309bc1716f9a868e7cf3af0a403a7b0da093df2/packages/shared/lib/clients/oauth2.client.ts
- `docs/guides/platform/security.mdx`:
  https://github.com/NangoHQ/nango/blob/b309bc1716f9a868e7cf3af0a403a7b0da093df2/docs/guides/platform/security.mdx

**Decision:** study its refresh locking, encrypted-credential, provider-config and connection
health patterns. Nango may be an optional external `AuthBackendAdapter` for deployments
that already use it. LiteCowork's default desktop path remains local SecretStore +
provider-specific ConnectorPackage auth.

## Arcade exact files

Portable plugin repository: https://github.com/arcadeai-labs/arcade

Checked at `41ff1e9aea3a410c85a6b0f61147c073ef606924`.

- `README.md`:
  https://github.com/arcadeai-labs/arcade/blob/41ff1e9aea3a410c85a6b0f61147c073ef606924/README.md
- `docs/install/agent-plugins.md`:
  https://github.com/arcadeai-labs/arcade/blob/41ff1e9aea3a410c85a6b0f61147c073ef606924/docs/install/agent-plugins.md
- `mcp.json`:
  https://github.com/arcadeai-labs/arcade/blob/41ff1e9aea3a410c85a6b0f61147c073ef606924/mcp.json
- `plugin.json`:
  https://github.com/arcadeai-labs/arcade/blob/41ff1e9aea3a410c85a6b0f61147c073ef606924/plugin.json

MCP framework repository: https://github.com/ArcadeAI/arcade-mcp

Checked at `79a2dc90d347e4cd1a96366a2d94c8ddc356f1c3`.

- `README.md`:
  https://github.com/ArcadeAI/arcade-mcp/blob/79a2dc90d347e4cd1a96366a2d94c8ddc356f1c3/README.md
- `LICENSE`:
  https://github.com/ArcadeAI/arcade-mcp/blob/79a2dc90d347e4cd1a96366a2d94c8ddc356f1c3/LICENSE

**Decision:** Arcade can be a user-selected external Auth/Tool Gateway or MCP provider. It
must never be required for Gmail/Calendar/etc. and never become the only place LiteCowork
can authenticate an app.

## Composio exact files

Repository: https://github.com/ComposioHQ/composio

Checked on branch `next` at `f9ce49953598e13371c406ac346846696a4d8385`.

- `README.md`:
  https://github.com/ComposioHQ/composio/blob/f9ce49953598e13371c406ac346846696a4d8385/README.md
- `LICENSE`:
  https://github.com/ComposioHQ/composio/blob/f9ce49953598e13371c406ac346846696a4d8385/LICENSE
- `docs/public/data/toolkits.json`:
  https://github.com/ComposioHQ/composio/blob/f9ce49953598e13371c406ac346846696a4d8385/docs/public/data/toolkits.json
- `docs/content/kb/guide/toolkits-gmail.mdx`:
  https://github.com/ComposioHQ/composio/blob/f9ce49953598e13371c406ac346846696a4d8385/docs/content/kb/guide/toolkits-gmail.mdx

**Decision:** optional external provider and catalog comparison only. Connector packages,
OAuth, triggers and workflow backends must function without Composio.

## Source-to-LiteCowork decision map

| Upstream lesson | Canonical LiteCowork owner | Outcome |
|---|---|---|
| Agent can update its own config | LearningService + CoworkerService + immutable CoworkerRevision | Model may propose; only policy-qualified memory can quietly commit. Instructions/skills/automations require review/versioned authority. |
| Schedule/event shown as one Automation concept | AutomationService + TriggerCoordinator + UI projection | Adopt. Internal TriggerDefinition variants remain distinct. |
| Automation run has a session that can continue | AutomationOccurrence + Task + on-demand Task follow-up Conversation | Adopt with explicit run Conversation linkage and visibility rules. |
| One agent has durable files + session scratch | Coworker private Resources + Environment/AgentSession | Adopt conceptually; do not allocate a permanent VM per Coworker by default. |
| Workforce decomposes and parallelizes | PlanRevision/Step + WorkerSelection + DelegationChannel | Adopt algorithms/tests, not CAMEL authority. |
| Browser/terminal/MCP assembled per agent | CapabilityBroker + LiteSPM + AgentControlDescriptor | Adopt lifecycle/cleanup test patterns. |
| Huge app ecosystem | LiteSPM ConnectorPackage + optional providers | Direct first-party connectors + community packages + MCP + qualified Activepieces compatibility. |
| Managed OAuth gateway | ConnectionService/AuthBackendAdapter | Optional external backend. Default desktop can remain local-first. |
| Existing deterministic workflow engine | WorkflowBackendAdapter | Invoke as a bounded capability; never replace LiteCowork Task Runtime. |
| Tool catalog kept out of model context | CapabilityBroker progressive discovery | Adopt; load/search/describe/execute only relevant capabilities. |

## Implementation rule

Every new implementation story that copies or substantially adapts code from one of these
repositories must record:

1. repository and exact commit;
2. exact file(s);
3. copied/adapted line/function scope;
4. upstream license and required notices;
5. dependency/transitive-license check;
6. security assumptions changed by LiteCowork;
7. tests proving LiteCowork authority/Trust boundaries still hold;
8. update strategy when upstream changes.

Architecture inspiration without copied source still cites this file in the implementation
review so future maintainers can distinguish deliberate borrowing from accidental similarity.

## V1 detailed design integration — 2026-10-12

The official upstream references and implementation constraints for native OpenCode commands/server/permissions and Chrome DevTools/Playwright/Chrome security are listed in V1-DESIGN-INDEX.md. Pin vendor source commit/version before relying on exact behavior. Source listing is evidence of capability to research, not that LiteCowork adapter has passed conformance.
