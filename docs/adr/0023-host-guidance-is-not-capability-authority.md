# ADR-0023: Built-in Host guidance is not capability authority

**Status:** Accepted

## Context

External native agents differ in how they accept instructions, Skills, system prompts, and
context. Rewriting native config would violate harness integrity. Modeling a bundled
response-design guide as a LiteSPM/MCP `CapabilityRef` would incorrectly imply package,
grant, activation, secret, or provider semantics.

## Decision

LiteCowork may deliver small optional host instructions and versioned bundled `HostSkill`
assets through adapter-negotiated session/context channels. HostSkill IDs use a distinct
namespace and carry `GUIDANCE_ONLY` authority. Unsupported delivery does not make an Agent
ineligible. HostSkills have no grant, activation, secret, resource scope, network,
filesystem, Effect, or invocation. Trust and Task requirements remain enforced by their
owning services; the compiler independently rejects fabricated trusted state.

## Consequences

- Claude/Codex/OpenCode/Cline native configuration remains unchanged.
- Deterministic host rendering still works if an Agent ignores or cannot receive guidance.
- LiteSPM and MCP Skills retain their existing capability lifecycle.
- HostSkill content/digest is auditable without being a capability installation or
  authority grant.
