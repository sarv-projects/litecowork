import tempfile
import unittest
import importlib
import time
from pathlib import Path


def switch_ledger_api():
    try:
        return importlib.import_module(
            "implementation.spikes.agent_switching_poc.switch_ledger"
        )
    except ModuleNotFoundError as error:
        if error.name == "implementation.spikes.agent_switching_poc.switch_ledger":
            raise AssertionError("PoC switch ledger has not been implemented") from error
        raise


class SwitchLedgerTests(unittest.TestCase):
    def setUp(self):
        api = switch_ledger_api()
        self.StaleTaskVersion = api.StaleTaskVersion
        self.SwitchBlocked = api.SwitchBlocked
        self.SwitchLedger = api.SwitchLedger
        self.ProcessExitEvidence = api.ProcessExitEvidence
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.database_path = Path(self.temporary_directory.name) / "task.sqlite3"
        self.ledger = self.SwitchLedger(self.database_path)
        self.ledger.create_task(
            task_id="task-poc-1",
            objective="Add a deterministic unit-tested helper.",
            lead_binding="codex-gpt-6-luna-medium",
        )
        self.ledger.start_attempt(
            task_id="task-poc-1",
            attempt_id="attempt-codex-1",
            agent_binding="codex-gpt-6-luna-medium",
        )

    def tearDown(self):
        self.ledger.close()
        self.temporary_directory.cleanup()

    def test_lead_change_does_not_rebind_running_attempt_or_admit_replacement(self):
        task = self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet={"objective": "Continue the helper task."},
        )

        self.assertEqual(task.current_lead_binding, "opencode-mimo-v2.6-flash-free")
        self.assertEqual(task.version, 2)
        self.assertEqual(task.active_attempt_id, "attempt-codex-1")

        old_attempt = self.ledger.get_attempt("attempt-codex-1")
        self.assertEqual(old_attempt.agent_binding, "codex-gpt-6-luna-medium")
        self.assertEqual(old_attempt.status, "RUNNING")
        with self.assertRaises(self.SwitchBlocked):
            self.ledger.admit_replacement(
                task_id="task-poc-1",
                expected_version=2,
                attempt_id="attempt-opencode-1",
            )

    def test_replacement_waits_for_quiescence_effect_reconciliation_and_lease_release(self):
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet={"objective": "Continue the helper task."},
        )

        stopped = self.ProcessExitEvidence(
            attempt_id="attempt-codex-1",
            process_group_id=1,
            exit_code=-15,
            quiescent=True,
            observed_at_monotonic_ns=time.monotonic_ns(),
        )
        unconfirmed = self.ProcessExitEvidence(
            attempt_id="attempt-codex-1",
            process_group_id=1,
            exit_code=-15,
            quiescent=False,
            observed_at_monotonic_ns=time.monotonic_ns(),
        )
        for process_evidence, effect_state, lease_state in (
            (unconfirmed, True, True),
            (stopped, False, True),
            (stopped, True, False),
        ):
            with self.subTest(process=process_evidence, effects=effect_state, lease=lease_state), self.assertRaises(self.SwitchBlocked):
                self.ledger.settle_attempt(
                    "attempt-codex-1",
                    process_exit_evidence=process_evidence,
                    checkpoint_digest="sha256:test-checkpoint",
                    effects_reconciled=effect_state,
                    lease_released=lease_state,
                    handoff_packet={"objective": "Final snapshot."},
                )
            self.assertEqual(self.ledger.get_attempt("attempt-codex-1").status, "RUNNING")

        with self.assertRaises(self.SwitchBlocked):
            self.ledger.settle_attempt(
                "attempt-codex-1",
                process_exit_evidence=self.ProcessExitEvidence(
                    attempt_id="different-attempt",
                    process_group_id=1,
                    exit_code=0,
                    quiescent=True,
                    observed_at_monotonic_ns=time.monotonic_ns(),
                ),
                effects_reconciled=True,
                lease_released=True,
                checkpoint_digest="sha256:test-checkpoint",
                handoff_packet={
                    "objective": "Final snapshot.",
                    "checkpoint": {"manifest_digest": "sha256:test-checkpoint"},
                },
            )

        final_packet = {
            "objective": "Continue the helper task.",
            "completed": ["Codex added and tested total_cents."],
            "unresolved": ["Review OpenCode's follow-up and rerun all tests."],
            "checkpoint": {"manifest_digest": "sha256:test-checkpoint"},
        }
        self.ledger.settle_attempt(
            "attempt-codex-1",
            process_exit_evidence=stopped,
            effects_reconciled=True,
            lease_released=True,
            checkpoint_digest="sha256:test-checkpoint",
            handoff_packet=final_packet,
        )
        replacement = self.ledger.admit_replacement(
            task_id="task-poc-1",
            expected_version=3,
            attempt_id="attempt-opencode-1",
        )

        self.assertEqual(replacement.agent_binding, "opencode-mimo-v2.6-flash-free")
        self.assertEqual(replacement.lease_epoch, 2)
        self.assertEqual(self.ledger.get_attempt("attempt-codex-1").status, "SETTLED")

    def test_settlement_rejects_checkpoint_manifest_digest_mismatch(self):
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet={"objective": "Continue the helper task."},
        )

        with self.assertRaises(self.SwitchBlocked):
            self.ledger.settle_attempt(
                "attempt-codex-1",
                process_exit_evidence=self.ProcessExitEvidence(
                    attempt_id="attempt-codex-1",
                    process_group_id=1,
                    exit_code=0,
                    quiescent=True,
                    observed_at_monotonic_ns=time.monotonic_ns(),
                ),
                effects_reconciled=True,
                lease_released=True,
                checkpoint_digest="sha256:expected-manifest",
                handoff_packet={
                    "objective": "Continue the helper task.",
                    "checkpoint": {"manifest_digest": "sha256:observed-manifest"},
                },
            )

        self.assertEqual(self.ledger.get_attempt("attempt-codex-1").status, "RUNNING")

    def test_handoff_and_attempt_epoch_survive_ledger_reopen(self):
        packet = {
            "objective": "Continue the helper task.",
            "completed": ["Codex added the pure function."],
            "unresolved": ["Review edge cases."],
            "checkpoint": {"manifest_digest": "sha256:test-checkpoint"},
        }
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet=packet,
        )
        self.ledger.settle_attempt(
            "attempt-codex-1",
            process_exit_evidence=self.ProcessExitEvidence(
                attempt_id="attempt-codex-1",
                process_group_id=1,
                exit_code=0,
                quiescent=True,
                observed_at_monotonic_ns=time.monotonic_ns(),
            ),
            effects_reconciled=True,
            lease_released=True,
            checkpoint_digest="sha256:test-checkpoint",
            handoff_packet=packet,
        )
        self.ledger.close()

        self.ledger = self.SwitchLedger(self.database_path)
        self.ledger.admit_replacement(
            task_id="task-poc-1",
            expected_version=3,
            attempt_id="attempt-opencode-1",
        )
        self.ledger.close()

        self.ledger = self.SwitchLedger(self.database_path)
        task = self.ledger.get_task("task-poc-1")
        attempt = self.ledger.get_attempt("attempt-opencode-1")
        self.assertEqual(task.handoff_packet, packet)
        self.assertEqual(task.version, 4)
        self.assertEqual(task.active_attempt_id, "attempt-opencode-1")
        self.assertEqual(attempt.lease_epoch, 2)
        self.assertEqual(attempt.status, "ADMITTED")
        self.assertEqual(self.ledger.get_attempt("attempt-codex-1").handoff_packet, packet)

    def test_failed_start_releases_task_and_allows_same_lead_to_retry(self):
        packet = {
            "objective": "Continue the helper task.",
            "checkpoint": {"manifest_digest": "sha256:retry-checkpoint"},
        }
        self.ledger.request_lead_switch(
            "task-poc-1", 1, "opencode-mimo-v2.6-flash-free", packet
        )
        self.ledger.settle_attempt(
            "attempt-codex-1",
            process_exit_evidence=self.ProcessExitEvidence(
                attempt_id="attempt-codex-1",
                process_group_id=1,
                exit_code=0,
                quiescent=True,
                observed_at_monotonic_ns=time.monotonic_ns(),
            ),
            effects_reconciled=True,
            lease_released=True,
            checkpoint_digest="sha256:retry-checkpoint",
            handoff_packet=packet,
        )
        first = self.ledger.admit_replacement("task-poc-1", 3, "attempt-opencode-1")
        self.assertEqual(first.status, "ADMITTED")
        failed = self.ledger.fail_attempt_start(
            first.attempt_id,
            failure_kind="CheckpointIntegrityError",
        )
        self.assertEqual(failed.status, "START_FAILED")
        self.assertEqual(self.ledger.get_task("task-poc-1").version, 5)
        self.assertIsNone(self.ledger.get_task("task-poc-1").active_attempt_id)

        retry = self.ledger.admit_replacement("task-poc-1", 5, "attempt-opencode-2")
        running = self.ledger.mark_attempt_running(retry.attempt_id)

        self.assertEqual(retry.lease_epoch, 3)
        self.assertEqual(running.status, "RUNNING")
        self.assertEqual(self.ledger.get_task("task-poc-1").active_attempt_id, retry.attempt_id)

    def test_restart_keeps_unresolved_admission_blocking_duplicate_worker(self):
        packet = {
            "objective": "Continue the helper task.",
            "checkpoint": {"manifest_digest": "sha256:restart-admission"},
        }
        self.ledger.request_lead_switch(
            "task-poc-1", 1, "opencode-mimo-v2.6-flash-free", packet
        )
        self.ledger.settle_attempt(
            "attempt-codex-1",
            process_exit_evidence=self.ProcessExitEvidence(
                attempt_id="attempt-codex-1",
                process_group_id=1,
                exit_code=0,
                quiescent=True,
                observed_at_monotonic_ns=time.monotonic_ns(),
            ),
            effects_reconciled=True,
            lease_released=True,
            checkpoint_digest="sha256:restart-admission",
            handoff_packet=packet,
        )
        admitted = self.ledger.admit_replacement("task-poc-1", 3, "attempt-opencode-1")
        self.ledger.close()
        self.ledger = self.SwitchLedger(self.database_path)

        task = self.ledger.get_task("task-poc-1")
        self.assertEqual(admitted.status, "ADMITTED")
        self.assertEqual(self.ledger.get_attempt(admitted.attempt_id).status, "ADMITTED")
        self.assertEqual(task.active_attempt_id, admitted.attempt_id)
        with self.assertRaises(self.SwitchBlocked):
            self.ledger.admit_replacement("task-poc-1", task.version, "attempt-duplicate")

        attempt_count = self.ledger._connection.execute(
            "SELECT COUNT(*) FROM attempts WHERE task_id = ?", ("task-poc-1",)
        ).fetchone()[0]
        self.assertEqual(attempt_count, 2)

    def test_replacement_rejects_task_packet_changed_after_settlement(self):
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet={"objective": "Continue the helper task."},
        )
        packet = {
            "objective": "Continue the helper task.",
            "checkpoint": {"manifest_digest": "sha256:committed"},
        }
        self.ledger.settle_attempt(
            "attempt-codex-1",
            process_exit_evidence=self.ProcessExitEvidence(
                attempt_id="attempt-codex-1",
                process_group_id=1,
                exit_code=0,
                quiescent=True,
                observed_at_monotonic_ns=time.monotonic_ns(),
            ),
            effects_reconciled=True,
            lease_released=True,
            checkpoint_digest="sha256:committed",
            handoff_packet=packet,
        )
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=3,
            new_lead_binding="opencode-alternate",
            handoff_packet={
                "objective": "Changed packet after settlement.",
                "checkpoint": {"manifest_digest": "sha256:changed"},
            },
        )

        with self.assertRaises(self.SwitchBlocked):
            self.ledger.admit_replacement(
                task_id="task-poc-1",
                expected_version=4,
                attempt_id="attempt-opencode-1",
            )

    def test_lead_switch_rejects_missing_or_malformed_packet(self):
        for packet in (None, {}, [], "bad"):
            with self.subTest(packet=packet), self.assertRaises(self.SwitchBlocked):
                self.ledger.request_lead_switch(
                    task_id="task-poc-1",
                    expected_version=1,
                    new_lead_binding="opencode-mimo-v2.6-flash-free",
                    handoff_packet=packet,
                )

    def test_quiescent_crashed_sender_can_be_replaced_after_reconciliation(self):
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet={"objective": "Continue the helper task."},
        )
        packet = {
            "objective": "Continue the helper task.",
            "checkpoint": {"manifest_digest": "sha256:crash-checkpoint"},
        }
        self.ledger.settle_attempt(
            "attempt-codex-1",
            process_exit_evidence=self.ProcessExitEvidence(
                attempt_id="attempt-codex-1",
                process_group_id=1,
                exit_code=-9,
                quiescent=True,
                observed_at_monotonic_ns=time.monotonic_ns(),
            ),
            effects_reconciled=True,
            lease_released=True,
            checkpoint_digest="sha256:crash-checkpoint",
            handoff_packet=packet,
        )

        replacement = self.ledger.admit_replacement(
            task_id="task-poc-1",
            expected_version=3,
            attempt_id="attempt-opencode-1",
        )

        self.assertEqual(replacement.agent_binding, "opencode-mimo-v2.6-flash-free")
        self.assertEqual(replacement.lease_epoch, 2)

    def test_stale_lead_switch_version_is_rejected(self):
        self.ledger.request_lead_switch(
            task_id="task-poc-1",
            expected_version=1,
            new_lead_binding="opencode-mimo-v2.6-flash-free",
            handoff_packet={"objective": "Continue the helper task."},
        )
        with self.assertRaises(self.StaleTaskVersion):
            self.ledger.request_lead_switch(
                task_id="task-poc-1",
                expected_version=1,
                new_lead_binding="codex-gpt-6-luna-medium",
                handoff_packet={"objective": "Stale request."},
            )


if __name__ == "__main__":
    unittest.main()
