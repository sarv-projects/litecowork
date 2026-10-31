# Error and Recovery Contract - Feature 001

**Status:** DRAFT  
**Owners:** Conversation, Agent Runtime, Storage, Security/Trust, UI.  
**Canonical owner:** [Failure Recovery](../../../docs/FAILURE-RECOVERY.md), [State Machines](../../../docs/STATE-MACHINES.md), [API](../../../docs/API.md), [Storage](../../../docs/STORAGE.md).

## 1. Classification and client rule

| Class | Examples | Server action | Client action |
|---|---|---|---|
| Owner/scope denial | foreign principal, Workspace mismatch, archived Workspace | deny before mutation or abort transaction | preserve draft; do not retry unchanged permissions |
| Request conflict | stale If-Match/version, changed payload on same idempotency key | conflict; no partial commit | refresh; use new key only for changed intent |
| Pinned target unavailable | binding disabled before admission, endpoint offer expired, Runtime wrong incarnation | typed admission failure; never fallback | preserve user content; show setup/target unavailable |
| Session readiness failure | adapter handshake mismatch, host DEGRADED/FAILED, launch timeout | mark STARTING LOST or terminal failure after bounded reconcile; never RUNNING | report not sent / agent unavailable |
| Input preparation failure | resource revoked, missing local bytes, unsupported media, oversize | typed failure before dispatch; no silent omission | retain message/draft and offer explicit correction |
| Provider uncertain | request timeout after send, event gap, lost connection | reconcile exact session/request before another dispatch | show recovering/unknown, not success |
| Provider terminal failure | auth revoked, rate limit, crash, explicit error | normalize safe code; settle FAILED after quiescence | retry only through explicit owner action |
| User cancellation | interrupt accepted but stop not observed | remain CANCEL_REQUESTED until quiescence/result | show stopping; don't claim cancelled |
| Persistence uncertainty | DB timeout/rollback after provider final | query stable result identity and durable state; retry only local idempotent commit | show saving/reconciling; never resend prompt |
| Projection gap | stale cursor, duplicate or missed transient sequence | durable snapshot resync | discard transient buffer; render persisted messages once |

## 2. Retry classes

- **SAFE_LOCAL_REPLAY:** same principal/key/payload receipt lookup for an already committed command.
- **SAFE_AFTER_RECONCILE:** adapter start/input/stop outcome is uncertain; query exact native request/session and Runtime incarnation first.
- **EXPLICIT_USER_RETRY:** turn is FAILED and owner chooses retry using expected version and a new idempotency identity; new AgentSession and retry ordinal.
- **NEVER_AUTOMATIC:** provider/model fallback, resend an ambiguous prompt, replay native hidden history, restore a handle to a new Runtime incarnation, or invent a final answer from partial events.

No retry sleeps and tries again without knowing whether an external side effect already occurred.

## 3. Required failure and crash matrix

