# Rich Response Contract

## Purpose and non-goals

This contract defines optional structured presentation for human-facing Conversation
responses. It does not define Task planning, capability execution, authorization, Artifact
creation, or verification. Those remain owned by their existing domain services.

The product has two compatible outputs:

```text
ConversationMessage = semantic, durable, portable response
RichPresentation    = optional immutable rendering enhancement
```

The semantic message is complete on its own. No factual claim, warning, citation,
deliverable, action, or system state may exist only in RichPresentation. Rich composition
can be omitted or arrive later without blocking the answer.

## User preference and policy

The shell also has a separate device-local **Show formatted answers** display preference
owned by Feature 011. It controls whether the desktop reads/renders the optional
RichPresentation projection; when disabled, the full semantic message remains visible and
new optional projection reads are suppressed. It defaults on for older local preference
snapshots. This setting is not the turn's `PresentationPreference`: it is not sent to an
agent, does not change a retry, and does not modify semantic message or domain state. If
local storage cannot save it, Settings reports that the choice is session-only. See the
Feature 011 local-preference contract and Feature 016-T09.

```text
PresentationPreference = AUTO | SIMPLE | RICH
PresentationPolicyDecision = PLAIN | RICH_PREFERRED
```

The preference is captured immutably on the ConversationTurn and reused on retry. A
separate explicit retry option may be added later; silently changing preference during
retry is forbidden.

- `SIMPLE` suppresses model-composed decoration. It does not hide functional host UI such
  as real Artifact links, an actual UserRequest, or committed Task status.
- `RICH` asks the compiler to compose a structured enhancement when supported. It never
  makes rich rendering necessary to understand or complete the response.
- `AUTO` uses a versioned deterministic policy. The policy considers explicit visual,
  comparison, file, media, diagram, and chart requests plus known Artifact, Resource,
  citation, structured-result, and quantitative-result counts. Its exact scoring is an
  implementation detail, not durable domain truth.

Evaluation occurs before session creation (whether to preload optional guidance) and after
semantic output (whether host-side deterministic enhancement is useful). Neither pass
requires a separate model call.

## Presentation intent

An Agent may propose layout intent through the first-party, session-bound operation
`litecowork.presentation.propose`. The operation is available only for an active
Conversation turn, is bounded and ephemeral, creates no CapabilityInvocation, Effect,
Artifact, Grant, or DomainEvent, and is not available to Task planning or worker Attempts
by default.

Implementation status (source rechecked 2026-11-24): `litecowork.presentation.propose` has no
implementation (no route, adapter or client call), and no production caller publishes a
RichPresentation document. Read projections can return a stored presentation if one exists;
the storage publication validator is exercised by tests, not a mounted write path. Renderer-v2
METRIC, CHART and SAFE_CALCULATOR have desktop CODE/UI components. SAFE_CALCULATOR is a
bounded, labelled, view-local calculator with inert formula text, deterministic local
recomputation/reset and an always-visible semantic fallback. The absent publisher/provider
ingress still keeps all renderer-v2 blocks out of provider answers.

```text
RichPresentationIntent {
  schema_version: 1
  strategy: AUTO | PROSE | REPORT | COMPARISON | EXPLAINER |
             DASHBOARD | TIMELINE | DELIVERABLES | GALLERY
  density: COMPACT | NORMAL | DETAILED
  elements: RichElementHint[]
  submitted_at_sequence: u32
}
```

Hints identify structures or references, not final host components:

```text
RichElementHint =
    COMPARISON
  | TABLE
  | CHART_FROM_RESULT { invocation_id, result_binding }
  | DIAGRAM { source, accessible_summary }
  | MEDIA_GROUP { resource_refs[] }
  | DELIVERABLE_GROUP { artifact_refs[] }
  | CHECKLIST { items[] }
  | TIMELINE
```

The compiler verifies every supplied invocation/resource/artifact against the current
Workspace, turn, source revision, and read authorization. An Agent cannot submit
host-owned status, verification, approval, download URLs, local paths, Runtime readiness,
or provenance origin. An invalid proposal is dropped while the semantic message remains.

