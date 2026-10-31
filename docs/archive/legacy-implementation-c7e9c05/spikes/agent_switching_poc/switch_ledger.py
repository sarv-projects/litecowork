"""Disposable local ledger for the SP04 safe-switch feasibility spike.

This module is experimental and is not LiteCowork product storage. Product code must use
the owning TaskService, LeaseCoordinator, EffectReconciler, and storage contracts.
"""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .process_supervisor import ProcessExitEvidence, ProcessIdentity


class SwitchError(RuntimeError):
    """Base class for rejected PoC switch operations."""


class StaleTaskVersion(SwitchError):
    """The caller's expected Task version is no longer current."""


class SwitchBlocked(SwitchError):
    """A safety precondition for switching is not satisfied."""


@dataclass(frozen=True)
class TaskView:
    task_id: str
    objective: str
    current_lead_binding: str
    version: int
    active_attempt_id: str | None
    handoff_packet: dict[str, Any] | None


@dataclass(frozen=True)
class AttemptView:
    attempt_id: str
    task_id: str
    agent_binding: str
    lease_epoch: int
    status: str
    process_quiescent: bool
    effects_reconciled: bool
    lease_released: bool
    checkpoint_digest: str | None
    handoff_packet: dict[str, Any] | None
    launch_owner_identity: ProcessIdentity | None
    worker_identity: ProcessIdentity | None


