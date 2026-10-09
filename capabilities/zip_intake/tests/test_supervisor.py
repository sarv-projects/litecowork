"""Real Linux namespace integration tests for the supervised ZIP preview path."""

from hashlib import sha256
from io import BytesIO
import os
import time
import stat
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from capabilities.zip_intake import ArchiveRejected, SourceRevision
from capabilities.zip_intake.supervisor import ZipWorker, WorkerFailed, WorkerOutputLimit, WorkerUnavailable, WorkerTimedOut


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

    def test_bubblewrap_probe_executes_parser_without_enabling_extraction(self):
        data = archive([("note.txt", b"hello")])

        manifest = self.worker.preview(source(data), data)

        self.assertEqual(manifest["entries"][0]["status"], "READY")
        self.assertEqual(manifest["source"]["resource_id"], "resource-1")
        self.assertIsNone(manifest["entries"][0]["content_sha256"])

    def test_malicious_member_names_are_reported_without_extraction(self):
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
            self.assertEqual(worker._run({}), {})

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