## Canonical document and binding

The trusted Core publisher allocates the `presentation_id` and publication EventId.
Agent presentation intent and external clients cannot choose those identifiers. The
internal storage port reports collisions as an opaque identity-unavailable result; it
does not disclose the owning Workspace or an existing aggregate revision. Publication
requires the current owner and an ACTIVE Workspace, checked before source lookup.
Previously published enhancements remain owner-readable after Workspace archive.

`RichPresentationDocument` is stored as bounded RFC 8785 JCS JSON with a SHA-256 digest.
It has a closed versioned block schema, `presentation_id`, `message_id`, semantic content
digest, root blocks, citation refs, navigation action refs, and optional accessibility
summary. It embeds neither binary files nor arbitrary HTML, JavaScript, CSS, URLs for
media fetches, nor React component code.

`semantic_content_digest` is SHA-256 over RFC 8785 JCS bytes of the committed Message's
immutable `{content, resource_refs}` value. It therefore binds semantic text and resource
attachments/revisions. For text binding, the host forms `SemanticTextProjection` by
concatenating `TEXT` blocks in message order with one LF between adjacent text blocks and
no added leading/trailing newline. UTF-8 byte offsets are zero-based, half-open offsets
into those exact canonical bytes. A slice stores the digest of the selected bytes. A
renderer uses the source slice; the model does not duplicate the same prose in presentation
JSON. The document digest is separately SHA-256 over canonical RichPresentationDocument
bytes.

```text
MessageTextSliceRef {
  start_utf8_byte: u32
  end_utf8_byte_exclusive: u32
  slice_digest: Sha256Digest
}
```

Every final block receives host-assigned provenance:

```text
BlockOrigin = SEMANTIC_MESSAGE | MODEL_INTENT | TOOL_RESULT_TEMPLATE |
              ARTIFACT_BINDING | RESOURCE_BINDING | CORE_PROJECTION | MCP_APP

BlockProvenance {
  origin: BlockOrigin
  agent_session_id?: AgentSessionId
  invocation_id?: CapabilityInvocationId
  resource_refs: PinnedResourceRef[]
  artifact_refs: ArtifactVersionRef[]
  evidence_refs: EvidenceId[]
  verification_refs: VerificationRunId[]
}
```

The host assigns `origin`. Model input cannot set `CORE_PROJECTION`, `ARTIFACT_BINDING`,
or `MCP_APP` provenance.

The canonical document carries a host-generated `block_provenance[]`, keyed by a unique
RFC 6901 JSON Pointer path into `root_blocks` (for example `/0/children/1`). Every rendered
block has exactly one provenance entry; entries for missing blocks, duplicate paths, or
unresolved source references fail validation. The compiler creates this list after binding
and authorization. Intent input cannot set it. `CORE_PROJECTION`, `ARTIFACT_BINDING`, and
`MCP_APP` origins can only be assigned by their corresponding trusted host binder.

## Block families

The renderer schema separates model-safe blocks from host-bound blocks.

```text
ModelSafeBlock =
  TEXT_SLICE | LAYOUT | CARD | CALLOUT | TABLE | CHART | DIAGRAM |
  MEDIA | CHECKLIST | DELIVERABLE_GROUP | ARTIFACT_COLLECTION

HostBoundBlock =
  HOST_PROJECTION | MCP_APP
```

The v1 wire schema has exactly 13 `kind` variants. `CODE`, `GALLERY`, and `TIMELINE`
are not RichPresentation v1 wire kinds. A gallery- or timeline-like arrangement may be
composed from registered blocks only where their source and semantics satisfy the v1
schema; it must not be emitted as an invented variant. Adding a new block kind requires a
schema/renderer-contract revision and owner review.

### Block schema and renderer status

`docs/schemas/rich-presentation.schema.json` is the canonical versioned document vocabulary.
The table separates profile declaration from current desktop support and Rust CODE; it does not
claim a component exists merely because the schema names a kind or a storage validator accepts it.

