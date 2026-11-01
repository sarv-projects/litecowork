"""Deterministic extraction planning and staging-root materialization.

This provider-side module turns an already validated, immutable ZIP archive into:

* a closed ``ExtractionPlan`` in archive order with a caller-visible ``order``, and
* optionally, streamed entry files under an explicit, provider-owned staging root.

It never creates LiteCowork Resources, provenance edges, Effects, receipts, events,
index rows, or Task/Attempt state. It never opens a host path supplied by the archive,
never overwrites a destination path, and never follows a symlink while materializing.
The caller/Core remains responsible for authorization, immutable publication, child
Resource registration, provenance, retention, and deletion.

The archive bytes are bounded by ``Limits.max_archive_bytes`` (the same immutable input
contract as ``ZipIntakeProvider``). Entry bodies are streamed in fixed chunks; this module
does not expand the whole archive into memory.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import errno
from hashlib import sha256
import os
import re
import stat
import struct
import time
import unicodedata
from typing import Final
import zipfile
import zlib

from .provider import (
    ArchiveRejected,
    Limits,
    Manifest,
    SourceRevision,
    ZipIntakeProvider,
    _nested,
    _virtual_path,
)

_CHUNK_BYTES: Final = 64 * 1024
_PREFIX_BYTES: Final = 512
_CODE: Final = re.compile(r"[A-Z0-9_]{1,64}")
_DIGEST: Final = re.compile(r"[0-9a-f]{64}")

_REASON_UNSAFE_PATH: Final = "UNSAFE_PATH"
_REASON_DESTINATION_ESCAPE: Final = "DESTINATION_ESCAPE"
_REASON_DESTINATION_CONFLICT: Final = "DESTINATION_CONFLICT"
_REASON_DESTINATION_UNAVAILABLE: Final = "DESTINATION_UNAVAILABLE"
_REASON_DESTINATION_UNSAFE: Final = "DESTINATION_UNSAFE"

# Bounded, deterministic media-type hints. Extensions are only hints (see
# docs/NETWORK-SECURITY.md); extraction additionally checks a bounded content prefix
# before claiming a type. A mismatch produces ``None`` rather than a false assertion.
_MEDIA_BY_SUFFIX: Final[dict[str, tuple[str, str]]] = {
    ".txt": ("text/plain", "text"),
    ".log": ("text/plain", "text"),
    ".md": ("text/markdown", "text"),
    ".markdown": ("text/markdown", "text"),
    ".csv": ("text/csv", "text"),
    ".tsv": ("text/tab-separated-values", "text"),
    ".json": ("application/json", "json"),
    ".html": ("text/html", "html"),
    ".htm": ("text/html", "html"),
    ".xml": ("application/xml", "xml"),
    ".svg": ("image/svg+xml", "svg"),
    ".yaml": ("application/yaml", "text"),
    ".yml": ("application/yaml", "text"),
    ".toml": ("application/toml", "text"),
    ".ini": ("text/plain", "text"),
    ".cfg": ("text/plain", "text"),
    ".css": ("text/css", "text"),
    ".py": ("text/plain", "text"),
    ".rs": ("text/plain", "text"),
    ".js": ("text/javascript", "text"),
    ".ts": ("text/plain", "text"),
    ".tsx": ("text/plain", "text"),
    ".jsx": ("text/plain", "text"),
    ".sh": ("text/plain", "text"),
    ".pdf": ("application/pdf", "pdf"),
    ".png": ("image/png", "png"),
    ".jpg": ("image/jpeg", "jpeg"),
    ".jpeg": ("image/jpeg", "jpeg"),
    ".gif": ("image/gif", "gif"),
    ".webp": ("image/webp", "webp"),
    ".rtf": ("application/rtf", "rtf"),
}
_MEDIA_VALUES: Final = frozenset(media for media, _kind in _MEDIA_BY_SUFFIX.values())


@dataclass(frozen=True)
class PlanEntry:
    """One archive entry, either admitted for extraction or typed as refused."""

    entry_index: int
    name_sha256: str
    virtual_path: str | None
    declared_bytes: int
    observed_compressed_bytes: int
    media_type_hint: str | None
    status: str  # PLANNED, DIRECTORY, REJECTED, EXTRACTED
    refusal: str | None
    order: int | None
    observed_bytes: int | None = None
    content_sha256: str | None = None
    media_type: str | None = None

    def __post_init__(self) -> None:
        if type(self.entry_index) is not int or self.entry_index < 0:
            raise ValueError("entry_index must be a non-negative integer")
        if not isinstance(self.name_sha256, str) or _DIGEST.fullmatch(self.name_sha256) is None:
            raise ValueError("name_sha256 must be a SHA-256 digest")
        if type(self.declared_bytes) is not int or self.declared_bytes < 0:
            raise ValueError("declared_bytes must be a non-negative integer")
        if type(self.observed_compressed_bytes) is not int or self.observed_compressed_bytes < 0:
            raise ValueError("observed_compressed_bytes must be a non-negative integer")
        if self.media_type_hint is not None and self.media_type_hint not in _MEDIA_VALUES:
            raise ValueError("media_type_hint must be a known bounded hint")
        if self.media_type is not None and self.media_type not in _MEDIA_VALUES:
            raise ValueError("media_type must be a known bounded type")
        if self.status not in ("PLANNED", "DIRECTORY", "REJECTED", "EXTRACTED"):
            raise ValueError("unknown plan entry status")
        if self.status == "REJECTED":
            if not isinstance(self.refusal, str) or _CODE.fullmatch(self.refusal) is None:
                raise ValueError("rejected entries require a safe refusal code")
            if self.order is not None:
                raise ValueError("rejected entries have no extraction order")
            if self.observed_bytes is not None or self.content_sha256 is not None or self.media_type is not None:
                raise ValueError("rejected entries never claim extracted facts")
            return
        if self.refusal is not None:
            raise ValueError("non-rejected entries have no refusal")
        if self.virtual_path is None:
            raise ValueError("non-rejected entries require a virtual path")
        if self.status == "DIRECTORY":
            if (
                self.order is not None
                or self.observed_bytes is not None
                or self.content_sha256 is not None
                or self.media_type is not None
            ):
                raise ValueError("directories carry no extraction facts")
            return
        if type(self.order) is not int or self.order < 0:
            if self.status != "PLANNED" or self.order is not None:
                raise ValueError("admitted entries require a non-negative order")
        if self.status == "PLANNED":
            if self.observed_bytes is not None or self.content_sha256 is not None or self.media_type is not None:
                raise ValueError("planned entries carry no extracted facts")
            return
        if self.observed_bytes != self.declared_bytes:
            raise ValueError("extracted size must equal the admitted declared size")
        if not isinstance(self.content_sha256, str) or _DIGEST.fullmatch(self.content_sha256) is None:
            raise ValueError("extracted entries require a content digest")


@dataclass(frozen=True)
class ExtractionPlan:
    """Closed, deterministic plan for one exact archive revision."""

    source: SourceRevision
    provider_version: str
    archive_bytes: int
    archive_sha256: str
    limits: Limits
    entries: tuple[PlanEntry, ...]  # archive order
    order: tuple[int, ...]  # entry_index values in the caller's required order

    def __post_init__(self) -> None:
        if not isinstance(self.source, SourceRevision):
            raise ValueError("plan requires a pinned source revision")
        if not isinstance(self.provider_version, str) or not self.provider_version:
            raise ValueError("plan requires a provider version")
        if type(self.archive_bytes) is not int or self.archive_bytes <= 0:
            raise ValueError("plan requires a positive archive size")
        if not isinstance(self.archive_sha256, str) or _DIGEST.fullmatch(self.archive_sha256) is None:
            raise ValueError("plan requires an archive digest")
        if self.archive_sha256 != self.source.content_sha256:
            raise ValueError("plan digest must match the pinned source digest")
        if not isinstance(self.limits, Limits):
            raise ValueError("plan requires valid limits")
        if not isinstance(self.entries, tuple) or any(not isinstance(entry, PlanEntry) for entry in self.entries):
            raise ValueError("plan entries must be a tuple of PlanEntry")
        if tuple(entry.entry_index for entry in self.entries) != tuple(range(len(self.entries))):
            raise ValueError("plan entries must be contiguous in archive order")
        if any(entry.status == "PLANNED" and entry.order is None for entry in self.entries):
            raise ValueError("every planned entry must have an extraction order")
        if not isinstance(self.order, tuple) or any(type(index) is not int for index in self.order):
            raise ValueError("plan order must be a tuple of entry indices")
        if len(set(self.order)) != len(self.order):
            raise ValueError("plan order cannot repeat an entry")
        by_index = {entry.entry_index: entry for entry in self.entries}
        if any(index not in by_index for index in self.order):
            raise ValueError("plan order references an unknown entry")
        ranked = [by_index[index] for index in self.order]
        if [entry.order for entry in ranked] != list(range(len(ranked))):
            raise ValueError("plan order ranks must be contiguous")
        if [entry.entry_index for entry in sorted(
            (entry for entry in self.entries if entry.order is not None),
            key=lambda entry: entry.order,
        )] != list(self.order):
            raise ValueError("plan order must list every admitted entry exactly once")
        admitted_paths = [by_index[index].virtual_path for index in self.order]
        if any(path is None for path in admitted_paths):
            raise ValueError("admitted entries require paths")
        if admitted_paths != sorted(admitted_paths, key=lambda value: value.encode("utf-8")):
            raise ValueError("plan order must sort by UTF-8 path bytes")
        keys: set[str] = set()
        for entry in self.entries:
            if entry.status in ("PLANNED", "EXTRACTED"):
                if entry.declared_bytes > self.limits.max_entry_bytes:
                    raise ValueError("planned entry exceeds the entry budget")
                if entry.declared_bytes > self.limits.max_compression_ratio * max(
                    entry.observed_compressed_bytes, 1
                ):
                    raise ValueError("planned entry exceeds the compression-ratio budget")
            if entry.status in ("PLANNED", "EXTRACTED", "DIRECTORY") and entry.virtual_path is not None:
                key = _normalized_key(entry.virtual_path)
                if key in keys:
                    raise ValueError("admitted paths must be unique")
                keys.add(key)
        if sum(entry.declared_bytes for entry in self.entries) > self.limits.max_expanded_bytes:
            raise ValueError("plan exceeds the total expanded-byte budget")


@dataclass(frozen=True)
class MaterializedEntry:
    """Observed facts for one streamed entry now under the staging root."""

    order: int
    entry_index: int
    name_sha256: str
    virtual_path: str
    declared_bytes: int
    observed_bytes: int
    observed_compressed_bytes: int
    content_sha256: str
    media_type: str | None

    def __post_init__(self) -> None:
        if type(self.order) is not int or self.order < 0:
            raise ValueError("order must be a non-negative integer")
        if type(self.entry_index) is not int or self.entry_index < 0:
            raise ValueError("entry_index must be a non-negative integer")
        if not isinstance(self.name_sha256, str) or _DIGEST.fullmatch(self.name_sha256) is None:
            raise ValueError("name_sha256 must be a SHA-256 digest")
        if not isinstance(self.virtual_path, str) or not self.virtual_path:
            raise ValueError("materialized entries require a virtual path")
        if type(self.declared_bytes) is not int or type(self.observed_bytes) is not int:
            raise ValueError("sizes must be integers")
        if self.observed_bytes != self.declared_bytes:
            raise ValueError("observed size must equal the admitted declared size")
        if type(self.observed_compressed_bytes) is not int or self.observed_compressed_bytes < 0:
            raise ValueError("compressed size must be a non-negative integer")
        if not isinstance(self.content_sha256, str) or _DIGEST.fullmatch(self.content_sha256) is None:
            raise ValueError("materialized entries require a content digest")
        if self.media_type is not None and self.media_type not in _MEDIA_VALUES:
            raise ValueError("materialized media type must be a known bounded type")


@dataclass(frozen=True)
class MaterializationResult:
    """Settled plan plus extracted entries in the caller's required order."""

    plan: ExtractionPlan
    entries: tuple[MaterializedEntry, ...]
    total_observed_bytes: int

    def __post_init__(self) -> None:
        if not isinstance(self.plan, ExtractionPlan):
            raise ValueError("result requires a settled plan")
        if any(entry.status == "PLANNED" for entry in self.plan.entries):
            raise ValueError("result plan must be settled")
        if not isinstance(self.entries, tuple) or any(
            not isinstance(entry, MaterializedEntry) for entry in self.entries
        ):
            raise ValueError("result entries must be a tuple of MaterializedEntry")
        if tuple(entry.order for entry in self.entries) != tuple(range(len(self.entries))):
            raise ValueError("result entries must be contiguous in extraction order")
        if tuple(entry.entry_index for entry in self.entries) != self.plan.order:
            raise ValueError("result entries must match the settled plan order")
        by_index = {entry.entry_index: entry for entry in self.plan.entries}
        for entry in self.entries:
            planned = by_index[entry.entry_index]
            if (
                planned.status != "EXTRACTED"
                or planned.virtual_path != entry.virtual_path
                or planned.name_sha256 != entry.name_sha256
                or planned.declared_bytes != entry.declared_bytes
                or planned.observed_compressed_bytes != entry.observed_compressed_bytes
                or planned.observed_bytes != entry.observed_bytes
                or planned.content_sha256 != entry.content_sha256
                or planned.media_type != entry.media_type
            ):
                raise ValueError("result entry does not match the settled plan")
        if self.total_observed_bytes != sum(entry.observed_bytes for entry in self.entries):
            raise ValueError("total observed bytes must match the extracted entries")


