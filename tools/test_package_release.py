#!/usr/bin/env python3
"""Focused tests for release packaging."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

import package_release


class RuntimeCompatibilityTests(unittest.TestCase):
    def test_records_a_static_musl_executable(self) -> None:
        compatibility = package_release.musl_compatibility(
            "Dynamic section contains relocation entries only\n",
            "Elf file type is DYN (Position-Independent Executable file)\n",
        )

        self.assertEqual(
            compatibility,
            {
                "libc": "musl",
                "linkage": "static",
                "runtime_shared_libraries": "none",
            },
        )

    def test_rejects_needed_entry(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "depends on a shared library"):
            package_release.musl_compatibility(
                "0x0000000000000001 (NEEDED) Shared library: [libc.so]\n",
                "Elf file type is DYN\n",
            )

    def test_rejects_an_elf_interpreter(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "requires an ELF interpreter"):
            package_release.musl_compatibility(
                "There is no dynamic section in this file.\n",
                "  INTERP         0x0000000000000270\n",
            )


class SubprocessTests(unittest.TestCase):
    def test_command_output_is_decoded_as_utf8(self) -> None:
        command = [sys.executable, "-X", "utf8", "-c", 'print("ā")']

        self.assertEqual(package_release.run(command), "ā")
        self.assertEqual(package_release.run_optional(command), "ā")


class ChecksumTests(unittest.TestCase):
    def test_writes_portable_checksum_line_endings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / "imglean.zip"
            archive.write_bytes(b"release archive")

            checksum = package_release.write_checksum(archive)

            self.assertEqual(
                checksum.read_bytes(),
                (
                    f"{package_release.sha256(archive)}  {archive.name}\n"
                ).encode("ascii"),
            )

if __name__ == "__main__":
    unittest.main()