| Block kind | v1 schema | Current desktop and Rust validator |
|---|---|---|
| `TEXT_SLICE` | Declared | Supported |
| `LAYOUT` | Declared | Supported |
| `CARD` | Declared | Supported |
| `CALLOUT` | Declared | Supported |
| `TABLE` | Declared | Supported |
| `METRIC` | Renderer-v2 CODE/UI | Rust/TypeScript contract validator and desktop component; exact source cell with visible grounded unit/scale/interval/aggregation context; unchanged value; provider structured-intent ingress not wired |
| `CHART` | Renderer-v2 CODE/UI | Rust/TypeScript contract validator and desktop static SVG/table component; provider structured-intent ingress not wired |
| `SAFE_CALCULATOR` | Renderer-v2 CODE/UI | Rust/schema/TypeScript reducer and desktop calculator; bounded local inputs, no host calls or persistence; provider structured-intent ingress not wired |
| `DIAGRAM` | Declared | Not supported by the current validators/renderer |
| `MEDIA` | Declared | Not supported by the current validators/renderer |
| `CHECKLIST` | Declared | Not supported by the current validators/renderer |
| `DELIVERABLE_GROUP` | Declared | Not supported by the current validators/renderer |
| `ARTIFACT_COLLECTION` | Declared | Not supported by the current validators/renderer |
| `HOST_PROJECTION` | Declared; requires a trusted host binder | Not supported by the current validators/renderer |
| `MCP_APP` | Declared; requires the separate qualified MCP Apps host boundary | Not supported by the current validators/renderer |

The detailed behavior below for CHART, DIAGRAM, MEDIA, deliverable, checklist, citation and
action families is target contract material unless the table marks the kind as supported.
Schema declaration alone does not make those features available in the current desktop.

The pure `domain-presentation` Rust validator, called by SQLite on publish/read, accepts the
five renderer-v1 display-only kinds and the user-directed renderer-v2 METRIC, CHART and
SAFE_CALCULATOR profile under a 512 KiB document limit and maximum block nesting depth of 8.
Both owner validators reject non-finite numbers and integer-valued numbers outside the exact
binary64 safe-integer range, regardless of whether JSON uses integer or exponent notation;
the JSON Schema's generic `number` type cannot enforce that semantic restriction on its own.
The desktop TypeScript parser/React renderer can render v2 METRIC and CHART but cannot make
them provider-visible: the production binder emits TABLE/TEXT_SLICE only and structured provider intent is not wired. Renderer-v2 METRIC
binds one finite numeric cell to an exact committed semantic Markdown table; the host derives
its label from the column header and row label and does not change units or derive arithmetic.
METRIC also requires a closed unit/scale/time-interval/aggregation context grounded with the
source header and row label in an exact committed semantic line. The renderer displays those
declarations beside the unchanged source value and does no conversion or aggregation. This
integrity check prevents the presentation from adding context absent from the answer; it does
not independently verify provider-authored claims or implement derived analytics in T20/T24.
Renderer-v2 CHART binds to exactly one committed semantic Markdown table slice. It names a
category column and 1–6 unique numeric series columns by zero-based index; the host derives
series labels from exact header cells and keeps source row order. It accepts BAR, LINE and
AREA only, with 2–60 rows, non-empty string categories and finite JSON numbers. Malformed
rows, duplicate/out-of-range columns or unsupported kinds fall back to the same table. The
renderer uses host SVG primitives, never model markup, and its accessible table is the
complete data equivalent. SAFE_CALCULATOR uses exact numeric table-cell bindings and the
bounded CalcAst; Rust evaluates the AST at original input values and requires an exact
semantic numeric result cell to match within `1e-9 * max(1, |calculated|, |expected|)`.
Output formatting never changes calculation semantics, and failure shows semantic fallback
text. T03/T04 provide Rust/TypeScript contract-parity CODE; T05 provides the pure reducer;
T06 provides METRIC/CHART components and T07 now provides the local SAFE_CALCULATOR renderer.
Calculator fields are bounded and transient, use the pure reducer, expose a polite result and
keyboard reset, and keep fallback text visible. Contract completion of the broader derived-metric cases owned by T20/T24 and a production
publisher/provider ingress remain prerequisites before renderer-v2 reaches provider answers.
Schema declaration does not replace owner-side source binding, provenance checks, or runtime
qualification. A document containing a schema-declared kind that the current validators do
not support is rejected as a complete RichPresentation and the client renders the complete
semantic message. The per-block Markdown fallback in T18 applies to semantic text only; it
does not yet provide per-block RichPresentation fallback. Introduce such behavior only with
an explicit contract/version and compatible Rust, TypeScript, schema, stream and acceptance
updates.