def _normalized_key(path: str) -> str:
    return unicodedata.normalize("NFKC", path).casefold()


def _suffix(path: str) -> str:
    name = path.rsplit("/", 1)[-1]
    _stem, dot, extension = name.rpartition(".")
    if not dot or not extension:
        return ""
    return "." + extension.casefold()


def media_type_hint(path: str) -> str | None:
    """Return a deterministic extension-only hint; never a parser-dispatch claim."""

    entry = _MEDIA_BY_SUFFIX.get(_suffix(path))
    return entry[0] if entry is not None else None


def _text_prefix(prefix: bytes) -> str | None:
    if b"\x00" in prefix:
        return None
    try:
        text = prefix.decode("utf-8")
    except UnicodeDecodeError as error:
        # A bounded prefix may cut a multibyte character. Drop only that tail;
        # invalid bytes before the tail still refuse a text claim.
        if error.end < len(prefix) - 3:
            return None
        try:
            text = prefix[: error.start].decode("utf-8")
        except UnicodeDecodeError:
            return None
    if any(character not in "\t\n\r\f" and character < " " for character in text):
        return None
    return text


def _magic_media_type(prefix: bytes) -> str | None:
    if prefix.startswith(b"%PDF-"):
        return "application/pdf"
    if prefix.startswith(b"\x89PNG\r\n\x1a\n"):
        return "image/png"
    if prefix.startswith(b"\xff\xd8\xff"):
        return "image/jpeg"
    if prefix.startswith((b"GIF87a", b"GIF89a")):
        return "image/gif"
    if prefix[:4] == b"RIFF" and prefix[8:12] == b"WEBP":
        return "image/webp"
    if prefix.startswith(b"{\\rtf"):
        return "application/rtf"
    return None


