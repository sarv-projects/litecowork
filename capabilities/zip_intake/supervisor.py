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
import selectors
import signal
import subprocess
import sys
import time
from typing import Any

from .provider import ArchiveRejected, Limits, SourceRevision


class WorkerUnavailable(RuntimeError):
    """The isolated worker cannot be launched with the required boundary."""


class WorkerFailed(RuntimeError):
    """The worker failed without exposing parser diagnostics or input content."""


class WorkerTimedOut(WorkerFailed):
    """The worker exceeded its wall-clock deadline and was terminated."""


class WorkerOutputLimit(WorkerFailed):
    """The worker exceeded the bounded metadata response size and was terminated."""


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

    def _terminate(self, process: subprocess.Popen[bytes]) -> None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()

    def _communicate_bounded(self, process: subprocess.Popen[bytes], payload: bytes) -> bytes:
        """Send bounded input and read bounded output without communicate() buffering."""
        if process.stdin is None or process.stdout is None:
            raise WorkerFailed("isolated ZIP worker pipes are unavailable")

        selector = selectors.DefaultSelector()
        output = bytearray()
        payload_offset = 0
        deadline = time.monotonic() + self.wall_timeout_seconds
        stdin_fd = process.stdin.fileno()
        stdout_fd = process.stdout.fileno()
        os.set_blocking(stdin_fd, False)
        os.set_blocking(stdout_fd, False)
        selector.register(stdin_fd, selectors.EVENT_WRITE, "stdin")
        selector.register(stdout_fd, selectors.EVENT_READ, "stdout")
        try:
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    self._terminate(process)
                    raise WorkerTimedOut("isolated ZIP worker exceeded its deadline")
                if process.poll() is not None and stdin_fd in selector.get_map():
                    # A dead child cannot consume more input. Close our side so an
                    # exited child can never leave this pump waiting for stdin space.
                    selector.unregister(stdin_fd)
                    process.stdin.close()
                for key, _mask in selector.select(remaining):
                    if key.data == "stdin":
                        try:
                            written = os.write(stdin_fd, payload[payload_offset:payload_offset + 65536])
                        except BlockingIOError:
                            continue
                        except BrokenPipeError:
                            written = 0
                            payload_offset = len(payload)
                        payload_offset += written
                        if payload_offset >= len(payload):
                            selector.unregister(stdin_fd)
                            process.stdin.close()
                    else:
                        remaining_output = _MAX_OUTPUT_BYTES + 1 - len(output)
                        try:
                            chunk = os.read(stdout_fd, min(65536, remaining_output))
                        except BlockingIOError:
                            continue
                        if not chunk:
                            selector.unregister(stdout_fd)
                            process.stdout.close()
                            continue
                        output.extend(chunk)
                        if len(output) > _MAX_OUTPUT_BYTES:
                            self._terminate(process)
                            raise WorkerOutputLimit("isolated ZIP worker exceeded its response limit")

            remaining = deadline - time.monotonic()
            if remaining <= 0:
                self._terminate(process)
                raise WorkerTimedOut("isolated ZIP worker exceeded its deadline")
            try:
                process.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                self._terminate(process)
                raise WorkerTimedOut("isolated ZIP worker exceeded its deadline") from None
            return bytes(output)
        except BaseException:
            if process.poll() is None:
                self._terminate(process)
            raise
        finally:
            selector.close()
            if process.stdin is not None and not process.stdin.closed:
                process.stdin.close()
            if process.stdout is not None and not process.stdout.closed:
                process.stdout.close()

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
                # Diagnostics are intentionally not returned. Discard them so a
                # compromised or noisy parser cannot fill an unbounded parent buffer.
                stderr=subprocess.DEVNULL,
                cwd="/",
                env={},
                close_fds=True,
                start_new_session=True,
            )
        except OSError:
            raise WorkerUnavailable("isolated ZIP worker could not be started") from None

        try:
            stdout = self._communicate_bounded(process, payload)
        except (WorkerTimedOut, WorkerOutputLimit):
            raise
        except OSError:
            self._terminate(process)
            raise WorkerFailed("isolated ZIP worker failed") from None
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