Agents may request a host-bound projection by kind only where the host API permits it;
the compiler resolves the actual projection from authenticated Core state. Agents cannot
emit a trusted block payload or set its status. Generated approval controls and arbitrary
mutating actions are never supported.

### Citations

```text
CitationRef {
  citation_id: CitationId
  resource_ref: PinnedResourceRef
  locator: SourceLocator
  observed_digest?: Sha256Digest
  extraction_generation?: string
  source_label?: string
}
```

`SourceLocator` is one of `TEXT_SPAN`, `PAGE`, `PAGE_REGION`, `TABLE_RANGE`, or
`CODE_RANGE`. Page and line values are 1-based. Text offsets are zero-based UTF-8 byte
offsets within the exact extracted segment/version. Page-region x/y/width/height are
normalized to `[0,1]`. Every citation is resolved against an authorized pinned Resource
revision; a changed head never silently changes the cited revision. (Source audit 2026-11-12:
neither the storage publisher nor desktop renderer has an authorized citation resolver; a
document with any non-empty citation list is rejected in full and falls back to the semantic
message.) A citation is a source reference, not proof that the associated claim is true.

### Tables and charts

Tables and charts use bounded `StructuredDataBinding` values, not unbounded copied data.
Chart `binding` is required and selects exactly one source: a capability result with its
Invocation/result binding, a pinned Resource revision, or a semantic-message table slice.
The compiler resolves the source under Workspace authorization before publication; the
schema's reference alone is not authority. Chart data points and series count have
explicit schema limits. Numeric values must be finite JSON numbers; NaN and Infinity are
rejected. Charts include an accessible summary and textual/table fallback. Unsupported
result schemas use a bounded safe text or JSON fallback rather than guessing a renderer.

### Diagrams and media

Diagrams accept Mermaid source or a host-generated graph, enforce node/depth/byte limits,
and include a textual accessible summary. Script, HTML handlers, external includes, and
unsafe SVG are rejected or sanitized before rendering. Animation values are `NONE`,
`DRAW_ONCE`, or user-controlled `HIGHLIGHT_SEQUENCE`; no infinite activity-like animation
is permitted.

Images/video/animated images refer only to authorized, pinned Resource revisions. Direct
model-provided URLs are not fetched. Remote media must first be imported through an
authorized Resource/capability path. Autoplay is false; alt text is required.

Responsive layouts use generic `ROW`/`STACK` composition: row at wide desktop, stacked
below 900 CSS px, single-column below 600 CSS px. These are starting layout contracts and
must be validated at 200% zoom and narrow desktop window sizes.

### Deliverables and bundles

`DELIVERABLE_GROUP` and `ARTIFACT_COLLECTION` bind exact `ArtifactVersionRef`s. The host
renders titles, media-type icons, provenance, verification state, and permitted OPEN,
PREVIEW, DOWNLOAD, or OPEN_IN_WORKBENCH actions from real Artifact metadata. (Source audit
2026-11-12: both kinds are rejected by the storage validator and desktop parser, so none of
these cards or actions is reachable in this build.) Collection
lists are virtualized and paginated; a large Resource or CSV is never copied into the
presentation document.

