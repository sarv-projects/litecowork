"""Linux-only Bubblewrap supervisor for bounded ZIP preview and extraction.

The worker receives a pinned, bounded byte object on stdin. It gets a private PID,
network, mount, IPC, UTS and user namespace, a read-only view of its code/runtime, and a
small private temporary filesystem. Extraction returns bounded member bytes in memory; it
never writes files or publishes extracted members as Resources. The caller must authorize
and publish those members through Core-owned transactions.
"""

from __future__ import annotations

from dataclasses import asdict
import base64
from hashlib import sha256
import json
import os
from pathlib import Path
import re
import selectors
import signal
import subprocess
import sys
from threading import Event
import time
from typing import Any
import unicodedata

from .provider import (
    ArchiveRejected,
    EntryReport,
    ExtractionResult,
    ExtractedEntry,
    Limits,
    Manifest,
    PROVIDER_VERSION,
    SourceRevision,
    _virtual_path,
)


class WorkerUnavailable(RuntimeError):
    """The isolated worker cannot be launched with the required boundary."""


class WorkerFailed(RuntimeError):
    """The worker failed without exposing parser diagnostics or input content."""


class WorkerTimedOut(WorkerFailed):
    """The worker exceeded its wall-clock deadline and was terminated."""


class WorkerCancelled(WorkerFailed):
    """The caller cancelled the worker and its process group was terminated."""


class WorkerOutputLimit(WorkerFailed):
    """The worker exceeded the bounded metadata response size and was terminated."""


_MAX_INPUT_BYTES = 24 * 1024 * 1024
_MAX_OUTPUT_BYTES = 8 * 1024 * 1024
_MAX_EXTRACTION_OUTPUT_BYTES = 48 * 1024 * 1024
_WALL_TIMEOUT_SECONDS = 12.0
_CPU_LIMIT_SECONDS = 8
_ADDRESS_SPACE_LIMIT_BYTES = 512 * 1024 * 1024
_PRLIMIT = Path("/usr/bin/prlimit")


