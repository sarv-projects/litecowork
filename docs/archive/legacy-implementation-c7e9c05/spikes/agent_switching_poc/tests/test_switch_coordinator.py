import importlib
import json
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import Mock, patch


def coordinator_api():
    try:
        return importlib.import_module(
            "implementation.spikes.agent_switching_poc.switch_coordinator"
        )
    except ModuleNotFoundError as error:
        if error.name == "implementation.spikes.agent_switching_poc.switch_coordinator":
            raise AssertionError("PoC safe-switch coordinator has not been implemented") from error
        raise


def support_api():
    ledger_module = importlib.import_module(
        "implementation.spikes.agent_switching_poc.switch_ledger"
    )
    integrity_module = importlib.import_module(
        "implementation.spikes.agent_switching_poc.handoff_integrity"
    )
    return ledger_module, integrity_module


class SafeSwitchCoordinatorTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        root = Path(self.temporary_directory.name)
        self.workspace = root / "workspace"
        self.workspace.mkdir()
        (self.workspace / "result.py").write_text("def complete(): return False\n")
        ledger_module, self.integrity = support_api()
        self.ledger = ledger_module.SwitchLedger(root / "ledger.sqlite3")
        self.ProcessExitEvidence = ledger_module.ProcessExitEvidence
        self.SwitchBlocked = ledger_module.SwitchBlocked
        self.ledger.create_task("task-coordinator", "Finish the helper.", "codex")
        self.ledger.start_attempt("task-coordinator", "attempt-codex", "codex")
        self.ledger.request_lead_switch(
            "task-coordinator",
            1,
            "opencode",
            {"objective": "Finish the helper."},
        )
        self.snapshot_store = root / "checkpoint-snapshots"
        self.coordinator = coordinator_api().SafeSwitchCoordinator(
            self.ledger,
            snapshot_store=self.snapshot_store,
        )

    def tearDown(self):
        self.ledger.close()
        self.temporary_directory.cleanup()

    def _packet(self):
        return {
            "objective": "Finish the helper.",
            "checkpoint": self.integrity.build_checkpoint_manifest(
                self.workspace,
                ["result.py"],
            ),
        }

    def _settle(self, packet):
        return self.coordinator.settle_attempt(
            "attempt-codex",
            checkpoint_root=self.workspace,
            handoff_packet=packet,
            process_exit_evidence=self.ProcessExitEvidence(
                attempt_id="attempt-codex",
                process_group_id=4,
                exit_code=0,
                quiescent=True,
                observed_at_monotonic_ns=time.monotonic_ns(),
            ),
            effects_reconciled=True,
            lease_released=True,
        )

    @staticmethod
    def _wait_for_quiet_heartbeat(path, *, quiet_seconds=0.25, timeout_seconds=3.0):
        deadline = time.monotonic() + timeout_seconds
        previous = path.read_text()
        unchanged_since = time.monotonic()
        while time.monotonic() < deadline:
            time.sleep(0.01)
            current = path.read_text()
            if current != previous:
                previous = current
                unchanged_since = time.monotonic()
            elif time.monotonic() - unchanged_since >= quiet_seconds:
                return current
        raise AssertionError("contained writer did not become quiescent before timeout")

    def test_sender_workspace_changes_after_settlement_do_not_change_pinned_snapshot(self):
        self._settle(self._packet())
        (self.workspace / "result.py").write_text("def complete(): return True\n")

        replacement = self.coordinator.admit_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
        )

        self.assertEqual(replacement.agent_binding, "opencode")
        self.assertEqual(
            (self.coordinator.snapshot_path_for_digest(replacement.handoff_packet["checkpoint"]["manifest_digest"])
             / "result.py").read_text(),
            "def complete(): return False\n",
        )
        task = self.ledger.get_task("task-coordinator")
        self.assertEqual(task.active_attempt_id, "attempt-opencode")
        self.assertEqual(self.ledger.get_attempt("attempt-codex").status, "SETTLED")

    def test_mutated_persisted_snapshot_blocks_receiver_admission(self):
        self._settle(self._packet())
        digest = self.ledger.get_attempt("attempt-codex").checkpoint_digest
        snapshot = self.coordinator.snapshot_path_for_digest(digest)
        (snapshot / "result.py").chmod(0o644)
        (snapshot / "result.py").write_text("tampered\n")

        with self.assertRaises(self.SwitchBlocked):
            self.coordinator.admit_replacement(
                "task-coordinator",
                expected_version=3,
                attempt_id="attempt-opencode",
            )

        self.assertIsNone(self.ledger.get_task("task-coordinator").active_attempt_id)

    def test_snapshot_identifier_rejects_path_traversal(self):
        with self.assertRaises(self.integrity.CheckpointIntegrityError):
            self.coordinator.snapshot_path_for_digest("../../outside")

    def test_verified_checkpoint_can_be_admitted_after_ledger_restart(self):
        packet = self._packet()
        self._settle(packet)
        self.ledger.close()
        self.ledger = support_api()[0].SwitchLedger(
            Path(self.temporary_directory.name) / "ledger.sqlite3"
        )
        self.coordinator = coordinator_api().SafeSwitchCoordinator(
            self.ledger,
            snapshot_store=self.snapshot_store,
        )

        replacement = self.coordinator.admit_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
        )

        self.assertEqual(replacement.agent_binding, "opencode")
        self.assertEqual(replacement.lease_epoch, 2)

    def test_admitted_receiver_reads_pinned_snapshot_without_writing_it(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        output = self.workspace / "receiver-observation.json"
        script = (
            "import json, pathlib\n"
            f"checkpoint = pathlib.Path({process_api.CHECKPOINT_MOUNT_PATH!r}) / 'result.py'\n"
            "original = checkpoint.read_text()\n"
            "try:\n"
            "    checkpoint.write_text('receiver overwrite\\n')\n"
            "    checkpoint_writable = True\n"
            "except OSError:\n"
            "    checkpoint_writable = False\n"
            f"pathlib.Path({str(self.workspace / 'receiver-output.py')!r}).write_text('receiver edit\\n')\n"
            f"pathlib.Path({str(output)!r}).write_text(json.dumps({{'original': original, 'checkpoint_writable': checkpoint_writable}}))\n"
        )
        supervisor = process_api.ProcessSupervisor()
        replacement, worker = self.coordinator.start_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
            supervisor=supervisor,
            command=[sys.executable, "-c", script],
            cwd=self.workspace,
        )
        checkpoint_path = self.coordinator.snapshot_path_for_digest(
            replacement.handoff_packet["checkpoint"]["manifest_digest"]
        )

        self.assertEqual(worker.process.wait(timeout=2), 0)
        supervisor.stop_and_confirm(worker, grace_seconds=0.1)

        observed = json.loads(output.read_text())
        self.assertEqual(observed["original"], "def complete(): return False\n")
        self.assertFalse(observed["checkpoint_writable"])
        self.assertEqual((checkpoint_path / "result.py").read_text(), "def complete(): return False\n")
        self.assertEqual((self.workspace / "receiver-output.py").read_text(), "receiver edit\n")

    def test_receiver_start_refuses_snapshot_changed_after_admission(self):
        self._settle(self._packet())
        task = self.ledger.get_task("task-coordinator")
        checkpoint = task.handoff_packet["checkpoint"]
        snapshot = self.coordinator.snapshot_path_for_digest(checkpoint["manifest_digest"])
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )

        class MutatingSupervisor:
            def start(self, command, **kwargs):
                snapshot_path = kwargs["read_only_checkpoint"]
                file_path = snapshot_path / "result.py"
                file_path.chmod(0o644)
                file_path.write_text("changed after admission, before process launch\n")
                return process_api.ProcessSupervisor().start(command, **kwargs)

            def stop_and_confirm(self, worker):
                return process_api.ProcessSupervisor().stop_and_confirm(worker)

        with self.assertRaises(self.integrity.CheckpointIntegrityError):
            self.coordinator.start_replacement(
                "task-coordinator",
                expected_version=3,
                attempt_id="attempt-opencode",
                supervisor=MutatingSupervisor(),
                command=[sys.executable, "-c", "raise SystemExit(0)"],
                cwd=self.workspace,
            )

        self.assertIsNone(self.ledger.get_task("task-coordinator").active_attempt_id)
        self.assertEqual(self.ledger.get_attempt("attempt-opencode").status, "START_FAILED")
        self.assertEqual(self.ledger.get_task("task-coordinator").version, 5)

    def test_replacement_process_start_transitions_admitted_attempt_to_running(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        replacement, worker = self.coordinator.start_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
            supervisor=process_api.ProcessSupervisor(),
            command=[sys.executable, "-c", "raise SystemExit(0)"],
            cwd=self.workspace,
        )
        self.assertEqual(replacement.status, "RUNNING")
        self.assertEqual(worker.process.wait(timeout=2), 0)
        process_api.ProcessSupervisor().stop_and_confirm(worker, grace_seconds=0.1)

    def test_runtime_restart_recovers_only_after_launch_owner_is_proven_gone(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        ledger_path = Path(self.temporary_directory.name) / "ledger.sqlite3"
        snapshot_store = Path(self.temporary_directory.name) / "checkpoint-snapshots"
        heartbeat = self.workspace / "runtime-crash-heartbeat"
        worker_pid_path = self.workspace / "runtime-crash-worker-pid"
        repository = Path(__file__).resolve().parents[4]
        writer_script = (
            "import pathlib, time\n"
            f"heartbeat = pathlib.Path({str(heartbeat)!r})\n"
            "counter = 0\n"
            "while True:\n"
            "    heartbeat.write_text(str(counter))\n"
            "    counter += 1\n"
            "    time.sleep(0.01)\n"
        )
        child_script = (
            "import os, pathlib, sys, time\n"
            f"sys.path.insert(0, {str(repository)!r})\n"
            "from implementation.spikes.agent_switching_poc.switch_coordinator import SafeSwitchCoordinator\n"
            "from implementation.spikes.agent_switching_poc.switch_ledger import SwitchLedger\n"
            "from implementation.spikes.agent_switching_poc.process_supervisor import ProcessSupervisor\n"
            f"ledger = SwitchLedger({str(ledger_path)!r})\n"
            f"coordinator = SafeSwitchCoordinator(ledger, snapshot_store={str(snapshot_store)!r})\n"
            "attempt = coordinator.admit_replacement("
            "'task-coordinator', expected_version=3, attempt_id='attempt-opencode')\n"
            f"checkpoint = coordinator.snapshot_path_for_digest(attempt.handoff_packet['checkpoint']['manifest_digest'])\n"
            f"worker = ProcessSupervisor().start([sys.executable, '-c', {writer_script!r}], "
            "attempt_id=attempt.attempt_id, "
            f"cwd={str(self.workspace)!r}, read_only_checkpoint=checkpoint, "
            "checkpoint_manifest=attempt.handoff_packet['checkpoint'])\n"
            f"pathlib.Path({str(worker_pid_path)!r}).write_text(str(worker.process.pid))\n"
            f"heartbeat = pathlib.Path({str(heartbeat)!r})\n"
            "while not heartbeat.exists(): time.sleep(0.005)\n"
            "os._exit(0)\n"
        )
        self.ledger.close()
        child = subprocess.run(
            [sys.executable, "-c", child_script],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=5,
            check=False,
        )
        self.assertEqual(child.returncode, 0, child.stderr)
        self.assertTrue(worker_pid_path.exists(), "Runtime did not launch its contained writer")
        self._wait_for_quiet_heartbeat(heartbeat)
        self.ledger = support_api()[0].SwitchLedger(ledger_path)
        self.coordinator = coordinator_api().SafeSwitchCoordinator(
            self.ledger,
            snapshot_store=snapshot_store,
        )
        replacement = self.ledger.get_attempt("attempt-opencode")
        self.assertEqual(replacement.status, "ADMITTED")
        self.assertNotEqual(
            replacement.launch_owner_identity.pid,
            process_api.ProcessSupervisor.current_process_identity().pid,
        )

        recovered = self.coordinator.recover_orphaned_admission(
            replacement.attempt_id,
            supervisor=process_api.ProcessSupervisor(),
        )

        self.assertEqual(recovered.status, "START_FAILED")
        self.assertIsNone(self.ledger.get_task("task-coordinator").active_attempt_id)
        self.assertEqual(self.ledger.get_task("task-coordinator").version, 5)
        retry, worker = self.coordinator.start_replacement(
            "task-coordinator",
            expected_version=5,
            attempt_id="attempt-opencode-retry",
            supervisor=process_api.ProcessSupervisor(),
            command=[sys.executable, "-c", "raise SystemExit(0)"],
            cwd=self.workspace,
        )
        self.assertEqual(retry.lease_epoch, 3)
        self.assertEqual(worker.process.wait(timeout=2), 0)
        process_api.ProcessSupervisor().stop_and_confirm(worker, grace_seconds=0.1)

    def test_running_attempt_recovery_blocks_while_runtime_or_worker_is_alive(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        running, worker = self.coordinator.start_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
            supervisor=process_api.ProcessSupervisor(),
            command=[sys.executable, "-c", "import time; time.sleep(10)"],
            cwd=self.workspace,
        )
        self.assertIsNotNone(running.worker_identity)

        with self.assertRaises(self.SwitchBlocked):
            self.coordinator.recover_orphaned_running_attempt(
                running.attempt_id,
                supervisor=process_api.ProcessSupervisor(),
                effects_reconciled=True,
                lease_released=True,
            )
        self.assertEqual(self.ledger.get_attempt(running.attempt_id).status, "RUNNING")
        process_api.ProcessSupervisor().stop_and_confirm(worker, grace_seconds=0.1)

    def test_running_attempt_recovery_fails_closed_without_worker_identity_or_settled_gates(self):
        self._settle(self._packet())
        replacement = self.coordinator.admit_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
        )
        running = self.ledger.mark_attempt_running(replacement.attempt_id)
        gone_supervisor = Mock()
        gone_supervisor.is_process_identity_alive.return_value = False

        with self.assertRaises(self.SwitchBlocked):
            self.ledger.abandon_orphaned_running_attempt(
                running.attempt_id,
                effects_reconciled=False,
                lease_released=True,
            )

        with self.assertRaises(self.SwitchBlocked):
            self.coordinator.recover_orphaned_running_attempt(
                running.attempt_id,
                supervisor=gone_supervisor,
                effects_reconciled=True,
                lease_released=True,
            )
        self.assertEqual(self.ledger.get_attempt(running.attempt_id).status, "RUNNING")
        self.assertEqual(gone_supervisor.is_process_identity_alive.call_count, 0)

    def test_worker_identity_sampling_failure_does_not_release_running_task_slot(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        supervisor = process_api.ProcessSupervisor()
        runtime_identity = process_api.ProcessSupervisor.current_process_identity()

        with patch.object(
            process_api.ProcessSupervisor,
            "_read_process_identity",
            side_effect=[runtime_identity, process_api.ProcessIdentityError("/proc unavailable")],
        ):
            running, worker = self.coordinator.start_replacement(
                "task-coordinator",
                expected_version=3,
                attempt_id="attempt-opencode",
                supervisor=supervisor,
                command=[sys.executable, "-c", "import time; time.sleep(10)"],
                cwd=self.workspace,
            )

        self.assertEqual(running.status, "RUNNING")
        self.assertIsNone(running.worker_identity)
        self.assertEqual(
            self.ledger.get_task("task-coordinator").active_attempt_id,
            running.attempt_id,
        )
        with self.assertRaises(self.SwitchBlocked):
            self.coordinator.recover_orphaned_running_attempt(
                running.attempt_id,
                supervisor=supervisor,
                effects_reconciled=True,
                lease_released=True,
            )
        self.assertEqual(self.ledger.get_attempt(running.attempt_id).status, "RUNNING")
        supervisor.stop_and_confirm(worker, grace_seconds=0.1)

    def test_recovery_blocks_while_admission_owner_is_alive(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        replacement = self.coordinator.admit_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
        )

        with self.assertRaises(self.SwitchBlocked):
            self.coordinator.recover_orphaned_admission(
                replacement.attempt_id,
                supervisor=process_api.ProcessSupervisor(),
            )
        self.assertEqual(self.ledger.get_attempt(replacement.attempt_id).status, "ADMITTED")

    def test_runtime_restart_recovers_running_attempt_only_after_worker_is_gone(self):
        self._settle(self._packet())
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        ledger_path = Path(self.temporary_directory.name) / "ledger.sqlite3"
        snapshot_store = Path(self.temporary_directory.name) / "checkpoint-snapshots"
        heartbeat = self.workspace / "running-crash-heartbeat"
        repository = Path(__file__).resolve().parents[4]
        writer_script = (
            "import pathlib, time\n"
            f"heartbeat = pathlib.Path({str(heartbeat)!r})\n"
            "counter = 0\n"
            "while True:\n"
            "    heartbeat.write_text(str(counter))\n"
            "    counter += 1\n"
            "    time.sleep(0.01)\n"
        )
        child_script = (
            "import os, pathlib, sys, time\n"
            f"sys.path.insert(0, {str(repository)!r})\n"
            "from implementation.spikes.agent_switching_poc.switch_coordinator import SafeSwitchCoordinator\n"
            "from implementation.spikes.agent_switching_poc.switch_ledger import SwitchLedger\n"
            "from implementation.spikes.agent_switching_poc.process_supervisor import ProcessSupervisor\n"
            f"ledger = SwitchLedger({str(ledger_path)!r})\n"
            f"coordinator = SafeSwitchCoordinator(ledger, snapshot_store={str(snapshot_store)!r})\n"
            "running, worker = coordinator.start_replacement("
            "'task-coordinator', expected_version=3, attempt_id='attempt-opencode', "
            f"supervisor=ProcessSupervisor(), command=[sys.executable, '-c', {writer_script!r}], "
            f"cwd={str(self.workspace)!r})\n"
            "assert running.status == 'RUNNING'\n"
            f"heartbeat = pathlib.Path({str(heartbeat)!r})\n"
            "deadline = time.monotonic() + 2\n"
            "while not heartbeat.exists() and time.monotonic() < deadline: time.sleep(0.005)\n"
            "assert heartbeat.exists()\n"
            "os._exit(0)\n"
        )
        self.ledger.close()
        child = subprocess.run(
            [sys.executable, "-c", child_script],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=5,
            check=False,
        )
        self.assertEqual(child.returncode, 0, child.stderr)
        self._wait_for_quiet_heartbeat(heartbeat)

        ledger_module, _ = support_api()
        self.ledger = ledger_module.SwitchLedger(ledger_path)
        self.coordinator = coordinator_api().SafeSwitchCoordinator(
            self.ledger,
            snapshot_store=snapshot_store,
        )
        running_attempt = self.ledger.get_attempt("attempt-opencode")
        self.assertEqual(running_attempt.status, "RUNNING")
        self.assertIsNotNone(running_attempt.worker_identity)
        recovered = self.coordinator.recover_orphaned_running_attempt(
            running_attempt.attempt_id,
            supervisor=process_api.ProcessSupervisor(),
            effects_reconciled=True,
            lease_released=True,
        )
        self.assertEqual(recovered.status, "ABANDONED")
        self.assertTrue(recovered.process_quiescent)
        self.assertTrue(recovered.effects_reconciled)
        self.assertTrue(recovered.lease_released)
        self.assertIsNone(self.ledger.get_task("task-coordinator").active_attempt_id)

        retry, worker = self.coordinator.start_replacement(
            "task-coordinator",
            expected_version=6,
            attempt_id="attempt-opencode-retry",
            supervisor=process_api.ProcessSupervisor(),
            command=[sys.executable, "-c", "raise SystemExit(0)"],
            cwd=self.workspace,
        )
        self.assertEqual(retry.lease_epoch, 3)
        self.assertEqual(worker.process.wait(timeout=2), 0)
        process_api.ProcessSupervisor().stop_and_confirm(worker, grace_seconds=0.1)

    def test_sender_quiescence_checkpoint_settlement_and_receiver_admission_compose(self):
        process_api = importlib.import_module(
            "implementation.spikes.agent_switching_poc.process_supervisor"
        )
        supervisor = process_api.ProcessSupervisor()
        ready = self.workspace / "sender-ready"
        sender_script = (
            "import pathlib, time\n"
            f"pathlib.Path({str(self.workspace / 'result.py')!r}).write_text('sender checkpoint\\n')\n"
            f"pathlib.Path({str(ready)!r}).write_text('ready')\n"
            "time.sleep(30)\n"
        )
        sender = supervisor.start(
            [sys.executable, "-c", sender_script],
            attempt_id="attempt-codex",
            cwd=self.workspace,
        )
        deadline = time.monotonic() + 2
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.005)
        self.assertTrue(ready.exists(), "sender did not reach the controlled checkpoint")

        sender_exit = supervisor.stop_and_confirm(sender, grace_seconds=0.1)
        packet = self._packet()
        self.coordinator.settle_attempt(
            "attempt-codex",
            checkpoint_root=self.workspace,
            handoff_packet=packet,
            process_exit_evidence=sender_exit,
            effects_reconciled=True,
            lease_released=True,
        )
        observed_path = self.workspace / "handoff-observed.json"
        receiver_script = (
            "import json, pathlib\n"
            f"checkpoint = pathlib.Path({process_api.CHECKPOINT_MOUNT_PATH!r}) / 'result.py'\n"
            "checkpoint_bytes = checkpoint.read_text()\n"
            "try:\n"
            "    checkpoint.write_text('receiver overwrite\\n')\n"
            "    checkpoint_writable = True\n"
            "except OSError:\n"
            "    checkpoint_writable = False\n"
            f"pathlib.Path({str(self.workspace / 'result.py')!r}).write_text('receiver completed\\n')\n"
            f"pathlib.Path({str(self.workspace / 'receiver-result.py')!r}).write_text('receiver work\\n')\n"
            f"pathlib.Path({str(observed_path)!r}).write_text(json.dumps({{'checkpoint': checkpoint_bytes, 'writable': checkpoint_writable}}))\n"
        )
        replacement, receiver = self.coordinator.start_replacement(
            "task-coordinator",
            expected_version=3,
            attempt_id="attempt-opencode",
            supervisor=supervisor,
            command=[sys.executable, "-c", receiver_script],
            cwd=self.workspace,
        )
        snapshot = self.coordinator.snapshot_path_for_digest(
            replacement.handoff_packet["checkpoint"]["manifest_digest"]
        )

        self.assertEqual(receiver.process.wait(timeout=2), 0)
        supervisor.stop_and_confirm(receiver, grace_seconds=0.1)
        observed = json.loads(observed_path.read_text())
        self.assertTrue(sender_exit.quiescent)
        self.assertEqual(observed, {"checkpoint": "sender checkpoint\n", "writable": False})
        self.assertEqual((self.workspace / "result.py").read_text(), "receiver completed\n")
        self.assertEqual((snapshot / "result.py").read_text(), "sender checkpoint\n")
        self.assertEqual((self.workspace / "receiver-result.py").read_text(), "receiver work\n")


if __name__ == "__main__":
    unittest.main()
