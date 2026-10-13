# Release scope

## V1 commitment

User alone reviews; coding agents implement. No additional developer, QA team,
product manager, or security engineer is assumed. Reviewer bandwidth is a release resource.
V1 is a complete, production-qualified **desktop/local product**. Cloud continuation and
remote Runtime are post-V1 releases and do not block the V1 launch. Cloud follows V1;
remote Runtime follows cloud. All finalized local contract features remain in scope,
including non-coding office, research,
personal-assistant, browser, and automation workflows. A coding-agent implementation team
does not make the end product coding-only.

The first desktop alpha is an increment, not the complete V1. V1 is released only after
the local feature matrix, real-provider workflows, supported desktop platform matrix, and
production gates pass. A missing external provider is a documented blocker or a visibly
unavailable optional integration; it cannot be marked implemented using a fixture. Any
actual reduction of committed local feature scope requires an explicit owner decision and
a revised coverage/release record.

## Included feature groups

- Independent Operator/daemon, lifecycle, tray/startup, Quick Entry, Workspace policy,
  roots, Resource intake/search/freshness, Conversations, Tasks, planning and recovery.
- Complete native harness integration; lead selection/switch/failover; same-harness and
  heterogeneous DelegationProfiles, cost/quality ranking, budgets, quotas, escalation.
- Gateway, LiteSPM integration when its real authority is available, MCP capabilities,
  asynchronous invocations, provider input, Skills, Plugins and sandboxed MCP Apps.
- Trust, approvals, grants, SecretLeases, egress/credential separation, Effects,
  reconciliation, Artifact versions, Evidence, independent verification, dependency drift.
- Worktrees, local/container/persistent browser/desktop environments, sharing and control
  leases, human takeover, warmth/prewarm, structured actions and deadline-sensitive work.
- Coworkers, primary identity, passive Goals, Suggestions/producers/dedupe/mutes/snooze,
  editable context with revision conflict handling/revocation/deletion.
- File/folder/ZIP ingestion and provider-backed RAG with grounded references, local models,
  progressive context and explicit local/cloud placement.
- Home, Needs You, Work, Live Desk, Library/Workbench, Discover, settings, Inspector,
  onboarding, keyboard/accessibility/reduced motion, truthful status/cost presentation.
- Routines, multi-trigger Automations, cursors/occurrences/misfires, test runs, health,
  local notifications, reusable work/Teach-a-task/SkillProposal.
- Local backups/restores, upgrades/rollback, observability, desktop deployment and
  production incident recovery.

## Post-V1 releases

- Cloud continuation: cloud deployment, pairing/replication/fencing/handoff, cloud
  operations, and one qualified cloud-hosted human channel.
- Remote Runtime: enrollment, placement, remote lifecycle, disconnection and recovery.

## Coverage versus integration breadth

Implement the complete framework/contract and prove representative providers per class.
Claude, Codex, OpenCode and Cline each get qualification work; publish each one's actual
capability matrix. Unsupported upstream features remain unavailable with an explanation.
Never claim complete harness equivalence based on a chat completion test. The owner
selects supported provider versions and target operating systems at the qualification gate.

Linux is the reference development host. Plan installer, lifecycle, filesystem, webview,
accessibility and actual-provider qualification for Linux, Windows and macOS separately.
No OS receives a supported label without a real run on that OS. Missing signing accounts
or macOS hardware block that platform's GA claim, not other proven platform alphas.

Future native mobile/web clients, team/RBAC, voice, a marketplace of templates, and every
possible messaging connector are not V1 release requirements. Existing mobile layouts
remain design constraints, but desktop must be delivered before client expansion.
Cloud services serve the desktop client first. Remote execution is the same Runtime model.

## Gate sequence

