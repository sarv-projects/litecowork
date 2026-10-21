"""Bounded ZIP capability implementation; no filesystem or Core authority."""

from .provider import (
    ArchiveRejected,
    EntryReport,
    ExtractionResult,
    ExtractedEntry,
    Limits,
    Manifest,
    SourceRevision,
    ZipIntakeProvider,
)
from .extraction import (
    BoundedZipExtractor,
    ExtractionPlan,
    MaterializationResult,
    MaterializedEntry,
    PlanEntry,
    detect_media_type,
    media_type_hint,
)
from .supervisor import WorkerCancelled, WorkerFailed, WorkerTimedOut, WorkerUnavailable, ZipWorker

__all__ = [
    "ArchiveRejected", "BoundedZipExtractor", "EntryReport", "ExtractionPlan",
    "ExtractionResult", "ExtractedEntry", "Limits", "Manifest", "MaterializationResult",
    "MaterializedEntry", "PlanEntry", "SourceRevision", "ZipIntakeProvider", "ZipWorker",
    "WorkerCancelled", "WorkerFailed", "WorkerTimedOut", "WorkerUnavailable",
    "detect_media_type", "media_type_hint",
]
