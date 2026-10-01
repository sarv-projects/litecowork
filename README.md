# AgentCowork

AgentCowork is a durable execution workspace for AI agents. It keeps user intent,
task state, artifacts, effects, and evidence durable while allowing compatible agents,
capabilities, runtimes, and environments to be replaced or combined.

The current target architecture is defined by [ARCHITECTURE.md](ARCHITECTURE.md).
Focused design documents live in [`docs/`](docs/), with decision rationale in
[`docs/adr/`](docs/adr/).

## Current phase

The previous implementation and v1 architecture corpus have been moved to local,
git-ignored `archive_code/` and `archives_docs/` folders. They are historical material,
not active implementation or design authority. The vNext system is being rebuilt from
the architecture contracts before implementation resumes.

## Product shape

One conversation can contain ordinary questions and durable tasks. When work needs a
persistent outcome, AgentCowork records a Task, lets an external agent own the reasoning,
and coordinates attempts across runtimes and environments. It records mediated effects,
artifacts, evidence, and verification so a task can resume without depending on one
agent transcript or one running process.
