"""Fixed local Runtime adapter for the supervised ZIP metadata preview.

The daemon sends one closed JSON request on stdin. This adapter never accepts paths,
URLs, provider selection, or payload-return mode; ZIP bytes are handed only to ZipWorker,
which enforces the Linux Bubblewrap boundary. Stdout is a bounded metadata-only response.
"""

from __future__ import annotations

import base64
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from capabilities.zip_intake import (  # noqa: E402
    ArchiveRejected,
    SourceRevision,
    WorkerCancelled,
    WorkerFailed,
    WorkerTimedOut,
    WorkerUnavailable,
    ZipWorker,
)


MAX_REQUEST_BYTES = 24 * 1024 * 1024
MAX_ARCHIVE_BYTES = 16 * 1024 * 1024
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
SOURCE_FIELDS = {"workspace_id", "resource_id", "revision_id", "content_sha256"}
REQUEST_FIELDS = {"operation", "source", "archive_b64"}


def _reject_duplicate_fields(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate protocol field")
        result[key] = value
    return result


def _read_request() -> tuple[SourceRevision, bytes]:
    raw = sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1)
    if len(raw) > MAX_REQUEST_BYTES:
        raise ArchiveRejected("REQUEST_SIZE_LIMIT")
    try:
        request = json.loads(raw, object_pairs_hook=_reject_duplicate_fields)
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError):
        raise ArchiveRejected("INVALID_REQUEST") from None
    if not isinstance(request, dict) or set(request) != REQUEST_FIELDS or request["operation"] != "preview":
        raise ArchiveRejected("INVALID_REQUEST")
    source_value = request["source"]
    encoded = request["archive_b64"]
    if not isinstance(source_value, dict) or set(source_value) != SOURCE_FIELDS:
        raise ArchiveRejected("INVALID_REQUEST")
    if not isinstance(encoded, str) or len(encoded) > ((MAX_ARCHIVE_BYTES + 2) // 3) * 4:
        raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
    try:
        source = SourceRevision(**source_value)
        archive = base64.b64decode(encoded, validate=True)
    except (TypeError, ValueError):
        raise ArchiveRejected("INVALID_REQUEST") from None
    if len(archive) > MAX_ARCHIVE_BYTES:
        raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
    return source, archive


def _reply(value: dict[str, object]) -> int:
    output = json.dumps(value, separators=(",", ":"), ensure_ascii=True).encode("ascii")
    if len(output) > MAX_RESPONSE_BYTES:
        output = b'{"ok":false,"code":"RESPONSE_SIZE_LIMIT"}'
    sys.stdout.buffer.write(output)
    sys.stdout.buffer.flush()
    return 0


def main() -> int:
    try:
        source, archive = _read_request()
        manifest = ZipWorker().preview(source, archive)
        return _reply({"ok": True, "manifest": manifest})
    except ArchiveRejected as error:
        return _reply({"ok": False, "code": error.code})
    except WorkerTimedOut:
        return _reply({"ok": False, "code": "TIMEOUT"})
    except WorkerCancelled:
        return _reply({"ok": False, "code": "CANCELLED"})
    except WorkerUnavailable:
        return _reply({"ok": False, "code": "DEPENDENCY_UNAVAILABLE"})
    except (WorkerFailed, OSError, ValueError):
        return _reply({"ok": False, "code": "DEPENDENCY_UNAVAILABLE"})


if __name__ == "__main__":
    raise SystemExit(main())
