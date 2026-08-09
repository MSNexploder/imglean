#!/usr/bin/env python3
"""Focused tests for deterministic Homebrew formula generation."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import generate_homebrew_formula as formula


class HomebrewFormulaTests(unittest.TestCase):
    def test_renders_both_native_macos_archives(self) -> None:
        rendered = formula.render_formula(
            "owner/imglean",
            "0.6.0",
            "a" * 64,
            "b" * 64,
        )

        self.assertIn("class Imglean < Formula", rendered)
        self.assertIn("depends_on macos: :sequoia", rendered)
        self.assertIn("if Hardware::CPU.arm?", rendered)
        self.assertIn(
            "https://github.com/owner/imglean/releases/download/v0.6.0/"
            "imglean-0.6.0-aarch64-apple-darwin.tar.gz",
            rendered,
        )
        self.assertIn(
            "https://github.com/owner/imglean/releases/download/v0.6.0/"
            "imglean-0.6.0-x86_64-apple-darwin.tar.gz",
            rendered,
        )
        self.assertIn('assert_match "imglean #{version}"', rendered)

    def test_checksum_must_name_the_exact_release_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            checksum = Path(temporary) / "archive.sha256"
            checksum.write_text(f"{'a' * 64}  wrong.tar.gz\n", encoding="ascii")

            with self.assertRaisesRegex(ValueError, "does not describe"):
                formula.read_checksum(
                    checksum,
                    "imglean-0.6.0-aarch64-apple-darwin.tar.gz",
                )

    def test_rejects_values_that_would_change_formula_syntax(self) -> None:
        with self.assertRaisesRegex(ValueError, "OWNER/REPOSITORY"):
            formula.render_formula("owner/repo\"", "0.6.0", "a" * 64, "b" * 64)
        with self.assertRaisesRegex(ValueError, "three numeric components"):
            formula.render_formula("owner/repo", "v0.6", "a" * 64, "b" * 64)
        with self.assertRaisesRegex(ValueError, "lowercase SHA-256"):
            with tempfile.TemporaryDirectory() as temporary:
                checksum = Path(temporary) / "archive.sha256"
                checksum.write_text(
                    f"{'A' * 64}  imglean-0.6.0-aarch64-apple-darwin.tar.gz\n",
                    encoding="ascii",
                )
                formula.read_checksum(
                    checksum,
                    "imglean-0.6.0-aarch64-apple-darwin.tar.gz",
                )

    def test_rejects_formula_downgrade(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            existing = Path(temporary) / "imglean.rb"
            existing.write_text(
                formula.render_formula("owner/repo", "0.7.0", "a" * 64, "b" * 64),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "older version 0.6.0"):
                formula.reject_downgrade(existing, "0.6.0")

            formula.reject_downgrade(existing, "0.7.0")
            formula.reject_downgrade(existing, "0.8.0")


if __name__ == "__main__":
    unittest.main()