def detect_media_type(path: str, prefix: bytes) -> str | None:
    """Claim a media type only when bounded content or suffix and content agree.

    Extensions are hints (see ``docs/NETWORK-SECURITY.md``); unambiguous file magic
    wins, a suffix/content mismatch returns ``None``, and an unknown suffix returns
    ``None``. This is not a parser-selection or trust decision.
    """

    magic = _magic_media_type(prefix)
    if magic is not None:
        return magic
    entry = _MEDIA_BY_SUFFIX.get(_suffix(path))
    if entry is None:
        return None
    media_type, kind = entry
    if kind == "text":
        return media_type if _text_prefix(prefix) is not None else None
    if kind == "json":
        text = _text_prefix(prefix)
        if text is None:
            return None
        stripped = text.lstrip("\ufeff \t\r\n")
        return media_type if stripped[:1] in ("{", "[") else None
    if kind == "html":
        text = _text_prefix(prefix)
        return media_type if text is not None and "<" in text else None
    if kind == "xml":
        text = _text_prefix(prefix)
        if text is None:
            return None
        stripped = text.lstrip("\ufeff \t\r\n")
        return media_type if stripped.startswith("<") else None
    if kind == "svg":
        text = _text_prefix(prefix)
        return media_type if text is not None and "<svg" in text.casefold() else None
    return None


