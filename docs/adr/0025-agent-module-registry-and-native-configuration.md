# ADR-0025: Agent Modules Own Agent-Specific Lifecycle and Native Configuration

- **Status:** Accepted for vNext
- **Date:** 2026-10-10

## Context

LiteCowork must support many external agents without reducing them to a universal model
wrapper. Agents differ in installation/update mechanisms, native sign-in, provider/API-key
configuration, model/session options, slash commands, reference semantics, extensions,
subagents, protocol versions, and update cadence.

A global Provider screen or hard-coded `if agent == ...` branches would create a second,
stale configuration authority and would make every upstream agent release a Core/UI
rewrite. Conversely, hiding native agent functionality would make LiteCowork strictly less
capable than using the original harness directly.

LiteSPM separately owns package discovery/lifecycle for connectors, MCP servers, plugins,
skills, and related capability packages. LiteCowork still owns scoped authorization and use
of those capabilities in Conversation/Task execution.

## Decision

Each supported agent is implemented as an independently versioned **AgentModule** with:

- an `AgentLifecycleAdapter` for installation observation, install/update, native
  authentication/configuration and control-descriptor discovery;
- an `AgentAdapter` for sessions, input, events and runtime protocol behavior;
- an optional `AgentCapabilityBridge` describing how LiteSPM-managed capabilities can
  be exposed to that harness.

Agent-specific models, reasoning/effort/session options, slash commands, `@` references,
input capabilities, native configuration targets, auth methods and credential slots are
projected through a time-bounded `AgentControlDescriptor`. LiteCowork does not create a
global provider/model catalog.

Secret-bearing configuration is either native-agent-owned or written directly to
SecretStore through an adapter-declared secure credential slot; ordinary AgentBinding
configuration remains non-secret.

The Operator Agent Registry consumes live/cached ACP Registry distribution metadata for
discovery, but registry presence never proves installation, authentication, capability
support, readiness or authorization. Registry metadata cannot supply arbitrary executable
commands or privileged auth actions.

See [Agent Control LLD](../AGENT-CONTROL.md) for the normative lifecycle, API, schema,
pseudocode and UI contract.

## Consequences

- Users retain the capabilities of the original harness instead of a reduced LiteCowork
  subset.
- Agent updates are isolated primarily to one module plus conformance fixtures.
- Core remains agent-brand-agnostic.
- The UI can vary per agent without inventing global provider/model abstractions.
- Install/auth/config readiness must be represented as separate states.
- Per-agent real-provider/version qualification is required before claiming support.
- LiteSPM can provide a common package ecosystem while AgentModules decide the qualified
  attachment route for each harness.