| Boundary | Failure window | Durable truth | Recovery |
|---|---|---|---|
| Before submit transaction | validation/auth fails | no USER message/turn/receipt | preserve draft; correct request |
| Submit transaction | conflict/DB rollback | no partial commit | exact key can be retried if no receipt; changed payload requires new key |
| After USER+OPEN commit, before reservation | daemon exits | turn exists OPEN | startup scanner finds turn/dispatch intent; if no safe intent exists, settle typed unavailable after bounded reconciliation |
| After STARTING reservation, before process launch | daemon exits | STARTING + receipt | list STARTING; prove no live child or same-incarnation child; settle LOST if no safe continuation |
| After process launch, before handshake | provider process exists, no readiness | STARTING | reconcile exact process/session; if ambiguous fail LOST and safely stop owned process |
| After handshake, before activation commit | readiness observed, DB commit uncertain | STARTING unless activation receipt committed | query activation/session/turn/host binding; never send prompt until activation is confirmed |
| After activation, before prompt dispatch | ACTIVE/RUNNING | active session, no provider input proof | reconcile native session; dispatch only if no prior input receipt |
| During prompt dispatch | timeout after write | unknown whether provider accepted | query stable native request/session/cursor; never blind resend |
| During event read | duplicate/out-of-order/gap/oversize | canonical transcript unchanged | reject frame; replay if adapter supports exact cursor, otherwise discard deltas/resync |
| After partial delta | adapter crash | no durable AGENT message | discard transient partial; settle FAILED after quiescence; preserve USER message |
| After final event, before DB commit | provider finished, result not saved | turn still nonterminal until receipt known | retry local result commit by stable identity after querying; do not re-prompt |
| During final result transaction | crash/rollback | either all message+turn+events+receipt commit or none | read transaction result; atomicity makes retry safe |
| During interrupt | request accepted, stop uncertain | CANCEL_REQUESTED | wait/query stop; completion/failure that wins race remains outcome |
| After terminal commit, before UI receives | client loses response | durable receipt/message/terminal event | replay receipt; reload projection by cursor |
| During daemon restart | different Runtime incarnation | old local native handle not usable | mark LOST / closed under recovery; create fresh session only on explicit retry/continuation |

## 4. Mandatory behavior

1. STARTING never means running, queued, delivered or ready.
2. Host READY and Runtime READY are not adapter session readiness.
3. All native calls and process operations occur outside SQLite transactions.
4. A failed admission after USER commit retains the USER message and settles typed failure; it never silently deletes input.
5. Only atomic activation moves ConversationTurn OPEN -> RUNNING.
6. Only atomic response commit makes final semantic message and turn terminal state durable.
7. Session LOST does not automatically mutate the turn; ConversationService/Coordinator owns exact turn settlement.
8. A session handle is local to exact Runtime incarnation; no restore on a new incarnation.
9. Stop is a request. Cancellation is committed only after observed quiescence, unless completion/failure wins first.
10. User retry is explicit and uses fresh session/retry ordinal; old AGENT messages and provenance remain immutable.
11. A valid authenticated response to a Conversation-scoped UserRequest atomically stores its immutable UserRequestResponse and moves only the exact WAITING_USER turn to WAITING_DEPENDENCY. The old session is closed; no new AgentSession is admitted until the exact provider input is accepted and a fresh session becomes ready. For MCP INPUT_REQUIRED, tasks/update acknowledgement alone is not acceptance: poll the same provider task for the exact request key. If the provider remains INPUT_REQUIRED after rejection, close the old request and create a new UserRequest for the current key, returning the turn to WAITING_USER. Ambiguous acceptance remains WAITING_DEPENDENCY and blocks a new session. See [STATE-MACHINES.md](../../../docs/STATE-MACHINES.md), [API.md](../../../docs/API.md), [STORAGE.md](../../../docs/STORAGE.md) and [TRUST.md](../../../docs/TRUST.md).
12. Native events are untrusted, bounded and correlated. A tool call/result from a provider does not prove Core-authorized Effect.

## 5. Safe error envelope

The route uses existing API error code taxonomy and status mapping. Exact error codes must be reused from API.md/SCHEMAS.md and not coined here. Never expose command lines, local paths, provider cookies, auth details, session IDs intended to be private or raw exception text. Response must indicate whether the USER message was committed and whether the outcome is known; if current response schema cannot express this, resolve with API owner before UI behavior changes.

## 6. Recovery sweeps

- Enumerate STARTING sessions in bounded pages ordered by binary AgentSessionId with exclusive cursor; repeat sweep from start to catch rows sorting before prior cursor.
- Reconcile each session against exact Runtime ID/incarnation and HostInstance.
- Remove stale local handles only after adapter/process state is reconciled and session is CLOSED/LOST; no blind kill of external process.
- Bound retries, page size and total work per daemon tick. Persist recovery result before releasing a concurrency slot.
- Recovery is idempotent and safe under repeated daemon restart. No provider prompt is redispatched by a recovery sweep.
- Reconnect UI from durable projection cursor. Transient stream state is disposable.

