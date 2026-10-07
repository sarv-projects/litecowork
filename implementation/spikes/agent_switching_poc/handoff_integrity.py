"""Checkpoint file-manifest validation for the disposable SP04 prototype."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path, PurePosixPath
from typing import Iterable


class CheckpointIntegrityError(ValueError):
    """The checkpoint manifest is malformed or no longer matches its files."""


_SHA256 = re.compile(r"^[0-9a-f]{64}$")


def build_checkpoint_manifest(
    root: str | Path,
    relative_paths: Iterable[str],
) -> dict[str, object]:
    root_path = _resolve_root(root)
    paths = list(relative_paths)
    if not paths:
        raise CheckpointIntegrityError("checkpoint manifest must include at least one file")

    files: dict[str, str] = {}
    for relative_path in paths:
        normalized = _normalize_relative_path(relative_path)
        if normalized in files:
            raise CheckpointIntegrityError(f"duplicate checkpoint path: {normalized}")
        files[normalized] = _hash_file(root_path, normalized)

    return {
        "files": dict(sorted(files.items())),
        "manifest_digest": _hash_manifest(files),
    }


def verify_checkpoint_manifest(
    root: str | Path,
    manifest: object,
) -> str:
    root_path = _resolve_root(root)
    if not isinstance(manifest, dict) or set(manifest) != {"files", "manifest_digest"}:
        raise CheckpointIntegrityError("checkpoint manifest has an invalid shape")

    files = manifest["files"]
    digest = manifest["manifest_digest"]
    if not isinstance(files, dict) or not files:
        raise CheckpointIntegrityError("checkpoint manifest must list at least one file")
    if not isinstance(digest, str) or not _SHA256.fullmatch(digest):
        raise CheckpointIntegrityError("checkpoint manifest digest must be lowercase SHA-256")

    expected_files: dict[str, str] = {}
    for relative_path, expected_digest in files.items():
        normalized = _normalize_relative_path(relative_path)
        if normalized in expected_files:
            raise CheckpointIntegrityError(f"duplicate checkpoint path: {normalized}")
        if not isinstance(expected_digest, str) or not _SHA256.fullmatch(expected_digest):
            raise CheckpointIntegrityError(f"invalid SHA-256 for checkpoint path: {normalized}")
        expected_files[normalized] = expected_digest

    if _hash_manifest(expected_files) != digest:
        raise CheckpointIntegrityError("checkpoint manifest digest does not match its file list")

    actual_files = {
        relative_path: _hash_file(root_path, relative_path)
        for relative_path in sorted(expected_files)
    }
    if actual_files != expected_files:
        raise CheckpointIntegrityError("checkpoint file content does not match its manifest")
    return digest


def _resolve_root(root: str | Path) -> Path:
    try:
        resolved = Path(root).resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise CheckpointIntegrityError("checkpoint root is unavailable") from error
    if not resolved.is_dir():
        raise CheckpointIntegrityError("checkpoint root must be a directory")
    return resolved


def _normalize_relative_path(relative_path: object) -> str:
    if (
        not isinstance(relative_path, str)
        or not relative_path
        or "\\" in relative_path
        or "\x00" in relative_path
    ):
        raise CheckpointIntegrityError("checkpoint paths must be non-empty POSIX relative paths")
    path = PurePosixPath(relative_path)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise CheckpointIntegrityError("checkpoint path escapes or aliases its root")
    normalized = path.as_posix()
    if normalized in {"", "."}:
        raise CheckpointIntegrityError("checkpoint path must name a file")
    return normalized


def _hash_file(root: Path, relative_path: str) -> str:
    candidate = root.joinpath(*PurePosixPath(relative_path).parts)
    current = root
    for part in PurePosixPath(relative_path).parts:
        current = current / part
        if current.is_symlink():
            raise CheckpointIntegrityError(f"symlink in checkpoint path: {relative_path}")
    try:
        resolved = candidate.resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise CheckpointIntegrityError(f"checkpoint file is unavailable: {relative_path}") from error
    if root not in resolved.parents or not resolved.is_file():
        raise CheckpointIntegrityError(f"checkpoint path is not a regular file: {relative_path}")
    try:
        digest = hashlib.sha256()
        with resolved.open("rb") as file:
            while chunk := file.read(1024 * 1024):
                digest.update(chunk)
        return digest.hexdigest()
    except OSError as error:
        raise CheckpointIntegrityError(f"checkpoint file cannot be read: {relative_path}") from error


def _hash_manifest(files: dict[str, str]) -> str:
    payload = json.dumps(files, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()
