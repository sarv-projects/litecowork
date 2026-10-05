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
Adapters negotiate a non-secret `AgentHarnessDescriptor` and session options. A supported
LiteCowork bridge may be added explicitly; native configuration is never overwritten.
Unsupported overrides fail before invocation and never silently fall back.

## Consequences

Adapters retain provider-specific features and may expose different capabilities. Feature
parity is not promised. Descriptor digests and option compatibility are revalidated when
configuration changes. Native subagents remain harness-owned unless a separate host
delegation is admitted by LiteCowork.