def _compressed_span(archive: bytes, header_offset: int, expected: int) -> int:
    if header_offset < 0 or header_offset + 30 > len(archive):
        raise ArchiveRejected("INVALID_LOCAL_HEADER")
    name_size, extra_size = struct.unpack_from("<HH", archive, header_offset + 26)
    data_offset = header_offset + 30 + name_size + extra_size
    end = data_offset + expected
    if data_offset > len(archive) or end > len(archive):
        raise ArchiveRejected("INVALID_LOCAL_HEADER")
    return end - data_offset


class _Budget:
    def __init__(self, maximum: int) -> None:
        self.maximum = maximum
        self.used = 0

    def remaining(self) -> int:
        return self.maximum - self.used

    def consume(self, count: int) -> None:
        self.used += count
        if self.used > self.maximum:
            raise ArchiveRejected("EXPANDED_SIZE_LIMIT")


_DESTINATION_O_CLOEXEC: Final = getattr(os, "O_CLOEXEC", 0)
_DESTINATION_DIR_FLAGS: Final = (
    os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0) | _DESTINATION_O_CLOEXEC
)
_DESTINATION_FILE_FLAGS: Final = (
    os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0) | _DESTINATION_O_CLOEXEC
)


def _destination_rejection(error: OSError) -> ArchiveRejected:
    if error.errno == errno.ELOOP:
        return ArchiveRejected(_REASON_DESTINATION_ESCAPE)
    if error.errno in (errno.ENOTDIR, errno.EEXIST, errno.EISDIR):
        return ArchiveRejected(_REASON_DESTINATION_CONFLICT)
    return ArchiveRejected(_REASON_DESTINATION_UNAVAILABLE)


def _path_parts(path: str) -> tuple[str, ...]:
    if not path or path.startswith("/") or "\\" in path or ":" in path:
        raise ArchiveRejected(_REASON_UNSAFE_PATH)
    parts = tuple(path.split("/"))
    if any(not part or part in (".", "..") for part in parts):
        raise ArchiveRejected(_REASON_UNSAFE_PATH)
    return parts


