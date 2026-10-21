# HorizonCode as a Native LiteCowork Agent — V1 Integration LLD

**Status:** target integration; no claim that HorizonCode is detected, installed or qualified in current LiteCowork builds.  
**Canonical owners:** [Agent Control](AGENT-CONTROL.md), [Agent Fabric](AGENT-FABRIC.md), [Delegation](DELEGATION.md), [Task Runtime](TASK-RUNTIME.md), [Trust](TRUST.md), [Agent Portfolio](../implementation/AGENT-PORTFOLIO-2026-10-21.md).

## Intent

HorizonCode is the separately developed long-horizon coding harness in the LiteCowork family. It must appear as an **ordinary native AgentModule candidate** in Discover/Agents, native composer, qualified Task chief selection and DelegationProfile worker selection. Do not create a hidden HorizonCode-specific orchestration engine or special Core privileges.

The installed HorizonCode executable, version/manifest and native auth/config may be detected/managed by its qualified AgentModule. If distributed through LiteSPM as an agent package, LiteSPM owns only the package's install/update/provenance mutations; HorizonCode's module still owns session and native feature semantics.

## Versioned AgentModule manifest

Require:
- `agent_id = horizoncode`, exact executable/transport version and supported OS targets (no assumption that source checkout means installed);
- supported native transport (ACP/CLI/RPC/SDK only if independently implemented and qualified), frame limits and protocol handshake;
- detected binaries, executable provenance, installation/version observation and native auth readiness;
- models/provider options native to HorizonCode (not LiteCowork global), defaults, effort/modes when supported;
- native commands, references, tool capabilities, subagents/worker limits and extension inventory with supported/unsupported flags;
- session lifecycle: create/send/stream/interrupt/resume/stop, cancellation guarantees, background child tracking and quiescence observation;
- declared root/file/env/egress permissions, native tool interception/approval assurance and honest support matrix;
- host quality/latency/cost observations only when real receipts exist.

## Chief Agent path

Conversation → native HorizonCode AgentSession through selected AgentModule, commit semantic messages and qualifying stream events. Ordinary chat must not create a Task. For a durable Task: selected lead plans under producer-scoped isolation; accepts a versioned PlanRevision after Core policy; authorized Steps produce Attempts and Effects. HorizonCode's internal long-horizon run/checkpoint is native provenance and may inform the Task; it is **not** a replacement for Core Task, Attempt or Event state.

Do not treat native “completed” as independently verified Task completion. External/native effects must be labelled `reported` or `observed` unless a qualified mediated receipt/proof exists. If HorizonCode holds private checkpoints or provider sessions, a chief change creates a portable new Attempt with pinned Task information, never assumes native session teleportation.

## Worker path

HorizonCode may be selected as a managed worker under an explicit DelegationProfile. Worker admission requires accepted Step, child-scoped authority, deadline/budget, Environment isolation, own AgentSession, bounded result envelope and integration verification. Native HorizonCode subagents used *inside* a parent HorizonCode session are not automatically separate LiteCowork Attempts unless explicitly mediated and recorded; distinguish internal native fan-out from heterogeneous host delegation.

## Tool and LiteSPM boundary

HorizonCode-native skills, MCP servers, plugins, hooks and commands remain visible as native config where a qualified adapter can observe them. LiteSPM may optionally adopt/modify eligible entries with explicit approval; it must not silently overwrite HorizonCode's native package manager or secrets. `AgentCapabilityBridge` advertises DIRECT_NATIVE_ATTACHMENT, LITECOWORK_GATEWAY, HOST_CONTEXT_ONLY or UNSUPPORTED with actual enforcement/observation guarantees.

## Failures

Missing executable, unsupported protocol version, expired auth, provider quota, native option drift, child process remaining after cancel, changed workspace revision, lost stream, unknown tool effect, stale lease and restart must all fail/restore explicitly. Preserve user drafts and durable Task state even if HorizonCode is removed mid-run. Fallback to another lead requires an owner-authorized handoff; no silent provider substitution.

## Qualification tests

1. Native HorizonCode cold discovery and clean no-touch observation; qualified install/upgrade path if enabled.
2. Authenticate through HorizonCode-native flow and select a model from its own current descriptor; reject unsupported/expired options.
3. Real Conversation Send, stream, Stop, interrupted stream, reconnect/history and native commands.
4. One real repository edit as chief: admitted Task → planning → Step/Attempt → workspace mutation → verified result → artifact.
5. One bounded child Attempt as worker under another eligible chief; test budget, root, cancellation, refusal and result integration.
6. Native internal worker does not get false Core Attempt or Grant claim.
7. Kill/restart, late outputs, unknown effect, native config drift and owner-driven chief replacement.
8. Actual host OS/version and CODE + SYSTEM + USER proof in `implementation/QUALIFICATION-MATRIX.md`.

Until those tests pass, HorizonCode is **rostered target / unqualified**, even if separately developed and available on disk.
