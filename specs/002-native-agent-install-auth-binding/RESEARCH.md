# Research - Native Agent Install, Auth, Binding and Agent Profile UX

**Feature:** 002 | **Checked:** 2026-11-06 (active source/UI audit and screenshot/competitor comparison; no native install or provider session executed).

## Primary authority
Canonical design and state ownership: docs/AGENT-CONTROL.md, docs/AGENT-FABRIC.md, docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md, docs/V1-SETTINGS-CONTRACT-LEDGER.md, docs/V1-UI-SCREEN-CONTRACT.md and root ARCHITECTURE.md. Feature 002 owns native AgentModule/profile/install/auth/binding boundaries. LiteSPM owns MCP/skill/plugin/capability-package lifecycle. Feature 001 owns Conversation admission/session execution. Feature 011 owns Settings shell/control authority. The agent catalogue LLD is normative target, not current implementation proof.

## External comparison and adoption
NeuralAgent public material describes a desktop workflow around discovering/using agents, configured/installed state, persistent agent work views, model routing, local voice, scheduling and computer-use. Its historic public repository describes a different Electron/React + Python/FastAPI architecture and plugin-like agent roles; current public product pages now describe a later computer-use product. Do not treat those web pages and the old code repository as one versioned implementation or copy either architecture wholesale. NeuralAgent's Settings screenshots are useful information-hierarchy references: persistent category navigation, installed vs available agents, notification preference, advanced runtime behavior and a separate cost/usage surface. Its Agents overview uses three summary filters (Needs You, Working, Done) and a grouped work list rather than separate permanent sidebar pages. Those counts/statuses must be real owner projections in LiteCowork.

Adopt:
- agent catalogue and agent profile are separate levels;
- installed/detected/configured/authenticated/ready are independent axes;
- installed and available distributions are separate sections with source, version, compatibility and action;
- status groups such as Needs You / Working / Done are filters over durable Work/Task states, not independent navigation routes;
- notification preferences are separate from work state and reflect the real notification delivery/attention queue;
- advanced controls distinguish local preference from native process/runtime behavior and show restart/recovery semantics;
- Usage is a first-class read-only observability surface, drillable by actual agent/model/session/Task with confidence/coverage.

Reject:
- a single Connected toggle;
- treating a registry listing as a safe install command;
- a global model/provider selector that overrides an agent's native model/auth contract;
- a top-level Working list that duplicates Work/Task or Conversation state;
- usage totals inferred from screenshots, flat-plan token guesses or assumed API list price;
- visual statuses for process, resume, keep-awake or notifications without owner service/OS observation.

## Screenshot-specific UI interpretation
1. Settings Agents: default agent selection is native-harness-owned and applies only to new work; installed versions are listed with exact executable/version and external official link; available candidates show install action only when a trusted install plan exists.
2. Notifications: explicit choices combine sound and OS notification delivery (both, sound only, notification only, neither), with quiet/in-focus behavior; a 'Try it' control is a real local preview that does not send a notification to another device.
3. Advanced: keep-awake choices are OS power assertions with distinct battery/power semantics; resume-after-restart is a scheduler/recovery policy for admitted agent jobs, not a promise that arbitrary external native sessions can be restored.
4. Agents overview: 3 status summary tiles are filters, not left-sidebar routes. LiteCowork's primary sidebar remains Conversations, Coworker roster, Work, Needs You, Automations, Library, Discover and Settings; Agents work should be accessible as a Work filter/overview, not a duplicate global task engine.
5. Usage: dashboard quick ranges Today, 7 days, 1 month, 3 months, 1 year and All time, plus YTD/Custom in expanded filters. Grouping and filters are governed by telemetry LLD and existing ledgers.
6. Work panels: keep separate Conversations and native Terminal panes within a Task/work context only when actual session/process identity exists; do not promote mock terminal output to durable completed work.

## Current LiteCowork source/UI evidence
- `AgentCatalogSettings` currently lists 15 curated candidates and ACP registry entries; registry results are read-only and map to NO_ADAPTER.
- Local inventory probes version/path metadata for a bounded candidate set; auth UNKNOWN, readiness NOT_PROBED. No actual AgentModule install/update action exists.
- Profile probe supports Codex/OpenCode only, Runtime enrollment required. Codex App Server initialize/account/model-list is observed; OpenCode is recorded incompatible. Neither guarantees an end-to-end Conversation session.
- Descriptor read/refresh and credential submission return 503 DEPENDENCY_UNAVAILABLE; auth-begin and native-config-open are target-only.
- Binding list/create/enable/disable/default actions are served; enable currently lacks required fresh AgentControlDescriptor gate.
- Conversation supervisor dispatches OpenCode only, but current OpenCode profile probe is incompatible; Codex can be bound but is not dispatched by the active supervisor.
- UI currently has no Install/Update or native sign-in flow; usage detail/projection is not served.

## V1 boundary
Deliver one complete exact native agent journey first. Other agents may be listed as detected/available/unsupported but must not be presented as runnable until their install/auth/descriptor/session contract is independently qualified on a declared OS/Runtime. Do not use catalog breadth as a V1 acceptance metric.

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

## OpenBot model/effort picker comparison — 2026-11-06

Pinned source: nightly-labs/openbot@6759fa3ee857f4f4966865e12de99ae84ab6de07 (package 0.62.0). The shared AgentSettingsPanel and ProviderModelPicker show a compact, searchable model list attached to one selected agent/profile/account, with a selected marker and an optional effort row. Provider runtime adapters supply model catalog and account-specific details; this is not a global model router.

Adopt the compact interaction hierarchy only: agent/profile identity, searchable exact-provider model list, selected/default marker, and effort only when the exact descriptor supports it. Keep provider-specific modes outside the generic model list. The model control is distinct from the Task agent/Coworker selector. Do not mix model options from different providers/accounts.

The OpenBot package declares PolyForm Noncommercial 1.0.0 and its README describes a development preview with danger-full-access / approvalPolicy never. No code or permission model is reused. Full source pin and limitations: [OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md](../../docs/OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md).

LiteCowork's model/effort picker remains target-only until fresh AgentControlDescriptor read/refresh, adapter compatibility, account scope, and an exact native Conversation/Task send path are all qualified. Existing discovery/profile/binding evidence does not establish that a selected model is dispatchable.
