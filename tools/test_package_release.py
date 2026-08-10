#!/usr/bin/env python3
"""Focused tests for release runtime compatibility records."""

from __future__ import annotations

import unittest

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


if __name__ == "__main__":
    unittest.main()
