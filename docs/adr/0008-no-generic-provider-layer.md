# ADR-0008: Keep Protocol Boundaries Explicit

- **Status:** Accepted for vNext
- **Date:** 2026-10-01

## Context

ACP/A2A agents, MCP capabilities, Environment providers, human channels, and Operator clients have different lifecycle and trust contracts.

## Decision

Do not unify them behind one generic Providers or Channels module. Keep Agent Fabric, Capability Fabric, Environment Fabric, human ChannelAdapters, Operator Protocol, and Runtime Mesh as explicit boundaries.

## Consequences

Adapters remain replaceable within their own contract, and one protocol's assumptions cannot leak into another.
