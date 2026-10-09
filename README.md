# LiteCowork

**Give your AI coworker a task. Keep your coding agents. Review the work.**

LiteCowork is a desktop-first workspace for getting useful work done with coding agents.
Keep your preferred agent in the lead, assign bounded work to other eligible agents, and
review the resulting changes, evidence and files in one place. A durable task keeps its
history when an agent, process or computer needs to be replaced.

> **Release status:** LiteCowork is in pre-release development. This README describes the
> intended product; a capability is available only after its release gate and provider
> qualification are complete.

## What LiteCowork is for

Start with a repository, a folder of project files, or a plain-language request. LiteCowork
organizes durable work around a Coworker you name and configure. Your selected coding
agent remains itself, with its native harness and supported tools. When enabled and
allowed, the lead can delegate bounded work to another coding agent, a lower-cost profile,
or a deterministic capability. You can see who handled each part, inspect the diff and
outputs, run verification, and decide what to accept.

LiteCowork is designed to cover more of the work around software projects too: research
and reports, local-file organization, spreadsheet and document workflows, recurring
project reviews, browser preparation, and reusable procedures. External providers and
the current machine determine which of these capabilities are available in a given setup.

## The finished product

- **One desktop workspace.** A calm composer, active work, items needing your attention,
  verified outputs and scheduled responsibilities. Detailed agent/runtime diagnostics stay
  in the Inspector until you need them.
- **A durable coworker.** Give a Coworker a name, role, instructions, source context and
  allowed worker profiles. Its identity continues when you change the lead agent.
- **Your coding agents remain native.** LiteCowork coordinates their work without
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
You
└── Coworker
    └── Conversation
        └── Task                         durable intent and execution history
            ├── Plan and Steps
            ├── Lead agent session
            ├── Delegated Attempts      bounded, replaceable workers
            ├── Effects and Artifacts
            └── Evidence and verification
```

A conversation can be an ordinary chat and does not create a Task unless work is made
durable. Attempts, agent sessions, Runtimes and Environments can change while the Task
remains the source of execution history. LiteCowork mediates shared effects and records
what it can verify; external providers still control their own models, quotas, tools and
native features.

## Desktop-first delivery

V1 is a complete, production-qualified desktop/local product for the finalized local
feature set, not a demo that substitutes mock workers for real provider integrations.
Cloud continuation and remote Runtime are post-V1 releases.

The intended use is that coding agents implement bounded changes while you review them.
The desktop/local V1 also targets real non-coding workflows: organizing files, working
from local documents, preparing reports, and handling approved recurring project work.
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

LiteSPM is the planned external package/capability ecosystem. It owns package discovery
and lifecycle; LiteCowork owns Task-scoped use. Its selected service endpoint and the
boundary are recorded in [Capability Fabric](docs/CAPABILITY-FABRIC.md). No package API
or authentication behavior is assumed before its service contract is available.

## Coworkers target evolution

The accepted desktop Coworker design is described in [docs/COWORKERS-TARGET.md](docs/COWORKERS-TARGET.md): optional chat-first persistent assistants, no connector ceiling on configured integrations, natural-language approved responsibilities, quiet scoped memory learning and honest background execution. This is a target design, NOT a claim that UI, memory, trigger or native agent execution are already complete. Current implementation status and gates remain in [implementation/CURRENT-RUN.md](implementation/CURRENT-RUN.md).
