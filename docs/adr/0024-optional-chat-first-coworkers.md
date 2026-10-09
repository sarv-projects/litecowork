# ADR 0024 — Optional chat-first Coworkers with bounded proactive authority

**Status:** Accepted target, 2026-10-10. Implementation, schema migration and owner acceptance remain pending.

## Context

The original LiteCowork design described a primary Coworker and a Task-first Home. It also relied on manually authored ContextDocuments and exposed worker/per-operation settings early. The revised product serves nontechnical users who expect ordinary conversations without mandatory assistant setup, but may also build persistent specialized assistants that keep context, use many integrations and execute approved responsibilities while the desktop window is closed.

Existing contracts distinguish Conversation, Coworker, Task, Goal, Routine, Automation/Occurrence, AgentSession, CapabilityGrant/Invocation, Effect, Approval and Resource. A second autonomous Coworker planner or scheduler would duplicate the established execution authority and risk grants, recovery and verification inconsistencies.

## Decision

1. **Optional identity:** An ordinary Conversation does not require a Coworker. Coworkers are Workspace-scoped named persistent identities. Selecting one opens its most recent same-Workspace Conversation; no default Coworker is forced at first use. Creating and changing identity cannot silently alter existing Task provenance.
2. **Chat as primary surface:** The Coworker opens Conversation first and provides secondary Responsibilities, Work, Memory & Knowledge, Connections & Tools and Settings views. A Task still has independent durable lifecycle, and the right Workbench continues to show real ArtifactVersions. The UI never fabricates conversation execution.
3. **Native reasoning remains native:** The selected eligible chief agent owns reasoning and its own subagents; LiteCowork coordinates via current Task, Capability, Trust and Environment services. Model/provider authentication remains harness-specific. External worker profiles are separately enabled.
4. **Configured connections not limited:** Coworker may associate any number of qualified apps, MCP servers, Skills, local app providers and resources, within actual host capacity. Catalog/discovery/activation are progressive. Shared installation and account authentication are separate from Coworker-specific assignment, live grant and Invocation. Ordinary Add Connection is simple; advanced access is optional. Consequential unmediated calls cannot falsely claim Core enforcement.
5. **Standing responsibility grouping:** One reviewed user responsibility may refer to several Goal/Routine/Automation revisions. StandingResponsibility only links configuration and fences new unattended Task admissions. TriggerCoordinator owns due occurrences; TaskService owns execution; Trust owns authorization. Mere Goals or memory do not self-trigger work. Autonomous monitoring is bounded, explicitly authorized and filtered before invoking a model.
6. **Automatic but scoped memory:** Eligible conversations and verified completed work can generate private memory candidates at bounded checkpoints via a qualified agent-backed extractor. Core owns source eligibility, privacy, provenance, revision and revocation, not semantic reasoning. Shared-scope learning needs separate policy. Temporary chats do not enter the learning pipeline. Automatic learning and autonomous actions are independent.
7. **Desktop continuity:** With an explicitly enabled background Runtime, closing the Operator does not stop eligible work while the host is awake; sleeping/powered-off devices cannot execute local Tasks. Missed schedules, ambiguous Effects, stale credentials and native-agent restarts require normal recovery checks.
8. **Versioned rollout:** Proposed Conversation owner, Responsibility, assignment and memory schemas are accepted requirements, not current SQLite/Event/OpenAPI types until implemented and validated. Current launch blockers and honest UI fallbacks are retained.

## Rejected alternatives

- Mandatory Coworker at first run: adds avoidable setup and couples ordinary chat to persistent identity.
- A model-controlled background Goal Keeper or Coworker scheduler: duplicates Task/Automation authority and consumes quotas while idle.
- Unlimited concurrently running MCP processes and eager tool manifests: unnecessary context/CPU/network cost and permission exposure.
- Per-tool permission-checkbox wizard: makes consumer setup laborious without supplying a trustworthy enforcement boundary.
- Automatic memory from every readable connector: violates source-scope separation and creates private-data risks.
- Faking cloud availability or native Conversation output while local-only Runtime/agent turns are unqualified: untrustworthy.

## Consequences and migration

Preserve existing Workspace primary_coworker_id data as an optional legacy Task-origin preference; it is not Conversation ownership. Existing Conversation rows migrate with nullable owner. New revisions require expected-version and RequestId, same-Workspace foreign keys and durable outbox where triggers change. New event/route families must be added to typed machine contracts only with service code and negative tests. No active provider dispatch is enabled merely by this ADR. Consumer UI may progressively expose Quick Create and optional customization, but no unsupported control may claim committed memory, scheduled execution or native responses. Cross-Coworker handoffs create separate authorized recipient Tasks with explicit attached Artifact refs; never transfer private memories or native secrets implicitly.

The detailed requirements, failures, UI controls and phased tests live in [Coworker target](../COWORKERS-TARGET.md), [Coworker flows](../COWORKER-FLOWS.md) and [UI acceptance](../../implementation/UI.md). This ADR is subordinate to ARCHITECTURE.md and the owning domain contracts.
