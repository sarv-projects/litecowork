# Messaging Channels LLD

Channels are human-facing transports, not Task owners and not agent protocols.

## ChannelAdapter

```text
interface ChannelAdapter {
  connect(ChannelConfig) -> ChannelBinding
  receive(Cursor?) -> Stream<InboundChannelEvent>
  send(OutboundChannelMessage) -> DeliveryReceipt
  capabilities() -> ChannelCapabilities
  disconnect() -> Ack
}
```

Providers may include Telegram, Slack, Discord, Teams, Email and Webhook.

## Provider setup and operator controls

Provider-specific account authorization and callback mechanics belong to the selected
ChannelAdapter/package and are intentionally not standardized here. After authenticating
an external account and identity, the adapter reports normalized Connection and
ChannelBinding metadata to their owning services. LiteCowork stores references, action
permissions, assurance, and status only; credential bytes remain in the provider-owned
secret mechanism. The owner reviews allowed actions through the Operator API. Disconnect
or revocation blocks future use without erasing history or deleting the external account.

## Identity mapping

```text
ChannelBinding {
  channel_binding_id
  workspace_id
  connection_id?
  provider
  external_account_id
  identity
  authentication_strength
  assurance_level
  allowed_actions: ChannelAction[]
  status
}
```

## Inbound message

```text
InboundChannelEvent {
  provider_event_id
  binding_id
  thread_ref
  sender_external_id
  timestamp
  text?
  attachments[]
  reply_to?
  edit_of?
}
```

`provider_event_id` is deduplicated by the composite key (channel_binding_id, provider_event_id). Its receipt moves through RECEIVED, PROCESSING, and one terminal ACCEPTED, REJECTED, or FAILED state. Every PROCESSING claim increments claim_epoch; a completion with a stale epoch is rejected after reclaim, preventing a late worker from overwriting the current receipt.

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

## Outbound delivery

Delivery receipts are Evidence at REPORTED/OBSERVED level depending on provider semantics. If actual delivery/read status matters, a verifier or provider reconciliation is required.
Outbound sends use a stable delivery/request identity and are tracked as Effects/Evidence, not ChannelEventReceipt rows. If the provider times out after
accepting a message, the delivery is ambiguous and must be reconciled before retry to
avoid duplicate user-visible replies. A delivery receipt does not complete its linked
Task.

## Sensitive approval

If channel assurance is below required level, the channel may present an approval notice with a deep link, but resolution must occur on a stronger surface.

## Capabilities and limits

Each ChannelAdapter declares whether it supports threads, attachments, edits, delivery
receipts, message limits, and idempotent send. Unsupported operations fail explicitly;
adapters do not emulate them with synthetic messages. Rate limits and retry-after values
are enforced per external account. Channel disconnect/revocation blocks new inbound
commands while preserving existing Conversation, Task, Artifact, and Effect history.

Channel actions are independently authorized (`view`, `steer`, `safe approval`,
`sensitive approval`). A sender's permission to message a bot does not grant any of
these. Sensitive Approval resolution requires an assurance level at least equal to the
Approval's `required_assurance` and is auditable.
