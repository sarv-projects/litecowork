# ADR-0008: Keep Protocol Boundaries Explicit

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

ACP/A2A agents, MCP capabilities, Environment providers, human channels, and Operator clients have different lifecycle and trust contracts.

## Decision

Do not unify them behind one generic Providers or Channels module. Keep Agent Fabric, Capability Fabric, Environment Fabric, human ChannelAdapters, Operator Protocol, and Runtime Mesh as explicit boundaries.

In particular, LiteCowork has no global **Providers** settings authority and no universal
model catalog/router. Provider accounts, API keys, native model catalogs and session
options belong to the selected agent unless its AgentModule explicitly exposes a secure
SecretStore-backed credential slot. The Agent Registry UI projects those agent-owned
controls inside the agent's own panel. Local model endpoints remain a separate inventory
and appear in an agent's Model picker only when that AgentModule reports compatibility.

## Consequences

Adapters remain replaceable within their own contract, and one protocol's assumptions cannot leak into another. Users do not lose native agent provider/model/auth features simply because the agent is hosted by LiteCowork. See ADR-0025 and `docs/AGENT-CONTROL.md`.
