"""Linux PID-namespace process control used only by the SP04 prototype."""

from __future__ import annotations

import ctypes
import fcntl
import hashlib
import os
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from pathlib import PurePosixPath

from .handoff_integrity import CheckpointIntegrityError, verify_checkpoint_manifest

CHECKPOINT_MOUNT_PATH = "/tmp/litecowork/checkpoint"
MAX_CHECKPOINT_FILE_COUNT = 4096
MAX_CHECKPOINT_TOTAL_BYTES = 512 * 1024 * 1024


class ProcessQuiescenceError(RuntimeError):
    """The worker process group could not be proven stopped."""


class ProcessIdentityError(RuntimeError):
    """The supervisor cannot establish a trustworthy Linux process identity."""


@dataclass(frozen=True)
class ProcessIdentity:
    """Linux process identity resistant to PID reuse and machine reboot."""

    pid: int
    start_time_ticks: int
    boot_id: str


@dataclass
class ManagedWorker:
    attempt_id: str
    process: subprocess.Popen
    process_group_id: int
    process_identity: ProcessIdentity | None


@dataclass(frozen=True)
class ProcessExitEvidence:
    attempt_id: str
    process_group_id: int
    exit_code: int
    quiescent: bool
    observed_at_monotonic_ns: int


class ProcessSupervisor:
    """Contain fixture processes; this read-only-root wrapper is not a CLI adapter.

    Native harnesses may require writable private state outside their project. Their
    App Server/session process must be separated from the writable worker Environment
    instead of weakening this fixture's filesystem boundary.
    """

    @staticmethod
    def current_process_identity() -> ProcessIdentity:
        if not sys.platform.startswith("linux"):
            raise RuntimeError("SP04 process identity checks require Linux /proc")
        return ProcessSupervisor._read_process_identity(os.getpid())

    @staticmethod
    def is_process_identity_alive(identity: ProcessIdentity) -> bool:
        if not sys.platform.startswith("linux"):
            raise RuntimeError("SP04 process identity checks require Linux /proc")
        try:
            current_boot_id = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
        except OSError as error:
            raise ProcessIdentityError("cannot verify Linux boot identity") from error
        if current_boot_id != identity.boot_id:
            return False
        try:
            observed = ProcessSupervisor._read_process_identity(identity.pid)
        except ProcessLookupError:
            return False
        return observed == identity

    @staticmethod
    def _read_process_identity(pid: int) -> ProcessIdentity:
        try:
            stat = Path(f"/proc/{pid}/stat").read_text()
        except FileNotFoundError as error:
            raise ProcessLookupError(pid) from error
        except OSError as error:
            raise ProcessIdentityError(f"cannot read process identity for PID {pid}") from error
        try:
            boot_id = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
        except OSError as error:
            raise ProcessIdentityError("cannot read Linux boot identity") from error
        closing_parenthesis = stat.rfind(")")
        if closing_parenthesis < 0:
            raise ProcessIdentityError("malformed /proc process stat")
        fields_after_comm = stat[closing_parenthesis + 1 :].split()
        if fields_after_comm and fields_after_comm[0] in {"Z", "X"}:
            raise ProcessLookupError(pid)
        try:
            start_time_ticks = int(fields_after_comm[19])
        except (IndexError, ValueError) as error:
            raise ProcessIdentityError("missing process start time in /proc stat") from error
        return ProcessIdentity(pid, start_time_ticks, boot_id)

    def start(
        self,
        command: list[str],
        *,
        attempt_id: str,
        cwd: str | Path,
        read_only_checkpoint: str | Path | None = None,
        checkpoint_manifest: object | None = None,
        stdout_path: str | Path | None = None,
        stderr_path: str | Path | None = None,
    ) -> ManagedWorker:
        if not sys.platform.startswith("linux"):
            raise RuntimeError("SP04 requires Linux PID-namespace containment")
        bwrap = shutil.which("bwrap")
        if bwrap is None:
            raise RuntimeError("SP04 refuses worker start because bubblewrap is unavailable")
        working_directory = Path(cwd).resolve()
        checkpoint_directory = (
            Path(read_only_checkpoint).resolve(strict=True)
            if read_only_checkpoint is not None
            else None
        )
        if (checkpoint_directory is None) != (checkpoint_manifest is None):
            raise ValueError("read_only_checkpoint and checkpoint_manifest must be provided together")
        if checkpoint_directory is not None and not checkpoint_directory.is_dir():
            raise ValueError("read_only_checkpoint must be a directory")
        contained_command = [
            bwrap,
            "--unshare-pid",
            "--die-with-parent",
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--tmpfs",
            "/tmp",
            "--bind",
            str(working_directory),
            str(working_directory),
        ]
        checkpoint_fds: list[int] = []
        if checkpoint_directory is not None:
            assert checkpoint_manifest is not None
            try:
                checkpoint_fds = self._sealed_checkpoint_files(
                    checkpoint_directory,
                    checkpoint_manifest,
                )
                contained_command.extend(
                    self._checkpoint_mount_args(checkpoint_manifest, checkpoint_fds)
                )
            except BaseException:
                self._close_fds(checkpoint_fds)
                raise
        contained_command.extend(["--chdir", str(working_directory), "--", *command])
        stdout_stream = Path(stdout_path).open("wb") if stdout_path else subprocess.DEVNULL
        stderr_stream = Path(stderr_path).open("wb") if stderr_path else subprocess.STDOUT
        child_environment = os.environ.copy()
        child_environment["PWD"] = str(working_directory)
        try:
            process = subprocess.Popen(
                contained_command,
                cwd=working_directory,
                env=child_environment,
                stdin=subprocess.DEVNULL,
                stdout=stdout_stream,
                stderr=stderr_stream,
                close_fds=True,
                pass_fds=tuple(checkpoint_fds),
                start_new_session=True,
            )
        finally:
            self._close_fds(checkpoint_fds)
            if stdout_path:
                stdout_stream.close()
            if stderr_path:
                stderr_stream.close()
        try:
            process_identity = self._read_process_identity(process.pid)
        except (ProcessIdentityError, ProcessLookupError):
            # If sampling races exit or /proc is unreadable, keep the live Attempt but make
            # restart recovery fail closed; never let the coordinator mistake this for a
            # failed process start and release its Task slot.
            process_identity = None
        return ManagedWorker(
            attempt_id=attempt_id,
            process=process,
            process_group_id=process.pid,
            process_identity=process_identity,
        )

    @staticmethod
    def _sealed_checkpoint_files(checkpoint_directory: Path, manifest: object) -> list[int]:
        verify_checkpoint_manifest(checkpoint_directory, manifest)
        if not isinstance(manifest, dict):
            raise CheckpointIntegrityError("checkpoint manifest is malformed")
        expected_files = manifest["files"]
        if not isinstance(expected_files, dict):
            raise CheckpointIntegrityError("checkpoint manifest file list is malformed")
        if len(expected_files) > MAX_CHECKPOINT_FILE_COUNT:
            raise CheckpointIntegrityError("checkpoint exceeds the file-count limit")

        sealed_fds: list[int] = []
        try:
            total_bytes = 0
            for relative_path, expected_digest in sorted(expected_files.items()):
                source = checkpoint_directory.joinpath(*PurePosixPath(relative_path).parts)
                descriptor, file_bytes = ProcessSupervisor._create_sealed_memfd(
                    relative_path,
                    source,
                    expected_digest,
                    MAX_CHECKPOINT_TOTAL_BYTES - total_bytes,
                )
                total_bytes += file_bytes
                sealed_fds.append(descriptor)
            return sealed_fds
        except BaseException:
            ProcessSupervisor._close_fds(sealed_fds)
            raise

    @staticmethod
    def _create_sealed_memfd(
        name: str,
        source: Path,
        expected_digest: str,
        remaining_budget: int,
    ) -> tuple[int, int]:
        libc = ctypes.CDLL(None, use_errno=True)
        memfd_create = libc.memfd_create
        memfd_create.argtypes = [ctypes.c_char_p, ctypes.c_uint]
        memfd_create.restype = ctypes.c_int
        descriptor = memfd_create(name.encode("utf-8"), 0x0001 | 0x0002)
        if descriptor < 0:
            error_number = ctypes.get_errno()
            raise OSError(error_number, os.strerror(error_number), "memfd_create")

        try:
            digest = hashlib.sha256()
            file_bytes = 0
            with source.open("rb") as checkpoint_file:
                while chunk := checkpoint_file.read(1024 * 1024):
                    file_bytes += len(chunk)
                    if file_bytes > remaining_budget:
                        raise CheckpointIntegrityError(
                            "checkpoint exceeds the 512 MiB receiver snapshot limit"
                        )
                    digest.update(chunk)
                    remaining = memoryview(chunk)
                    while remaining:
                        written = os.write(descriptor, remaining)
                        if written == 0:
                            raise OSError("memfd write made no progress")
                        remaining = remaining[written:]
            if digest.hexdigest() != expected_digest:
                raise CheckpointIntegrityError(
                    f"checkpoint changed while preparing receiver mount: {name}"
                )
            seals = 0x0001 | 0x0002 | 0x0004 | 0x0008
            fcntl.fcntl(descriptor, 1033, seals)  # F_ADD_SEALS
            os.lseek(descriptor, 0, os.SEEK_SET)
            return descriptor, file_bytes
        except BaseException:
            os.close(descriptor)
            raise

    @staticmethod
    def _checkpoint_mount_args(manifest: object, descriptors: list[int]) -> list[str]:
        if not isinstance(manifest, dict) or not isinstance(manifest.get("files"), dict):
            raise CheckpointIntegrityError("checkpoint manifest file list is malformed")
        files = sorted(manifest["files"])
        if len(files) != len(descriptors):
            raise CheckpointIntegrityError("checkpoint file descriptors do not match manifest")

        mount_root = PurePosixPath(CHECKPOINT_MOUNT_PATH)
        directories = {mount_root.parent.as_posix(), mount_root.as_posix()}
        for relative_path in files:
            parent = (mount_root / PurePosixPath(relative_path)).parent
            while parent != mount_root.parent:
                directories.add(parent.as_posix())
                parent = parent.parent

        arguments: list[str] = []
        for directory in sorted(directories, key=lambda item: (item.count("/"), item)):
            arguments.extend(["--dir", directory])
        for relative_path, descriptor in zip(files, descriptors, strict=True):
            target = (mount_root / PurePosixPath(relative_path)).as_posix()
            arguments.extend(["--ro-bind-data", str(descriptor), target])
        return arguments

    @staticmethod
    def _close_fds(descriptors: list[int]) -> None:
        for descriptor in descriptors:
            try:
                os.close(descriptor)
            except OSError:
                pass

    def stop_and_confirm(
        self,
        worker: ManagedWorker,
        *,
        grace_seconds: float,
    ) -> ProcessExitEvidence:
        if grace_seconds < 0:
            raise ValueError("grace_seconds must be non-negative")

        if worker.process.poll() is None:
            self._signal_group(worker.process_group_id, signal.SIGTERM)
        try:
            exit_code = worker.process.wait(timeout=grace_seconds)
        except subprocess.TimeoutExpired:
            self._signal_group(worker.process_group_id, signal.SIGKILL)
            exit_code = worker.process.wait()

        self._wait_for_empty_group(worker.process_group_id, grace_seconds)
        return ProcessExitEvidence(
            attempt_id=worker.attempt_id,
            process_group_id=worker.process_group_id,
            exit_code=exit_code,
            quiescent=True,
            observed_at_monotonic_ns=time.monotonic_ns(),
        )

    @staticmethod
    def _signal_group(process_group_id: int, signal_number: int) -> None:
        try:
            os.killpg(process_group_id, signal_number)
        except ProcessLookupError:
            return

    @staticmethod
    def _wait_for_empty_group(process_group_id: int, grace_seconds: float) -> None:
        deadline = time.monotonic() + max(grace_seconds, 0.1)
        forced_kill = False
        while ProcessSupervisor._group_has_live_members(process_group_id):
            if time.monotonic() >= deadline:
                if forced_kill:
                    raise ProcessQuiescenceError(
                        f"process group {process_group_id} still has live members after SIGKILL"
                    )
                ProcessSupervisor._signal_group(process_group_id, signal.SIGKILL)
                forced_kill = True
                deadline = time.monotonic() + max(grace_seconds, 0.1)
            time.sleep(0.01)

    @staticmethod
    def _group_has_live_members(process_group_id: int) -> bool:
        if sys.platform.startswith("linux"):
            # Linux keeps killed orphan descendants as zombies until their parent
            # reaps them. A zombie cannot execute or write, so it is safe to ignore
            # for quiescence; any unreadable member keeps the result fail-closed.
            for entry in Path("/proc").iterdir():
                if not entry.name.isdigit():
                    continue
                try:
                    stat = (entry / "stat").read_text()
                except FileNotFoundError:
                    continue
                except PermissionError:
                    return True
                try:
                    fields_after_command = stat.rsplit(")", 1)[1].split()
                    state = fields_after_command[0]
                    member_group = int(fields_after_command[2])
                except (IndexError, ValueError):
                    return True
                if member_group == process_group_id and state not in {"Z", "X"}:
                    return True
            return False

        try:
            os.killpg(process_group_id, 0)
        except ProcessLookupError:
            return False
        return True
