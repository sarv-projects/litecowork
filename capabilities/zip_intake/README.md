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
for successfully extracted entries. The production isolation boundary is
`ZipWorker.preview()` / `ZipWorker.extract()`; do not invoke the provider directly on
untrusted bytes in a Runtime process. Only the exact SHA-256 checked immutable input is
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
bytes to this capability. The in-memory provider port does not accept a filesystem
destination. A separate `BoundedZipExtractor` in `extraction.py` can plan and materialize
entries under an explicit provider-owned staging root; it still creates no Resources,
provenance edges, Effects, receipts, or events. Paths are virtual provenance labels; they
cannot grant filesystem access. Core owns Resource registration, immutable blob
publication, provenance edges, grants, Invocation/Effect recording, retention, deletion,
and UI projections. Payloads must be published only through those Core contracts, after
scope/revocation checks. A successful extraction is not an Evidence or verification
receipt.

`BoundedZipExtractor.plan(SourceRevision, bytes)` returns a closed `ExtractionPlan` without
expanding payloads. Entries remain in archive order; `order` lists admitted entry indexes
sorted by UTF-8 path bytes, and each entry carries its stable identity (archive index plus
raw-name SHA-256), declared bytes, measured compressed bytes, a bounded media-type hint,
and a typed refusal code for every rejected path. `materialize(plan, bytes, staging_root)`
rebuilds and compares the plan before any write, streams admitted entries in 64 KiB chunks
under the per-entry, compression-ratio, and total expanded-byte budgets, and returns
observed bytes, content SHA-256, and a content-checked media type (or `None`) in the
required order. It refuses an unsafe or missing staging root, existing destination paths,
symlink components, and any path outside that root; partial files are removed. It requires
an existing non-symlink directory and anchored directory-relative OS support; otherwise it
fails closed. Staging files are provider-owned private material until Core publishes them;
they are not Resources, index entries, Task context, or Artifacts.

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

`ZipIntakeProvider` itself is a library, not a process sandbox. Its `ZipWorker` boundary
is a Linux-only preview/extraction runner: it starts the fixed `worker.py` entry point using
Bubblewrap and `/usr/bin/prlimit`, passes a pinned source identity and bounded archive bytes
over stdin, and returns a bounded metadata manifest or bounded in-memory payloads. Bubblewrap gives the worker private
user/PID/network/IPC/UTS/mount namespaces, a read-only runtime and code view, no host home or
workspace mount, and a 1 MiB private `/tmp`. Inherited limits bound CPU to 8 seconds,
address space to 512 MiB, file size to zero, file descriptors to 32, and the parent kills
the process group after 12 seconds. The parent streams at most 24 MiB of request JSON; it
reads at most 8 MiB plus one sentinel byte for preview or 48 MiB plus one sentinel byte for
extraction before killing an over-producing child. Stderr is discarded because it is not
part of the safe protocol. The worker uses no filesystem extraction path. `qualified()`
executes a real round-trip probe and returns false if required Linux commands, mounts,
namespace, or parser execution fail.

Both worker calls accept an optional thread-safe cancellation event. The parent checks it
while pumping the bounded pipes, kills the worker process group, and raises
`WorkerCancelled`; cancellation does not create partial extraction state because the worker
invocation writes no files or database rows. The Runtime route and Library do not yet pass
such a token, so the product exposes no cancel control for ZIP processing.

This boundary has only been exercised on the current Ubuntu development host (Bubblewrap
0.9.0, system Python 3.12, util-linux `prlimit`). It does not establish support for other
distributions or desktop operating systems. The worker now supports both metadata preview
and bounded in-memory member extraction. Extraction output is bounded to 48 MiB including
base64 and manifest overhead; the total unencoded expansion remains bounded to 32 MiB and
each entry to 8 MiB. Parent-side decoding checks the exact source revision, every member's
path/index/length/digest, and manifest consistency. The end-to-end `qualified()` probe tests
both preview and extraction. A fixed source-build-only preview CLI calls the supervised
provider from `litecoworkd`; neither the CLI nor provider is packaged in the desktop
Runtime, and daemon extraction is not wired. The authenticated Runtime readiness route
continues to report ZIP unavailable. There is no extraction Operator command, product
cancellation command, or Core child-Resource/provenance/deletion transaction. Do not expose
extracted payloads or enable ZIP readiness until those integrations and platform
qualification are complete.

## Delivery and deferred verification

Focused standard-library fixtures and real Linux Bubblewrap integration tests are in
`tests/`. The narrow check is:

```sh
uv run python -m unittest discover -s capabilities/zip_intake/tests -v
```

SP06 remains unperformed: there is no qualified PDF/Office/OCR parser, RAG provider,
embedding index, Runtime subprocess integration, Core upload integration, storage
transaction coverage, or UI extraction report in this slice. E06-S01 and the user/system
acceptance gates remain incomplete. The current desktop upload/search path still treats
ZIPs as opaque Resources, as specified in [WORLD-RESOURCES](../../docs/WORLD-RESOURCES.md).

The current LiteCowork boundary is explicit: the daemon router mounts
`GET /v1/capabilities/zip-intake` and the Linux source-build-only
`POST /v1/resources/{resource_id}/archive/preview`; the Tauri command handler registers the
readiness bridge and the Library mounts the notice. The readiness route reports
`UNAVAILABLE / ISOLATED_WORKER_NOT_QUALIFIED`. The preview route accepts an exact managed
Resource revision, calls the fixed isolated CLI, and returns metadata only. It does not
enable extraction or change the opaque Resource behavior: archive members are not exposed
to indexing, Task context, or Artifact publication. Packaged and non-Linux preview remains
unavailable. No extraction command, Core child-Resource/provenance/deletion transaction, or
product cancellation flow is implemented.