| Gate | Required evidence | Owner decision |
|---|---|---|
| G0 contract/development readiness | validators, pinned tooling, first qualification spikes | Accept first story |
| G1 local durable work | installed app, native agent, verified Artifact, kill/restart recovery | Desktop alpha usable |
| G2 desktop feature complete | all local epics and representative real workflows pass | Start cloud qualification |
| G3 V1 production release | local release matrix, signed qualified desktop installers, local restore/rollback, no open critical defects | Publish desktop/local V1 |
| Post-V1 Cloud | two Runtime fault tests, real cloud deployment, restore, handoff | Start remote qualification |
| Post-V1 Remote | enrollment/revocation, remote dependency and fencing tests | Remote release candidate |

No calendar date or velocity is asserted before measured delivery. Gates represent
capability and evidence; dependency-ready stories can overlap within a gate's safe bounds.

## Coworker target inclusion and implementation status

The [accepted Coworker target](../docs/COWORKERS-TARGET.md) is part of the desktop/local UX and autonomy design: ordinary chats do not require Coworkers, Coworkers are chat-first; optional connections, standing responsibilities, scoped automatic memory, Workers, background schedules, recovery and Workbench integration are planned behind the existing native-agent, Trust, Environment and Task gates. Do not push these into cloud-only scope. A feature described by target docs is not functional until its API/persistence, real native provider execution and owner acceptance are qualified. Post-V1 cloud only adds execution location; it does not create a separate autonomous Bot engine.


## Native-agent completeness within V1 scope

“Complete native harness integration” includes the user-facing setup and control surface,
not merely sending prompts to an installed binary. For every agent/version LiteCowork
claims as supported, the qualification matrix must cover, where the upstream harness
offers them:

- discover/install/update/repair;
- native sign-in or provider/API-key configuration through the correct owner;
- native model/session options, including reasoning/effort/mode where exposed;
- text/image/file/resource inputs;
- native slash commands and reference/mention affordances;
- native configuration launch points;
- native extension/subagent behavior and its distinction from LiteSPM-managed packages;
- cancel/resume/steer/stop/quiescence behavior required by LiteCowork Tasks.

There is no separate global Providers feature to qualify. Local Models is inventory only
and appears through an agent when that AgentModule reports compatibility.

Registry presence or successful version output is not support evidence. The exact
supported agent/module/harness/OS combination must pass E03-S11 CODE + SYSTEM + USER
conformance.


## Connector, workflow, and learning interpretation within V1

V1 includes the connector/auth framework and representative real providers, not a promise
to hand-build every SaaS integration. Direct connectors, MCP/LiteSPM packages, a qualified
compatibility route, and optional external workflow/auth backends are distinct extension
paths.

The framework must work without Composio/Nango/Arcade/n8n/Activepieces cloud dependencies.
Remote event relay/cloud hosting is required only for event behavior that cannot operate
while a local machine is asleep; local schedules/polling remain valid when Runtime is
available.

Coworker learning is also desktop/local scope. Learns automatically cannot be marketed
until source filtering, redaction, conflicts, policy, storage and revocation pass gates.

## V1 detailed design integration — 2026-10-12

V1 target detailed design: ../docs/V1-DESIGN-INDEX.md, ../docs/NATIVE-COMPOSER-LLD.md, ../docs/CHROME-BROWSER-LLD.md, ../docs/DESKTOP-SHELL-LLD.md and ../docs/RENDERING-CONFORMANCE-LLD.md. Desktop V1 scope retains agent-native fidelity and browser/computer takeover but does not silently promise cloud continuation. Full support is per-OS/provider/harness qualification, not a binary yes/no from the spec alone.

## 2026-10-13 reconciled V1 qualification boundary

The contract corrections do **not** reduce V1 scope. E03-S03 planning and authenticated producer-scoped plan acceptance, E03-S02 native Send, E07-S00 cross-OS isolation plus Windows WorkspaceRoot startup identity revalidation, E08-S03 private automatic-memory lifecycle, and E13-S03 real platform/provider SYSTEM+USER qualification remain required for any support claim that includes those capabilities. A Windows named pipe or ConPTY does not qualify Windows root handling. Keep unsupported roots/agents/providers unavailable and block unsupported OS release-matrix cells rather than relaxing isolation or silently counting them complete. See [the precise owner/evidence matrix](V1-CONTRACT-RECONCILIATION-2026-10-13.md).
