# ADR-0012: Preserve Native Agent Harnesses

- **Status:** Accepted for vNext
- **Date:** 2026-10-05

## Context

Flattening Claude Code, Codex, OpenCode, Cline, or future agents into a universal model
wrapper would discard native tools, session behavior, permissions, hooks, skills, memory,
and subagent semantics. Copying native configuration into LiteCowork would create a
second, stale configuration authority and could expose private prompts or secrets.

## Decision

LiteCowork owns the outer Task/Attempt/Trust contract and preserves each native harness.
Each supported harness is owned by an independently versioned AgentModule. Its lifecycle
adapter projects installation/update status, native sign-in/API-key methods, native
configuration targets and a time-bounded non-secret `AgentControlDescriptor`; its runtime
adapter negotiates the `AgentHarnessDescriptor` and session options. The Operator may
surface all of these controls inside the agent's own panel, but it does not copy them into
a global provider/model authority.

A supported LiteCowork/LiteSPM bridge may be added explicitly; native configuration is
never overwritten. Slash commands, `@` references, input types, provider configuration,
native extensions and native subagents remain available when the adapter reports them.
Unsupported or stale overrides fail before invocation and never silently fall back.

## Consequences

Adapters retain provider-specific features and may expose different capabilities. Feature
parity is not promised, but LiteCowork must not intentionally remove a safely supportable
native feature merely to preserve a common-denominator UI. Descriptor digests and option
compatibility are revalidated when configuration or upstream versions change. Native
subagents remain harness-owned unless a separate host delegation is admitted by LiteCowork.
See ADR-0025 and `docs/AGENT-CONTROL.md`.
