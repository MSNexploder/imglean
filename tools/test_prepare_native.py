#!/usr/bin/env python3
"""Regression checks for the native source archive boundary."""

from __future__ import annotations

import io
import json
from unittest import mock
import tarfile
import tempfile
import unittest
from pathlib import Path

import prepare_native


class NativeArchiveTests(unittest.TestCase):
    def archive(self, root: Path, name: str, contents: bytes) -> Path:
        path = root / "source.tar.gz"
        with tarfile.open(path, "w:gz") as archive:
            member = tarfile.TarInfo(name)
            member.size = len(contents)
            archive.addfile(member, io.BytesIO(contents))
        return path

    def test_extracts_source_without_archive_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = self.archive(root, "upstream/header.h", b"source")
            destination = root / "unpacked"
            destination.mkdir()
            prepare_native.unpack(archive, destination)
            self.assertEqual((destination / "header.h").read_bytes(), b"source")

    def test_checksum_failure_preserves_existing_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sources = root / "sources"
            sources.mkdir()
            (sources / "existing.h").write_bytes(b"previous source")
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps([{
                "name": "test", "url": "https://example.invalid/source.tar.gz",
                "sha256": "0" * 64, "path": "test",
            }]))
            with mock.patch.object(prepare_native, "MANIFEST", manifest), \
                 mock.patch.object(prepare_native, "SOURCES", sources), \
                 mock.patch.object(prepare_native.urllib.request, "urlopen", return_value=io.BytesIO(b"invalid archive")):
                with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                    prepare_native.prepare()
            self.assertEqual((sources / "existing.h").read_bytes(), b"previous source")

    def test_rejects_archive_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = self.archive(root, "../escaped.h", b"source")
            destination = root / "unpacked"
            destination.mkdir()
            with self.assertRaises(tarfile.FilterError):
                prepare_native.unpack(archive, destination)
            self.assertFalse((root / "escaped.h").exists())

    def test_rejects_external_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "source.tar.gz"
            with tarfile.open(archive, "w:gz") as contents:
                member = tarfile.TarInfo("upstream/link")
                member.type = tarfile.SYMTYPE
                member.linkname = "../../outside"
                contents.addfile(member)
            destination = root / "unpacked"
            destination.mkdir()
            with self.assertRaises(tarfile.FilterError):
                prepare_native.unpack(archive, destination)


if __name__ == "__main__":
    unittest.main()
