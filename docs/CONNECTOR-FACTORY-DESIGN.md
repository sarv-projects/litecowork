# LiteCowork — Connector Factory and Provider Import Design

**Status:** source-derived design; generated candidates are not production-ready operations.  
**Inputs:** the supplied 29 October 2026 Activepieces/Nango deep dive, Revision 9 §8B and Appendix AB, and the existing CONNECTORS-WORKFLOWS.md owner contract.

## 1. Decision

Use Activepieces and Nango for different jobs. Do not embed either application or its runtime in LiteCowork.

- **Activepieces:** reference concrete provider operation implementations, input/output fields, authentication variants, action/trigger behavior, polling/webhook edge cases and provider-specific business semantics.
- **Nango:** import declarative provider setup metadata: auth mode, authorization/token endpoints, scopes, connection fields, API base URLs, proxy templates, verification hints and pagination/retry metadata.
- **OpenAPI / official docs:** enumerate candidate endpoints, parameters and payload schemas. They do not establish real auth semantics, business rules, idempotency, consequential effects or trigger guarantees.
- **LiteCowork:** owns canonical Connection, SecretStore/SecretLease, grants, invocations, Effects, Evidence, trigger observations, AutomationOccurrences and recovery.

The reusable product is a **small importer, authoring SDK and qualification workflow**, not a second provider platform.

## 2. Source roles and non-goals

### Activepieces extraction

The source audit identified provider packages under packages/pieces/community and framework contracts for app connections, actions, triggers, OAuth2 properties and polling. The useful extraction unit is one provider action/trigger at a time.

For each selected action, adapt:
- stable action ID, title and description;
- typed input fields and validation;
- provider request mapping;
- output schema and bounded projection;
- auth recipe and scope requirements;
- provider-specific pagination, retry and trigger behavior;
- tests for the business mapping and edge cases.

Do not port the Activepieces piece framework or the whole runtime. Replace piece execution context/token access with LiteCowork's opaque ConnectionRef and brokered HTTP client.

**Security warning from the inspected source:** Activepieces FetchHttpClient sets NODE_TLS_REJECT_UNAUTHORIZED=0 globally. Do not port this client wholesale. Any reusable request logic requires independent TLS, SSRF, redirect, timeout, response-size and redaction review. Do not retain raw credential-bearing request/response bodies in errors or logs.

### Nango metadata extraction

The inspected provider catalogue and scope catalogue are useful for provider connection metadata:
- auth modes and authorization/token endpoints;
- default scopes and credential/configuration fields;
- API base URL templates;
- proxy headers/query/body templates;
- connection verification hints;
- pagination, retry and webhook metadata.

Nango YAML is not an implementation of every provider action. If the importer sees only provider metadata and no operation source, it must produce an auth/connection candidate with **zero invokable actions**.

Do not embed Nango's database, OAuth server, proxy, jobs, sync checkpoints, webhook runtime or deployment stack. That would create a second connection/execution authority beside LiteCowork.

### OpenAPI and provider documentation

OpenAPI may generate candidate path/method/parameter/request/response schemas. Official provider docs and live test-account fixtures determine final scopes, request quirks, error handling and behavior. OpenAPI alone cannot determine whether a POST is safe to retry or whether a provider action is consequential.

## 3. Pipeline

1. **Import** pinned Nango provider metadata, provider OpenAPI and selected Activepieces action examples.
2. **Normalize** into a provisional LiteCowork ConnectorPackage/operation manifest and candidate JSON Schemas.
3. **Review** auth recipe, required scopes, origin policy, effect class, idempotency, retries, pagination, rate limits, sensitive fields and data retention.
4. **Implement** a small idiomatic LiteCowork operation handler using the brokered client.
5. **Test** with fixtures and an owner-approved provider test account; include negative and timeout-after-effect behavior.
6. **Qualify** the exact provider API version, auth mode, OS, Runtime, LiteCowork and LiteSPM versions.
7. **Publish/enable** only after owner review and passing CODE/SYSTEM/USER evidence. Generated candidates stay non-invokable until this promotion.

No hidden direct network fallback and no operation becomes available merely because an importer generated a schema.

## 4. Operation contract

The operation manifest must carry at least:

- stable provider and operation IDs, version, display title and user-facing description;
- HTTP method and relative path; never an arbitrary absolute model-supplied URL;
- path/query/header/body input schemas and bounded output schema;
- auth recipe reference and exact required scopes;
- allowed provider origin and redirect policy;
- effect classification, idempotency classification, retry policy and reconciliation strategy;
- pagination/cursor semantics, rate limit/deadline/response-byte limits;
- sensitive input/output annotations and redaction policy;
- provider API version, source/provenance and qualification evidence.

