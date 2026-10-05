# Messaging Channels LLD

Channels are human-facing transports, not Task owners and not agent protocols.

## ChannelAdapter

```text
interface ChannelAdapter {
  connect(ChannelConfig) -> ChannelBinding
  receive(ChannelCursor?) -> Stream<InboundChannelEnvelope>
  acknowledge_ingress(IngressAcknowledgementHandle) -> Ack
  send(OutboundChannelMessage) -> DeliveryReceipt
  capabilities() -> ChannelCapabilities
  disconnect() -> Ack
}

InboundChannelEnvelope {
  event: InboundChannelEvent
  next_cursor?: ChannelCursor # opaque; persisted only after receipt + replication barrier
  acknowledgement_handle?: IngressAcknowledgementHandle # process-memory only
}

ChannelCapabilities {
  supports_reply_to_message: boolean
  supports_threads: boolean
  supports_attachments: boolean
  supports_edits: boolean
  supports_delivery_receipts: boolean
  supports_idempotent_send: boolean
  supports_stable_event_ids: boolean
  supports_replay_from_event_id: boolean
  supports_deferred_acknowledgement: boolean
  replay_window_ms?: u64
}

DeliveryReceipt {
  acknowledged: boolean
  provider_message_ref?: string # returned only when reply-to correlation is supported
  provider_receipt_ref?: string
}
```

Providers may include Telegram, Slack, Discord, Teams, Email and Webhook.

`supports_deferred_acknowledgement` means the adapter can avoid finalizing provider
delivery until LiteCowork has durably committed the receipt. A pull adapter meets this by
not advancing its provider cursor; an acknowledgement-based adapter waits for
`acknowledge_ingress`. The acknowledgement handle is opaque, process-memory-only, and is
never stored in a receipt, event, log, API response, or backup. An adapter that cannot defer
acknowledgement may host ingress only where its Runtime is the authoritative Workspace
Store; it cannot claim automatic cross-Runtime no-loss continuity.

## Provider setup and operator controls

Provider-specific account authorization and callback mechanics belong to the selected
ChannelAdapter/package and are intentionally not standardized here. After authenticating
an external account and identity, the adapter reports normalized Connection and
ChannelBinding metadata to their owning services. LiteCowork stores references, action
permissions, assurance, and status only; credential bytes remain in the provider-owned
secret mechanism. The owner reviews allowed actions through the Operator API. Disconnect
or revocation blocks future use without erasing history or deleting the external account.

Each active ChannelBinding also has one `ChannelHostAssignment`, owned by RuntimeMesh.
It identifies the Runtime that owns channel polling/webhook processing and outbound
delivery, with a monotonically increasing host epoch and bounded host lease. New bindings
default to the Workspace Hub when it advertises the required provider; local-only channel
adapters may be pinned to a selected Runtime. Provider credentials must be available to
that host under the existing SecretRef placement policy. Assignment is distinct from
binding identity and permission: moving a channel does not grant new actions or copy
credentials. Inbound claims and outbound sends require the assigned Runtime's current,
unexpired lease.

The adapter's opaque ingress cursor is stored only in an encrypted
`ChannelIngressCursorBinding` on the assigned Runtime. The service commits a provider
event receipt and receives the Hub replication acknowledgement before advancing the cursor
or acknowledging deferred provider ingress. Stable provider event IDs are deduplicated by
ChannelBinding; the same ID with a different payload digest is a conflict. Automatic host
reassignment requires either provider-supported replay from the last Hub-replicated receipt
within a declared replay window or provider-owned opaque cursor transfer. An adapter without
either guarantee is pinned to its Runtime for automatic continuity. An owner can still
request an explicit move, but must confirm `accept_ingress_gap`, which is recorded on the
new ChannelHostAssignment and shown in history. If the Hub replication barrier is
unavailable, the Runtime stops advancing/acknowledging ingress and applies backpressure.

When a host epoch changes, the target resumes from the replicated receipt with the largest
`(origin_host_epoch, ingress_sequence)` pair. It imports/validates the provider cursor or
replays from that receipt's provider event ID before polling normally. Opaque cursor bytes
never move between Runtime stores. A process restart changes the Runtime incarnation and
marks its cursor binding `RECONCILIATION_REQUIRED` until validated; stale cursor state never
authorizes skipping messages.

## Identity mapping

Persisted channel identity uses the canonical `ChannelBinding` from `DATA-MODEL.md`:

```text
ChannelBinding {
  channel_binding_id: ChannelBindingId
  workspace_id: WorkspaceId
  connection_id: ConnectionId?
  provider_ref: string
  external_account_ref: string
  identity_ref: PrincipalRef
  assurance_level: AssuranceLevel
  allowed_actions: ChannelAction[]
  status: ACTIVE | REVOKED | DEGRADED
  created_at: Timestamp
  updated_at: Timestamp
  provenance: ProvenanceRecord
  verification_refs: EvidenceId[]
  version: u64
}
```

`InboundChannelEvent` below is a provider transport DTO, not another persisted channel
identity schema.

## Inbound message

```text
InboundChannelEvent {
  provider_event_id
  channel_binding_id
  thread_ref: ChannelThreadRef
  sender_external_id
  timestamp
  text?
  attachments[]
  reply_to?
  edit_of?
}
```

