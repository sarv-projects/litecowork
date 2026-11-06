# Errors and recovery - Agent Relay on One Conversation

## Target behavior

If B is unavailable, turn admission fails, the user's draft remains available, and the system does not fall back to A or the Workspace default. A partially dispatched B turn settles or fails through Feature 001's turn/session rules before another relay is admitted. Restart reloads committed Conversation turns and per-message producer identity; it never infers a missing response from a native process that may have disappeared.

Stale Conversation versions, idempotency-key reuse with a different payload, disabled/non-lead-eligible bindings, unsupported provider adapters, stale descriptors, unauthenticated profiles, changed native configuration, quota exhaustion, timeout after dispatch, cancellation and Runtime restart use the canonical Feature 001/002 error and recovery contracts. This package must not introduce duplicate error taxonomies.

## Current gaps

- The active supervisor has no second-provider dispatch branch. Unsupported provider keys fail closed; no B turn can currently be executed by Codex through the Conversation supervisor.
- Feature 002's descriptor route currently returns 503, and the desktop descriptor-aware composer helper is not integrated into the production picker/send path.
- The current OpenCode offer is incompatible for new binding creation in the Agent Catalog; the only supervisor dispatch branch is therefore not a complete selectable end-to-end path.
- Resource/artifact refs are not materialized into the native text prompt; the encoder returns `ResourceMaterializationRequired` for nonempty refs/RESOURCE blocks.
- Feature 001 has unresolved dispatch durability, retry ordering and native cancellation/quiescence gaps. These must be closed before relay recovery can be called reliable.
- The UI does not render stored producer identity and the add-context control is inert; a successful API projection cannot be treated as a completed user-visible relay.

## Qualification gate

Denial, duplicate replay, timeout-after-dispatch, provider failure, binding change during admission, cancellation, crash and restart must be exercised for both adapters on each declared OS/auth route. A source-level unit test or mounted route does not qualify the cross-provider journey.