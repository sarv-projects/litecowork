"""Linux-only Bubblewrap supervisor for metadata-only ZIP previews.

The worker receives a pinned, bounded byte object on stdin. It gets a private PID,
network, mount, IPC, UTS and user namespace, a read-only view of its code/runtime, and a
small private temporary filesystem. This boundary is only qualified on the current Linux
host when ``qualified()`` succeeds; it does not publish extracted members as Resources.
"""

from __future__ import annotations

from dataclasses import asdict
import base64
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
from typing import Any

from .provider import ArchiveRejected, Limits, SourceRevision


class WorkerUnavailable(RuntimeError):
    """The isolated worker cannot be launched with the required boundary."""


class WorkerFailed(RuntimeError):
    """The worker failed without exposing parser diagnostics or input content."""


class WorkerTimedOut(WorkerFailed):
    """The worker exceeded its wall-clock deadline and was terminated."""


_MAX_INPUT_BYTES = 24 * 1024 * 1024
_MAX_OUTPUT_BYTES = 8 * 1024 * 1024
_WALL_TIMEOUT_SECONDS = 12.0
_CPU_LIMIT_SECONDS = 8
_ADDRESS_SPACE_LIMIT_BYTES = 512 * 1024 * 1024
_PRLIMIT = Path("/usr/bin/prlimit")


class ZipWorker:
    """Launch only the fixed metadata-only preview worker inside Bubblewrap."""

    def __init__(
        self,
        *,
        bwrap_path: str | None = None,
        python_path: str | None = None,
        wall_timeout_seconds: float = _WALL_TIMEOUT_SECONDS,
    ) -> None:
        # Do not select a sandbox binary through an ambient user-controlled PATH.
        self.bwrap_path = bwrap_path if bwrap_path is not None else "/usr/bin/bwrap"
        self.python_path = python_path if python_path is not None else "/usr/bin/python3"
        self.wall_timeout_seconds = wall_timeout_seconds
        self.package_path = Path(__file__).resolve().parent

    def _command(self) -> list[str]:
        if sys.platform != "linux":
            raise WorkerUnavailable("isolated ZIP worker is supported only on Linux")
        if (
            not self.bwrap_path
            or not os.path.isabs(self.bwrap_path)
            or not os.path.isfile(self.bwrap_path)
            or not Path(self.bwrap_path).resolve().is_relative_to(Path("/usr"))
        ):
            raise WorkerUnavailable("Bubblewrap is not installed")
        if not _PRLIMIT.is_file():
            raise WorkerUnavailable("the Linux resource-limit launcher is unavailable")
        executable = Path(self.python_path).resolve()
        if not executable.is_absolute() or not executable.is_relative_to(Path("/usr")) or not executable.is_file():
            raise WorkerUnavailable("system Python is unavailable in the isolated runtime")
        for path in (Path("/usr"), Path("/lib"), Path("/lib64")):
            if not path.exists():
                raise WorkerUnavailable("required read-only runtime mount is unavailable")
        if not self.package_path.is_dir() or not (self.package_path / "worker.py").is_file():
            raise WorkerUnavailable("packaged ZIP worker files are unavailable")

        sandbox_command = [
            self.bwrap_path,
            "--unshare-all",
            "--unshare-user",
            "--disable-userns",
            "--die-with-parent",
            "--new-session",
            "--ro-bind", "/usr", "/usr",
            "--ro-bind", "/lib", "/lib",
            "--ro-bind", "/lib64", "/lib64",
            "--dir", "/opt",
            "--dir", "/opt/litecowork",
            "--ro-bind", str(self.package_path), "/opt/litecowork/zip_intake",
            "--proc", "/proc",
            "--dev", "/dev",
            "--size", "1048576",
            "--tmpfs", "/tmp",
            "--chdir", "/tmp",
            "--clearenv",
            "--setenv", "PATH", "/usr/bin:/bin",
            "--setenv", "PYTHONDONTWRITEBYTECODE", "1",
            "--",
            str(executable),
            "-I",
            "/opt/litecowork/zip_intake/worker.py",
        ]
        # prlimit execs Bubblewrap after setting inherited hard limits. Avoid
        # preexec_fn: it is unsafe in a multithreaded Runtime process.
        return [
            str(_PRLIMIT),
            f"--cpu={_CPU_LIMIT_SECONDS}",
            f"--as={_ADDRESS_SPACE_LIMIT_BYTES}",
            "--fsize=0",
            "--nofile=32",
            "--core=0",
            "--",
            *sandbox_command,
        ]

    def _run(self, request: dict[str, Any]) -> dict[str, Any]:
        command = self._command()
        payload = json.dumps(request, separators=(",", ":"), ensure_ascii=True).encode("ascii")
        if len(payload) > _MAX_INPUT_BYTES:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        try:
            process = subprocess.Popen(
                command,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                cwd="/",
                env={},
                close_fds=True,
                start_new_session=True,
            )
        except OSError:
            raise WorkerUnavailable("isolated ZIP worker could not be started") from None

        try:
            stdout, _stderr = process.communicate(payload, timeout=self.wall_timeout_seconds)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.communicate()
            raise WorkerTimedOut("isolated ZIP worker exceeded its deadline") from None
        if len(stdout) > _MAX_OUTPUT_BYTES:
            raise WorkerFailed("isolated ZIP worker exceeded its response limit")
        if process.returncode != 0:
            raise WorkerFailed("isolated ZIP worker failed")
        try:
            response = json.loads(stdout)
        except (UnicodeDecodeError, json.JSONDecodeError):
            raise WorkerFailed("isolated ZIP worker returned an invalid response") from None
        if not isinstance(response, dict) or response.get("ok") is not True:
            code = response.get("code") if isinstance(response, dict) else None
            if isinstance(code, str) and code.isascii() and code.replace("_", "").isalnum() and len(code) <= 64:
                raise ArchiveRejected(code)
            raise WorkerFailed("isolated ZIP worker returned an invalid response")
        manifest = response.get("manifest")
        if not isinstance(manifest, dict):
            raise WorkerFailed("isolated ZIP worker returned an invalid manifest")
        return manifest

    def preview(self, source: SourceRevision, archive: bytes) -> dict[str, Any]:
        if type(archive) is not bytes:
            raise ArchiveRejected("IMMUTABLE_BYTES_REQUIRED")
        if len(archive) > Limits().max_archive_bytes:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        request = {
            "source": asdict(source),
            "archive_b64": base64.b64encode(archive).decode("ascii"),
        }
        result = self._run(request)
        if result.get("source") != asdict(source):
            raise WorkerFailed("isolated ZIP worker returned mismatched source provenance")
        return result

    def qualified(self) -> bool:
        """Perform an isolated end-to-end probe; false means fail closed."""
        from io import BytesIO
        from hashlib import sha256
        import zipfile

        stream = BytesIO()
        with zipfile.ZipFile(stream, "w") as package:
            package.writestr("probe.txt", b"litecowork-zip-worker-probe")
        data = stream.getvalue()
        source = SourceRevision("probe-workspace", "probe-resource", "probe-revision", sha256(data).hexdigest())
        try:
            result = self.preview(source, data)
        except (ArchiveRejected, WorkerFailed, WorkerUnavailable, OSError, ValueError):
            return False
        entries = result.get("entries")
        return (
            isinstance(entries, list)
            and len(entries) == 1
            and isinstance(entries[0], dict)
            and entries[0].get("status") == "READY"
            and entries[0].get("virtual_path") == "probe.txt"
        )
