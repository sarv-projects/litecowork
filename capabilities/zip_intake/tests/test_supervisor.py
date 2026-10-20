"""Real Linux namespace integration tests for the supervised ZIP preview path."""

from hashlib import sha256
from dataclasses import asdict
import base64
from io import BytesIO
import json
import os
import time
import stat
import struct
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch
import zipfile

from capabilities.zip_intake import ArchiveRejected, ExtractionResult, SourceRevision, ZipIntakeProvider
from capabilities.zip_intake.supervisor import ZipWorker, WorkerCancelled, WorkerFailed, WorkerOutputLimit, WorkerUnavailable, WorkerTimedOut


def archive(entries):
    stream = BytesIO()
    with zipfile.ZipFile(stream, "w", compression=zipfile.ZIP_DEFLATED) as package:
        for name, content in entries:
            package.writestr(name, content)
    return stream.getvalue()


def source(data):
    return SourceRevision("workspace-1", "resource-1", "revision-1", sha256(data).hexdigest())


@unittest.skipUnless(sys.platform == "linux" and os.path.isfile("/usr/bin/bwrap"), "requires /usr/bin/bwrap")
class ZipWorkerIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.worker = ZipWorker()

    def test_bubblewrap_preview_returns_metadata_without_member_content(self):
        data = archive([("note.txt", b"hello")])

        manifest = self.worker.preview(source(data), data)

        self.assertEqual(manifest["entries"][0]["status"], "READY")
        self.assertEqual(manifest["source"]["resource_id"], "resource-1")
        self.assertIsNone(manifest["entries"][0]["content_sha256"])

    def test_preview_rejects_incomplete_worker_manifest(self):
        data = archive([("note.txt", b"hello")])
        pinned = source(data)
        with patch.object(self.worker, "_run", return_value={"manifest": {"source": asdict(pinned)}}):
            with self.assertRaises(WorkerFailed):
                self.worker.preview(pinned, data)

    def test_preview_rejects_ready_entries_outside_worker_limits(self):
        data = archive([("note.txt", b"hello")])
        pinned = source(data)
        valid = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))
        invalid_manifests = []
        oversized = json.loads(json.dumps(valid))
        oversized["entries"][0]["declared_bytes"] = oversized["limits"]["max_entry_bytes"] + 1
        invalid_manifests.append(oversized)
        high_ratio = json.loads(json.dumps(valid))
        high_ratio["entries"][0]["declared_bytes"] = (
            high_ratio["limits"]["max_compression_ratio"]
            * max(high_ratio["entries"][0]["compressed_bytes"], 1)
            + 1
        )
        invalid_manifests.append(high_ratio)

        for manifest in invalid_manifests:
            with self.subTest(declared=manifest["entries"][0]["declared_bytes"]):
                with patch.object(self.worker, "_run", return_value={"manifest": manifest}):
                    with self.assertRaises(WorkerFailed):
                        self.worker.preview(pinned, data)

    def test_preview_rejects_extraction_status_and_content_metadata(self):
        data = archive([("note.txt", b"hello")])
        pinned = source(data)
        extracted = ZipIntakeProvider().extract(pinned, data)
        manifest = json.loads(json.dumps(asdict(extracted.manifest)))
        with patch.object(self.worker, "_run", return_value={"manifest": manifest}):
            with self.assertRaises(WorkerFailed):
                self.worker.preview(pinned, data)

    def test_preview_rejects_extraction_metadata_on_ready_and_directory_entries(self):
        data = archive([("folder/", b""), ("note.txt", b"hello")])
        pinned = source(data)
        base_manifest = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))
        self.assertEqual([entry["status"] for entry in base_manifest["entries"]], ["DIRECTORY", "READY"])
        # A valid compressed empty directory entry is part of ordinary ZIP files.
        self.worker._manifest_from_wire(base_manifest, pinned, len(data))

        for entry_index in range(len(base_manifest["entries"])):
            manifest = json.loads(json.dumps(base_manifest))
            entry = manifest["entries"][entry_index]
            entry["observed_bytes"] = entry["declared_bytes"]
            entry["content_sha256"] = "a" * 64
            with self.subTest(status=entry["status"]):
                with patch.object(self.worker, "_run", return_value={"manifest": manifest}):
                    with self.assertRaises(WorkerFailed):
                        self.worker.preview(pinned, data)

    def test_preview_rejects_unexpected_provider_version(self):
        data = archive([("note.txt", b"hello")])
        pinned = source(data)
        manifest = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))
        manifest["provider_version"] = "unreviewed-provider/9.9.9"
        with patch.object(self.worker, "_run", return_value={"manifest": manifest}):
            with self.assertRaises(WorkerFailed):
                self.worker.preview(pinned, data)

    def test_bubblewrap_extract_returns_only_valid_members_for_the_exact_source(self):
        data = archive([("../outside.txt", b"no"), ("safe/note.txt", b"hello")])
        pinned = source(data)

        result = self.worker.extract(pinned, data)

        self.assertIsInstance(result, ExtractionResult)
        self.assertEqual(result.manifest.source, pinned)
        self.assertEqual([entry.reason for entry in result.manifest.entries], ["UNSAFE_PATH", None])
        self.assertEqual(len(result.entries), 1)
        entry = result.entries[0]
        self.assertEqual((entry.entry_index, entry.virtual_path, entry.content), (1, "safe/note.txt", b"hello"))
        self.assertEqual(entry.content_sha256, sha256(entry.content).hexdigest())
        self.assertEqual(result.manifest.entries[1].content_sha256, entry.content_sha256)

    def test_bubblewrap_extract_accepts_explicit_compressed_directory_markers(self):
        data = archive([("folder/", b""), ("folder/note.txt", b"hello")])

        result = self.worker.extract(source(data), data)

        self.assertEqual([entry.status for entry in result.manifest.entries], ["DIRECTORY", "EXTRACTED"])
        self.assertEqual(result.manifest.entries[0].declared_bytes, 0)
        self.assertGreater(result.manifest.entries[0].compressed_bytes, 0)
        self.assertEqual([(entry.virtual_path, entry.content) for entry in result.entries], [
            ("folder/note.txt", b"hello"),
        ])

    def test_bubblewrap_extract_rejects_nested_members_and_keeps_safe_siblings(self):
        nested = archive([("inner.txt", b"hidden")])
        data = archive([("folder/inner.zip", nested), ("safe.txt", b"visible")])

        result = self.worker.extract(source(data), data)

        self.assertEqual(result.manifest.entries[0].reason, "NESTED_ARCHIVE")
        self.assertEqual([entry.virtual_path for entry in result.entries], ["safe.txt"])
        self.assertEqual(result.entries[0].content, b"visible")

    def test_bubblewrap_extract_rejects_actual_expansion_over_limit(self):
        data = archive([("large.txt", b"A" * (32 * 1024 * 1024 + 1))])

        with self.assertRaises(ArchiveRejected) as error:
            self.worker.extract(source(data), data)

        self.assertEqual(error.exception.code, "EXPANDED_SIZE_LIMIT")

    def test_bubblewrap_extract_rejects_archive_bytes_that_do_not_match_pinned_digest(self):
        data = archive([("safe.txt", b"hello")])
        pinned = source(data)
        changed = archive([("safe.txt", b"other")])

        with self.assertRaises(ArchiveRejected) as error:
            self.worker.extract(pinned, changed)

        self.assertEqual(error.exception.code, "SOURCE_DIGEST_MISMATCH")

    def test_bubblewrap_extract_rejects_inconsistent_worker_payload_digest(self):
        data = archive([("safe.txt", b"hello")])
        pinned = source(data)
        result = ZipIntakeProvider().extract(pinned, data)
        payload = result.entries[0]
        response = {
            # Worker IPC is JSON, where dataclass tuples become arrays. Keep the fake
            # response on the same wire shape so this test exercises the digest guard.
            "manifest": json.loads(json.dumps(asdict(result.manifest))),
            "payloads": [{
                "entry_index": payload.entry_index,
                "virtual_path": payload.virtual_path,
                "content_sha256": "0" * 64,
                "content_b64": base64.b64encode(payload.content).decode("ascii"),
            }],
        }
        with patch.object(self.worker, "_run", return_value=response):
            with self.assertRaises(WorkerFailed):
                self.worker.extract(pinned, data)

    def test_bubblewrap_extract_rejects_declared_and_observed_size_disagreement(self):
        data = archive([("safe.txt", b"hello")])
        pinned = source(data)
        result = ZipIntakeProvider().extract(pinned, data)
        entry = result.entries[0]
        response = {
            "manifest": json.loads(json.dumps(asdict(result.manifest))),
            "payloads": [{
                "entry_index": entry.entry_index,
                "virtual_path": entry.virtual_path,
                "content_sha256": entry.content_sha256,
                "content_b64": base64.b64encode(entry.content).decode("ascii"),
            }],
        }
        response["manifest"]["entries"][0]["declared_bytes"] += 1

        with patch.object(self.worker, "_run", return_value=response):
            with self.assertRaises(WorkerFailed):
                self.worker.extract(pinned, data)

    def test_bubblewrap_extract_rejects_manifest_entries_left_unsettled_as_ready(self):
        data = archive([("safe.txt", b"hello")])
        pinned = source(data)
        manifest = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))
        response = {"manifest": manifest, "payloads": []}

        with patch.object(self.worker, "_run", return_value=response):
            with self.assertRaises(WorkerFailed):
                self.worker.extract(pinned, data)

    def test_bubblewrap_extract_rejects_worker_manifest_path_traversal(self):
        data = archive([("safe.txt", b"hello")])
        pinned = source(data)
        result = ZipIntakeProvider().extract(pinned, data)
        entry = result.entries[0]
        response = {
            "manifest": json.loads(json.dumps(asdict(result.manifest))),
            "payloads": [{
                "entry_index": entry.entry_index,
                "virtual_path": "../outside.txt",
                "content_sha256": entry.content_sha256,
                "content_b64": base64.b64encode(entry.content).decode("ascii"),
            }],
        }
        response["manifest"]["entries"][0]["virtual_path"] = "../outside.txt"

        with patch.object(self.worker, "_run", return_value=response):
            with self.assertRaises(WorkerFailed):
                self.worker.extract(pinned, data)

    def test_worker_manifest_rejects_noncanonical_failure_reason(self):
        data = archive([("../unsafe.txt", b"private")])
        pinned = source(data)
        manifest = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))
        manifest["entries"][0]["reason"] = "parser leaked /private/path"

        with self.assertRaises(WorkerFailed):
            self.worker._manifest_from_wire(manifest, pinned, len(data))

    def test_worker_manifest_rejects_unquarantined_casefold_path_collisions(self):
        data = archive([("Report.txt", b"one"), ("report.TXT", b"two")])
        pinned = source(data)
        manifest = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))
        self.assertTrue(all(entry["reason"] == "PATH_CONFLICT" for entry in manifest["entries"]))
        for entry in manifest["entries"]:
            entry["status"] = "READY"
            entry["reason"] = None

        with self.assertRaises(WorkerFailed):
            self.worker._manifest_from_wire(manifest, pinned, len(data))

    def test_worker_manifest_accepts_provider_quarantined_path_collisions(self):
        data = archive([("Report.txt", b"one"), ("report.TXT", b"two")])
        pinned = source(data)
        manifest = json.loads(json.dumps(asdict(ZipIntakeProvider().preview(pinned, data))))

        parsed = self.worker._manifest_from_wire(manifest, pinned, len(data))

        self.assertTrue(all(entry.status == "REJECTED" for entry in parsed.entries))
        self.assertTrue(all(entry.reason == "PATH_CONFLICT" for entry in parsed.entries))

    def test_qualified_runs_both_preview_and_sandboxed_extraction(self):
        with patch.object(self.worker, "extract", wraps=self.worker.extract) as extract:
            self.assertTrue(self.worker.qualified())
        extract.assert_called_once()

    def test_bubblewrap_preview_reports_malicious_names_without_member_content(self):
        data = archive([("../outside.txt", b"not-written"), ("safe.txt", b"ok")])

        manifest = self.worker.preview(source(data), data)

        self.assertEqual(manifest["entries"][0]["status"], "REJECTED")
        self.assertIsNone(manifest["entries"][0]["virtual_path"])
        self.assertEqual(manifest["entries"][1]["status"], "READY")

    def test_worker_is_unavailable_when_bubblewrap_is_missing(self):
        worker = ZipWorker(bwrap_path="/definitely/missing/bwrap")

        with self.assertRaises(WorkerUnavailable):
            data = archive([])
            worker.preview(source(data), data)

    def test_worker_rejects_sandbox_binary_outside_trusted_system_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            fake_bwrap = os.path.join(directory, "bwrap")
            with open(fake_bwrap, "w", encoding="ascii") as stream:
                stream.write("#!/bin/sh\nexit 0\n")
            os.chmod(fake_bwrap, 0o755)

            with self.assertRaises(WorkerUnavailable):
                ZipWorker(bwrap_path=fake_bwrap)._command()

    def test_probe_reports_success_only_after_sandboxed_parser_round_trip(self):
        self.assertTrue(self.worker.qualified())

    def test_worker_is_unavailable_outside_linux(self):
        data = archive([])
        with patch("capabilities.zip_intake.supervisor.sys.platform", "darwin"):
            with self.assertRaises(WorkerUnavailable):
                self.worker.preview(source(data), data)

    def test_wall_clock_deadline_kills_worker_process_group(self):
        data = archive([("note.txt", b"hello")])
        worker = ZipWorker(wall_timeout_seconds=0.001)

        with self.assertRaises(WorkerTimedOut):
            worker.preview(source(data), data)

    def test_expansion_bomb_fails_closed_inside_worker(self):
        data = archive([("large.txt", b"A" * (32 * 1024 * 1024 + 1))])

        with self.assertRaises(ArchiveRejected) as error:
            self.worker.preview(source(data), data)

        self.assertEqual(error.exception.code, "EXPANDED_SIZE_LIMIT")

    def test_unicode_duplicate_paths_are_rejected_inside_worker(self):
        data = archive([("café.txt", b"one"), ("cafe\u0301.txt", b"two")])

        manifest = self.worker.preview(source(data), data)

        self.assertEqual([entry["reason"] for entry in manifest["entries"]], ["PATH_CONFLICT", "PATH_CONFLICT"])

    def test_encrypted_and_symlink_entries_are_rejected_inside_worker(self):
        encrypted = bytearray(archive([("secret.txt", b"secret")]))
        local = encrypted.index(b"PK\x03\x04")
        central = encrypted.index(b"PK\x01\x02")
        for offset in (local + 6, central + 8):
            flags = struct.unpack_from("<H", encrypted, offset)[0]
            struct.pack_into("<H", encrypted, offset, flags | 1)

        symlink_info = zipfile.ZipInfo("link")
        symlink_info.create_system = 3
        symlink_info.external_attr = (stat.S_IFLNK | 0o777) << 16
        symlink = archive([(symlink_info, b"target")])

        for data, reason in ((bytes(encrypted), "ENCRYPTED_ENTRY"), (symlink, "SPECIAL_FILE")):
            with self.subTest(reason=reason):
                manifest = self.worker.preview(source(data), data)
                self.assertEqual(manifest["entries"][0]["reason"], reason)