class SwitchLedger:
    """Small SQLite-backed Task/Attempt journal used only by SP04."""

    def __init__(self, database_path: str | Path):
        self._database_path = Path(database_path)
        self._database_path.parent.mkdir(parents=True, exist_ok=True)
        self._connection = sqlite3.connect(
            self._database_path,
            timeout=5.0,
            isolation_level=None,
        )
        self._connection.row_factory = sqlite3.Row
        self._connection.execute("PRAGMA foreign_keys = ON")
        self._connection.execute("PRAGMA journal_mode = WAL")
        self._connection.execute("PRAGMA synchronous = FULL")
        self._connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                objective TEXT NOT NULL,
                current_lead_binding TEXT NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0),
                next_lease_epoch INTEGER NOT NULL CHECK(next_lease_epoch > 0),
                active_attempt_id TEXT,
                handoff_packet_json TEXT,
                FOREIGN KEY(task_id, active_attempt_id)
                    REFERENCES attempts(task_id, attempt_id)
                    DEFERRABLE INITIALLY DEFERRED
            );

            CREATE TABLE IF NOT EXISTS attempts (
                attempt_id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(task_id),
                agent_binding TEXT NOT NULL,
                lease_epoch INTEGER NOT NULL CHECK(lease_epoch > 0),
                status TEXT NOT NULL CHECK(status IN ('ADMITTED', 'RUNNING', 'SETTLED', 'START_FAILED', 'ABANDONED')),
                process_quiescent INTEGER NOT NULL DEFAULT 0 CHECK(process_quiescent IN (0, 1)),
                effects_reconciled INTEGER NOT NULL DEFAULT 0 CHECK(effects_reconciled IN (0, 1)),
                lease_released INTEGER NOT NULL DEFAULT 0 CHECK(lease_released IN (0, 1)),
                checkpoint_digest TEXT,
                handoff_packet_json TEXT,
                start_failure_kind TEXT,
                launch_owner_pid INTEGER,
                launch_owner_start_time_ticks INTEGER,
                launch_owner_boot_id TEXT,
                worker_pid INTEGER,
                worker_start_time_ticks INTEGER,
                worker_boot_id TEXT,
                CHECK (
                    (launch_owner_pid IS NULL AND launch_owner_start_time_ticks IS NULL
                        AND launch_owner_boot_id IS NULL)
                    OR
                    (launch_owner_pid > 0 AND launch_owner_start_time_ticks >= 0
                        AND length(launch_owner_boot_id) > 0)
                ),
                CHECK (
                    (worker_pid IS NULL AND worker_start_time_ticks IS NULL
                        AND worker_boot_id IS NULL)
                    OR
                    (worker_pid > 0 AND worker_start_time_ticks >= 0
                        AND length(worker_boot_id) > 0)
                ),
                UNIQUE(task_id, lease_epoch),
                UNIQUE(task_id, attempt_id)
            );
            """
        )

    def close(self) -> None:
        self._connection.close()

    def create_task(self, task_id: str, objective: str, lead_binding: str) -> TaskView:
        with self._transaction():
            self._connection.execute(
                """INSERT INTO tasks
                   (task_id, objective, current_lead_binding, version, next_lease_epoch)
                   VALUES (?, ?, ?, 1, 1)""",
                (task_id, objective, lead_binding),
            )
        return self.get_task(task_id)

    def start_attempt(self, task_id: str, attempt_id: str, agent_binding: str) -> AttemptView:
        with self._transaction():
            task = self._require_task(task_id)
            if task["active_attempt_id"] is not None:
                raise SwitchBlocked("Task already has an active Attempt")
            if agent_binding != task["current_lead_binding"]:
                raise SwitchBlocked("initial Attempt must use the current lead binding")
            epoch = task["next_lease_epoch"]
            self._connection.execute(
                """INSERT INTO attempts
                   (attempt_id, task_id, agent_binding, lease_epoch, status)
                   VALUES (?, ?, ?, ?, 'RUNNING')""",
                (attempt_id, task_id, agent_binding, epoch),
            )
            self._connection.execute(
                """UPDATE tasks
                   SET active_attempt_id = ?, next_lease_epoch = ?
                   WHERE task_id = ?""",
                (attempt_id, epoch + 1, task_id),
            )
        return self.get_attempt(attempt_id)

    def request_lead_switch(
        self,
        task_id: str,
        expected_version: int,
        new_lead_binding: str,
        handoff_packet: dict[str, Any],
    ) -> TaskView:
        packet_json = self._encode_handoff_packet(handoff_packet)
        with self._transaction():
            task = self._require_task(task_id)
            self._check_version(task, expected_version)
            if not new_lead_binding.strip():
                raise SwitchBlocked("new lead binding must not be empty")
            self._connection.execute(
                """UPDATE tasks
                   SET current_lead_binding = ?, handoff_packet_json = ?, version = version + 1
                   WHERE task_id = ?""",
                (new_lead_binding, packet_json, task_id),
            )
        return self.get_task(task_id)

    def settle_attempt(
        self,
        attempt_id: str,
        *,
        process_exit_evidence: ProcessExitEvidence,
        effects_reconciled: bool,
        lease_released: bool,
        checkpoint_digest: str,
        handoff_packet: dict[str, Any],
    ) -> AttemptView:
        packet_json = self._encode_handoff_packet(handoff_packet)
        if not process_exit_evidence.quiescent:
            raise SwitchBlocked("process supervisor did not confirm worker quiescence")
        if not (effects_reconciled and lease_released):
            raise SwitchBlocked(
                "cannot settle until Effects are reconciled and the ExecutionLease is released"
            )
        if not checkpoint_digest.strip():
            raise SwitchBlocked("settlement requires a committed checkpoint digest")
        if self._checkpoint_manifest_digest(handoff_packet) != checkpoint_digest:
            raise SwitchBlocked("handoff packet checkpoint manifest digest does not match")

        with self._transaction():
            attempt = self._require_attempt(attempt_id)
            if process_exit_evidence.attempt_id != attempt_id:
                raise SwitchBlocked("process exit evidence belongs to a different Attempt")
            if attempt["status"] != "RUNNING":
                raise SwitchBlocked("only the active Attempt can be settled")
            task = self._require_task(attempt["task_id"])
            if task["active_attempt_id"] != attempt_id:
                raise SwitchBlocked("Attempt is not the Task's current active Attempt")
            self._connection.execute(
                """UPDATE attempts
                   SET status = 'SETTLED', process_quiescent = 1,
                       effects_reconciled = 1, lease_released = 1,
                       checkpoint_digest = ?, handoff_packet_json = ?
                   WHERE attempt_id = ?""",
                (checkpoint_digest, packet_json, attempt_id),
            )
            self._connection.execute(
                """UPDATE tasks
                   SET active_attempt_id = NULL, handoff_packet_json = ?, version = version + 1
                   WHERE task_id = ?""",
                (packet_json, attempt["task_id"]),
            )
        return self.get_attempt(attempt_id)

    def admit_replacement(
        self,
        task_id: str,
        expected_version: int,
        attempt_id: str,
        *,
        launch_owner_identity: ProcessIdentity | None = None,
    ) -> AttemptView:
        with self._transaction():
            task = self._require_task(task_id)
            self._check_version(task, expected_version)
            if task["active_attempt_id"] is not None:
                raise SwitchBlocked("prior Attempt still owns the Task")
            prior = self._connection.execute(
                """SELECT * FROM attempts WHERE task_id = ?
                   ORDER BY lease_epoch DESC LIMIT 1""",
                (task_id,),
            ).fetchone()
            if prior is None or prior["status"] not in ("SETTLED", "START_FAILED", "ABANDONED"):
                raise SwitchBlocked("prior Attempt has not settled or failed before process start")
            prior_packet = self._decode_packet(prior["handoff_packet_json"])
            task_packet = self._decode_packet(task["handoff_packet_json"])
            if prior_packet is None or task_packet is None:
                raise SwitchBlocked("replacement handoff packet is missing")
            if prior["status"] in ("SETTLED", "ABANDONED"):
                if not all(
                    prior[field]
                    for field in ("process_quiescent", "effects_reconciled", "lease_released")
                ):
                    raise SwitchBlocked("prior Attempt lacks safe settlement evidence")
                expected_digest = prior["checkpoint_digest"] or self._checkpoint_manifest_digest(prior_packet)
            else:
                expected_digest = self._checkpoint_manifest_digest(prior_packet)
            if (
                self._checkpoint_manifest_digest(prior_packet) != expected_digest
                or self._checkpoint_manifest_digest(task_packet) != expected_digest
            ):
                raise SwitchBlocked("replacement handoff packet does not match pinned checkpoint")
            if prior["status"] == "SETTLED" and prior["agent_binding"] == task["current_lead_binding"]:
                raise SwitchBlocked("replacement must use a different lead binding")

            epoch = task["next_lease_epoch"]
            self._connection.execute(
                """INSERT INTO attempts
                   (attempt_id, task_id, agent_binding, lease_epoch, status, handoff_packet_json)
                   VALUES (?, ?, ?, ?, 'ADMITTED', ?)""",
                (
                    attempt_id,
                    task_id,
                    task["current_lead_binding"],
                    epoch,
                    task["handoff_packet_json"],
                ),
            )
            self._connection.execute(
                """UPDATE tasks
                   SET active_attempt_id = ?, next_lease_epoch = ?, version = version + 1
                   WHERE task_id = ?""",
                (attempt_id, epoch + 1, task_id),
            )
            if launch_owner_identity is not None:
                self._connection.execute(
                    """UPDATE attempts SET launch_owner_pid = ?,
                           launch_owner_start_time_ticks = ?, launch_owner_boot_id = ?
                       WHERE attempt_id = ?""",
                    (
                        launch_owner_identity.pid,
                        launch_owner_identity.start_time_ticks,
                        launch_owner_identity.boot_id,
                        attempt_id,
                    ),
                )
        return self.get_attempt(attempt_id)

    def mark_attempt_running(
        self,
        attempt_id: str,
        *,
        worker_identity: ProcessIdentity | None = None,
    ) -> AttemptView:
        """Record that an admitted replacement process was successfully launched."""
        with self._transaction():
            attempt = self._require_attempt(attempt_id)
            task = self._require_task(attempt["task_id"])
            if attempt["status"] != "ADMITTED" or task["active_attempt_id"] != attempt_id:
                raise SwitchBlocked("only the admitted active Attempt can start")
            self._connection.execute(
                """UPDATE attempts SET status = 'RUNNING', worker_pid = ?,
                       worker_start_time_ticks = ?, worker_boot_id = ?
                   WHERE attempt_id = ?""",
                (
                    worker_identity.pid if worker_identity else None,
                    worker_identity.start_time_ticks if worker_identity else None,
                    worker_identity.boot_id if worker_identity else None,
                    attempt_id,
                ),
            )
            self._connection.execute(
                "UPDATE tasks SET version = version + 1 WHERE task_id = ?",
                (attempt["task_id"],),
            )
        return self.get_attempt(attempt_id)

    def abandon_orphaned_running_attempt(
        self,
        attempt_id: str,
        *,
        effects_reconciled: bool,
        lease_released: bool,
    ) -> AttemptView:
        """Release a Runtime-lost worker after coordinator-verified process quiescence."""
        if not (effects_reconciled and lease_released):
            raise SwitchBlocked("cannot abandon until Effects are reconciled and lease is fenced")
        with self._transaction():
            attempt = self._require_attempt(attempt_id)
            task = self._require_task(attempt["task_id"])
            if attempt["status"] != "RUNNING" or task["active_attempt_id"] != attempt_id:
                raise SwitchBlocked("only the active RUNNING Attempt can be recovered")
            packet = self._decode_packet(attempt["handoff_packet_json"])
            checkpoint_digest = self._checkpoint_manifest_digest(packet or {})
            if not checkpoint_digest:
                raise SwitchBlocked("orphaned Attempt has no pinned checkpoint")
            self._connection.execute(
                """UPDATE attempts
                   SET status = 'ABANDONED', process_quiescent = 1,
                       effects_reconciled = 1, lease_released = 1,
                       checkpoint_digest = ?
                   WHERE attempt_id = ?""",
                (checkpoint_digest, attempt_id),
            )
            self._connection.execute(
                """UPDATE tasks SET active_attempt_id = NULL, version = version + 1
                   WHERE task_id = ?""",
                (attempt["task_id"],),
            )
        return self.get_attempt(attempt_id)

    def fail_attempt_start(self, attempt_id: str, *, failure_kind: str) -> AttemptView:
        """Release an admission whose worker process failed before it could run."""
        if not failure_kind.strip():
            raise SwitchBlocked("startup failure requires a failure kind")
        with self._transaction():
            attempt = self._require_attempt(attempt_id)
            task = self._require_task(attempt["task_id"])
            if attempt["status"] != "ADMITTED" or task["active_attempt_id"] != attempt_id:
                raise SwitchBlocked("only the admitted active Attempt can fail startup")
            self._connection.execute(
                """UPDATE attempts
                   SET status = 'START_FAILED', start_failure_kind = ?
                   WHERE attempt_id = ?""",
                (failure_kind[:120], attempt_id),
            )
            self._connection.execute(
                """UPDATE tasks SET active_attempt_id = NULL, version = version + 1
                   WHERE task_id = ?""",
                (attempt["task_id"],),
            )
        return self.get_attempt(attempt_id)

    def get_task(self, task_id: str) -> TaskView:
        row = self._require_task(task_id)
        return TaskView(
            task_id=row["task_id"],
            objective=row["objective"],
            current_lead_binding=row["current_lead_binding"],
            version=row["version"],
            active_attempt_id=row["active_attempt_id"],
            handoff_packet=self._decode_packet(row["handoff_packet_json"]),
        )

    def get_attempt(self, attempt_id: str) -> AttemptView:
        row = self._require_attempt(attempt_id)
        return AttemptView(
            attempt_id=row["attempt_id"],
            task_id=row["task_id"],
            agent_binding=row["agent_binding"],
            lease_epoch=row["lease_epoch"],
            status=row["status"],
            process_quiescent=bool(row["process_quiescent"]),
            effects_reconciled=bool(row["effects_reconciled"]),
            lease_released=bool(row["lease_released"]),
            checkpoint_digest=row["checkpoint_digest"],
            handoff_packet=self._decode_packet(row["handoff_packet_json"]),
            launch_owner_identity=(
                ProcessIdentity(
                    pid=row["launch_owner_pid"],
                    start_time_ticks=row["launch_owner_start_time_ticks"],
                    boot_id=row["launch_owner_boot_id"],
                )
                if row["launch_owner_pid"] is not None
                else None
            ),
            worker_identity=(
                ProcessIdentity(
                    pid=row["worker_pid"],
                    start_time_ticks=row["worker_start_time_ticks"],
                    boot_id=row["worker_boot_id"],
                )
                if row["worker_pid"] is not None
                else None
            ),
        )

    class _Transaction:
        def __init__(self, connection: sqlite3.Connection):
            self.connection = connection

        def __enter__(self) -> None:
            self.connection.execute("BEGIN IMMEDIATE")

        def __exit__(self, exception_type, exception, traceback) -> bool:
            self.connection.execute("ROLLBACK" if exception_type else "COMMIT")
            return False

    def _transaction(self) -> SwitchLedger._Transaction:
        return self._Transaction(self._connection)

    def _require_task(self, task_id: str) -> sqlite3.Row:
        row = self._connection.execute(
            "SELECT * FROM tasks WHERE task_id = ?", (task_id,)
        ).fetchone()
        if row is None:
            raise SwitchError(f"unknown Task: {task_id}")
        return row

    def _require_attempt(self, attempt_id: str) -> sqlite3.Row:
        row = self._connection.execute(
            "SELECT * FROM attempts WHERE attempt_id = ?", (attempt_id,)
        ).fetchone()
        if row is None:
            raise SwitchError(f"unknown Attempt: {attempt_id}")
        return row

    @staticmethod
    def _check_version(task: sqlite3.Row, expected_version: int) -> None:
        if task["version"] != expected_version:
            raise StaleTaskVersion(
                f"expected Task version {expected_version}, observed {task['version']}"
            )

    @staticmethod
    def _encode_handoff_packet(packet: dict[str, Any]) -> str:
        if not isinstance(packet, dict) or not packet:
            raise SwitchBlocked("handoff packet must be a non-empty object")
        return json.dumps(packet, sort_keys=True, separators=(",", ":"))

    @staticmethod
    def _decode_packet(encoded: str | None) -> dict[str, Any] | None:
        return json.loads(encoded) if encoded is not None else None

    @staticmethod
    def _checkpoint_manifest_digest(packet: dict[str, Any]) -> str | None:
        checkpoint = packet.get("checkpoint")
        if not isinstance(checkpoint, dict):
            return None
        digest = checkpoint.get("manifest_digest")
        return digest if isinstance(digest, str) and digest else None