`reply_to` is the provider's opaque message reference, scoped to the authenticated
channel binding. It is used only to correlate a reply to a specific outbound prompt; it
is never interpreted as authority by itself. For an acknowledged prompt that can accept
a structured response, ChannelService keeps a durable Runtime-local
`ChannelReplyTarget(channel_binding_id, provider_message_ref) -> user_request_id` mapping.
The target is created only when the delivered notification is for exactly one pending
`FORM` UserRequest, the adapter confirms replies are supported, and the UserRequest's
response policy permits channel response. The correlation mapping survives daemon restart
on its owning Runtime. Each target pins the current host epoch. Reassignment fences the old
Runtime immediately, so its targets stop authorizing replies even if that Runtime is
offline; it closes them when reachable. The new Runtime does not inherit opaque message
references. Replies to old prompts fall back to the Operator inbox unless a new prompt is
explicitly delivered and acknowledged there.

`provider_event_id` is deduplicated by the composite key (channel_binding_id, provider_event_id). The receipt preserves immutable origin Runtime/host epoch separately from the current claim Runtime/host epoch. A claim can be accepted only while that Runtime holds the current unexpired host lease. Its receipt moves through RECEIVED, PROCESSING, and one terminal ACCEPTED, REJECTED, or FAILED state. Every PROCESSING claim increments `claim_epoch`; an identical provider redelivery may be reclaimed by the new assigned Runtime after the old claim expires, while a changed payload digest conflicts. A completion with a stale claim or host epoch is rejected, preventing a late worker from overwriting the current receipt.

Attachments become ResourceRefs/Artifacts before Task consumption.
Inbound content is untrusted. Sender identity comes from the authenticated provider
account and configured ChannelBinding, never from message text or display name. Unknown
senders cannot select a Workspace by supplying an ID. Edits/reactions/deletes are
separate provider events and do not rewrite already-persisted Conversation history;
LiteCowork may append a correction/tombstone projection under user policy.

## Conversation mapping

A Channel thread may map to an existing Conversation. A message creates a Task only when the same Conversation-to-Task materialization rules would create one on desktop.

No channel-specific Task/session store.

Thread-to-Conversation mapping is unique per `(channel_binding_id, provider_thread_id)`
unless an operator explicitly merges threads. Inbound provider event IDs are unique per
binding and persisted with message materialization so redelivery cannot create another
message or Task. Attachments are size/type checked and stored as bounded Resources before
agents can access them.

## Replying to a specific pending UserRequest

`ChannelAction.RESPOND` is separate from `STEER` and both approval actions. It permits
responding only to the one pending UserRequest identified by a provider-authenticated
reply to a previously delivered prompt on that same ChannelBinding. Plain inbound text,
even in the correct Conversation thread, never answers “the latest” pending request.
The channel reply must resolve exactly one active `ChannelReplyTarget`; no match, multiple
matches, stale target, mismatched sender identity, revoked binding, or missing `RESPOND`
permission is rejected without creating a UserRequestResponse.

Channel response is supported only for non-sensitive `FORM` requests whose pinned schema
and size limits are accepted by the channel-response policy: an open response must be a
bounded root string, or a single choice must match one unique choice ID/label exactly and
map to that choice's pinned value. Nested/multi-field schemas, attachments, and implicit
free-form JSON are not accepted over channel replies. `EXTERNAL_URL` sign-in, Approval
decisions, credentials, and unsupported structured forms require the Operator surface.
The normal response-schema validation and bounded suspicious-secret checks still run. A
successful response commits the immutable UserRequestResponse and
ChannelEventReceipt together, records the `channel_binding_id` and `provider_event_id` as
response provenance, consumes that reply target, and closes sibling targets for the same
request. The Runtime-local target retains the consuming provider event ID for audit;
Invalid responses do not consume the target, so the user may correct a reply.
The response does not create a ConversationMessage, approve an Effect, create a grant, or
authorize an external action. UserRequests and Approvals remain distinct.

If the adapter cannot prove an exact reply-to reference, it may deliver a notification
with an Operator deep link but cannot make it reply-targetable. Channel actions are
rechecked at response time; changing assurance or revoking the binding closes targets.

## Outbound delivery

Delivery receipts are Evidence at REPORTED/OBSERVED level depending on provider semantics. If actual delivery/read status matters, a verifier or provider reconciliation is required.
Outbound sends use a stable delivery/request identity and are tracked as Effects/Evidence, not ChannelEventReceipt rows. If the provider times out after
accepting a message, the delivery is ambiguous and must be reconciled before retry to
avoid duplicate user-visible replies. A delivery receipt does not complete its linked
Task.
Each delivery attempt pins the Runtime and host epoch that performed it. A later
reassignment cannot turn an old Runtime's acknowledged provider message reference into a
target on the new Runtime; a reply to that message falls back to the Operator inbox.

## Sensitive approval

If channel assurance is below required level, the channel may present an approval notice with a deep link, but resolution must occur on a stronger surface.

## Capabilities and limits

Each ChannelAdapter declares whether it supports threads, attachments, edits, delivery
receipts, message limits, and idempotent send. Unsupported operations fail explicitly;
adapters do not emulate them with synthetic messages. Rate limits and retry-after values
are enforced per external account. Channel disconnect/revocation blocks new inbound
commands while preserving existing Conversation, Task, Artifact, and Effect history.

Channel actions are independently authorized (`view`, `steer`, `respond to an explicitly
replied-to UserRequest`, `safe approval`, `sensitive approval`). A sender's permission to
message a bot does not grant any of these. Sensitive Approval resolution requires an
assurance level at least equal to the Approval's `required_assurance` and is auditable.
