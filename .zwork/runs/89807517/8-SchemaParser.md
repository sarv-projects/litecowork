# Schema Parser — run 89807517 participant 8

**Bucket schemas-01** — 8 files read in full + 2 partial / 37 listed; ~3,650 of 33,506 lines. Partial: deadline cut tool calls mid-enumeration.

Top findings:
1. `operator-api.openapi.yaml` line 1740 is invalid YAML: path key `/runtimes/pair-token=[redacted] lost its `:`, swallowing `/runtimes/{runtimeId}/revoke` — a literal `[redacted]` sits in a normative path.
2. `domain-event.schema.json` has 148 event types (exactly enumerated), 15 required envelope fields; `entity_type` is free string, not an enum — event↔entity binding is unenforced.
3. Only 5 event types carry `.v2` siblings, all alongside live v1 branches; no upgrader map or deprecation marker.
4. `rich-presentation` charts can only bind `CAPABILITY_RESULT`/`PINNED_RESOURCE`/`SEMANTIC_TABLE` — no inline data.
5. `operator-stream` `rich.draft` loosens `BLOCK_REPLACED.block` to a 32-property object, contradicting its own `$ref` to the rich-presentation `block`.

**Unanswered:** all 28 `sqlite-v*.sql` migrations (31,942 lines) and OpenAPI components/security/ETag/error-envelope sections.

Digest: [reading/digest-schemas-01.md](reading/digest-schemas-01.md)