def _dirfd_supported() -> bool:
    required = (os.open, os.mkdir, os.stat, os.unlink, os.rmdir)
    return (
        all(function in os.supports_dir_fd for function in required)
        and hasattr(os, "O_DIRECTORY")
        and hasattr(os, "O_NOFOLLOW")
    )


class _Destination:
    """Anchored, no-follow staging root for one materialization call."""

    def __init__(self, root_fd: int, root: str) -> None:
        self.root_fd = root_fd
        self.root = root
        self.created_files: list[tuple[str, ...]] = []
        self.created_dirs: list[tuple[str, ...]] = []
        self._closed = False

    @classmethod
    def open(cls, destination_root: str | os.PathLike[str]) -> "_Destination":
        if not _dirfd_supported():
            # Do not fall back to a racy path-only writer. This provider is
            # qualified only where anchored directory-relative operations exist.
            raise ArchiveRejected(_REASON_DESTINATION_UNAVAILABLE)
        try:
            root = os.fspath(destination_root)
        except TypeError:
            raise ArchiveRejected(_REASON_DESTINATION_UNSAFE) from None
        if not isinstance(root, str) or not os.path.isabs(root):
            raise ArchiveRejected(_REASON_DESTINATION_UNSAFE)
        try:
            info = os.lstat(root)
        except OSError:
            raise ArchiveRejected(_REASON_DESTINATION_UNSAFE) from None
        if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
            raise ArchiveRejected(_REASON_DESTINATION_UNSAFE)
        try:
            root_fd = os.open(root, _DESTINATION_DIR_FLAGS)
        except OSError as error:
            raise _destination_rejection(error) from None
        try:
            opened = os.fstat(root_fd)
            if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                raise ArchiveRejected(_REASON_DESTINATION_UNSAFE)
        except OSError:
            os.close(root_fd)
            raise ArchiveRejected(_REASON_DESTINATION_UNSAFE) from None
        except BaseException:
            os.close(root_fd)
            raise
        return cls(root_fd, root)

    def _open_existing_dir(self, parent_fd: int, part: str) -> int:
        try:
            return os.open(part, _DESTINATION_DIR_FLAGS, dir_fd=parent_fd)
        except FileNotFoundError:
            raise
        except OSError as error:
            try:
                info = os.stat(part, dir_fd=parent_fd, follow_symlinks=False)
            except OSError:
                info = None
            if info is not None and stat.S_ISLNK(info.st_mode):
                raise ArchiveRejected(_REASON_DESTINATION_ESCAPE) from None
            raise _destination_rejection(error) from None

    def ensure_parent(self, parts: tuple[str, ...]) -> int:
        """Return an fd for the parent directory, creating only missing components."""

        try:
            current = os.dup(self.root_fd)
        except OSError as error:
            raise _destination_rejection(error) from None
        prefix: tuple[str, ...] = ()
        try:
            for part in parts:
                try:
                    child = self._open_existing_dir(current, part)
                except FileNotFoundError:
                    try:
                        os.mkdir(part, 0o700, dir_fd=current)
                    except FileExistsError:
                        pass
                    except OSError as mkdir_error:
                        raise _destination_rejection(mkdir_error) from None
                    else:
                        self.created_dirs.append(prefix + (part,))
                    try:
                        child = self._open_existing_dir(current, part)
                    except FileNotFoundError:
                        raise ArchiveRejected(_REASON_DESTINATION_UNAVAILABLE) from None
                os.close(current)
                current = child
                prefix += (part,)
            return current
        except BaseException:
            os.close(current)
            raise

    def create_file(self, parent_fd: int, name: str, parts: tuple[str, ...]) -> int:
        try:
            existing = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            existing = None
        except OSError as error:
            raise _destination_rejection(error) from None
        if existing is not None:
            if stat.S_ISLNK(existing.st_mode):
                raise ArchiveRejected(_REASON_DESTINATION_ESCAPE)
            raise ArchiveRejected(_REASON_DESTINATION_CONFLICT)
        try:
            fd = os.open(name, _DESTINATION_FILE_FLAGS, 0o600, dir_fd=parent_fd)
        except FileExistsError:
            raise ArchiveRejected(_REASON_DESTINATION_CONFLICT) from None
        except OSError as error:
            raise _destination_rejection(error) from None
        self.created_files.append(parts)
        try:
            os.fchmod(fd, 0o600)
        except OSError as error:
            os.close(fd)
            self.remove_file(parent_fd, parts)
            raise _destination_rejection(error) from None
        return fd

    def remove_file(self, parent_fd: int, parts: tuple[str, ...]) -> None:
        try:
            os.unlink(parts[-1], dir_fd=parent_fd)
        except OSError:
            pass
        try:
            self.created_files.remove(parts)
        except ValueError:
            pass

    def discard_empty_dirs(self, since: int) -> None:
        """Best-effort removal of directories created for one failed entry."""

        for parts in reversed(self.created_dirs[since:]):
            try:
                parent_fd = self._existing_parent(parts[:-1])
            except (ArchiveRejected, OSError):
                continue
            try:
                os.rmdir(parts[-1], dir_fd=parent_fd)
            except OSError:
                continue
            finally:
                os.close(parent_fd)
            try:
                self.created_dirs.remove(parts)
            except ValueError:
                pass

    def _existing_parent(self, parts: tuple[str, ...]) -> int:
        current = os.dup(self.root_fd)
        try:
            for part in parts:
                child = self._open_existing_dir(current, part)
                os.close(current)
                current = child
            return current
        except BaseException:
            os.close(current)
            raise

    def cleanup(self) -> None:
        """Remove files/directories this call created; never follow a link."""

        for parts in reversed(self.created_files):
            try:
                parent_fd = self._existing_parent(parts[:-1])
            except (ArchiveRejected, OSError):
                continue
            try:
                os.unlink(parts[-1], dir_fd=parent_fd)
            except OSError:
                pass
            finally:
                os.close(parent_fd)
        self.created_files.clear()
        for parts in reversed(self.created_dirs):
            try:
                parent_fd = self._existing_parent(parts[:-1])
            except (ArchiveRejected, OSError):
                continue
            try:
                os.rmdir(parts[-1], dir_fd=parent_fd)
            except OSError:
                pass
            finally:
                os.close(parent_fd)
        self.created_dirs.clear()
        self.close()

    def close(self) -> None:
        if not self._closed:
            self._closed = True
            os.close(self.root_fd)


