#!/usr/bin/env python3
"""Generate the ImgLean Homebrew formula from qualified release checksums."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


REPOSITORY_PATTERN = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")
VERSION_PATTERN = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
CHECKSUM_PATTERN = re.compile(r"[0-9a-f]{64}")
FORMULA_VERSION_PATTERN = re.compile(
    r'^    url "[^"\n]+/releases/download/v([0-9]+\.[0-9]+\.[0-9]+)/'
    r'imglean-[^"\n]+-apple-darwin\.tar\.gz"$',
    re.MULTILINE,
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--arm64-checksum", required=True, type=Path)
    parser.add_argument("--x86-64-checksum", required=True, type=Path)
    parser.add_argument("--existing-formula", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()

    if arguments.existing_formula is not None and arguments.existing_formula.exists():
        reject_downgrade(arguments.existing_formula, arguments.version)

    formula = render_formula(
        arguments.repository,
        arguments.version,
        read_checksum(arguments.arm64_checksum, archive_name(arguments.version, "aarch64")),
        read_checksum(arguments.x86_64_checksum, archive_name(arguments.version, "x86_64")),
    )
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(formula, encoding="utf-8")
    return 0


def archive_name(version: str, architecture: str) -> str:
    return f"imglean-{version}-{architecture}-apple-darwin.tar.gz"


def read_checksum(path: Path, expected_archive: str) -> str:
    fields = path.read_text(encoding="ascii").split()
    if len(fields) != 2 or fields[1] != expected_archive:
        raise ValueError(f"{path} does not describe {expected_archive}")
    checksum = fields[0]
    if CHECKSUM_PATTERN.fullmatch(checksum) is None:
        raise ValueError(f"{path} does not contain a lowercase SHA-256 checksum")
    return checksum


def reject_downgrade(existing_formula: Path, new_version: str) -> None:
    matches = FORMULA_VERSION_PATTERN.findall(
        existing_formula.read_text(encoding="utf-8")
    )
    if len(matches) != 2 or len(set(matches)) != 1:
        raise ValueError(
            f"{existing_formula} does not contain one consistent archive version"
        )
    current_version = matches[0]
    if version_key(new_version) < version_key(current_version):
        raise ValueError(
            f"refusing to replace Homebrew formula {current_version} "
            f"with older version {new_version}"
        )


def version_key(version: str) -> tuple[int, int, int]:
    if VERSION_PATTERN.fullmatch(version) is None:
        raise ValueError("version must contain three numeric components")
    return tuple(int(component) for component in version.split("."))


def render_formula(repository: str, version: str, arm64: str, x86_64: str) -> str:
    if REPOSITORY_PATTERN.fullmatch(repository) is None:
        raise ValueError("repository must be a GitHub OWNER/REPOSITORY name")
    if VERSION_PATTERN.fullmatch(version) is None:
        raise ValueError("version must contain three numeric components")
    for checksum in (arm64, x86_64):
        if CHECKSUM_PATTERN.fullmatch(checksum) is None:
            raise ValueError("formula checksums must be lowercase SHA-256 values")

    release = f"https://github.com/{repository}/releases/download/v{version}"
    return f'''class Imglean < Formula
  desc "Select the smallest valid same-format image optimization result"
  homepage "https://github.com/{repository}"
  license "Apache-2.0"

  depends_on macos: :sequoia

  if Hardware::CPU.arm?
    url "{release}/{archive_name(version, 'aarch64')}"
    sha256 "{arm64}"
  else
    url "{release}/{archive_name(version, 'x86_64')}"
    sha256 "{x86_64}"
  end

  def install
    bin.install "imglean"
  end

  test do
    assert_match "imglean #{{version}}", shell_output("#{{bin}}/imglean --version")
  end
end
'''


if __name__ == "__main__":
    raise SystemExit(main())
