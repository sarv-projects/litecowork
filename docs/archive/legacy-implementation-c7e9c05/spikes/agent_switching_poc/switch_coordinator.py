"""Compose checkpoint-file verification with the disposable SP04 switch ledger.

This coordinator is a feasibility-spike component, not product orchestration code.
The sender must already be quiescent before settlement is attempted.
"""

from __future__ import annotations

import os
import re
import shutil
import tempfile
from pathlib import Path
from typing import Any

from .handoff_integrity import CheckpointIntegrityError, verify_checkpoint_manifest
from .process_supervisor import ProcessExitEvidence, ProcessSupervisor
from .switch_ledger import AttemptView, SwitchBlocked, SwitchLedger


class SafeSwitchCoordinator:
    """Require the on-disk checkpoint to match at settle and receiver admission."""

    def __init__(self, ledger: SwitchLedger, *, snapshot_store: str | Path):
        self._ledger = ledger
        self._snapshot_store = Path(snapshot_store)
        self._snapshot_store.mkdir(parents=True, exist_ok=True)
        self._snapshot_store.chmod(0o700)

    def settle_attempt(
        self,
        attempt_id: str,
        *,
        checkpoint_root: str | Path,
        handoff_packet: dict[str, Any],
        process_exit_evidence: ProcessExitEvidence,
        effects_reconciled: bool,
        lease_released: bool,
    ) -> AttemptView:
        checkpoint = self._checkpoint_from_packet(handoff_packet)
        try:
            digest = self._materialize_snapshot(checkpoint_root, checkpoint)
        except CheckpointIntegrityError as error:
            raise SwitchBlocked(f"checkpoint verification failed: {error}") from error

        return self._ledger.settle_attempt(
            attempt_id,
            process_exit_evidence=process_exit_evidence,
            effects_reconciled=effects_reconciled,
            lease_released=lease_released,
            checkpoint_digest=digest,
            handoff_packet=handoff_packet,
        )

    def admit_replacement(
        self,
        task_id: str,
        *,
        expected_version: int,
        attempt_id: str,
    ) -> AttemptView:
        task = self._ledger.get_task(task_id)
        if task.handoff_packet is None:
            raise SwitchBlocked("Task has no persisted handoff packet")
        checkpoint = self._checkpoint_from_packet(task.handoff_packet)
        try:
            digest = verify_checkpoint_manifest(
                self.snapshot_path_for_digest(
                    self._manifest_digest(checkpoint)
                ),
                checkpoint,
            )
        except CheckpointIntegrityError as error:
            raise SwitchBlocked(f"pinned checkpoint snapshot verification failed: {error}") from error
        if digest != self._manifest_digest(checkpoint):
            raise SwitchBlocked("pinned checkpoint snapshot digest changed")

        return self._ledger.admit_replacement(
            task_id,
            expected_version,
            attempt_id,
            launch_owner_identity=ProcessSupervisor.current_process_identity(),
        )

    def recover_orphaned_admission(
        self,
        attempt_id: str,
        *,
        supervisor: object,
    ) -> AttemptView:
        """Release an admitted start only after its Linux launch owner is proven gone."""
        attempt = self._ledger.get_attempt(attempt_id)
        if attempt.status != "ADMITTED":
            raise SwitchBlocked("only an unresolved admitted Attempt can be recovered")
        identity = attempt.launch_owner_identity
        if identity is None:
            raise SwitchBlocked("admitted Attempt has no persisted launch-owner identity")
        try:
            owner_alive = supervisor.is_process_identity_alive(identity)
        except (OSError, RuntimeError) as error:
            raise SwitchBlocked("launch-owner identity could not be verified") from error
        if owner_alive:
            raise SwitchBlocked("launch owner is still alive; replacement startup is unresolved")
        return self._ledger.fail_attempt_start(
            attempt_id,
            failure_kind="LAUNCH_OWNER_EXITED_BEFORE_START_COMMIT",
        )

    def recover_orphaned_running_attempt(
        self,
        attempt_id: str,
        *,
        supervisor: object,
        effects_reconciled: bool,
        lease_released: bool,
    ) -> AttemptView:
        """Abandon a worker only after both its Runtime and contained process are gone."""
        attempt = self._ledger.get_attempt(attempt_id)
        if attempt.status != "RUNNING":
            raise SwitchBlocked("only an unresolved RUNNING Attempt can be recovered")
        if attempt.launch_owner_identity is None or attempt.worker_identity is None:
            raise SwitchBlocked("Attempt is missing verified Runtime or worker identity")
        try:
            owner_alive = supervisor.is_process_identity_alive(attempt.launch_owner_identity)
            worker_alive = supervisor.is_process_identity_alive(attempt.worker_identity)
        except (OSError, RuntimeError) as error:
            raise SwitchBlocked("Runtime or worker identity could not be verified") from error
        if owner_alive:
            raise SwitchBlocked("old Runtime is still alive; Attempt recovery is unresolved")
        if worker_alive:
            raise SwitchBlocked("worker process is still alive; Attempt recovery is unresolved")
        return self._ledger.abandon_orphaned_running_attempt(
            attempt_id,
            effects_reconciled=effects_reconciled,
            lease_released=lease_released,
        )

    def start_replacement(
        self,
        task_id: str,
        *,
        expected_version: int,
        attempt_id: str,
        supervisor: object,
        command: list[str],
        cwd: str | Path,
        stdout_path: str | Path | None = None,
        stderr_path: str | Path | None = None,
    ) -> tuple[AttemptView, object]:
        """Admit, launch, then mark a replacement running; release failed launches."""
        admitted = self.admit_replacement(
            task_id,
            expected_version=expected_version,
            attempt_id=attempt_id,
        )
        checkpoint = self._checkpoint_from_packet(admitted.handoff_packet)
        snapshot = self.snapshot_path_for_digest(self._manifest_digest(checkpoint))
        try:
            worker = supervisor.start(
                command,
                attempt_id=attempt_id,
                cwd=cwd,
                read_only_checkpoint=snapshot,
                checkpoint_manifest=checkpoint,
                stdout_path=stdout_path,
                stderr_path=stderr_path,
            )
        except BaseException as error:
            self._ledger.fail_attempt_start(
                attempt_id,
                failure_kind=type(error).__name__,
            )
            raise
        try:
            running = self._ledger.mark_attempt_running(
                attempt_id,
                worker_identity=getattr(worker, "process_identity", None),
            )
        except BaseException:
            # The worker exists, so don't release the Task admission until it is stopped.
            try:
                supervisor.stop_and_confirm(worker)
            finally:
                raise
        return running, worker

    def snapshot_path_for_digest(self, digest: str) -> Path:
        """Return the persisted, read-only checkpoint snapshot selected by manifest digest."""
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise CheckpointIntegrityError("checkpoint snapshot ID must be lowercase SHA-256")
        return self._snapshot_store / digest

    def _materialize_snapshot(self, source_root: str | Path, manifest: object) -> str:
        digest = verify_checkpoint_manifest(source_root, manifest)
        destination = self.snapshot_path_for_digest(digest)
        if destination.exists():
            verify_checkpoint_manifest(destination, manifest)
            return digest

        staging = Path(tempfile.mkdtemp(prefix=".checkpoint-", dir=self._snapshot_store))
        try:
            files = manifest["files"]
            for relative_path in sorted(files):
                source = Path(source_root).joinpath(*Path(relative_path).parts)
                target = staging.joinpath(*Path(relative_path).parts)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target, follow_symlinks=False)
            verify_checkpoint_manifest(staging, manifest)
            self._seal_read_only(staging)
            try:
                os.rename(staging, destination)
            except FileExistsError:
                verify_checkpoint_manifest(destination, manifest)
            return digest
        except BaseException:
            if staging.exists():
                self._remove_staging(staging)
            raise

    @staticmethod
    def _seal_read_only(root: Path) -> None:
        for path in sorted(root.rglob("*"), key=lambda item: len(item.parts), reverse=True):
            path.chmod(0o555 if path.is_dir() else 0o444)
        root.chmod(0o555)

    @staticmethod
    def _remove_staging(root: Path) -> None:
        for path in root.rglob("*"):
            if path.is_dir():
                path.chmod(0o700)
            else:
                path.chmod(0o600)
        root.chmod(0o700)
        shutil.rmtree(root)

    @staticmethod
    def _checkpoint_from_packet(packet: object) -> object:
        if not isinstance(packet, dict):
            raise SwitchBlocked("handoff packet must be an object")
        checkpoint = packet.get("checkpoint")
        if not isinstance(checkpoint, dict):
            raise SwitchBlocked("handoff packet is missing its checkpoint manifest")
        return checkpoint

    @staticmethod
    def _manifest_digest(manifest: object) -> str:
        if not isinstance(manifest, dict):
            raise CheckpointIntegrityError("checkpoint manifest is malformed")
        digest = manifest.get("manifest_digest")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise CheckpointIntegrityError("checkpoint manifest digest must be lowercase SHA-256")
        return digest
