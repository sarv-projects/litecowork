"""ZIP intake from immutable, already authorized Resource bytes.

All paths are virtual provenance labels. This implementation never calls extract(),
opens a filesystem path, creates Resources, or asserts authorization/Evidence truth.
"""

from dataclasses import dataclass, replace
from hashlib import sha256
from io import BytesIO
import math
import re
import stat
import struct
import time
import unicodedata
import zipfile
import zlib

PROVIDER_VERSION = "litecowork-zip-intake/0.1.0"
_ARCHIVE_SUFFIXES = (".zip", ".zipx", ".jar", ".war", ".apk", ".tar", ".tgz", ".gz", ".bz2", ".xz", ".7z", ".rar")
_ARCHIVE_MAGIC = (b"PK\x03\x04", b"PK\x05\x06", b"PK\x07\x08", b"\x1f\x8b", b"BZh", b"\xfd7zXZ\x00", b"7z\xbc\xaf\x27\x1c", b"Rar!\x1a\x07")
_RESERVED = re.compile(r"^(CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])(?:\.|$)", re.IGNORECASE)


class ArchiveRejected(ValueError):
    """Safe reason code, deliberately excluding parser messages and raw names."""

    def __init__(self, code: str):
        self.code = code
        super().__init__(code)


@dataclass(frozen=True)
class Limits:
    max_archive_bytes: int = 16 * 1024 * 1024
    max_entries: int = 2048
    max_entry_bytes: int = 8 * 1024 * 1024
    max_expanded_bytes: int = 32 * 1024 * 1024
    max_compression_ratio: int = 100
    max_path_bytes: int = 1024
    max_path_depth: int = 32
    max_seconds: float = 10.0

    def __post_init__(self):
        for field in ("max_archive_bytes", "max_entries", "max_entry_bytes", "max_expanded_bytes", "max_compression_ratio", "max_path_bytes", "max_path_depth"):
            value = getattr(self, field)
            if type(value) is not int or value <= 0:
                raise ValueError("limits must be positive integers")
        if isinstance(self.max_seconds, bool) or not isinstance(self.max_seconds, (int, float)) or not math.isfinite(self.max_seconds) or self.max_seconds <= 0:
            raise ValueError("max_seconds must be finite and positive")