class BoundedZipExtractor:
    """Provider-side planner/materializer; it never publishes LiteCowork records."""

    def __init__(self, provider: ZipIntakeProvider | None = None) -> None:
        self.provider = provider if provider is not None else ZipIntakeProvider()

    def _check_deadline(self, deadline: float) -> None:
        if time.monotonic() >= deadline:
            raise ArchiveRejected("PROCESSING_TIMEOUT")

    def _from_manifest(self, manifest: Manifest, archive: bytes, central: tuple) -> ExtractionPlan:
        if sha256(archive).hexdigest() != manifest.source.content_sha256:
            raise ArchiveRejected("SOURCE_DIGEST_MISMATCH")
        if len(manifest.entries) != len(central):
            raise ArchiveRejected("INVALID_MANIFEST")
        entries: list[PlanEntry] = []
        for report in manifest.entries:
            raw = central[report.entry_index]
            if raw.header_offset < 0 or raw.header_offset >= len(archive):
                raise ArchiveRejected("INVALID_LOCAL_HEADER")
            compressed = _compressed_span(archive, raw.header_offset, report.compressed_bytes)
            if compressed != report.compressed_bytes or raw.compressed_bytes != report.compressed_bytes:
                raise ArchiveRejected("INVALID_LOCAL_HEADER")
            if report.status == "READY":
                status = "PLANNED"
            elif report.status == "DIRECTORY":
                status = "DIRECTORY"
            elif report.status == "REJECTED":
                status = "REJECTED"
            else:
                raise ArchiveRejected("INVALID_MANIFEST")
            path = report.virtual_path
            entries.append(
                PlanEntry(
                    entry_index=report.entry_index,
                    name_sha256=report.name_sha256,
                    virtual_path=path,
                    declared_bytes=report.declared_bytes,
                    observed_compressed_bytes=compressed,
                    media_type_hint=media_type_hint(path) if path is not None else None,
                    status=status,
                    refusal=report.reason if status == "REJECTED" else None,
                    order=None,
                )
            )
        admitted = sorted(
            (entry for entry in entries if entry.status == "PLANNED"),
            key=lambda entry: entry.virtual_path.encode("utf-8"),
        )
        rank = {entry.entry_index: index for index, entry in enumerate(admitted)}
        entries = [
            replace(entry, order=rank[entry.entry_index]) if entry.entry_index in rank else entry
            for entry in entries
        ]
        return ExtractionPlan(
            source=manifest.source,
            provider_version=manifest.provider_version,
            archive_bytes=len(archive),
            archive_sha256=manifest.source.content_sha256,
            limits=manifest.limits,
            entries=tuple(entries),
            order=tuple(entry.entry_index for entry in admitted),
        )

    def _open_plan(
        self, source: SourceRevision, archive: bytes, deadline: float
    ) -> tuple[zipfile.ZipFile, ExtractionPlan]:
        package = None
        try:
            package, central = self.provider._open(source, archive, deadline)
            manifest = self.provider._manifest(source, archive, package, central, deadline)
            plan = self._from_manifest(manifest, archive, central)
            return package, plan
        except BaseException:
            if package is not None:
                package.close()
            raise

    def plan(self, source: SourceRevision, archive: bytes) -> ExtractionPlan:
        """Build a metadata-only plan; no payload is expanded or written."""

        deadline = time.monotonic() + self.provider.limits.max_seconds
        package, plan = self._open_plan(source, archive, deadline)
        package.close()
        return plan

    def _write_entry(
        self,
        package: zipfile.ZipFile,
        entry: PlanEntry,
        destination: _Destination,
        deadline: float,
        budget: _Budget,
    ) -> tuple[int, str, str | None]:
        limits = self.provider.limits
        if entry.virtual_path is None:
            raise ArchiveRejected("INVALID_PLAN")
        canonical = _virtual_path(entry.virtual_path, limits)
        if canonical != entry.virtual_path:
            raise ArchiveRejected(_REASON_UNSAFE_PATH)
        parts = _path_parts(canonical)
        parent_fd = destination.ensure_parent(parts[:-1])
        try:
            fd = destination.create_file(parent_fd, parts[-1], parts)
            sink = None
            try:
                try:
                    sink = os.fdopen(fd, "wb")
                except OSError as error:
                    raise _destination_rejection(error) from None
                digest = sha256()
                prefix = bytearray()
                written = 0
                try:
                    source_stream = package.open(package.infolist()[entry.entry_index], "r")
                except (
                    zipfile.BadZipFile,
                    NotImplementedError,
                    RuntimeError,
                    EOFError,
                    ValueError,
                    zlib.error,
                    OSError,
                ):
                    raise ArchiveRejected("INVALID_ENTRY") from None
                with source_stream as source, sink:
                    while True:
                        self._check_deadline(deadline)
                        remaining_entry = limits.max_entry_bytes - written
                        remaining_total = budget.remaining()
                        limit = min(_CHUNK_BYTES, remaining_entry + 1, remaining_total + 1)
                        if limit <= 0:
                            raise ArchiveRejected("ENTRY_SIZE_LIMIT")
                        try:
                            chunk = source.read(limit)
                        except OSError:
                            raise ArchiveRejected("INVALID_ENTRY") from None
                        self._check_deadline(deadline)
                        if not chunk:
                            break
                        budget.consume(len(chunk))
                        written += len(chunk)
                        if written > limits.max_entry_bytes:
                            raise ArchiveRejected("ENTRY_SIZE_LIMIT")
                        digest.update(chunk)
                        if len(prefix) < _PREFIX_BYTES:
                            prefix.extend(chunk[: _PREFIX_BYTES - len(prefix)])
                        try:
                            sink.write(chunk)
                        except OSError as error:
                            raise _destination_rejection(error) from None
                if written != entry.declared_bytes:
                    raise ArchiveRejected("ENTRY_SIZE_MISMATCH")
                if _nested(bytes(prefix)):
                    raise ArchiveRejected("NESTED_ARCHIVE")
                # Detect a self-extracting/prefixed ZIP that does not begin with ZIP
                # magic. This reads only the bounded end record of the file just
                # written; no archive body is accumulated in memory.
                try:
                    read_fd = os.open(
                        parts[-1],
                        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | _DESTINATION_O_CLOEXEC,
                        dir_fd=parent_fd,
                    )
                except OSError as error:
                    raise _destination_rejection(error) from None
                try:
                    materialized = os.fdopen(read_fd, "rb")
                except OSError as error:
                    os.close(read_fd)
                    raise _destination_rejection(error) from None
                except BaseException:
                    os.close(read_fd)
                    raise
                with materialized:
                    try:
                        nested = zipfile.is_zipfile(materialized)
                    except OSError as error:
                        raise _destination_rejection(error) from None
                    except (ValueError, struct.error):
                        raise ArchiveRejected("INVALID_ENTRY") from None
                if nested:
                    raise ArchiveRejected("NESTED_ARCHIVE")
                return written, digest.hexdigest(), detect_media_type(canonical, bytes(prefix))
            except BaseException:
                if sink is not None:
                    try:
                        sink.close()
                    except OSError:
                        pass
                destination.remove_file(parent_fd, parts)
                raise
            finally:
                if sink is None:
                    try:
                        os.close(fd)
                    except OSError:
                        pass
        finally:
            os.close(parent_fd)

    def materialize(
        self,
        plan: ExtractionPlan,
        archive: bytes,
        destination_root: str | os.PathLike[str],
    ) -> MaterializationResult:
        """Stream admitted entries under an existing, non-symlink staging root.

        The plan is rebuilt from the exact archive and compared before any write, so a
        stale, forged, or policy-mismatched plan fails closed without touching staging.
        Per-entry refusals settle that entry as ``REJECTED``; archive-level failures
        remove every file/directory this call created.
        """

        if not isinstance(plan, ExtractionPlan):
            raise ArchiveRejected("INVALID_PLAN")
        if type(archive) is not bytes:
            raise ArchiveRejected("IMMUTABLE_BYTES_REQUIRED")
        deadline = time.monotonic() + plan.limits.max_seconds
        package, rebuilt = self._open_plan(plan.source, archive, deadline)
        try:
            if rebuilt != plan:
                raise ArchiveRejected("PLAN_MISMATCH")
            destination = _Destination.open(destination_root)
        except BaseException:
            package.close()
            raise
        try:
            budget = _Budget(rebuilt.limits.max_expanded_bytes)
            outcomes: dict[int, tuple[int, str, str | None] | str] = {}
            for index in rebuilt.order:
                entry = rebuilt.entries[index]
                self._check_deadline(deadline)
                created_dirs_before = len(destination.created_dirs)
                try:
                    if budget.remaining() <= 0:
                        raise ArchiveRejected("EXPANDED_SIZE_LIMIT")
                    outcomes[index] = self._write_entry(
                        package, entry, destination, deadline, budget
                    )
                except ArchiveRejected as error:
                    destination.discard_empty_dirs(created_dirs_before)
                    if error.code == "PROCESSING_TIMEOUT":
                        raise
                    outcomes[index] = error.code
                except (
                    zipfile.BadZipFile,
                    NotImplementedError,
                    RuntimeError,
                    EOFError,
                    ValueError,
                    zlib.error,
                ):
                    destination.discard_empty_dirs(created_dirs_before)
                    outcomes[index] = "INVALID_ENTRY"
            package.close()
            settled_entries: list[PlanEntry] = []
            for entry in rebuilt.entries:
                outcome = outcomes.get(entry.entry_index)
                if entry.status != "PLANNED":
                    settled_entries.append(entry)
                elif isinstance(outcome, tuple):
                    observed, digest, media = outcome
                    settled_entries.append(
                        replace(
                            entry,
                            status="EXTRACTED",
                            observed_bytes=observed,
                            content_sha256=digest,
                            media_type=media,
                        )
                    )
                elif isinstance(outcome, str):
                    settled_entries.append(
                        replace(entry, status="REJECTED", refusal=outcome, order=None)
                    )
                else:
                    raise ArchiveRejected("INVALID_PLAN")
            by_index = {entry.entry_index: entry for entry in settled_entries}
            extracted = [
                by_index[index]
                for index in rebuilt.order
                if by_index[index].status == "EXTRACTED"
            ]
            rank = {entry.entry_index: position for position, entry in enumerate(extracted)}
            settled_entries = [
                replace(entry, order=rank[entry.entry_index])
                if entry.status == "EXTRACTED"
                else entry
                for entry in settled_entries
            ]
            by_index = {entry.entry_index: entry for entry in settled_entries}
            materialized: list[MaterializedEntry] = []
            for entry in extracted:
                settled = by_index[entry.entry_index]
                if (
                    settled.order is None
                    or settled.virtual_path is None
                    or settled.observed_bytes is None
                    or settled.content_sha256 is None
                ):
                    raise ArchiveRejected("INVALID_PLAN")
                materialized.append(
                    MaterializedEntry(
                        order=settled.order,
                        entry_index=settled.entry_index,
                        name_sha256=settled.name_sha256,
                        virtual_path=settled.virtual_path,
                        declared_bytes=settled.declared_bytes,
                        observed_bytes=settled.observed_bytes,
                        observed_compressed_bytes=settled.observed_compressed_bytes,
                        content_sha256=settled.content_sha256,
                        media_type=settled.media_type,
                    )
                )
            settled_plan = ExtractionPlan(
                source=rebuilt.source,
                provider_version=rebuilt.provider_version,
                archive_bytes=rebuilt.archive_bytes,
                archive_sha256=rebuilt.archive_sha256,
                limits=rebuilt.limits,
                entries=tuple(settled_entries),
                order=tuple(entry.entry_index for entry in extracted),
            )
            return MaterializationResult(
                plan=settled_plan,
                entries=tuple(materialized),
                total_observed_bytes=sum(entry.observed_bytes for entry in materialized),
            )
        except BaseException:
            package.close()
            destination.cleanup()
            raise
        else:
            destination.close()

    def extract(
        self,
        source: SourceRevision,
        archive: bytes,
        destination_root: str | os.PathLike[str],
    ) -> MaterializationResult:
        """Convenience wrapper: plan, then materialize the same archive."""

        return self.materialize(self.plan(source, archive), archive, destination_root)
