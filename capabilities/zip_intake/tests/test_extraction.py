"""Bounded extraction-plan and staging-root materialization tests.

Fixtures are constructed in memory; no archive or extracted file is committed.
"""

from dataclasses import replace
from hashlib import sha256
from io import BytesIO
import os
from pathlib import Path
import stat
import struct
import tempfile
import unittest
from unittest.mock import patch
import warnings
import zipfile

from capabilities.zip_intake import (
    ArchiveRejected,
    BoundedZipExtractor,
    Limits,
    SourceRevision,
    ZipIntakeProvider,
    detect_media_type,
)


def make_zip(entries, compression=zipfile.ZIP_STORED):
    stream = BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        with zipfile.ZipFile(stream, "w", compression=compression) as package:
            for name, content in entries:
                package.writestr(name, content)
    return stream.getvalue()


def source(data):
    return SourceRevision("workspace-1", "resource-1", "revision-1", sha256(data).hexdigest())


def special(name, kind):
    info = zipfile.ZipInfo(name)
    info.create_system = 3
    info.external_attr = (kind | 0o777) << 16
    return info


def with_extra(name, tag, payload):
    info = zipfile.ZipInfo(name)
    info.extra = struct.pack("<HH", tag, len(payload)) + payload
    return info


class ExtractionPlanTests(unittest.TestCase):
    def extractor(self, **limits):
        return BoundedZipExtractor(ZipIntakeProvider(Limits(**limits)))

    def test_traversal_and_absolute_paths_are_refused_before_any_write(self):
        hostile = (
            "../outside.txt",
            "a/../../outside.txt",
            "a/../b.txt",
            "/absolute.txt",
            "C:/windows.txt",
            "//server/share.txt",
            "a\\..\\escape.txt",
            "a//b.txt",
        )
        for name in hostile:
            with self.subTest(name=name):
                data = make_zip([(name, b"payload"), ("safe.txt", b"safe")])
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary, "root")
                    root.mkdir()
                    plan = self.extractor().plan(source(data), data)
                    refused = plan.entries[0]
                    self.assertEqual(refused.status, "REJECTED")
                    self.assertEqual(refused.refusal, "UNSAFE_PATH")
                    self.assertIsNone(refused.virtual_path)
                    self.assertIsNone(refused.order)
                    result = BoundedZipExtractor().materialize(plan, data, root)
                    self.assertEqual([entry.virtual_path for entry in result.entries], ["safe.txt"])
                    self.assertEqual((root / "safe.txt").read_bytes(), b"safe")
                    self.assertFalse((Path(temporary) / "outside.txt").exists())

    def test_symlink_hardlink_and_device_entries_are_refused(self):
        cases = (
            (special("link.txt", stat.S_IFLNK), b"target.txt", "SPECIAL_FILE"),
            (special("device.txt", stat.S_IFCHR), b"device", "SPECIAL_FILE"),
            (special("pipe.txt", stat.S_IFIFO), b"pipe", "SPECIAL_FILE"),
            (special("socket.txt", stat.S_IFSOCK), b"socket", "SPECIAL_FILE"),
            (with_extra("hardlink.txt", 0x756E, b"\x00" * 12), b"target", "UNSUPPORTED_LINK_METADATA"),
            (with_extra("pkware-link.txt", 0x000D, b"\x00" * 12), b"target", "UNSUPPORTED_LINK_METADATA"),
        )
        for info, content, reason in cases:
            with self.subTest(name=info.filename):
                data = make_zip([(info, content)])
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary, "root")
                    root.mkdir()
                    plan = self.extractor().plan(source(data), data)
                    self.assertEqual(plan.order, ())
                    self.assertEqual(plan.entries[0].status, "REJECTED")
                    self.assertEqual(plan.entries[0].refusal, reason)
                    result = BoundedZipExtractor().materialize(plan, data, root)
                    self.assertEqual(result.entries, ())
                    self.assertEqual(os.listdir(root), [])

    def test_duplicate_normalized_paths_and_case_collisions_are_refused(self):
        cases = (
            [("caf\u00e9.txt", b"1"), ("cafe\u0301.txt", b"2")],
            [("FILE.txt", b"1"), ("file.txt", b"2")],
            [("\uff26\uff29\uff2c\uff25.txt", b"1"), ("file.txt", b"2")],
            [("dup.txt", b"1"), ("dup.txt", b"2")],
            [("folder", b"1"), ("folder/file.txt", b"2")],
        )
        for entries in cases:
            with self.subTest(entries=entries):
                data = make_zip(entries)
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary, "root")
                    root.mkdir()
                    plan = self.extractor().plan(source(data), data)
                    self.assertEqual(plan.order, ())
                    self.assertTrue(all(entry.status == "REJECTED" for entry in plan.entries))
                    self.assertTrue(all(entry.refusal == "PATH_CONFLICT" for entry in plan.entries))
                    result = BoundedZipExtractor().materialize(plan, data, root)
                    self.assertEqual(result.entries, ())
                    self.assertEqual(os.listdir(root), [])

    def test_compression_ratio_bomb_is_refused_without_writing(self):
        data = make_zip([("bomb.txt", b"A" * 100_000)], zipfile.ZIP_DEFLATED)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            extractor = self.extractor(max_compression_ratio=2)
            plan = extractor.plan(source(data), data)
            self.assertEqual(plan.order, ())
            self.assertEqual(plan.entries[0].status, "REJECTED")
            self.assertEqual(plan.entries[0].refusal, "COMPRESSION_RATIO_LIMIT")
            result = extractor.materialize(plan, data, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(os.listdir(root), [])

    def test_total_expanded_budget_is_enforced_at_admission_and_streaming(self):
        data = make_zip([("a.txt", b"12"), ("b.txt", b"34")])
        with self.assertRaises(ArchiveRejected) as error:
            self.extractor(max_expanded_bytes=3).plan(source(data), data)
        self.assertEqual(error.exception.code, "EXPANDED_SIZE_LIMIT")

        observed = make_zip([("a.txt", b"a")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            extractor = self.extractor(max_expanded_bytes=4)
            plan = extractor.plan(source(observed), observed)
            with patch(
                "capabilities.zip_intake.provider.zipfile.ZipFile.open",
                return_value=BytesIO(b"123456"),
            ):
                result = extractor.materialize(plan, observed, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(result.plan.entries[0].status, "REJECTED")
            self.assertEqual(result.plan.entries[0].refusal, "EXPANDED_SIZE_LIMIT")
            self.assertEqual(os.listdir(root), [])

    def test_per_entry_observed_size_is_enforced_independently_of_declared_size(self):
        data = make_zip([("a.txt", b"a")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            extractor = self.extractor(max_entry_bytes=3)
            plan = extractor.plan(source(data), data)
            with patch(
                "capabilities.zip_intake.provider.zipfile.ZipFile.open",
                return_value=BytesIO(b"123456"),
            ):
                result = extractor.materialize(plan, data, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(result.plan.entries[0].refusal, "ENTRY_SIZE_LIMIT")
            self.assertEqual(os.listdir(root), [])

    def test_ordering_and_digests_are_deterministic_across_calls_and_compression(self):
        entries = [("z.txt", b"z" * 200), ("a.txt", b"a" * 200), ("m/n.txt", b"n" * 200)]
        stored = make_zip(entries, zipfile.ZIP_STORED)
        deflated = make_zip(entries, zipfile.ZIP_DEFLATED)
        extractor = BoundedZipExtractor()
        first = extractor.plan(source(stored), stored)
        second = extractor.plan(source(stored), stored)
        self.assertEqual(first, second)
        self.assertEqual(
            [first.entries[index].virtual_path for index in first.order],
            ["a.txt", "m/n.txt", "z.txt"],
        )
        self.assertEqual(first.entries[0].name_sha256, sha256(b"z.txt").hexdigest())
        self.assertEqual(first.entries[0].observed_compressed_bytes, 200)
        deflated_plan = extractor.plan(source(deflated), deflated)
        self.assertLess(deflated_plan.entries[0].observed_compressed_bytes, 200)
        with tempfile.TemporaryDirectory() as temporary:
            root_a = Path(temporary, "a")
            root_b = Path(temporary, "b")
            root_c = Path(temporary, "c")
            for root in (root_a, root_b, root_c):
                root.mkdir()
            result_a = extractor.materialize(first, stored, root_a)
            result_b = extractor.materialize(second, stored, root_b)
            result_c = extractor.materialize(extractor.plan(source(deflated), deflated), deflated, root_c)
        self.assertEqual(
            [(entry.virtual_path, entry.content_sha256) for entry in result_a.entries],
            [(entry.virtual_path, entry.content_sha256) for entry in result_b.entries],
        )
        self.assertEqual(
            [(entry.virtual_path, entry.content_sha256) for entry in result_a.entries],
            [(entry.virtual_path, entry.content_sha256) for entry in result_c.entries],
        )
        self.assertNotEqual(first.archive_sha256, extractor.plan(source(deflated), deflated).archive_sha256)
        self.assertEqual(result_a.entries[0].content_sha256, sha256(b"a" * 200).hexdigest())
        self.assertEqual(result_a.entries[0].observed_bytes, 200)

    def test_disguised_and_prefixed_nested_archives_are_refused_streaming(self):
        nested = make_zip([("inside.txt", b"inside")])
        for name, content in (("disguised.txt", nested), ("prefixed.dat", b"stub" + nested)):
            with self.subTest(name=name):
                data = make_zip([(name, content), ("safe.txt", b"safe")])
                with tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary, "root")
                    root.mkdir()
                    extractor = BoundedZipExtractor()
                    plan = extractor.plan(source(data), data)
                    result = extractor.materialize(plan, data, root)
                    self.assertEqual(result.plan.entries[0].refusal, "NESTED_ARCHIVE")
                    self.assertEqual([entry.virtual_path for entry in result.entries], ["safe.txt"])
                    self.assertFalse((root / name).exists())
                    self.assertEqual((root / "safe.txt").read_bytes(), b"safe")

    def test_rejected_entry_removes_empty_directories_it_created(self):
        nested = make_zip([("inside.txt", b"inside")])
        data = make_zip([("folder/disguised.txt", nested)])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            result = BoundedZipExtractor().extract(source(data), data, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(result.plan.entries[0].refusal, "NESTED_ARCHIVE")
            self.assertEqual(os.listdir(root), [])

    def test_media_type_is_a_bounded_hint_and_content_checked(self):
        entries = [
            ("doc.pdf", b"%PDF-1.7\n"),
            ("fake.pdf", b"not-a-pdf"),
            ("image.png", b"\x89PNG\r\n\x1a\n" + b"0" * 8),
            ("notes.txt", b"hello\n"),
            ("empty.txt", b""),
            ("renamed.bin", b"%PDF-1.7\n"),
            ("payload.unknown", b"\x00\x01"),
        ]
        data = make_zip(entries)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            extractor = BoundedZipExtractor()
            result = extractor.extract(source(data), data, root)
        by_path = {entry.virtual_path: entry for entry in result.entries}
        self.assertEqual(by_path["doc.pdf"].media_type, "application/pdf")
        self.assertIsNone(by_path["fake.pdf"].media_type)
        self.assertEqual(by_path["image.png"].media_type, "image/png")
        self.assertEqual(by_path["notes.txt"].media_type, "text/plain")
        self.assertEqual(by_path["empty.txt"].media_type, "text/plain")
        self.assertEqual(by_path["renamed.bin"].media_type, "application/pdf")
        self.assertIsNone(by_path["payload.unknown"].media_type)
        self.assertEqual(detect_media_type("fake.pdf", b"plain"), None)
        self.assertEqual(detect_media_type("fake.pdf", b"%PDF-"), "application/pdf")

    def test_empty_archive_and_empty_file_are_settled(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            data = make_zip([])
            extractor = BoundedZipExtractor()
            result = extractor.extract(source(data), data, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(result.total_observed_bytes, 0)
            self.assertEqual(os.listdir(root), [])

            data = make_zip([("empty.txt", b"")])
            result = extractor.extract(source(data), data, root)
            self.assertEqual(len(result.entries), 1)
            self.assertEqual(result.entries[0].observed_bytes, 0)
            self.assertEqual(result.entries[0].content_sha256, sha256(b"").hexdigest())
            self.assertEqual((root / "empty.txt").read_bytes(), b"")

    def test_plan_mismatch_and_source_digest_mismatch_touch_no_staging_paths(self):
        data = make_zip([("a.txt", b"a")])
        weak = BoundedZipExtractor(ZipIntakeProvider(Limits(max_entries=10)))
        plan = weak.plan(source(data), data)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            strict = BoundedZipExtractor(ZipIntakeProvider(Limits(max_entries=20)))
            with self.assertRaises(ArchiveRejected) as error:
                strict.materialize(plan, data, root)
            self.assertEqual(error.exception.code, "PLAN_MISMATCH")
            with self.assertRaises(ArchiveRejected) as error:
                BoundedZipExtractor().materialize(
                    replace(plan, provider_version="other-provider"),
                    data,
                    root,
                )
            self.assertEqual(error.exception.code, "PLAN_MISMATCH")
            other = make_zip([("other.txt", b"other")])
            with self.assertRaises(ArchiveRejected) as error:
                BoundedZipExtractor().materialize(plan, other, root)
            self.assertEqual(error.exception.code, "SOURCE_DIGEST_MISMATCH")
            self.assertEqual(os.listdir(root), [])


class DestinationContainmentTests(unittest.TestCase):
    def test_never_writes_outside_root_or_follows_a_link(self):
        data = make_zip([("escape/pwned.txt", b"pwned"), ("safe.txt", b"safe")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            outside = Path(temporary, "outside")
            root.mkdir()
            outside.mkdir()
            (outside / "target.txt").write_bytes(b"outside")
            os.symlink(outside, root / "escape")
            result = BoundedZipExtractor().extract(source(data), data, root)
            self.assertEqual([entry.virtual_path for entry in result.entries], ["safe.txt"])
            self.assertEqual(result.plan.entries[0].refusal, "DESTINATION_ESCAPE")
            self.assertFalse((outside / "pwned.txt").exists())
            self.assertEqual((root / "safe.txt").read_bytes(), b"safe")
            self.assertEqual((outside / "target.txt").read_bytes(), b"outside")

    def test_existing_symlink_target_is_refused_without_following(self):
        data = make_zip([("link.txt", b"pwned")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            outside = Path(temporary, "outside")
            root.mkdir()
            outside.mkdir()
            (outside / "target.txt").write_bytes(b"outside")
            os.symlink(outside / "target.txt", root / "link.txt")
            result = BoundedZipExtractor().extract(source(data), data, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(result.plan.entries[0].refusal, "DESTINATION_ESCAPE")
            self.assertEqual((outside / "target.txt").read_bytes(), b"outside")

    def test_existing_regular_target_is_not_overwritten(self):
        data = make_zip([("existing.txt", b"replace")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            (root / "existing.txt").write_bytes(b"keep")
            result = BoundedZipExtractor().extract(source(data), data, root)
            self.assertEqual(result.entries, ())
            self.assertEqual(result.plan.entries[0].refusal, "DESTINATION_CONFLICT")
            self.assertEqual((root / "existing.txt").read_bytes(), b"keep")

    def test_symlink_or_missing_root_is_refused(self):
        data = make_zip([("a.txt", b"a")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            link = Path(temporary, "root-link")
            os.symlink(root, link)
            for destination in (link, Path(temporary, "missing"), Path(temporary, "root") / ".." / "outside"):
                with self.subTest(destination=destination):
                    with self.assertRaises(ArchiveRejected) as error:
                        BoundedZipExtractor().extract(source(data), data, destination)
                    self.assertEqual(error.exception.code, "DESTINATION_UNSAFE")

    def test_directories_are_created_only_inside_the_root(self):
        data = make_zip([("folder/", b""), ("folder/file.txt", b"content")])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary, "root")
            root.mkdir()
            result = BoundedZipExtractor().extract(source(data), data, root)
            self.assertEqual([entry.virtual_path for entry in result.entries], ["folder/file.txt"])
            self.assertTrue((root / "folder").is_dir())
            self.assertEqual((root / "folder" / "file.txt").read_bytes(), b"content")
            self.assertFalse((Path(temporary) / "folder").exists())


if __name__ == "__main__":
    unittest.main()