@dataclass(frozen=True)
class SourceRevision:
    workspace_id: str
    resource_id: str
    revision_id: str
    content_sha256: str

    def __post_init__(self):
        if any(not isinstance(value, str) or not value or len(value) > 256 or any(unicodedata.category(char).startswith("C") for char in value) for value in (self.workspace_id, self.resource_id, self.revision_id)):
            raise ValueError("source must identify one pinned Workspace Resource revision")
        if not isinstance(self.content_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", self.content_sha256):
            raise ValueError("source requires a lowercase SHA-256 content digest")


@dataclass(frozen=True)
class EntryReport:
    entry_index: int
    name_sha256: str
    virtual_path: str | None
    compressed_bytes: int
    declared_bytes: int
    crc32: int
    status: str  # READY, DIRECTORY, REJECTED, EXTRACTED
    reason: str | None = None
    observed_bytes: int | None = None
    content_sha256: str | None = None


@dataclass(frozen=True)
class Manifest:
    source: SourceRevision
    provider_version: str
    archive_bytes: int
    limits: Limits
    entries: tuple[EntryReport, ...]


@dataclass(frozen=True)
class ExtractedEntry:
    entry_index: int
    virtual_path: str
    content_sha256: str
    content: bytes


@dataclass(frozen=True)
class ExtractionResult:
    manifest: Manifest
    entries: tuple[ExtractedEntry, ...]


@dataclass(frozen=True)
class _CentralEntry:
    raw_name: bytes
    decoded_name: str
    flags: int
    compression: int
    crc32: int
    compressed_bytes: int
    declared_bytes: int
    external_attr: int
    header_offset: int
    extra: bytes
    reason: str | None


def _virtual_path(raw: str, limits: Limits) -> str:
    # ZIP permits backslashes as characters; reject them to prevent cross-OS escapes.
    if not raw or raw.startswith("/") or "\\" in raw or ":" in raw:
        raise ArchiveRejected("UNSAFE_PATH")
    if any(unicodedata.category(char).startswith("C") for char in raw):
        raise ArchiveRejected("UNSAFE_PATH")
    path = unicodedata.normalize("NFC", raw)
    if path.endswith("/"):
        path = path[:-1]
    parts = path.split("/")
    if len(parts) > limits.max_path_depth or len(path.encode("utf-8")) > limits.max_path_bytes:
        raise ArchiveRejected("PATH_LIMIT")
    if any(not part or part in (".", "..") or part.endswith((" ", ".")) or any(char in part for char in '<>"|?*') or _RESERVED.match(part) for part in parts):
        raise ArchiveRejected("UNSAFE_PATH")
    return path


def _nested(content: bytes) -> bool:
    return content.startswith(_ARCHIVE_MAGIC) or content[257:262] == b"ustar"


class ZipIntakeProvider:
    """Read-only provider port. Caller supplies policy limits and resolved bytes.

    preview() only admits metadata; READY is never a parsed/searchable claim.
    extract() repeats all checks on the exact bytes, then validates bounded streams.
    Run in an isolated supervised process before exposing this to untrusted callers.
    """

    def __init__(self, limits: Limits | None = None):
        self.limits = limits if limits is not None else Limits()

    def _check_deadline(self, deadline: float):
        if time.monotonic() >= deadline:
            raise ArchiveRejected("PROCESSING_TIMEOUT")

    def _central_entries(self, archive: bytes, start: int, end: int, count: int, deadline: float) -> tuple[_CentralEntry, ...]:
        entries = []
        position = start
        for _ in range(count):
            self._check_deadline(deadline)
            if end - position < 46:
                raise ArchiveRejected("INVALID_CENTRAL_DIRECTORY")
            (signature, made_by, needed, flags, compression, modified_time,
             modified_date, crc, compressed, expanded, name_size, extra_size,
             comment_size, disk, internal_attr, external_attr, local_offset) = struct.unpack_from("<4s6H3I5H2I", archive, position)
            if signature != b"PK\x01\x02" or disk:
                raise ArchiveRejected("INVALID_CENTRAL_DIRECTORY")
            variable_size = name_size + extra_size + comment_size
            if variable_size > end - position - 46:
                raise ArchiveRejected("INVALID_CENTRAL_DIRECTORY")
            if compressed == 0xFFFFFFFF or expanded == 0xFFFFFFFF or local_offset == 0xFFFFFFFF:
                raise ArchiveRejected("ZIP64_UNSUPPORTED")
            raw_name = archive[position + 46:position + 46 + name_size]
            extra = archive[position + 46 + name_size:position + 46 + name_size + extra_size]
            encoding = "utf-8" if flags & 0x800 else "cp437"
            try:
                decoded = raw_name.decode(encoding, "strict")
            except UnicodeDecodeError:
                raise ArchiveRejected("INVALID_ENTRY_NAME") from None
            if decoded.encode(encoding, "strict") != raw_name:
                raise ArchiveRejected("AMBIGUOUS_ENTRY_NAME")
            reason = "UNSAFE_PATH" if b"\x00" in raw_name else None
            # Check local names as bytes too: decoded-string equality could hide
            # truncation or different encoded names. No offset may leave the local
            # file region, and compressed payloads must stop before the directory.
            if local_offset > start or start - local_offset < 30:
                raise ArchiveRejected("INVALID_LOCAL_HEADER")
            (local_signature, local_needed, local_flags, local_compression,
             local_time, local_date, local_crc, local_compressed, local_expanded,
             local_name_size, local_extra_size) = struct.unpack_from("<4s5H3I2H", archive, local_offset)
            if local_signature != b"PK\x03\x04":
                raise ArchiveRejected("INVALID_LOCAL_HEADER")
            local_variable = local_name_size + local_extra_size
            if local_variable > start - local_offset - 30:
                raise ArchiveRejected("INVALID_LOCAL_HEADER")
            data_offset = local_offset + 30 + local_variable
            if compressed > start - data_offset:
                raise ArchiveRejected("INVALID_LOCAL_HEADER")
            local_name = archive[local_offset + 30:local_offset + 30 + local_name_size]
            if local_name != raw_name:
                reason = "RAW_NAME_MISMATCH"
            if local_flags != flags or local_compression != compression or local_needed != needed:
                reason = "ENTRY_METADATA_MISMATCH"
            if not flags & 8 and (local_crc, local_compressed, local_expanded) != (crc, compressed, expanded):
                reason = "ENTRY_METADATA_MISMATCH"
            entries.append(_CentralEntry(raw_name, decoded, flags, compression, crc, compressed, expanded, external_attr, local_offset, extra, reason))
            position += 46 + variable_size
        if position != end:
            raise ArchiveRejected("INVALID_CENTRAL_DIRECTORY")
        return tuple(entries)

    def _open(self, source: SourceRevision, archive: bytes, deadline: float) -> tuple[zipfile.ZipFile, tuple[_CentralEntry, ...]]:
        if type(archive) is not bytes:
            raise ArchiveRejected("IMMUTABLE_BYTES_REQUIRED")
        if len(archive) > self.limits.max_archive_bytes:
            raise ArchiveRejected("ARCHIVE_SIZE_LIMIT")
        if sha256(archive).hexdigest() != source.content_sha256:
            raise ArchiveRejected("SOURCE_DIGEST_MISMATCH")
        # Exclude self-extracting/prefixed archives and multipart transports.
        if not archive.startswith((b"PK\x03\x04", b"PK\x05\x06")):
            raise ArchiveRejected("INVALID_ZIP")
        # Inspect the bounded end record before zipfile allocates entry objects.
        end = archive.rfind(b"PK\x05\x06", max(0, len(archive) - 65557))
        if end < 0 or len(archive) - end < 22:
            raise ArchiveRejected("INVALID_ZIP")
        disk, central_disk, disk_entries, total_entries, central_size, central_offset, comment_size = struct.unpack_from("<HHHHIIH", archive, end + 4)
        if end + 22 + comment_size != len(archive):
            raise ArchiveRejected("INVALID_ZIP")
        if disk or central_disk or disk_entries != total_entries:
            raise ArchiveRejected("MULTIPART_ARCHIVE")
        if total_entries == 0xFFFF or central_size == 0xFFFFFFFF or central_offset == 0xFFFFFFFF:
            raise ArchiveRejected("ZIP64_UNSUPPORTED")
        if total_entries > self.limits.max_entries:
            raise ArchiveRejected("ENTRY_COUNT_LIMIT")
        if central_offset + central_size != end:
            raise ArchiveRejected("INVALID_ZIP")
        central = self._central_entries(archive, central_offset, end, total_entries, deadline)
        try:
            package = zipfile.ZipFile(BytesIO(archive), "r")
            if len(package.infolist()) != total_entries:
                package.close()
                raise ArchiveRejected("INVALID_ZIP")
            return package, central
        except (zipfile.BadZipFile, ValueError, OverflowError):
            raise ArchiveRejected("INVALID_ZIP") from None

    def _manifest(self, source: SourceRevision, archive: bytes, package: zipfile.ZipFile, central: tuple[_CentralEntry, ...], deadline: float) -> Manifest:
        self._check_deadline(deadline)
        infos = package.infolist()
        if len(infos) > self.limits.max_entries:
            raise ArchiveRejected("ENTRY_COUNT_LIMIT")
        if sum(info.file_size for info in infos) > self.limits.max_expanded_bytes:
            raise ArchiveRejected("EXPANDED_SIZE_LIMIT")
        reports = []
        for index, info in enumerate(infos):
            self._check_deadline(deadline)
            raw = central[index]
            path = None
            reason = None
            try:
                if (info.flag_bits, info.compress_type, info.CRC, info.compress_size,
                    info.file_size, info.external_attr, info.header_offset, info.extra) != (
                    raw.flags, raw.compression, raw.crc32, raw.compressed_bytes,
                    raw.declared_bytes, raw.external_attr, raw.header_offset, raw.extra):
                    raise ArchiveRejected("ENTRY_METADATA_MISMATCH")
                if raw.reason:
                    raise ArchiveRejected(raw.reason)
                path = _virtual_path(raw.decoded_name, self.limits)
                if info.filename != raw.decoded_name or info.orig_filename != raw.decoded_name:
                    raise ArchiveRejected("RAW_NAME_MISMATCH")
                mode = info.external_attr >> 16
                kind = stat.S_IFMT(mode)
                if kind not in (0, stat.S_IFREG, stat.S_IFDIR):
                    raise ArchiveRejected("SPECIAL_FILE")
                offset = 0
                while offset < len(info.extra):
                    if len(info.extra) - offset < 4:
                        raise ArchiveRejected("INVALID_ENTRY_METADATA")
                    tag, length = struct.unpack_from("<HH", info.extra, offset)
                    offset += 4
                    if offset + length > len(info.extra):
                        raise ArchiveRejected("INVALID_ENTRY_METADATA")
                    # Unix/ASi link-target extensions have ambiguous link semantics.
                    # No external attributes or extra fields may reintroduce links.
                    if tag in (0x000D, 0x756E):
                        raise ArchiveRejected("UNSUPPORTED_LINK_METADATA")
                    # Alternate filename encodings are not silently chosen or
                    # ignored; callers must never observe two names for one entry.
                    if tag in (0x7075, 0x0008):
                        raise ArchiveRejected("AMBIGUOUS_ENTRY_NAME")
                    offset += length
                if kind == stat.S_IFDIR and not info.is_dir() or kind == stat.S_IFREG and info.is_dir():
                    raise ArchiveRejected("FILE_TYPE_MISMATCH")
                if info.flag_bits & (1 | 0x40):
                    raise ArchiveRejected("ENCRYPTED_ENTRY")
                if info.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                    raise ArchiveRejected("UNSUPPORTED_COMPRESSION")
                if info.file_size > self.limits.max_entry_bytes:
                    raise ArchiveRejected("ENTRY_SIZE_LIMIT")
                if info.file_size > self.limits.max_compression_ratio * max(info.compress_size, 1):
                    raise ArchiveRejected("COMPRESSION_RATIO_LIMIT")
                if info.is_dir() and info.file_size:
                    raise ArchiveRejected("DIRECTORY_HAS_CONTENT")
                if not info.is_dir() and path.casefold().endswith(_ARCHIVE_SUFFIXES):
                    raise ArchiveRejected("NESTED_ARCHIVE")
            except ArchiveRejected as error:
                reason = error.code
                if reason in ("RAW_NAME_MISMATCH", "AMBIGUOUS_ENTRY_NAME", "ENTRY_METADATA_MISMATCH"):
                    path = None
            reports.append(EntryReport(index, sha256(raw.raw_name).hexdigest(), path, raw.compressed_bytes, raw.declared_bytes, raw.crc32, "REJECTED" if reason else "DIRECTORY" if info.is_dir() else "READY", reason))
        # Every duplicate is rejected, including already-rejected entries; none wins.
        paths: dict[str, list[int]] = {}
        for report in reports:
            if report.virtual_path is not None:
                key = unicodedata.normalize("NFKC", report.virtual_path).casefold()
                paths.setdefault(key, []).append(report.entry_index)
        conflicting = set()
        for key, indices in paths.items():
            if len(indices) > 1:
                conflicting.update(indices)
            parts = key.split("/")
            for depth in range(1, len(parts)):
                ancestors = paths.get("/".join(parts[:depth]), [])
                files = [index for index in ancestors if not infos[index].is_dir()]
                if files:
                    conflicting.update(files)
                    conflicting.update(indices)
        for index in conflicting:
            reports[index] = replace(reports[index], status="REJECTED", reason="PATH_CONFLICT")
        return Manifest(source, PROVIDER_VERSION, len(archive), self.limits, tuple(reports))

    def preview(self, source: SourceRevision, archive: bytes) -> Manifest:
        deadline = time.monotonic() + self.limits.max_seconds
        package, central = self._open(source, archive, deadline)
        with package:
            return self._manifest(source, archive, package, central, deadline)

    def extract(self, source: SourceRevision, archive: bytes) -> ExtractionResult:
        deadline = time.monotonic() + self.limits.max_seconds
        package, central = self._open(source, archive, deadline)
        with package:
            manifest = self._manifest(source, archive, package, central, deadline)
            reports = list(manifest.entries)
            outputs = []
            used = 0
            for report in manifest.entries:
                if report.status != "READY":
                    continue
                content = bytearray()
                try:
                    self._check_deadline(deadline)
                    if used > self.limits.max_expanded_bytes:
                        raise ArchiveRejected("EXPANDED_SIZE_LIMIT")
                    with package.open(package.infolist()[report.entry_index], "r") as entry:
                        while True:
                            self._check_deadline(deadline)
                            # One sentinel byte permits detecting budget overruns without
                            # materializing an unbounded expansion or trusting metadata.
                            remaining = min(self.limits.max_entry_bytes - len(content), self.limits.max_expanded_bytes - used)
                            chunk = entry.read(min(64 * 1024, remaining + 1))
                            self._check_deadline(deadline)
                            if not chunk:
                                break
                            used += len(chunk)
                            if used > self.limits.max_expanded_bytes:
                                raise ArchiveRejected("EXPANDED_SIZE_LIMIT")
                            if len(content) + len(chunk) > self.limits.max_entry_bytes:
                                raise ArchiveRejected("ENTRY_SIZE_LIMIT")
                            content.extend(chunk)
                            if _nested(content[:512]):
                                raise ArchiveRejected("NESTED_ARCHIVE")
                    if len(content) != report.declared_bytes:
                        raise ArchiveRejected("ENTRY_SIZE_MISMATCH")
                    # A self-extracting or prefixed ZIP lacks leading ZIP magic.
                    # Inspect its bounded end record before returning any payload.
                    if zipfile.is_zipfile(BytesIO(content)):
                        raise ArchiveRejected("NESTED_ARCHIVE")
                    self._check_deadline(deadline)
                    digest = sha256(content).hexdigest()
                    reports[report.entry_index] = replace(report, status="EXTRACTED", observed_bytes=len(content), content_sha256=digest)
                    outputs.append(ExtractedEntry(report.entry_index, report.virtual_path, digest, bytes(content)))
                except ArchiveRejected as error:
                    reports[report.entry_index] = replace(report, status="REJECTED", reason=error.code)
                except (zipfile.BadZipFile, NotImplementedError, RuntimeError, EOFError, ValueError, zlib.error):
                    reports[report.entry_index] = replace(report, status="REJECTED", reason="INVALID_ENTRY")
            return ExtractionResult(replace(manifest, entries=tuple(reports)), tuple(outputs))
