# Repository Agent Instructions

These instructions apply to the vNext rebuild. Existing application code and the former
architecture set have been moved to local, git-ignored archives and are not the active
implementation or design authority.

## 1. Mission and architecture authority

AgentCowork is a durable execution workspace around external agent intelligence. Its
current architecture authority is [`ARCHITECTURE.md`](ARCHITECTURE.md). Focused contracts
live under `docs/`; ADRs explain rationale and never silently amend the authority.

Read the relevant architecture contracts before implementation. Preserve the core
distinctions between Conversation, Task, Step, Attempt, Agent, AgentSession, Runtime,
Environment, Capability, Effect, Artifact, Evidence, and ExecutionLease. External agents
own reasoning and native tools. AgentCowork owns durable Task coordination and
Core-mediated shared effects.

`archive_code/`, `archives_docs/`, and `ARCHIVE/` are ignored historical material. Do not
build on, copy from, or treat them as current requirements unless the user explicitly
asks to recover a specific item. The `.agents/skills/` utilities remain available;
repository-specific notes in `.agents/` do not override this file or current architecture
authority.

## 2. Understand before changing

Before substantial edits:

1. locate the repository root and inspect Git status/current branch;
2. read this file and the relevant architecture contracts;
3. inspect implementation, callers/dependents, configuration, and tests narrowly;
4. use the `codebase-intelligence` skill for architectural, unfamiliar, or repository-wide
   code work when available;
5. use structural evidence for non-trivial dependency relationships and distinguish
   observed facts from inference.

Do not map the entire repository indiscriminately or invent relationships from names.

## 3. Architecture change workflow

For a behavior or ownership change:

1. identify the owning boundary and invariant in `ARCHITECTURE.md`;
2. update the current authority and relevant focused contract when the design changes;
3. add/amend an ADR for the decision and tradeoffs;
4. update acceptance coverage and implementation plan as appropriate;
5. implement the smallest coherent vertical slice;
6. validate the stated acceptance criteria and report gaps plainly.

Do not silently contradict the architecture. If the task and current contract conflict,
explain the conflict and propose the authority change before coding around it.

## 4. Core boundaries

- A Task outlives Attempts, AgentSessions, processes, devices, and Environments.
- External agents own reasoning, model choice, private configuration, and native tools.
- Core policy, tickets, audit, and receipts cover Core-mediated calls only.
- LitePSM owns package ecosystem discovery and lifecycle; AgentCowork owns scoped use.
- Runtime is `agentcoworkd`; Environment is where an Attempt acts.
- Cloud continuation creates a new Attempt from portable Task state after effect
  reconciliation and lease fencing; a live process is not assumed to migrate.
- Durable artifacts, effects, and evidence remain Core concerns. Domain-specific Office,
  browser, computer-use, search, code-intelligence, connector, memory, and workflow
  engines are external capabilities/providers unless the current authority is amended.
- Replicate domain events and immutable artifacts; do not sync database files or native
  agent private state.

## 5. Editing and validation

- Make the smallest coherent change that fulfills the request.
- Follow the conventions of the new module and toolchain; do not assume conventions from
  archived code. Document a new dependency and why it is needed.
- Do not modify generated files manually when a source/template exists.
- Add or update focused tests for behavior changes. Run relevant tests, formatting, lint,
  and type checks; if one cannot run, record the exact reason.
- Review the complete diff, run `git diff --check`, verify intended files only changed,
  and keep caches, secrets, local indexes, and archives out of Git.

## 6. Git discipline

After a meaningful completed change, inspect `git status --short`, stage intended paths
explicitly, review the staged diff, and commit a clear software-focused message. Never
use `git add .` blindly. Do not rewrite, reset, or discard unrelated work without
explicit instruction.

Commit messages, comments, implementation notes, generated files, and documentation must
not attribute authorship to an AI tool or model. Named external products may be cited as
technical prior art when relevant; do not claim a tool authored the change.

## 7. Repository skills

Use a reusable workflow when one exists. `codebase-intelligence` supports structural
repository analysis. The skill store's portable code is MIT and its instructional content
is CC0; proposals changing the skill store require the documented seven-day lazy-consensus
review window before merge.
