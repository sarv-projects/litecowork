"""Fixed stdin/stdout protocol for the Linux Bubblewrap ZIP worker."""

from __future__ import annotations

import base64
from dataclasses import asdict
import json
import sys

sys.path.insert(0, "/opt/litecowork/zip_intake")

from provider import ArchiveRejected, Limits, SourceRevision, ZipIntakeProvider  # noqa: E402


MAX_REQUEST_BYTES = 24 * 1024 * 1024
MAX_RESPONSE_BYTES = 8 * 1024 * 1024


def main() -> int:
    raw = sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1)
    try:
        if len(raw) > MAX_REQUEST_BYTES:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        request = json.loads(raw)
        if not isinstance(request, dict) or set(request) != {"source", "archive_b64"}:
            raise ArchiveRejected("INVALID_REQUEST")
        source_value = request["source"]
        if not isinstance(source_value, dict) or set(source_value) != {
            "workspace_id", "resource_id", "revision_id", "content_sha256"
        }:
            raise ArchiveRejected("INVALID_REQUEST")
        source = SourceRevision(**source_value)
        encoded = request["archive_b64"]
        if not isinstance(encoded, str) or len(encoded) > ((Limits().max_archive_bytes + 2) // 3) * 4:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        archive = base64.b64decode(encoded, validate=True)
        manifest = ZipIntakeProvider().preview(source, archive)
        result = {"ok": True, "manifest": asdict(manifest)}
    except ArchiveRejected as error:
        result = {"ok": False, "code": error.code}
    except Exception:
        # Keep parser internals, archive names, and environment details out of IPC.
        result = {"ok": False, "code": "INVALID_REQUEST"}
    output = json.dumps(result, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    if len(output) > MAX_RESPONSE_BYTES:
        output = b'{"ok":false,"code":"MANIFEST_SIZE_LIMIT"}'
    sys.stdout.buffer.write(output)
    sys.stdout.buffer.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