The operation handler receives workspace ID, opaque Connection ID, invocation ID, idempotency key, deadline and a brokered HTTP client. It never receives token bytes.

Core resolves connection, lease, grants, destination, allowlisted origin, rate limits, timeouts and effect policy out of band. It independently classifies consequential operations; operation metadata is a candidate, not authority.

## 5. Safety rules

- No token or secret bytes in the model-visible input, agent environment, request DTO, logs, traces or operation errors.
- No arbitrary absolute URL; requests are constrained to a qualified provider origin and relative path.
- SSRF and redirect protections apply to every hop; reject embedded URL credentials and unsafe schemes.
- TLS verification remains on. No process-wide TLS override.
- Enforce per-operation deadlines, request/response byte limits, content types, pagination caps and redaction.
- Read operations may settle without an Effect where the owner contract allows it.
- Consequential writes require a Core-owned Effect intent, correct approval/grant, idempotency classification, durable receipt and reconciliation.
- Timeout after dispatch is ambiguous. Do not blindly resend sends, purchases, deletes, or other non-idempotent mutations.
- Webhook/poll triggers require stable dedupe identity, cursor semantics and durable TriggerObservation acceptance before cursor advancement.
- Imported scopes and operation schemas are re-reviewed when provider/package version changes. Schema or permission drift invalidates affected grants.

## 6. Recommended first slices

| Slice | Scope | Exit |
|---|---|---|
| 0 — Importer and SDK | Nango metadata parser, OpenAPI candidate generator, operation manifest, brokered client and schema tests | Deterministic import; zero invokable ops without review |
| 1 — GitHub read | Repository, issue/PR search and bounded read | Real connection and grounded read through Core |
| 2 — GitHub write | One issue/PR comment or issue creation | Exact approval, idempotency and receipt/reconcile |
| 3 — Gmail read/draft | Search/read and draft creation before send | Scopes minimized; draft is distinguishable from send |
| 4 — Gmail send / Calendar write | Approval-gated consequential operation | Timeout-after-effect and no blind retry |
| 5 — Reusable triggers | Poll and webhook templates with cursor/dedupe | Duplicate, reorder, replay, expiry and restart tests |
| 6 — Repeatability | Generate a second provider with the same toolchain | No Core changes; exact operation qualification per provider |

The first golden connector may be GitHub for developer workflows. Google Workspace or Microsoft 365 follows based on target-user needs. Breadth follows successful, qualified slices rather than catalogue counts.

## 7. LiteSPM boundary

Revision 9 makes LiteSPM the integrated package-system authority. Connector operations may be distributed through a supported LiteSPM package kind, but LiteCowork must not invent a second package manager or package catalog.

The supplied LiteSPM source deep dive records that the inspected upstream snapshot deprecated the connector package kind for V2 and emphasized MCP, Skill and Plugin listings with different installability thresholds. This is a dated source observation; re-check current LiteSPM HEAD and schemas before publishing any package-kind promise. A connector-like ability may have to be shipped as a qualified MCP-backed capability, not a fictional connector listing.

## 8. Acceptance suite

- Nango metadata with no operation source generates no invokable actions.
- OpenAPI import creates candidates, not production grants or effect classifications.
- Imported provider scope set is exact and visible; no silent scope widening.
- Credentials never appear in tool inputs, model context, logs or errors.
- Disallowed origins, redirects, embedded credentials, HTTP downgrade and SSRF targets are rejected.
- TLS certificate failure fails closed.
- Oversized body, deep pagination, deadline expiry and rate limit are bounded.
- Read operation returns exact source/provider/version and bounded output.
- Consequential write requires current grant and exact approval.
- Duplicate idempotency replay returns the original receipt; conflicting payload is rejected.
- Timeout after dispatch becomes UNKNOWN/AMBIGUOUS until provider reconciliation.
- Poll cursor advances only after all accepted observations are durable.
- Duplicate/reordered webhook deliveries do not create duplicate occurrences.
- Schema, auth recipe or permission drift forces re-review and invalidates affected grants.
- Provider disable/revoke fences future calls while preserving historical effects/evidence.

## 9. Source traceability

The supplied source deep dive pinned Activepieces at b7be720aae54de37e2a646c96a23a83ad459b22f and Nango at 870ffa2cca32550c18f59956bb1f8241b3b2fb5a. It inspected their recursive Git trees and fetched the named provider/framework/catalog/auth/trigger files through GitHub APIs; it did not locally clone/build/test either upstream repository. Treat these snapshots as research provenance, not live dependency versions or qualification evidence.

The detailed owner remains docs/CONNECTORS-WORKFLOWS.md. Machine contracts remain under docs/schemas/. No connector generated output should become canonical until promoted through the existing schema/ADR process.
