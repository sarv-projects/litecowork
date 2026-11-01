"""Closed request protocol tests for the fixed local Runtime ZIP adapter."""

from hashlib import sha256
from io import BytesIO
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import zipfile
from unittest.mock import patch

from capabilities.zip_intake import SourceRevision
from capabilities.zip_intake import runtime_cli


class FakeStdin:
    def __init__(self, value: bytes):
        self.buffer = BytesIO(value)


class RuntimeCliProtocolTests(unittest.TestCase):
    def request(self, **overrides):
        archive = b"fixed archive bytes"
        source = {
            "workspace_id": "workspace-1",
            "resource_id": "resource-1",
            "revision_id": "revision-1",
            "content_sha256": sha256(archive).hexdigest(),
        }
        value = {
            "operation": "preview",
            "source": source,
            "archive_b64": base64.b64encode(archive).decode("ascii"),
        }
        value.update(overrides)
        return value, source, archive

    def test_runtime_request_decodes_only_the_exact_pinned_archive(self):
        value, expected_source, expected_archive = self.request()
        wire = json.dumps(value, separators=(",", ":")).encode("ascii")

        with patch.object(runtime_cli.sys, "stdin", FakeStdin(wire)):
            source, archive = runtime_cli._read_request()

        self.assertEqual(source, SourceRevision(**expected_source))
        self.assertEqual(archive, expected_archive)

    def test_runtime_request_rejects_paths_and_unknown_fields(self):
        value, _, _ = self.request(host_path="/etc/passwd")
        wire = json.dumps(value).encode("ascii")

        with patch.object(runtime_cli.sys, "stdin", FakeStdin(wire)):
            with self.assertRaises(runtime_cli.ArchiveRejected) as error:
                runtime_cli._read_request()

        self.assertEqual(error.exception.code, "INVALID_REQUEST")

    def test_runtime_request_rejects_duplicate_protocol_fields(self):
        value, _, _ = self.request()
        wire = json.dumps(value, separators=(",", ":")).encode("ascii")
        wire = wire.replace(b'"operation":"preview"', b'"operation":"extract","operation":"preview"')

        with patch.object(runtime_cli.sys, "stdin", FakeStdin(wire)):
            with self.assertRaises(runtime_cli.ArchiveRejected) as error:
                runtime_cli._read_request()

        self.assertEqual(error.exception.code, "INVALID_REQUEST")

    def test_runtime_request_rejects_extraction_mode_and_cannot_return_member_bytes(self):
        value, _, _ = self.request(operation="extract")
        wire = json.dumps(value).encode("ascii")

        with patch.object(runtime_cli.sys, "stdin", FakeStdin(wire)):
            with self.assertRaises(runtime_cli.ArchiveRejected) as error:
                runtime_cli._read_request()

        self.assertEqual(error.exception.code, "INVALID_REQUEST")


@unittest.skipUnless(sys.platform == "linux" and os.path.isfile("/usr/bin/bwrap"), "requires isolated Linux worker")
class RuntimeCliSystemTests(unittest.TestCase):
    def test_fixed_cli_runs_bubblewrap_and_returns_manifest_without_member_bytes(self):
        archive = BytesIO()
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as package:
            package.writestr("folder/note.txt", b"private body")
        data = archive.getvalue()
        source = {
            "workspace_id": "workspace-1",
            "resource_id": "resource-1",
            "revision_id": "revision-1",
            "content_sha256": sha256(data).hexdigest(),
        }
        request = {
            "operation": "preview",
            "source": source,
            "archive_b64": base64.b64encode(data).decode("ascii"),
        }
        cli = Path(runtime_cli.__file__).resolve()

        result = subprocess.run(
            ["/usr/bin/python3", "-I", str(cli)],
            input=json.dumps(request, separators=(",", ":")).encode("ascii"),
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd="/",
            env={},
            timeout=20,
            check=False,
        )

        self.assertEqual(result.returncode, 0)
        self.assertLessEqual(len(result.stdout), runtime_cli.MAX_RESPONSE_BYTES)
        response = json.loads(result.stdout)
        self.assertIs(response["ok"], True)
        self.assertNotIn("payloads", response)
        manifest = response["manifest"]
        self.assertEqual(manifest["source"], source)
        self.assertEqual(manifest["entries"][0]["virtual_path"], "folder/note.txt")
        self.assertEqual(manifest["entries"][0]["status"], "READY")
        self.assertNotIn("private body", result.stdout.decode("ascii"))

if __name__ == "__main__":
    unittest.main()