Creating files is Task work. A Conversation AgentSession cannot publish Artifacts. A
request to create files or a ZIP materializes a Task and uses the ordinary Attempt,
Artifact, Effect, and Verification contracts. A deterministic `ArtifactBundleBuilder`
(target; no implementation exists in this repository) may create a ZIP from explicitly selected exact ArtifactVersions after authorization. It
publishes an ordinary immutable `BUNDLE_ARCHIVE` Artifact with exact input refs and a
canonical member manifest in provenance; it is not a new aggregate or transactional
multi-effect operation. Each consequential/member operation retains its ordinary
idempotency and reconciliation rules. ZIP paths are normalized and traversal, duplicate
normalized paths, symlinks, and decompression hazards are rejected. A later Artifact head
does not change an already-built ZIP.

### Checklist and actions

Checklist selections are `LOCAL_TRANSIENT` or `LOCAL_DURABLE` Operator UI state, never
Task Steps or Verification. V1 local durable state stays in the desktop's local UI store
and is not Workspace-replicated. (Source audit 2026-11-12: CHECKLIST is rejected by both
the storage validator and desktop parser, and no checklist store/component exists, so
neither selection kind is implemented.) Task progress is shown only through a host-bound trusted
Task projection. Safe navigation actions may open exact authorized Artifact/Resource,
Task, or Conversation identities; external navigation is HTTPS-only and passes the
Operator confirmation/egress rules. Mutating operations route only through their owning
service. There is no generic execute action. (Source audit 2026-11-12: the storage publisher
and desktop parser reject every non-empty `actions` list because no authorized action
resolver is available.)

## Deterministic presentation and cost

`ToolResultPresentationRegistry` (target; no implementation exists in this repository)
maps `(CapabilityRef, operation, result_schema_digest)` to a versioned, closed renderer
descriptor and bounded fallback (`JSON`, `TABLE`, or `TEXT`). A known tabular result becomes a table without another model call; committed
Artifacts become host-bound Artifact cards; approved tool results may use their registered
renderer. Registry renderers cannot alter source truth.

Response presentation uses one optional guidance preload and at most one bounded intent
proposal per response policy. There is no mandatory second planner, per-block model call,
or model-based verification. The lead reasons; Core applies deterministic policy and
eligibility; tools perform predictable operations; verifiers choose deterministic checks
before semantic model review; the host compiler lays out the answer.

## Search, export, and channels

Conversation search indexes semantic message content and authorized Resource/Artifact
metadata, never rendered HTML, React trees, MCP iframe DOM, local checklist state, or
ephemeral rich frames. Export starts from semantic content. Optional Markdown/PDF/print
views are derived outputs and cannot become message truth.

`PortableResponseRenderer` (target; no implementation exists in this repository)
flattens rich blocks to bounded text for channels: prose stays
prose; tables become bounded text/Markdown; cards become title/summary; charts/diagrams
include textual summaries; checklists use textual boxes; deliverables list authenticated
links or supported attachments; MCP Apps receive a fallback description and LiteCowork
deep-link. Approval/UserRequest channel rules remain those of their owning domains. A
weaker channel never gains stronger authority from rich formatting.


## Automation follow-up presentation

When a user continues an Automation Task, the initial run-context summary is a host-bound
control/presentation, not model-generated conversation history. RichResponse may enhance
subsequent agent answers normally, but cannot rewrite trigger/Task/Artifact/Evidence truth
or imply native-session continuity when the adapter used a durable cold rebuild.

## V1 detailed design integration — 2026-10-12

RENDERING-CONFORMANCE-LLD.md defines native output fallback and conformance golden corpus. RichPresentation is optional; never treat generated React/HTML/JS controls as authorized Core actions. The main semantic message stays available, accessible and complete. Unsupported vendor-specific native output must render safe diagnostic detail rather than silently disappear.

## Intelligent UI and image content audit

RichPresentation v1 is a static, display-only subset. It does not include generated images or interactive controls, and its existing block list must remain closed. The product contract for interactive response views, generated image references, and the current implementation gap is [Intelligent UI and Image Content](INTELLIGENT-UI-AND-IMAGE-CONTENT.md).
