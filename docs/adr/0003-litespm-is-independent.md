# ADR-0003: Keep LiteSPM Independent

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

Package discovery, verification, installation, updates, provider supervision, and
removal form a separate ecosystem lifecycle from task-specific authorization and use.

## Decision

LiteSPM owns package/ecosystem truth for connectors, MCP servers, plugins, skills and
related capability packages: discovery, verification, installation, updates, removal and
provider/package supervision. It is the shared package substrate used to make a qualified
package available across multiple compatible agents.

LiteCowork owns CapabilityRefs, Offers, Grants, Activations, Invocations, Effects and
Evidence scoped to Conversation/Task/Attempt use, and integrates through a LiteSPM client.
Each AgentModule owns the qualified bridge from a LiteSPM-managed capability into that
agent (native attachment, LiteCowork Gateway, context-only, or unsupported).

## Consequences

Catalog discovery and package installation grant no execution authority. Active Tasks pin
exact versions/digests. LiteCowork does not build a second marketplace or silently rewrite
an agent's existing native extension configuration. A package being usable through one
agent does not imply another agent supports the same attachment route; that compatibility
is adapter-qualified. See `docs/AGENT-CONTROL.md` and
`docs/CAPABILITY-FABRIC.md`.