@unittest.skipUnless(sys.platform == "linux", "process-group output containment requires Linux")
class ZipWorkerPipeSystemTests(unittest.TestCase):
    def test_cancellation_terminates_running_worker_process_group(self):
        worker = ZipWorker(wall_timeout_seconds=5.0)
        cancellation = threading.Event()
        command = [sys.executable, "-c", "import time; time.sleep(3)"]
        with patch.object(worker, "_command", return_value=command):
            timer = threading.Timer(0.03, cancellation.set)
            timer.start()
            started = time.monotonic()
            try:
                with self.assertRaises(WorkerCancelled):
                    worker._run({"operation": "preview"}, cancellation=cancellation)
            finally:
                timer.cancel()
        self.assertLess(time.monotonic() - started, 1.0)

    def test_stdout_limit_terminates_a_chatty_child_before_wall_deadline(self):
        worker = ZipWorker(wall_timeout_seconds=5.0)
        command = [
            sys.executable,
            "-c",
            "import os; chunk=b'x'*65536; exec('while True: os.write(1, chunk)')",
        ]
        started = time.monotonic()
        with patch.object(worker, "_command", return_value=command):
            with self.assertRaises(WorkerOutputLimit):
                worker._run({})
        self.assertLess(time.monotonic() - started, 2.0)

    def test_unneeded_child_stderr_is_discarded_without_blocking_response(self):
        worker = ZipWorker(wall_timeout_seconds=2.0)
        command = [
            sys.executable,
            "-c",
            "import os; os.write(2, b'x' * (2 * 1024 * 1024)); os.write(1, b'{\\\"ok\\\":true,\\\"manifest\\\":{}}')",
        ]
        with patch.object(worker, "_command", return_value=command):
            self.assertEqual(worker._run({"operation": "preview"}), {"manifest": {}})

    def test_duplicate_worker_response_fields_fail_closed(self):
        worker = ZipWorker(wall_timeout_seconds=2.0)
        command = [
            sys.executable,
            "-c",
            "import os; os.write(1, b'{\\\"ok\\\":true,\\\"manifest\\\":{\\\"source\\\":1,\\\"source\\\":2}}')",
        ]
        with patch.object(worker, "_command", return_value=command):
            with self.assertRaises(WorkerFailed):
                worker._run({"operation": "preview"})

    def test_deeply_nested_worker_json_fails_closed(self):
        worker = ZipWorker(wall_timeout_seconds=2.0)
        command = [
            sys.executable,
            "-c",
            "import os; os.write(1, b'[' * 4096 + b']' * 4096)",
        ]
        with patch.object(worker, "_command", return_value=command):
            with self.assertRaises(WorkerFailed):
                worker._run({"operation": "preview"})

    def test_child_exit_while_request_pipe_is_open_fails_without_waiting_for_wall_timeout(self):
        worker = ZipWorker(wall_timeout_seconds=2.0)
        command = [sys.executable, "-c", "pass"]
        started = time.monotonic()
        with patch.object(worker, "_command", return_value=command):
            with self.assertRaises(WorkerFailed) as error:
                worker._run({"large": "x" * (1024 * 1024)})
        self.assertNotIsInstance(error.exception, WorkerTimedOut)
        self.assertLess(time.monotonic() - started, 1.0)


if __name__ == "__main__":
    unittest.main()
