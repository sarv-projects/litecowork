import importlib
import json
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch


def process_supervisor_api():
    try:
        return importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
    except ModuleNotFoundError as error:
        if error.name == "implementation.spikes.agent_switching_poc.process_supervisor":
            raise AssertionError("PoC process supervisor has not been implemented") from error
        raise


class UnsupportedPlatformGuardTests(unittest.TestCase):
    def test_start_refuses_non_linux_platforms_before_spawning(self):
        api = process_supervisor_api()
        for platform in ("darwin", "win32"):
            with self.subTest(platform=platform):
                with patch.object(api.sys, "platform", platform), patch.object(
                    api.subprocess, "Popen"
                ) as spawn:
                    with self.assertRaisesRegex(RuntimeError, "requires Linux"):
                        api.ProcessSupervisor().start(
                            ["untrusted-worker"],
                            attempt_id="unsupported-platform",
                            cwd=".",
                        )
                    spawn.assert_not_called()


@unittest.skipUnless(sys.platform.startswith("linux"), "SP04 containment prototype targets Linux")
class ProcessSupervisorTests(unittest.TestCase):
    def test_process_identity_uses_boot_and_start_time_to_fence_pid_reuse(self):
        api = process_supervisor_api()
        identity = api.ProcessSupervisor.current_process_identity()

        self.assertTrue(api.ProcessSupervisor.is_process_identity_alive(identity))
        self.assertFalse(
            api.ProcessSupervisor.is_process_identity_alive(
                api.ProcessIdentity(
                    pid=identity.pid,
                    start_time_ticks=identity.start_time_ticks + 1,
                    boot_id=identity.boot_id,
                )
            )
        )
        self.assertFalse(
            api.ProcessSupervisor.is_process_identity_alive(
                api.ProcessIdentity(
                    pid=identity.pid,
                    start_time_ticks=identity.start_time_ticks,
                    boot_id=identity.boot_id + "-previous-boot",
                )
            )
        )

    def test_unreadable_boot_identity_fails_closed(self):
        api = process_supervisor_api()
        identity = api.ProcessSupervisor.current_process_identity()
        read_text = Path.read_text

        def read_text_without_boot(path, *args, **kwargs):
            if str(path) == "/proc/sys/kernel/random/boot_id":
                raise FileNotFoundError("synthetic hidden boot id")
            return read_text(path, *args, **kwargs)

        with patch.object(Path, "read_text", new=read_text_without_boot):
            with self.assertRaises(api.ProcessIdentityError):
                api.ProcessSupervisor.is_process_identity_alive(identity)

    def test_cancelled_worker_is_quiescent_before_late_write_can_happen(self):
        api = process_supervisor_api()
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "late-write.txt"
            script = (
                "import pathlib, time; time.sleep(0.5); "
                f"pathlib.Path({str(marker)!r}).write_text('stale write')"
            )
            supervisor = api.ProcessSupervisor()
            worker = supervisor.start(
                [sys.executable, "-c", script],
                attempt_id="attempt-delayed-write",
                cwd=directory,
            )
            time.sleep(0.03)

            evidence = supervisor.stop_and_confirm(worker, grace_seconds=0.5)
            time.sleep(0.6)

            self.assertTrue(evidence.quiescent)
            self.assertIsNotNone(evidence.exit_code)
            self.assertFalse(marker.exists())

    def test_already_finished_worker_returns_observed_exit_evidence(self):
        api = process_supervisor_api()
        with tempfile.TemporaryDirectory() as directory:
            supervisor = api.ProcessSupervisor()
            worker = supervisor.start(
                [sys.executable, "-c", "raise SystemExit(7)"],
                attempt_id="attempt-already-exited",
                cwd=directory,
            )
            worker.process.wait(timeout=2)
            evidence = supervisor.stop_and_confirm(worker, grace_seconds=0.5)

            self.assertTrue(evidence.quiescent)
            self.assertEqual(evidence.exit_code, 7)

    def test_supervisor_process_crash_stops_contained_writer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            heartbeat = root / "heartbeat"
            report = root / "worker-pid"
            child_script = (
                "import pathlib, time\n"
                f"heartbeat = pathlib.Path({str(heartbeat)!r})\n"
                "counter = 0\n"
                "while True:\n"
                "    heartbeat.write_text(str(counter))\n"
                "    counter += 1\n"
                "    time.sleep(0.01)\n"
            )
            repository = Path(__file__).resolve().parents[4]
            supervisor_script = (
                "import os, pathlib, sys, time\n"
                f"sys.path.insert(0, {str(repository)!r})\n"
                "from implementation.spikes.agent_switching_poc.process_supervisor import ProcessSupervisor\n"
                f"root = pathlib.Path({str(root)!r})\n"
                f"worker = ProcessSupervisor().start([sys.executable, '-c', {child_script!r}], "
                "attempt_id='attempt-parent-crash', cwd=root)\n"
                f"pathlib.Path({str(report)!r}).write_text(str(worker.process.pid))\n"
                f"heartbeat = pathlib.Path({str(heartbeat)!r})\n"
                "while not heartbeat.exists(): time.sleep(0.005)\n"
                "os._exit(0)\n"
            )

            supervisor_process = subprocess.Popen(
                [sys.executable, "-c", supervisor_script],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            deadline = time.monotonic() + 3
            while not report.exists() and time.monotonic() < deadline:
                time.sleep(0.005)
            self.assertTrue(report.exists(), "supervisor did not report its contained worker")
            _, stderr_bytes = supervisor_process.communicate(timeout=3)
            self.assertEqual(supervisor_process.returncode, 0)
            stderr = stderr_bytes.decode("utf-8", errors="replace")
            observed = heartbeat.read_text()
            time.sleep(0.25)

            self.assertEqual(heartbeat.read_text(), observed, stderr)

    def test_child_pwd_matches_supervised_working_directory(self):
        api = process_supervisor_api()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "cwd.json"
            script = (
                "import json, os, pathlib; "
                f"pathlib.Path({str(output)!r}).write_text(json.dumps([os.getcwd(), os.environ['PWD']]))"
            )
            supervisor = api.ProcessSupervisor()
            worker = supervisor.start(
                [sys.executable, "-c", script],
                attempt_id="attempt-cwd",
                cwd=directory,
            )
            self.assertEqual(worker.process.wait(timeout=2), 0)
            supervisor.stop_and_confirm(worker, grace_seconds=0.1)

            self.assertEqual(json.loads(output.read_text()), [directory, directory])

    def test_checkpoint_snapshot_is_read_only_while_receiver_workspace_is_writable(self):
        api = process_supervisor_api()
        integrity = importlib.import_module(
            "implementation.spikes.agent_switching_poc.handoff_integrity"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = root / "workspace"
            checkpoint = root / "checkpoint"
            workspace.mkdir()
            checkpoint.mkdir()
            (checkpoint / "source.py").write_text("original\n")
            manifest = integrity.build_checkpoint_manifest(checkpoint, ["source.py"])
            output = workspace / "write-check.json"
            stderr = root / "worker-stderr.txt"
            script = (
                "import json, pathlib\n"
                f"checkpoint = pathlib.Path({api.CHECKPOINT_MOUNT_PATH!r}) / 'source.py'\n"
                "try:\n"
                "    checkpoint.write_text('tampered\\n')\n"
                "    checkpoint_writable = True\n"
                "except OSError:\n"
                "    checkpoint_writable = False\n"
                f"pathlib.Path({str(workspace / 'receiver.py')!r}).write_text('receiver edit\\n')\n"
                f"pathlib.Path({str(output)!r}).write_text(json.dumps({{'checkpoint_writable': checkpoint_writable}}))\n"
            )
            supervisor = api.ProcessSupervisor()
            worker = supervisor.start(
                [sys.executable, "-c", script],
                attempt_id="attempt-readonly-checkpoint",
                cwd=workspace,
                read_only_checkpoint=checkpoint,
                checkpoint_manifest=manifest,
                stderr_path=stderr,
            )

            exit_code = worker.process.wait(timeout=2)
            self.assertEqual(exit_code, 0, stderr.read_text() if stderr.exists() else "")
            supervisor.stop_and_confirm(worker, grace_seconds=0.1)

            self.assertFalse(json.loads(output.read_text())["checkpoint_writable"])
            self.assertEqual((checkpoint / "source.py").read_text(), "original\n")
            self.assertEqual((workspace / "receiver.py").read_text(), "receiver edit\n")

    def test_receiver_checkpoint_bytes_do_not_follow_host_snapshot_mutation(self):
        api = process_supervisor_api()
        integrity = importlib.import_module(
            "implementation.spikes.agent_switching_poc.handoff_integrity"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = root / "workspace"
            checkpoint = root / "checkpoint"
            workspace.mkdir()
            checkpoint.mkdir()
            (checkpoint / "nested").mkdir()
            checkpoint_file = checkpoint / "nested" / "source.py"
            checkpoint_file.write_text("pinned bytes\n")
            manifest = integrity.build_checkpoint_manifest(checkpoint, ["nested/source.py"])
            release = workspace / "read-checkpoint"
            output = workspace / "checkpoint-seen.txt"
            script = (
                "import pathlib, time\n"
                f"release = pathlib.Path({str(release)!r})\n"
                "while not release.exists(): time.sleep(0.005)\n"
                f"pathlib.Path({api.CHECKPOINT_MOUNT_PATH!r}, 'nested', 'source.py').read_text()\n"
                f"pathlib.Path({str(output)!r}).write_text(pathlib.Path({api.CHECKPOINT_MOUNT_PATH!r}, 'nested', 'source.py').read_text())\n"
            )
            worker = api.ProcessSupervisor().start(
                [sys.executable, "-c", script],
                attempt_id="attempt-sealed-checkpoint",
                cwd=workspace,
                read_only_checkpoint=checkpoint,
                checkpoint_manifest=manifest,
            )

            checkpoint_file.write_text("host changed backing file\n")
            release.write_text("go")
            self.assertEqual(worker.process.wait(timeout=2), 0)

            self.assertEqual(output.read_text(), "pinned bytes\n")

    def test_receiver_checkpoint_size_limit_fails_before_worker_start(self):
        api = process_supervisor_api()
        integrity = importlib.import_module(
            "implementation.spikes.agent_switching_poc.handoff_integrity"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = root / "workspace"
            checkpoint = root / "checkpoint"
            workspace.mkdir()
            checkpoint.mkdir()
            (checkpoint / "source.py").write_text("more than three bytes")
            manifest = integrity.build_checkpoint_manifest(checkpoint, ["source.py"])

            with patch.object(api, "MAX_CHECKPOINT_TOTAL_BYTES", 3):
                with self.assertRaises(integrity.CheckpointIntegrityError):
                    api.ProcessSupervisor().start(
                        [sys.executable, "-c", "raise SystemExit(0)"],
                        attempt_id="attempt-oversized-checkpoint",
                        cwd=workspace,
                        read_only_checkpoint=checkpoint,
                        checkpoint_manifest=manifest,
                    )

    @unittest.skipUnless(sys.platform.startswith("linux"), "descendant-state check uses /proc")
    def test_cancel_waits_until_live_descendants_cannot_write(self):
        api = process_supervisor_api()
        with tempfile.TemporaryDirectory() as directory:
            ready = Path(directory) / "child-ready"
            marker = Path(directory) / "late-write.txt"
            child = (
                "import pathlib, signal, time; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"pathlib.Path({str(ready)!r}).write_text('ready'); "
                "time.sleep(0.8); "
                f"pathlib.Path({str(marker)!r}).write_text('stale write')"
            )
            parent = (
                "import subprocess, sys, time; "
                "subprocess.Popen([sys.executable, '-c', sys.argv[1]]); "
                "time.sleep(30)"
            )
            supervisor = api.ProcessSupervisor()
            worker = supervisor.start(
                [sys.executable, "-c", parent, child],
                attempt_id="attempt-descendant",
                cwd=directory,
            )
            deadline = time.monotonic() + 2
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(0.005)
            self.assertTrue(ready.exists(), "child did not reach its controlled state")

            evidence = supervisor.stop_and_confirm(worker, grace_seconds=0.1)
            time.sleep(0.9)

            self.assertTrue(evidence.quiescent)
            self.assertFalse(marker.exists(), "descendant wrote after quiescence was reported")

    def test_pid_namespace_contains_descendant_that_creates_new_session(self):
        api = process_supervisor_api()
        with tempfile.TemporaryDirectory() as directory:
            ready = Path(directory) / "detached-ready"
            marker = Path(directory) / "detached-write.txt"
            child = (
                "import os, pathlib, time; os.setsid(); "
                f"pathlib.Path({str(ready)!r}).write_text('ready'); "
                "time.sleep(0.8); "
                f"pathlib.Path({str(marker)!r}).write_text('stale write')"
            )
            parent = (
                "import subprocess, sys, time; "
                "subprocess.Popen([sys.executable, '-c', sys.argv[1]]); "
                "time.sleep(30)"
            )
            supervisor = api.ProcessSupervisor()
            worker = supervisor.start(
                [sys.executable, "-c", parent, child],
                attempt_id="attempt-detached-descendant",
                cwd=directory,
            )
            deadline = time.monotonic() + 2
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(0.005)
            self.assertTrue(ready.exists(), "detached child did not reach its controlled state")

            evidence = supervisor.stop_and_confirm(worker, grace_seconds=0.1)
            time.sleep(0.9)

            self.assertTrue(evidence.quiescent)
            self.assertFalse(marker.exists(), "detached descendant escaped the PID namespace")


if __name__ == "__main__":
    unittest.main()