def _object_without_duplicate_fields(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON object field")
        result[key] = value
    return result


class ZipWorker:
    """Launch only the fixed preview/extraction worker inside Bubblewrap."""

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

    def _communicate_bounded(
        self,
        process: subprocess.Popen[bytes],
        payload: bytes,
        output_limit: int,
        cancellation: Event | None,
    ) -> bytes:
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
                if cancellation is not None and cancellation.is_set():
                    self._terminate(process)
                    raise WorkerCancelled("isolated ZIP worker was cancelled")
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    self._terminate(process)
                    raise WorkerTimedOut("isolated ZIP worker exceeded its deadline")
                if process.poll() is not None and stdin_fd in selector.get_map():
                    # A dead child cannot consume more input. Close our side so an
                    # exited child can never leave this pump waiting for stdin space.
                    selector.unregister(stdin_fd)
                    process.stdin.close()
                poll_interval = min(remaining, 0.05) if cancellation is not None else remaining
                for key, _mask in selector.select(poll_interval):
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
                        remaining_output = output_limit + 1 - len(output)
                        try:
                            chunk = os.read(stdout_fd, min(65536, remaining_output))
                        except BlockingIOError:
                            continue
                        if not chunk:
                            selector.unregister(stdout_fd)
                            process.stdout.close()
                            continue
                        output.extend(chunk)
                        if len(output) > output_limit:
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

    def _run(
        self,
        request: dict[str, Any],
        *,
        output_limit: int = _MAX_OUTPUT_BYTES,
        cancellation: Event | None = None,
    ) -> dict[str, Any]:
        if cancellation is not None and cancellation.is_set():
            raise WorkerCancelled("isolated ZIP worker was cancelled")
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
            stdout = self._communicate_bounded(process, payload, output_limit, cancellation)
        except (WorkerTimedOut, WorkerOutputLimit):
            raise
        except OSError:
            self._terminate(process)
            raise WorkerFailed("isolated ZIP worker failed") from None
        if process.returncode != 0:
            raise WorkerFailed("isolated ZIP worker failed")
        try:
            response = json.loads(stdout, object_pairs_hook=_object_without_duplicate_fields)
        except (UnicodeDecodeError, json.JSONDecodeError, ValueError):
            raise WorkerFailed("isolated ZIP worker returned an invalid response") from None
        operation = request.get("operation")
        expected_fields = {"ok", "manifest"} if operation == "preview" else {"ok", "manifest", "payloads"}
        if not isinstance(response, dict) or set(response) != expected_fields:
            # Error replies have their own intentionally small, safe schema.
            if isinstance(response, dict) and set(response) == {"ok", "code"} and response.get("ok") is False:
                code = response.get("code")
                if isinstance(code, str) and code.isascii() and code.replace("_", "").isalnum() and len(code) <= 64:
                    raise ArchiveRejected(code)
            raise WorkerFailed("isolated ZIP worker returned an invalid response")
        if not isinstance(response, dict) or response.get("ok") is not True:
            code = response.get("code") if isinstance(response, dict) else None
            if isinstance(code, str) and code.isascii() and code.replace("_", "").isalnum() and len(code) <= 64:
                raise ArchiveRejected(code)
            raise WorkerFailed("isolated ZIP worker returned an invalid response")
        result = {key: value for key, value in response.items() if key != "ok"}
        if not isinstance(result.get("manifest"), dict):
            raise WorkerFailed("isolated ZIP worker returned an invalid manifest")
        return result

    def preview(
        self,
        source: SourceRevision,
        archive: bytes,
        *,
        cancellation: Event | None = None,
    ) -> dict[str, Any]:
        if type(archive) is not bytes:
            raise ArchiveRejected("IMMUTABLE_BYTES_REQUIRED")
        if len(archive) > Limits().max_archive_bytes:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        request = {
            "operation": "preview",
            "source": asdict(source),
            "archive_b64": base64.b64encode(archive).decode("ascii"),
        }
        result = self._run(request, cancellation=cancellation)["manifest"]
        self._manifest_from_wire(result, source, len(archive))
        return result

    @staticmethod
    def _manifest_from_wire(
        value: dict[str, Any],
        source: SourceRevision,
        archive_bytes: int,
        *,
        require_settled: bool = False,
    ) -> Manifest:
        try:
            if set(value) != {"source", "provider_version", "archive_bytes", "limits", "entries"}:
                raise ValueError
            if value["source"] != asdict(source):
                raise ValueError
            if value["provider_version"] != PROVIDER_VERSION:
                raise ValueError
            if type(value["archive_bytes"]) is not int or value["archive_bytes"] != archive_bytes:
                raise ValueError
            limits_value = value["limits"]
            if not isinstance(limits_value, dict) or set(limits_value) != set(asdict(Limits())):
                raise ValueError
            limits = Limits(**limits_value)
            if limits != Limits():
                raise ValueError
            reports = value["entries"]
            if not isinstance(reports, list) or len(reports) > limits.max_entries:
                raise ValueError
            report_fields = set(asdict(EntryReport(0, "0" * 64, None, 0, 0, 0, "READY")))
            parsed_reports = []
            declared_total = 0
            compressed_total = 0
            for report in reports:
                if not isinstance(report, dict) or set(report) != report_fields:
                    raise ValueError
                if (
                    type(report["entry_index"]) is not int
                    or report["entry_index"] != len(parsed_reports)
                    or report["entry_index"] >= limits.max_entries
                    or not isinstance(report["name_sha256"], str)
                    or len(report["name_sha256"]) != 64
                    or any(char not in "0123456789abcdef" for char in report["name_sha256"])
                    or type(report["compressed_bytes"]) is not int
                    or type(report["declared_bytes"]) is not int
                    or type(report["crc32"]) is not int
                    or report["compressed_bytes"] < 0
                    or report["declared_bytes"] < 0
                    or not 0 <= report["crc32"] <= 0xFFFFFFFF
                    or report["status"] not in ("READY", "DIRECTORY", "REJECTED", "EXTRACTED")
                    or (
                        report["reason"] is not None
                        and (
                            not isinstance(report["reason"], str)
                            or re.fullmatch(r"[A-Z0-9_]{1,64}", report["reason"]) is None
                        )
                    )
                    or (report["virtual_path"] is not None and not isinstance(report["virtual_path"], str))
                    or (report["observed_bytes"] is not None and type(report["observed_bytes"]) is not int)
                    or (report["content_sha256"] is not None and not isinstance(report["content_sha256"], str))
                ):
                    raise ValueError
                if report["virtual_path"] is not None and _virtual_path(
                    report["virtual_path"], limits
                ) != report["virtual_path"]:
                    # The child is an isolation boundary, not an authority: validate
                    # provenance labels again before returning them to Core callers.
                    raise ValueError
                if report["status"] in ("READY", "DIRECTORY", "EXTRACTED") and report["virtual_path"] is None:
                    raise ValueError
                if report["status"] != "EXTRACTED" and (
                    report["observed_bytes"] is not None
                    or report["content_sha256"] is not None
                ):
                    # Preview and rejected-entry records never claim extracted
                    # payload facts, even when the child happens to return them.
                    raise ValueError
                if report["status"] == "REJECTED":
                    if not isinstance(report["reason"], str) or not report["reason"]:
                        raise ValueError
                elif report["reason"] is not None:
                    raise ValueError
                if report["status"] == "EXTRACTED":
                    if (
                        not require_settled
                        or report["virtual_path"] is None
                        or type(report["observed_bytes"]) is not int
                        or not 0 <= report["observed_bytes"] <= limits.max_entry_bytes
                        or report["declared_bytes"] != report["observed_bytes"]
                        or report["declared_bytes"]
                        > limits.max_compression_ratio * max(report["compressed_bytes"], 1)
                        or not isinstance(report["content_sha256"], str)
                        or len(report["content_sha256"]) != 64
                        or any(char not in "0123456789abcdef" for char in report["content_sha256"])
                        or report["reason"] is not None
                    ):
                        raise ValueError
                elif report["status"] == "READY":
                    if require_settled:
                        # The extraction operation must settle every admitted file as
                        # EXTRACTED or REJECTED before its result can be consumed.
                        raise ValueError
                    if (
                        report["declared_bytes"] > limits.max_entry_bytes
                        or report["declared_bytes"]
                        > limits.max_compression_ratio * max(report["compressed_bytes"], 1)
                    ):
                        raise ValueError
                elif report["status"] == "DIRECTORY":
                    # ZIP writers may DEFLATE the empty directory marker into a
                    # non-empty compressed stream. Its expanded size must remain 0.
                    if report["declared_bytes"] != 0:
                        raise ValueError
                declared_total += report["declared_bytes"]
                compressed_total += report["compressed_bytes"]
                parsed_reports.append(EntryReport(**report))
            if (
                declared_total > limits.max_expanded_bytes
                or compressed_total > archive_bytes
            ):
                raise ValueError
            if len({report.entry_index for report in parsed_reports}) != len(parsed_reports):
                raise ValueError
            # The worker is an isolation boundary, not a trusted parser. Recheck that
            # its returned manifest has already quarantined every path collision before
            # any caller can use these labels to publish extracted members.
            paths: dict[str, list[EntryReport]] = {}
            for report in parsed_reports:
                if report.virtual_path is not None:
                    key = unicodedata.normalize("NFKC", report.virtual_path).casefold()
                    paths.setdefault(key, []).append(report)
            conflicting: set[int] = set()
            for key, reports_for_path in paths.items():
                if len(reports_for_path) > 1:
                    conflicting.update(report.entry_index for report in reports_for_path)
                parts = key.split("/")
                for depth in range(1, len(parts)):
                    for parent in paths.get("/".join(parts[:depth]), ()):
                        if parent.status != "DIRECTORY":
                            conflicting.add(parent.entry_index)
                            conflicting.update(report.entry_index for report in reports_for_path)
            if any(
                parsed_reports[index].status != "REJECTED"
                or parsed_reports[index].reason != "PATH_CONFLICT"
                for index in conflicting
            ):
                raise ValueError
            return Manifest(
                source=source,
                provider_version=value["provider_version"],
                archive_bytes=value["archive_bytes"],
                limits=limits,
                entries=tuple(parsed_reports),
            )
        except (TypeError, ValueError, KeyError):
            raise WorkerFailed("isolated ZIP worker returned an invalid manifest") from None

    def extract(
        self,
        source: SourceRevision,
        archive: bytes,
        *,
        cancellation: Event | None = None,
    ) -> ExtractionResult:
        """Extract admitted members in the bounded Linux worker, never to disk.

        The caller remains responsible for resolving and authorizing this exact Resource
        revision and publishing returned members through Core-owned storage transactions.
        """
        if type(archive) is not bytes:
            raise ArchiveRejected("IMMUTABLE_BYTES_REQUIRED")
        if len(archive) > Limits().max_archive_bytes:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        request = {
            "operation": "extract",
            "source": asdict(source),
            "archive_b64": base64.b64encode(archive).decode("ascii"),
        }
        response = self._run(
            request,
            output_limit=_MAX_EXTRACTION_OUTPUT_BYTES,
            cancellation=cancellation,
        )
        manifest_value = response["manifest"]
        manifest = self._manifest_from_wire(
            manifest_value,
            source,
            len(archive),
            require_settled=True,
        )
        payloads = response.get("payloads")
        if not isinstance(payloads, list) or len(payloads) > manifest.limits.max_entries:
            raise WorkerFailed("isolated ZIP worker returned invalid extraction payloads")
        reports = {report.entry_index: report for report in manifest.entries}
        extracted: list[ExtractedEntry] = []
        seen: set[int] = set()
        total_bytes = 0
        try:
            for payload in payloads:
                if not isinstance(payload, dict) or set(payload) != {
                    "entry_index", "virtual_path", "content_sha256", "content_b64"
                }:
                    raise ValueError
                index = payload["entry_index"]
                if type(index) is not int or index in seen:
                    raise ValueError
                seen.add(index)
                report = reports.get(index)
                if report is None or report.status != "EXTRACTED" or report.virtual_path is None:
                    raise ValueError
                if payload["virtual_path"] != report.virtual_path:
                    raise ValueError
                encoded = payload["content_b64"]
                if (
                    not isinstance(encoded, str)
                    or len(encoded) > ((manifest.limits.max_entry_bytes + 2) // 3) * 4
                ):
                    raise ValueError
                content = base64.b64decode(encoded, validate=True)
                digest = sha256(content).hexdigest()
                if (
                    digest != payload["content_sha256"]
                    or digest != report.content_sha256
                    or len(content) != report.observed_bytes
                    or len(content) > manifest.limits.max_entry_bytes
                ):
                    raise ValueError
                total_bytes += len(content)
                if total_bytes > manifest.limits.max_expanded_bytes:
                    raise ValueError
                extracted.append(ExtractedEntry(index, report.virtual_path, digest, content))
            expected = {report.entry_index for report in manifest.entries if report.status == "EXTRACTED"}
            if seen != expected:
                raise ValueError
        except (TypeError, ValueError, KeyError):
            raise WorkerFailed("isolated ZIP worker returned inconsistent extraction data") from None
        return ExtractionResult(manifest=manifest, entries=tuple(extracted))

    def qualified(self) -> bool:
        """Perform an isolated end-to-end probe; false means fail closed."""
        from io import BytesIO
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
        if not (
            isinstance(entries, list)
            and len(entries) == 1
            and isinstance(entries[0], dict)
            and entries[0].get("status") == "READY"
            and entries[0].get("virtual_path") == "probe.txt"
        ):
            return False
        try:
            extraction = self.extract(source, data)
        except (ArchiveRejected, WorkerFailed, WorkerUnavailable, OSError, ValueError):
            return False
        return (
            extraction.manifest.source == source
            and len(extraction.entries) == 1
            and extraction.entries[0].virtual_path == "probe.txt"
            and extraction.entries[0].content == b"litecowork-zip-worker-probe"
            and extraction.entries[0].content_sha256 == sha256(extraction.entries[0].content).hexdigest()
        )
