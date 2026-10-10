# LiteCowork

**Give your AI real work. Use the agents you already have. Stay in control.**

LiteCowork is a desktop-first general AI cowork workspace for conversation, local files,
web/app work, documents, research, automation, and software projects. Use a supported
native agent such as Codex, Claude Code, or OpenCode as the chief when appropriate; keep
its native harness and configuration, optionally delegate bounded work to other eligible
agents, and review the resulting actions, evidence, and files in one place. A durable Task
keeps its history when an agent, process, or computer needs to be replaced.

> **Release status:** LiteCowork is in pre-release development. This README describes the
> intended product; a capability is available only after its release gate and provider
> qualification are complete.

## What LiteCowork is for

Start with an ordinary conversation, a plain-language request, local files, a browser task,
a document, or a software project. A Coworker is optional: create one when you want a
persistent named specialist with its own scoped memory, connections, and approved
responsibilities; one-off conversations do not require it. Your selected native chief
agent remains itself, with its own harness and supported tools. When enabled and allowed,
the chief can use native subagents, delegate bounded work to another LiteCowork worker
profile, or invoke a deterministic capability. You can see who handled each part, inspect
outputs and evidence, and decide what to accept.

LiteCowork is designed for broad desktop knowledge work: research and reports, local-file
organization, spreadsheet and document workflows, recurring reviews, browser/app work,
software projects, and reusable procedures. External providers and
the current machine determine which of these capabilities are available in a given setup.

## The finished product

- **One desktop workspace.** A calm composer, active work, items needing your attention,
  verified outputs and scheduled responsibilities. Detailed agent/runtime diagnostics stay
  in the Inspector until you need them.
- **Optional persistent Coworkers.** Give a specialist a name, purpose, scoped memory,
  connections, and approved responsibilities when you want continuity; ordinary chats do
  not require one. Its identity can continue when you change the chief agent.
- **Your native agents remain native.** LiteCowork coordinates supported harnesses without
  replacing their native configuration or claiming ownership of their internal subagents.
  Host-delegated work gets a separate task attempt, scope, environment and result record.
- **Reviewable work.** A Task survives agent and process changes. Attempts, changes,
  artifacts and verification evidence remain inspectable. A worker report alone does not
  mark work complete.
- **More help when useful.** Select worker profiles for cost, quality or latency goals.
  Verification can trigger a bounded retry or escalation. Usage that a provider does not
  report stays unknown; LiteCowork does not invent savings or quota remaining.
- **Files and knowledge.** Add files, folders and ZIPs, see what parsed and indexed, and
  use source-linked retrieval. Resource scope, revision, provider placement and deletion
  remain visible. Local files do not automatically become cloud inputs.
- **Local work first.** The desktop Operator and the `litecoworkd` Runtime have separate
  lifecycles. Start with local files and agents; keep durable work when the window closes.
- **Cloud, then remote.** Cloud continuation starts a new Attempt from portable Task state
  after effect reconciliation and lease fencing. A remote machine runs the same Runtime
  contract. A live process or private agent session is never assumed to teleport.
- **Human authority stays clear.** Browse, prepare, draft, review or verify according to
  policy. Consequential changes can pause for approval or hand control to you. An agent
  cannot grant itself authority.
- **Useful artifacts, not just chat.** Review code changes and qualified documents,
  spreadsheets, reports and other outputs with their revisions and provenance.
- **Repeat work safely.** Turn reviewed work into a Routine; Automations schedule ordinary
  Tasks. Test and review before enabling a recurring responsibility.

## How work is organized

```text
You / Workspace
├── Conversation                         ordinary chat; Coworker optional
│   └── Task?                            durable intent and execution history when needed
│       ├── Plan and Steps
│       ├── Lead agent session
│       ├── Delegated Attempts          bounded, replaceable workers
│       ├── Effects and Artifacts
│       └── Evidence and verification
└── Coworker?                             optional persistent specialist
    ├── Conversations
    ├── scoped Memory / Knowledge / Connections
    └── approved Responsibilities → ordinary Tasks
```

A conversation can be an ordinary chat and does not create a Task unless work is made
durable. A Coworker can own multiple conversations and approved responsibilities, but it
is not required for ordinary sessions and never replaces Task execution truth. Attempts, agent sessions, Runtimes and Environments can change while the Task
remains the source of execution history. LiteCowork mediates shared effects and records
what it can verify; external providers still control their own models, quotas, tools and
native features.

## Desktop-first delivery

V1 is a complete, production-qualified desktop/local product for the finalized local
feature set, not a demo that substitutes mock workers for real provider integrations.
Cloud continuation and remote Runtime are post-V1 releases.

The intended use is broad desktop cowork: ask questions, work with files and documents,
research through qualified browser/app capabilities, prepare reports and spreadsheets,
organize local work, automate approved recurring responsibilities, and use coding agents
for software work when that is the job.
Provider account requirements, model quality, local hardware, site rules and operating-
system support set practical limits; supported capabilities will be published against
tested versions.

## Architecture and development

- [Product architecture](ARCHITECTURE.md)
- [Implementation plan and release gates](implementation/README.md)
- [Contract map](docs/COVERAGE-MATRIX.md)
- [Product contract](docs/PRODUCT.md)
- [User experience](docs/EXPERIENCE.md)
- [Testing contract](docs/TESTING.md)
- [Research sources and workflow comparisons](implementation/SOURCES.md)
- [Decision records](docs/adr/)

LiteSPM is the selected external package/capability ecosystem. It owns package discovery
and lifecycle; LiteCowork owns Task-scoped use. Its LiteCowork wire integration remains
subject to the verified service contract. Its selected service endpoint and the
boundary are recorded in [Capability Fabric](docs/CAPABILITY-FABRIC.md). No package API
or authentication behavior is assumed before its service contract is available.

## Coworkers target evolution

The accepted desktop Coworker design is described in [docs/COWORKERS-TARGET.md](docs/COWORKERS-TARGET.md): optional chat-first persistent assistants, no connector ceiling on configured integrations, natural-language approved responsibilities, quiet scoped memory learning and honest background execution. This is a target design, NOT a claim that UI, memory, trigger or native agent execution are already complete. Current implementation status and gates remain in [implementation/CURRENT-RUN.md](implementation/CURRENT-RUN.md).


### Native agent philosophy

LiteCowork is designed around full native agent harnesses rather than a universal model
wrapper. Agent-specific install/update, sign-in/API-key setup, models/session options,
slash commands, references and native configuration are exposed through modular
AgentModules. LiteSPM owns the cross-agent connector/MCP/plugin/skill package ecosystem;
LiteCowork owns scoped authorization and durable work around their use. See
docs/AGENT-CONTROL.md.
