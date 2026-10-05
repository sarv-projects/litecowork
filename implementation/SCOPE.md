# Release scope

## V1 commitment

User alone reviews; coding agents implement. No additional developer, QA team,
product manager, or security engineer is assumed. Reviewer bandwidth is a release resource.
Desktop/local is the first usable product. Cloud follows that proven local product;
remote Runtime follows cloud. All finalized contract features remain in the release
backlog, including non-coding office, research, personal-assistant, browser, and automation
workflows. A coding-agent implementation team does not make the end product coding-only.

The first desktop alpha is an increment, not the complete V1. Full V1 is released only
when the local, cloud, remote, and finalized capability gates pass. A missing external
provider is a documented blocker or a visibly unavailable optional integration; it cannot
be marked implemented using a fixture. Any actual reduction of the committed feature
scope requires an explicit owner decision and a revised coverage/release record.

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
  notifications, a cloud-hosted qualified human channel, reusable work/Teach-a-task/SkillProposal.
- Cloud pairing/replication/fencing/handoff, remote Runtime enrollment, backups/restores,
  upgrades/rollback, observability, deployment and production incident recovery.

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
| G3 cloud complete | two Runtime fault tests, real cloud deployment, restore, handoff | Start remote qualification |
| G4 remote complete | enrollment/revocation, remote dependency and fencing tests | V1 candidate |
| G5 production release | release matrix, signed qualified installers, no open critical defects | Publish V1 |

No calendar date or velocity is asserted before measured delivery. Gates represent
capability and evidence; dependency-ready stories can overlap within a gate's safe bounds.
