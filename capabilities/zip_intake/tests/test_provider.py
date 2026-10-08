"""Focused fixture tests; no filesystem extraction and no external services."""

from dataclasses import replace
from hashlib import sha256
from io import BytesIO
import stat
import struct
import unittest
from unittest.mock import patch
import zipfile

from capabilities.zip_intake import ArchiveRejected, Limits, SourceRevision, ZipIntakeProvider


def make_zip(entries, compression=zipfile.ZIP_STORED):
    stream = BytesIO()
    with zipfile.ZipFile(stream, "w", compression=compression) as package:
        for name, content in entries:
            package.writestr(name, content)
    return stream.getvalue()


def source(data):
    return SourceRevision("workspace-1", "resource-1", "revision-1", sha256(data).hexdigest())


class ZipIntakeTests(unittest.TestCase):
    def test_manifest_is_metadata_only_and_extraction_pins_provenance(self):
        data = make_zip([("folder/", b""), ("folder/readme.txt", b"hello")])
        original = bytes(data)
        provider = ZipIntakeProvider()
        preview = provider.preview(source(data), data)
        self.assertEqual([entry.status for entry in preview.entries], ["DIRECTORY", "READY"])
        self.assertIsNone(preview.entries[1].content_sha256)
        result = provider.extract(source(data), data)
        self.assertEqual(result.manifest.source, source(data))
        self.assertEqual(result.entries[0].content, b"hello")
        self.assertEqual(result.entries[0].content_sha256, sha256(b"hello").hexdigest())
        self.assertEqual(result.manifest.entries[1].status, "EXTRACTED")
        self.assertEqual(data, original)

    def test_unsafe_names_are_rejected_and_not_reflected(self):
        for name in ("../escape", "/absolute", "C:/escape", "a\\..\\escape", "a//b", "a/./b", "a/../b", "NUL.txt", "a/b.", "a/b ", "a\x00secret", "a\u202esecret", "//server/share"):
            with self.subTest(name=name):
                # Construct NUL by mutating both headers, since the ZIP writer
                # itself truncates at NUL before serializing the hostile name.
                data = make_zip([(name.replace("\x00", "!"), b"hello")])
                if "\x00" in name:
                    data = data.replace(b"a!secret", b"a\x00secret")
                result = ZipIntakeProvider().extract(source(data), data)
                self.assertEqual(result.manifest.entries[0].reason, "UNSAFE_PATH")
                self.assertIsNone(result.manifest.entries[0].virtual_path)
                self.assertEqual(result.entries, ())

    def test_all_unicode_case_and_file_ancestor_collisions_are_rejected(self):
        cases = [
            [("caf\u00e9.txt", b"1"), ("cafe\u0301.txt", b"2")],
            [("FILE.txt", b"1"), ("file.txt", b"2")],
            [("\uff26\uff29\uff2c\uff25.txt", b"1"), ("file.txt", b"2")],
            [("folder", b"1"), ("folder/file.txt", b"2")],
            [("folder/", b""), ("folder", b"1")],
            [("dup.txt", b"1"), ("dup.txt", b"2")],
        ]
        for entries in cases:
            with self.subTest(entries=entries):
                data = make_zip(entries)
                result = ZipIntakeProvider().extract(source(data), data)
                self.assertEqual(result.entries, ())
                self.assertTrue(all(entry.reason == "PATH_CONFLICT" for entry in result.manifest.entries))

    def test_symlink_and_device_members_are_rejected(self):
        for kind in (stat.S_IFLNK, stat.S_IFCHR, stat.S_IFIFO, stat.S_IFSOCK):
            info = zipfile.ZipInfo("link")
            info.create_system = 3
            info.external_attr = (kind | 0o777) << 16
            data = make_zip([(info, b"target")])
            result = ZipIntakeProvider().extract(source(data), data)
            self.assertEqual(result.manifest.entries[0].reason, "SPECIAL_FILE")
            self.assertEqual(result.entries, ())

    def test_encrypted_flag_is_rejected_before_read(self):
        data = bytearray(make_zip([("secret.txt", b"secret")]))
        local = data.index(b"PK\x03\x04")
        central = data.index(b"PK\x01\x02")
        for offset in (local + 6, central + 8):
            flags = struct.unpack_from("<H", data, offset)[0]
            struct.pack_into("<H", data, offset, flags | 1)
        data = bytes(data)
        report = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(report.manifest.entries[0].reason, "ENCRYPTED_ENTRY")
        self.assertEqual(report.entries, ())

    def test_unix_link_extra_metadata_is_rejected(self):
        info = zipfile.ZipInfo("link")
        info.extra = struct.pack("<HH", 0x000D, 12) + b"\x00" * 12
        data = make_zip([(info, b"target")])
        result = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(result.manifest.entries[0].reason, "UNSUPPORTED_LINK_METADATA")
        self.assertEqual(result.entries, ())

    def test_unsupported_compression_is_rejected(self):
        data = bytearray(make_zip([("data", b"data")]))
        struct.pack_into("<H", data, data.index(b"PK\x03\x04") + 8, 99)
        struct.pack_into("<H", data, data.index(b"PK\x01\x02") + 10, 99)
        data = bytes(data)
        result = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(result.manifest.entries[0].reason, "UNSUPPORTED_COMPRESSION")

    def test_multipart_zip64_and_trailing_bytes_are_rejected(self):
        original = make_zip([("data", b"data")])
        for relative_offset, value, reason in (
            (4, 1, "MULTIPART_ARCHIVE"),
            (8, 0xFFFF, "MULTIPART_ARCHIVE"),
        ):
            data = bytearray(original)
            struct.pack_into("<H", data, data.rfind(b"PK\x05\x06") + relative_offset, value)
            data = bytes(data)
            with self.assertRaises(ArchiveRejected) as error:
                ZipIntakeProvider().preview(source(data), data)
            self.assertEqual(error.exception.code, reason)
        data = bytearray(original)
        end = data.rfind(b"PK\x05\x06")
        struct.pack_into("<HH", data, end + 8, 0xFFFF, 0xFFFF)
        data = bytes(data)
        with self.assertRaises(ArchiveRejected) as error:
            ZipIntakeProvider().preview(source(data), data)
        self.assertEqual(error.exception.code, "ZIP64_UNSUPPORTED")
        data = original + b"unparsed-tail"
        with self.assertRaises(ArchiveRejected) as error:
            ZipIntakeProvider().preview(source(data), data)
        self.assertEqual(error.exception.code, "INVALID_ZIP")

    def test_nested_extension_and_disguised_magic_are_rejected(self):
        nested = make_zip([("inside.txt", b"inside")])
        for name, content in (("inner.zip", b"garbage"), ("innocent.txt", nested), ("prefixed.dat", b"executable-stub" + nested), ("hidden.dat", b"\x1f\x8bnot-a-real-gzip")):
            data = make_zip([(name, content), ("safe.txt", b"safe")])
            result = ZipIntakeProvider().extract(source(data), data)
            self.assertEqual(result.manifest.entries[0].reason, "NESTED_ARCHIVE")
            self.assertEqual([entry.content for entry in result.entries], [b"safe"])

    def test_archive_size_entry_count_and_total_limits_fail_before_output(self):
        data = make_zip([("a", b"12"), ("b", b"34")])
        for limits, reason in (
            (Limits(max_archive_bytes=1), "ARCHIVE_SIZE_LIMIT"),
            (Limits(max_entries=1), "ENTRY_COUNT_LIMIT"),
            (Limits(max_expanded_bytes=3), "EXPANDED_SIZE_LIMIT"),
        ):
            with self.subTest(reason=reason):
                with self.assertRaises(ArchiveRejected) as error:
                    ZipIntakeProvider(limits).extract(source(data), data)
                self.assertEqual(error.exception.code, reason)

    def test_entry_size_ratio_and_path_limits(self):
        data = make_zip([("long/name", b"A" * 10000)], zipfile.ZIP_DEFLATED)
        for limits, reason in (
            (Limits(max_entry_bytes=3), "ENTRY_SIZE_LIMIT"),
            (Limits(max_compression_ratio=2), "COMPRESSION_RATIO_LIMIT"),
            (Limits(max_path_bytes=3), "PATH_LIMIT"),
            (Limits(max_path_depth=1), "PATH_LIMIT"),
        ):
            with self.subTest(reason=reason):
                result = ZipIntakeProvider(limits).extract(source(data), data)
                self.assertEqual(result.manifest.entries[0].reason, reason)
                self.assertEqual(result.entries, ())

    def test_corrupt_crc_is_per_entry_failure_and_safe_file_survives(self):
        data = make_zip([("bad.txt", b"bad-payload"), ("safe.txt", b"safe-payload")])
        data = data.replace(b"bad-payload", b"BAD-payload")
        result = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(result.manifest.entries[0].reason, "INVALID_ENTRY")
        self.assertEqual([entry.virtual_path for entry in result.entries], ["safe.txt"])

    def test_stream_budget_does_not_trust_declared_expansion(self):
        data = make_zip([("a", b"a"), ("b", b"b")])
        limits = Limits(max_expanded_bytes=3)
        # Fault-inject a provider reader violating central-directory size claims.
        # This verifies the independent byte budget, not ZIP library correctness.
        with patch("capabilities.zip_intake.provider.zipfile.ZipFile.open", return_value=BytesIO(b"123456")):
            result = ZipIntakeProvider(limits).extract(source(data), data)
        self.assertEqual(result.entries, ())
        self.assertEqual([entry.reason for entry in result.manifest.entries], ["EXPANDED_SIZE_LIMIT", "EXPANDED_SIZE_LIMIT"])

    def test_local_central_name_mismatch_is_rejected(self):
        data = make_zip([("bad.txt", b"bad")])
        data = data.replace(b"bad.txt", b"odd.txt", 1)
        result = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(result.manifest.entries[0].reason, "RAW_NAME_MISMATCH")
        self.assertIsNone(result.manifest.entries[0].virtual_path)
        self.assertEqual(result.manifest.entries[0].name_sha256, sha256(b"bad.txt").hexdigest())
        self.assertEqual(result.entries, ())

    def test_embedded_nul_uses_exact_raw_name_digest_and_never_truncated_path(self):
        raw = b"safe\x00secret.txt"
        data = make_zip([("safe!secret.txt", b"payload"), ("sibling.txt", b"safe")])
        data = data.replace(b"safe!secret.txt", raw)
        for method in ("preview", "extract"):
            with self.subTest(method=method):
                result = getattr(ZipIntakeProvider(), method)(source(data), data)
                manifest = result if method == "preview" else result.manifest
                report = manifest.entries[0]
                self.assertEqual(report.reason, "UNSAFE_PATH")
                self.assertIsNone(report.virtual_path)
                self.assertEqual(report.name_sha256, sha256(raw).hexdigest())
                self.assertNotEqual(report.name_sha256, sha256(b"safe").hexdigest())
                if method == "extract":
                    self.assertEqual([entry.virtual_path for entry in result.entries], ["sibling.txt"])

    def test_cp437_digest_comes_from_archive_bytes_not_utf8_reencoding(self):
        data = make_zip([("cafe.txt", b"payload")])
        raw = b"caf\x82.txt"
        data = data.replace(b"cafe.txt", raw)
        result = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(result.manifest.entries[0].virtual_path, "caf\u00e9.txt")
        self.assertEqual(result.manifest.entries[0].name_sha256, sha256(raw).hexdigest())
        self.assertNotEqual(result.manifest.entries[0].name_sha256, sha256("caf\u00e9.txt".encode("utf-8")).hexdigest())
        self.assertEqual(result.entries[0].content, b"payload")

    def test_invalid_utf8_names_fail_before_zipinfo_decoding(self):
        data = bytearray(make_zip([("cafe.txt", b"payload")]))
        data = bytearray(bytes(data).replace(b"cafe.txt", b"caf\xff.txt"))
        for offset in (data.index(b"PK\x03\x04") + 6, data.index(b"PK\x01\x02") + 8):
            flags = struct.unpack_from("<H", data, offset)[0]
            struct.pack_into("<H", data, offset, flags | 0x800)
        data = bytes(data)
        with self.assertRaises(ArchiveRejected) as error:
            ZipIntakeProvider().preview(source(data), data)
        self.assertEqual(error.exception.code, "INVALID_ENTRY_NAME")

    def test_alternate_unicode_name_metadata_is_rejected(self):
        info = zipfile.ZipInfo("ordinary.txt")
        alternate = b"different.txt"
        info.extra = struct.pack("<HH", 0x7075, len(alternate)) + alternate
        data = make_zip([(info, b"payload")])
        result = ZipIntakeProvider().extract(source(data), data)
        self.assertEqual(result.manifest.entries[0].reason, "AMBIGUOUS_ENTRY_NAME")
        self.assertIsNone(result.manifest.entries[0].virtual_path)
        self.assertEqual(result.entries, ())

    def test_raw_central_name_is_cross_checked_against_zipinfo(self):
        data = make_zip([("original.txt", b"payload")])
        real_infolist = zipfile.ZipFile.infolist

        def altered_infolist(package):
            infos = real_infolist(package)
            infos[0].filename = "substituted.txt"
            infos[0].orig_filename = "substituted.txt"
            return infos

        with patch("capabilities.zip_intake.provider.zipfile.ZipFile.infolist", altered_infolist):
            result = ZipIntakeProvider().preview(source(data), data)
        self.assertEqual(result.entries[0].reason, "RAW_NAME_MISMATCH")
        self.assertEqual(result.entries[0].name_sha256, sha256(b"original.txt").hexdigest())
        self.assertIsNone(result.entries[0].virtual_path)

    def test_central_lengths_counts_and_local_offsets_are_bounded(self):
        original = make_zip([("data.txt", b"payload")])
        central = original.index(b"PK\x01\x02")
        for field_offset, format_string, value, reason in (
            (central + 28, "<H", 0xFFFF, "INVALID_CENTRAL_DIRECTORY"),
            (central + 42, "<I", central, "INVALID_LOCAL_HEADER"),
        ):
            data = bytearray(original)
            struct.pack_into(format_string, data, field_offset, value)
            data = bytes(data)
            with self.assertRaises(ArchiveRejected) as error:
                ZipIntakeProvider().preview(source(data), data)
            self.assertEqual(error.exception.code, reason)
        data = bytearray(original)
        end = data.rfind(b"PK\x05\x06")
        struct.pack_into("<HH", data, end + 8, 2, 2)
        data = bytes(data)
        with self.assertRaises(ArchiveRejected) as error:
            ZipIntakeProvider().preview(source(data), data)
        self.assertEqual(error.exception.code, "INVALID_CENTRAL_DIRECTORY")

    def test_source_digest_mutability_and_malformed_archive_fail_closed(self):
        data = make_zip([])
        with self.assertRaises(ArchiveRejected) as error:
            ZipIntakeProvider().preview(replace(source(data), content_sha256="0" * 64), data)
        self.assertEqual(error.exception.code, "SOURCE_DIGEST_MISMATCH")
        with self.assertRaises(ArchiveRejected) as error:
            ZipIntakeProvider().preview(source(data), bytearray(data))
        self.assertEqual(error.exception.code, "IMMUTABLE_BYTES_REQUIRED")
        for invalid in (b"bad", b"PK\x03\x04broken"):
            with self.assertRaises(ArchiveRejected) as error:
                ZipIntakeProvider().preview(source(invalid), invalid)
            self.assertEqual(error.exception.code, "INVALID_ZIP")

    def test_empty_archive_and_empty_file_are_supported(self):
        for entries in ([], [("empty.txt", b"")]):
            data = make_zip(entries)
            result = ZipIntakeProvider().extract(source(data), data)
            self.assertEqual(len(result.entries), len(entries))

    def test_processing_deadline_fails_closed(self):
        data = make_zip([("a", b"a")])
        with patch("capabilities.zip_intake.provider.time.monotonic", side_effect=[0.0, 11.0]):
            with self.assertRaises(ArchiveRejected) as error:
                ZipIntakeProvider().preview(source(data), data)
        self.assertEqual(error.exception.code, "PROCESSING_TIMEOUT")

    def test_invalid_limits_are_rejected(self):
        for arguments in ({"max_entries": 0}, {"max_seconds": float("nan")}, {"max_seconds": float("inf")}, {"max_entry_bytes": True}):
            with self.assertRaises(ValueError):
                Limits(**arguments)


if __name__ == "__main__":
    unittest.main()
