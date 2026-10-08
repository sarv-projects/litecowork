# Bounded ZIP intake capability slice

This is the local Python archive implementation for the ZIP portion of
[E06-S01](../../implementation/epics/E06.md). It uses only Python's standard
library and follows the optional isolated-provider direction in
[STACK](../../implementation/STACK.md). No dependency or lockfile changes are required.
There is no generic provider registry, LiteSPM wire protocol, or Core parsing service.

## Port and ownership

`ZipIntakeProvider.preview(SourceRevision, bytes)` returns a bounded metadata
`Manifest`. `READY` means metadata admitted; it does not mean extracted, indexed,
searchable, trusted, or authorized. `extract(SourceRevision, bytes)` repeats admission
and returns `ExtractionResult`: an updated manifest plus immutable in-memory payloads
for successfully extracted entries. Only the exact SHA-256 checked immutable input is
used. The original bytes are unchanged. Both results preserve the Workspace, Resource,
revision, input digest, provider version, and limits; each entry retains its archive index,
raw-name digest, safe virtual path when available, sizes, CRC, and status/reason. Successful
entries also carry actual byte count and SHA-256. Unsafe raw names and library error text
are excluded from reports. Safe virtual paths remain untrusted display labels.

Name provenance hashes the exact central-directory filename bytes, including rejected
names. A bounded central-directory reader strictly decodes UTF-8 or ZIP-default CP437,
rejects embedded NUL and alternate filename encodings, checks exact raw names against
local headers, and cross-checks names and size/CRC/flags/offset/attribute metadata against
`ZipInfo`. It does not rely on `ZipInfo` to preserve hostile raw names. Malformed directory
records, counts, lengths, local offsets, and compressed ranges fail closed before payloads
are opened; rejected names never expose truncated or substituted display paths.

The caller must resolve and authorize the exact pinned Resource revision before handing
bytes to this capability. The provider does not accept a filesystem destination and never
extracts to disk. Paths are virtual provenance labels; they cannot grant filesystem access.
Core owns Resource registration, immutable blob publication, provenance edges, grants,
Invocation/Effect recording, retention, deletion, and UI projections. Payloads must be
published only through those Core contracts, after scope/revocation checks. A successful
extraction is not an Evidence or verification receipt.

## Guards

Admission bounds compressed input bytes, entries, declared expansion, per-entry sizes,
compression ratio, normalized path bytes and depth, and elapsed processing time.
Streaming additionally bounds actual expansion, checks CRC through the ZIP reader,
rejects disguised nested archive signatures, and reports file failures independently.
Only stored and DEFLATE compression are accepted. Multipart, ZIP64, prefixed and
trailing-payload archives are refused. Encrypted members, symlinks, Unix link metadata,
special files, absolute/drive/UNC/traversal paths, control characters, Windows reserved
names, Unicode canonical/compatibility normalization and case collisions, duplicate paths,
and file/child conflicts
are rejected. No collision has a winning entry. Nested archives are rejected by extension
or bounded magic inspection; the nesting depth is fixed at zero. ZIP-based Office documents
are consequently rejected as nested containers in this conservative slice; a future
qualified type parser needs an explicit allowlist and its own bounded container contract.

Archive-wide admission failures raise `ArchiveRejected` with a safe reason code and return
no payload. Individual failures produce `REJECTED` entries; safe siblings can succeed.
Metadata preview cannot certify body integrity or nested content, so extraction reopens
and rechecks the archive. No preview result can be used as an admission token.

Time checks are cooperative. This library is **not a process sandbox** and cannot interrupt
a blocked native decompressor or enforce hard RSS/CPU limits. Before integration, run it
in a supervised isolated worker without ambient filesystem/network authority, enforce
hard wall-clock/CPU/memory/output budgets, and discard results on cancellation or worker
failure. The default expanded-byte limit bounds returned payloads, not process RSS.

## Delivery and deferred verification

Focused standard-library fixture tests are in `tests/test_provider.py`. The intended
narrow check, once the owner's verification deferral is lifted, is:

```sh
uv run python -m unittest discover -s capabilities/zip_intake/tests -v
```

No tests, builds, formatters, or validators were run for this change. SP06 remains
unperformed: there is no qualified PDF/Office/OCR parser, RAG provider, embedding index,
provider subprocess transport, Core upload integration, storage transaction coverage,
or UI extraction report in this slice. E06-S01 and the user/system acceptance gates
remain incomplete. The current desktop upload/search path still treats ZIPs as opaque
Resources, as specified in [WORLD-RESOURCES](../../docs/WORLD-RESOURCES.md).

The current LiteCowork boundary is explicit: the daemon router mounts
`GET /v1/capabilities/zip-intake`, the Tauri command handler registers the readiness bridge,
and the Library mounts the notice. The route reports
`UNAVAILABLE / ISOLATED_WORKER_NOT_QUALIFIED` without passing bytes to this provider. These
status surfaces do not make the parser production-safe or extraction available; ZIP upload
continues to preserve an opaque Resource and archive members are not exposed to indexing,
Task context, or Artifact publication.
