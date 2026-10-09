# Host Guidance and Built-in Skills

## Boundary

LiteCowork may provide small optional presentation instructions and versioned built-in
guidance assets to compatible AgentSessions. This improves consistency across external
agents without rewriting their native harness or treating a prompt as enforcement.

Host guidance is not a CapabilityRef or LiteSPM package. It has:

```text
zero capability grants
zero activations
zero secrets
zero network access
zero filesystem access
zero Effects
zero authority
```

Task, Trust, Artifact, Effect, Approval, and Verification requirements remain enforced by
their owning Core services. An Agent that ignores guidance must remain safe and the host
compiler must still reject fabricated trusted state.

## HostSkill

```text
HostSkillRef {
  host_skill_id: HostSkillId
  uri: string
  version: SemVer
  content_digest: Sha256Digest
  purpose: RESPONSE_DESIGN
  authority_class: GUIDANCE_ONLY
}
```

The v1 `rich-response-design` skill is a bundled immutable asset. The `HostSkillRegistry`
verifies its manifest, version, and digest before loading. Host skills cannot shadow
external LiteSPM/MCP Skills; their identity uses a separate `HostSkillId` namespace. The
Agent Gateway may expose familiar `litecowork.skills.search/load` tool shapes, but
resolution returns a typed target:

```text
SkillLoadTarget =
    HOST_SKILL { host_skill_ref }
  | CAPABILITY_SKILL { capability_ref }
```

A HOST_SKILL load requires no CapabilityGrant or CapabilityActivation, has no resource
scope, creates no CapabilityInvocation, and cannot supply tool operations. A
CAPABILITY_SKILL follows the existing capability approval, activation, provenance, and
content-integrity contract.

## HostInstructionDeliveryMode

Adapters negotiate delivery as informational capability metadata:

```text
HostInstructionDeliveryMode =
    NATIVE_SYSTEM
  | NATIVE_DEVELOPER
  | SESSION_GUIDANCE
  | CONTEXT_ATTACHMENT
  | UNSUPPORTED
```

If unsupported, the Agent remains eligible. The host can still compile deterministic
rich responses from committed messages and authorized projections. LiteCowork never edits
`CLAUDE.md`, `AGENTS.md`, Codex/OpenCode configuration, native Skills, hooks, MCP servers,
or permission files to inject guidance.

`SessionSpec` may include `host_instruction_bundle_ref?` and
`preloaded_host_skill_refs[]`. Delivery is scoped to the active session/turn, bounded by
size, digest-pinned, and recorded only by non-secret digest/ref provenance. By default,
presentation guidance is eligible for human-facing `CONVERSATION` sessions only. It is
not loaded into Task planning or worker Attempts unless a future explicit contract
demonstrates a need and token/cost benchmarks justify it.

## Minimal policy instruction

The small always-on instruction is limited to these points:

1. Default to a clear ordinary answer.
2. Use the rich-response-design HostSkill only when richer structure materially improves
   comprehension or interaction.
3. Prefer rich structure for comparisons, substantive research, quantitative data,
   diagrams, media, multiple deliverables, and large structured results.
4. Prefer prose for brief facts, explanations, troubleshooting, and casual conversation.
5. Refer only to host-provided source and system identities.
6. Never invent Artifact, Task, Approval, Verification, Citation, download, Runtime, or
   capability state.
7. Preserve a complete semantic-message fallback.

This instruction is a hint, not security policy. The schema, compiler, source resolvers,
and owning services enforce the boundary.

## Built-in response-design skill

The skill is preloaded only when the deterministic pre-session presentation policy says
that it is useful and the Agent adapter can carry optional guidance. It does not trigger
an extra model call by itself. It teaches the Agent to:

- write a complete semantic answer before proposing layout;
- call `litecowork.presentation.propose` only when useful;
- select an appropriate bounded layout, density, and width;
- use real supplied Resource/Artifact/Invocation refs;
- cite exact authorized source revisions where available;
- provide textual and accessibility fallbacks for charts and diagrams;
- distinguish informal local checklists from real Task Steps and UserRequests;
- never encode host-owned status, authority, or provenance.

The skill is guidance only. The rich-response compiler can apply deterministic renderers
even when the Agent does not load or follow it.

## Integrity failure and fallback

If the built-in asset digest, manifest, version, or policy bundle fails validation, do not
load it. Continue with the semantic Conversation response and deterministic safe
renderers, and record a sanitized local diagnostic. Guidance integrity failure does not
block ordinary Conversation or Task work. The Agent receives no partial or unverified
asset bytes.
