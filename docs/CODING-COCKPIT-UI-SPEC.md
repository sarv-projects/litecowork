# LiteCowork — Coding Cockpit and Unified Workbench UI Specification

**Status:** product/UI target derived from Revision 9, the V7 UI completeness audit and the Blaze cockpit cross-analysis.  
**Authority:** DESKTOP-SHELL-LLD.md and V1-UI-SCREEN-CONTRACT.md remain the UI owners; this document makes the coding/work experience concrete.

## 1. Product rule

LiteCowork is one workspace, not separate Chat and Code products. A coding task is an intent/preset over the same Conversation, Workspace, AgentBinding, Workbench, Task and Artifact authorities. Switching from coding to research or ordinary conversation must not discard history, resources or native agent controls.

The UI should feel like a calm, polished desktop workspace: fast navigation, little chrome, predictable panel behavior, immediate feedback and progressively disclosed technical detail. A three-column structure is the default model, not a mandate to keep every panel permanently expanded.

## 2. Global shell

### Left sidebar — navigation and durable objects

- Workspace switcher and current Workspace identity.
- New conversation and searchable recent conversations.
- Optional Coworkers.
- Needs You as an actionable inbox with counts derived from real pending work.
- Work/Tasks and run history.
- Automations and Routines.
- Library/Artifacts and Files/Knowledge.
- Discover/Marketplace backed by LiteSPM.
- Settings at the bottom; technical Agent Registry, environment/runtime and diagnostics live there or in Inspector.

Keep recents visible. Do not add duplicate top-level Projects or a separate coding universe. Workspace Home is a projection over the Workspace and its existing objects.

### Center — conversation and active work

Header:
- Conversation title and Workspace context.
- Selected native lead agent and route readiness.
- Agent-owned model, effort/mode and supported native options.
- Task state when the Conversation is linked to a Task.
- Context indicator showing the actual attached Resources, not an inferred list.

Timeline:
- Human messages and committed agent messages.
- Streaming state shown separately from committed history.
- Typed activity/tool cards for calls, approvals, questions, artifacts, changes and verification.
- Expandable details for exact tool arguments/results, provenance and timing.
- Structured error and recovery states; never replace failure with a generic “something went wrong” toast.

Composer:
- Text, supported attachments, image/audio input and agent-native affordances.
- Explicit selected agent; never silently switch lead.
- Send for ordinary Conversation.
- Run as Task when a durable outcome is intended.
- Schedule and Save as Routine as explicit preview/confirmation actions, not hidden natural-language side effects.
- Stop while a turn is running; Retry only when its owner contract permits a safe retry.
- Preserve unsent drafts when an agent or required capability is unavailable.

### Right — contextual Workbench

A single resizable Workbench changes its content with the current Conversation/Task and user selection. It may collapse on narrow screens but must preserve its current tab/focus and return state.

Tabs:
1. **Activity** — event timeline, active tools, child sessions, approvals, waits, retries and verification.
2. **Files** — selected Workspace roots, files attached to this work and file provenance.
3. **Diffs** — actual working changes, per-file review, accept/reject/restore where supported.
4. **Terminal** — real Environment-bound PTY with process state and stop/interrupt.
5. **Browser** — dedicated browser session, current page, navigation/interaction state and takeover/return.
6. **Computer** — only when a qualified provider exists; visible owner, session epoch and active control.
7. **Artifacts** — output preview, versions, source references, export and Library publication.
8. **Inspector** — exact agent route, ContextAttachments, grants, Runtime/Environment, invocation/effect/evidence IDs and diagnostics.

A tab may be present in the design system but must be disabled with a specific reason when the provider is absent or unqualified. Never render a fake terminal/browser pane with static sample data as a working capability.

## 3. Coding cockpit behaviors

- **Agent activity:** show the exact native agent and session that produced each event. Distinguish native opaque activity from LiteCowork-mediated capability invocation.
- **Tool cards:** render structured operation, inputs/outputs, status, timing, and links to affected files or artifacts. Keep untrusted tool output sandboxed and bounded.
- **Approval inbox:** present action, target, exact scope, plan/effect digest and consequence. Approval applies to that exact action; material changes require a new approval.
- **Diff-first review:** display actual filesystem/git changes, not model-generated summaries. Separate applied change, proposed patch and uncommitted native-agent change.
- **Worktrees:** represent a worktree as an Environment/resource with identity, base revision, dirty state and lifecycle. Parallel writers require isolation; cleanup must never delete uncommitted user data.
- **Subagent tree:** show parent/child relationship, assigned plan Step, agent, state, budget/usage where known, handoff and evidence. Native subagents are distinguished from LiteCowork-managed delegated Attempts.
- **Context view:** show only actual context attachments/retrieval receipts with source, scope, revision/digest and freshness. Do not claim to know hidden agent reasoning.
- **Needs You:** link the item directly to the blocked action, approval, question, stale Connection or unresolved Task. User resolution is an append-only decision, not a rewrite of history.

## 4. Interaction and visual rules

- Use immediate local response for focus, selection and panel state; show pending/settled separately for remote or daemon-backed mutations.
- Preserve keyboard focus when switching tabs or returning from browser/computer takeover.
- Support keyboard shortcuts, command palette, accessible names, visible focus, reduced motion and high-contrast states.
- Keep motion restrained and purposeful. Do not use loading spinners as a substitute for durable progress.
- Make resizing, collapsing and restoring the Workbench deterministic.
- Provide empty, loading, stale, offline, unsupported, permission-denied, running, ambiguous and failed states.
- Do not use color alone for status; pair with text/icon and exact state.
- Show costs, usage, quotas and progress only from a grounded source. Unknown stays Unknown.
- Avoid a second inspector panel or duplicated settings drawers; one context-aware right Workbench is the rule.

## 5. Trust and native fidelity

Native agents keep their own reasoning, auth, model catalog, private prompts, tools, slash commands, references and subagents. The UI projects supported controls per exact AgentModule/version/OS route. Unsupported controls must not appear to work.

LiteCowork governs only mediated effects. A native tool or native subagent outside the Gateway is labelled as native reported/observed activity unless independent enforcement is qualified. A package being installed or connected does not make it granted or invocable.

## 6. Minimum acceptance for the shell

- Resize/collapse/restore the right Workbench without losing tab/focus.
- New Conversation, recents, Workspace switching and deep links resolve durable objects.
- Send/stream/stop/reload operates through the real daemon and selected native agent.
- A real file diff matches filesystem/git state.
- Terminal is attached to the admitted Environment and can interrupt/settle a process.
- Browser attach/takeover/return uses current session epochs and revokes stale handles.
- Approval card binds to the exact action and has a durable decision receipt.
- Subagent view distinguishes native subagents, LiteCowork workers and unavailable routes.
- Responsive, accessibility and reduced-motion checks pass in the shipped desktop shell.
- With providers disabled, each affected panel explains why it is unavailable and performs no fake action.

## 7. Inspiration and code-use boundary

The Blaze repository was treated as a coding-cockpit UX reference only. Adopt useful patterns such as a three-column shell, activity/tool cards, approval review, worktree awareness and subagent timelines. Do not copy its AGPL-3.0 source code into LiteCowork without a separate legal decision. The broader LiteCowork architecture remains BYOA cowork, not a coding-only product.
