# Product Contract Index

**Current target:** LiteCowork V1 is a complete local desktop/laptop cowork product. Cloud continuation, mobile continuation, cross-device synchronization and remote Runtime placement are post‑V1. Narrow-window desktop responsiveness is still V1.

## Product contract in one view

- **Conversation is the front door.** Choose a native agent, then a model/options supported by that agent's own route. The native agent owns its reasoning and account; LiteCowork owns managed Task coordination, policy, evidence and outputs. Switching agents keeps the same user-visible Conversation; the next adapter receives the committed cross-agent transcript prefix. Usage accumulates under that Conversation across bindings, with measured, estimated, billed, currency, and unknown coverage kept distinct.
- **Work is durable when needed.** A Conversation may remain ordinary chat or admit a durable Task. Task, Step, Attempt, AgentSession, Runtime, Environment, Capability, Effect, Artifact and Evidence remain distinct.
- **Subagents have explicit routes.** The lead may use its own harness subagents, or configured external AgentBindings as LiteCowork-hosted workers. Settings provides a Workspace default; a Coworker may inherit it or select a narrower override. A Task pins the resolved policy at admission; it has no independent route override in V1. Native child model behavior is harness-specific. External workers use their own model/auth/entitlement. Only host-delegated workers receive LiteCowork child Attempts, leases and grants.
- **The default desktop shell is three-pane.** Navigation and Settings on the left; conversation and composer in the center; a visible compact Browser/Terminal/File Workbench on the right. It can collapse, resize or expand. It remains one right-side surface.
- **Routines use one execution model.** Automation gallery templates create editable drafts; they do not install packages, connect accounts, grant capabilities, enable triggers or start runs. Gallery examples cover coding and general cowork tasks.
- **Outputs are product surfaces.** Answers, RichPresentations, editable file artifacts and isolated interactive app artifacts have distinct storage, rendering, trust and verification contracts.
- **LiteSPM is the inbuilt package manager.** It owns package discovery/install/update/unmanage/removal for its supported package kinds. LiteCowork owns assignment, authorization, use, Effects and Evidence. Account Connections and agent eligibility remain separate.

## Authority map

| Topic | Primary owner | Supporting contract |
|---|---|---|
| Product scope and invariants | [Architecture](../ARCHITECTURE.md) | [Product](PRODUCT.md), [V1 release contract](V1-PRODUCT-RELEASE-CONTRACT.md) |
| Agent/account/model route eligibility | [Agent route matrix](AGENT-ACCOUNT-ROUTE-MATRIX.md) | [Delegation](DELEGATION.md), [Agent Fabric](AGENT-FABRIC.md), [Agent Control](AGENT-CONTROL.md) |
| Same-chat cross-agent continuity and session-wide usage | [Native Composer LLD](NATIVE-COMPOSER-LLD.md), [Shared schemas](SCHEMAS.md) | [Data model](DATA-MODEL.md), [Storage](STORAGE.md), [API](API.md), [Usage LLD](V1-TELEMETRY-USAGE-RESILIENCE-LLD.md), [Testing](TESTING.md) |
| Native and host delegation | [Delegation](DELEGATION.md) | [Task Runtime](TASK-RUNTIME.md), [Coworker target](COWORKERS-TARGET.md) |
| Desktop shell and screen state | [V1 UI screen contract](V1-UI-SCREEN-CONTRACT.md) | [Desktop Shell LLD](DESKTOP-SHELL-LLD.md), [Motion](MOTION.md) |
| Routines and automation templates | [Automation](AUTOMATION.md) | [Routines](ROUTINES.md), [UI action/state ledger](V1-UI-ACTION-STATE-LEDGER.md) |
| Interactive app artifacts, package/runtime IPC schemas, exact-version local run, isolation and fallback | [Interactive Artifact Runtime](INTERACTIVE-ARTIFACT-RUNTIME.md); schemas/interactive-artifact-package.schema.json and schemas/interactive-artifact-runtime.schema.json | [Artifacts and Evidence](ARTIFACTS-EVIDENCE.md), [Presentation Runtime](PRESENTATION-RUNTIME.md), [Security](SECURITY.md), [State Machines](STATE-MACHINES.md); E08-S09 |
| LiteSPM catalog, installability, host targets, plan/approval and package lifecycle | [Capability Fabric](CAPABILITY-FABRIC.md), [LiteSPM interoperability LLD](LITESPM-LITECOWORK-INTEROP-LLD.md) | [V1 UI screen contract](V1-UI-SCREEN-CONTRACT.md), [Capability Invocations](CAPABILITY-INVOCATIONS.md), [Trust](TRUST.md) |
| Contract coverage and conflict resolution | [Coverage Matrix](COVERAGE-MATRIX.md) | Validators in the repository scripts directory |

The COMPETITIVE-RESEARCH.md file is dated, source-linked design input, not normative product behavior. The implementation plan records build order and evidence; it does not redefine product scope. Machine schemas and APIs must implement the owning contracts and remain consistent with them.

## Product assumptions rejected

- No universal “all models” endpoint or model list independent of agent/provider route.
- No subscription sharing, credential copying or silent cross-agent fallback.
- LiteSPM catalog presence is not proof that an agent, model, connection, package or capability is launchable.
- A generated interactive app is a typed immutable Artifact, distinct from an ordinary file Artifact; it is never a privileged renderer or permission to invoke external services.
- Cloud/mobile continuation is not required for V1 desktop completion.
- A template or installed package cannot turn on background work or widen Trust policy by itself.
