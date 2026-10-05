# File, folder, ZIP and local-model delivery

Primary stories: E02-S03, E06-S01–S05, E08-S03. Core Resource identity/authorization/
revision/deletion contracts remain authoritative. Parsing and semantic retrieval are
provider implementations; the app presents them as one integrated experience. No extra
Core knowledge aggregate or hidden reasoning loop is introduced.

## User flow and placement

1. Choose files, a folder/root, or ZIP. Preview selected paths/types/count/estimated bytes
   and scope. Choosing a folder does not authorize all parent folders or machine capture.
2. Commit original inputs through Resource intake; digest-check resumable upload. Preserve
   original source identity even when parsing derives text/table/image artifacts.
3. ZIP preview reads the archive manifest in bounded isolation. Deny absolute/parent paths,
   symlinks/hardlinks/device entries, duplicate normalized names and unsafe nesting. Set
   archive entry/expanded-byte/compression/time/depth limits before extract; reject encrypted
   archives without a supported owner-mediated path. Never extract into a live checkout.
4. Parse in isolated CPU/memory/time quotas. OCR is explicit when text extraction fails;
   preserve page/layout/table coordinates and extraction warnings. No document macro runs.
5. Provider indexes authorized immutable revisions; report per-file ready/stale/failed.
   Upload completion does not imply semantic readiness. User can retry unsupported files.
6. Search deterministic metadata first or invoke qualified semantic provider. Build bounded
   attachments only after last-boundary ResourceResolver authorization/revision checks.
7. Answer with exact source links/spans and extraction warnings. No adequate source means
   explicit uncertainty; a generated citation is not independently verified Evidence.
8. File change invalidates derivative chunks/evidence; incremental reindex creates a new
   generation. Revocation blocks reads immediately, then receipt-based purge deletes all
   registered owned indexes/blobs. Never resurrect revoked sources during restore.

Local storage, parsing, embeddings and inference are separate placements. With a cloud
lead, selected text may leave the laptop even if files/index are local. Show provider and
egress policy before allowing that path. A genuinely offline configuration needs local
parser, embedding, retrieval, inference and a qualified agent harness, tested with network
disabled. Cloud continuation needs explicitly portable resources; local-only sources wait.

## Provider contract qualification

The provider adapter must expose bounded parse/index/search/remove/reconcile behavior via
existing capability/provider ports. If additional normalized result fields are needed,
update the owning domain, schema, API and event contract together before coding. The
following is an implementation checklist, not an invented third-party wire protocol:

| Operation | Inputs pinned | Outputs / guarantees |
|---|---|---|
| Parse | Resource ID/revision/digest, media type, parser/config version, limits | Segments with page/span/table locators, derived refs/digests, warnings, partial failure |
| Index | Exact segment refs, scope, embedding model/options digest | Generation/status, counts, capability/placement metadata; no credentials |
| Search | Authorized scope, optional revision filters, query, result/byte/token budget | Scored refs with exact revision/segment/locator; score not truth or confidence guarantee |
| Remove | Sealed purge-plan target and exact revision set | Idempotent receipt matching owned-replica plan; pending/failure explicit |
| Reconcile | Registered source revision manifest and generation | Missing/stale/orphaned entries and bounded rebuild plan |

Each query/result carries the provider/version, observed time, extraction/index generation
and authorization context needed to audit it. Recheck ACL/tombstone before exposing bytes,
including cache hits. Query content is untrusted and cannot expand scope or permissions.

## Parsing and retrieval choices

Begin TXT/MD/CSV/code/PDF text; then scanned PDF/images and DOCX/XLSX/PPTX/HTML; ZIP wraps
safe batch intake, not arbitrary execution. Preserve spreadsheet formula and evaluated
value separately where the provider supports them. Unsupported formats show a specific
warning and conversion path, not an empty successful parse. Docling/type parsers require
license/model-download/CPU-memory qualification, not automatic bundled approval.

Provider can combine lexical+vector retrieval with reranking if gold-corpus evidence
improves over FTS. Chunk by document semantics/page/table/code boundaries where possible;
retain overlap rationale and IDs derived from immutable revision plus parser settings.
Embedding model changes create a new generation; do not compare incompatible vectors.
Bound top-k/rerank budget and attachment bytes separately. Cache by scope/revision/provider
config; never cache authorization decisions across revocation.

## Gold corpus and release evidence

Synthetic fixture corpus: 20 text PDFs, scanned multilingual receipts, merged-cell XLSX,
formula workbook, DOCX/PPTX, Markdown/code repo, UTF-8/non-UTF text, duplicate pages,
corrupt files and adversarial ZIPs. Maintain gold questions with answer spans, unanswerable
questions, conflicting revisions, private sources and expected denials. Measure citation
resolvability/correctness, retrieval recall at stated k, grounded answer accuracy, OCR/table
accuracy, stale/leaked hits, p50/p95 latency and memory at recorded corpus/hardware sizes.

Do not invent a local-LLM-app parity number. SP05/SP06 freeze quality/performance targets
from real baseline and user task acceptance. Required safety targets are categorical:
zero out-of-scope hits, zero revoked-source exposure, no archive escape, no secret logs.

## Local-agent quality

Qualify OpenCode/another enabled native harness with local engine: streaming/cancel,
context compression, tool-call reliability, files/worktrees, structured results, replay,
model load errors and resource limits. A raw OpenAI-compatible completion endpoint is
not full agent functionality. Publish which model/hardware supports each benchmark;
small local models may fail sophisticated coding/office reasoning even with good RAG.
Offer explicit profile escalation under policy rather than silent cloud fallback.

## Existing local AI application comparison

Inspect Open WebUI's knowledge bases and incremental directory sync, Jan's project/file
context and local/remote provider options, and GPT4All LocalDocs' on-device indexing. These
show user-visible expectations: add folders, wait for indexing, see readiness/chunks, ask
with scoped context, and keep sources associated with a project. LiteCowork must add durable
Resource revisions, policy/placement, source citations, deletion fences, and the verified
Task/Artifact flow around those experiences. Avoid implying any single local app is the
market leader; the references are selected examples, not a popularity ranking.
