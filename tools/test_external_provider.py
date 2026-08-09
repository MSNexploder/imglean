#!/usr/bin/env python3
"""Exercise one real OptiPNG override through preflight and the controller."""

from __future__ import annotations

import argparse
import binascii
import os
import shutil
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path


def chunk(name: bytes, data: bytes) -> bytes:
    crc = binascii.crc32(name + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + name + data + struct.pack(">I", crc)


def compressible_png() -> bytes:
    width = height = 64
    filtered = b"".join(b"\x00" + bytes(width * 4) for _ in range(height))
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"tEXt", b"Comment\x00" + b"metadata" * 1024)
        + chunk(b"IDAT", zlib.compress(filtered, 0))
        + chunk(b"IEND", b"")
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--provider", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    provider = args.provider.resolve()

    with tempfile.TemporaryDirectory(prefix="imglean-provider-ci-") as temporary:
        root = Path(temporary)
        provider_directory = root / "bin"
        provider_directory.mkdir()
        provider_name = "optipng.exe" if os.name == "nt" else "optipng"
        discovered_provider = provider_directory / provider_name
        shutil.copy2(provider, discovered_provider)
        source = root / "source.png"
        source_bytes = compressible_png()
        source.write_bytes(source_bytes)
        output = root / "out"
        output.mkdir()
        environment = os.environ.copy()
        environment["PATH"] = str(provider_directory) + os.pathsep + environment.get("PATH", "")

        result = subprocess.run(
            [
                binary,
                "--disable-strategy",
                "oxipng-libdeflate",
                "--disable-strategy",
                "oxipng-zopfli",
                "--require-strategy",
                "optipng",
                "--provider",
                "optipng",
                discovered_provider,
                "--strip-metadata",
                "--output",
                output,
                source,
            ],
            check=False,
            capture_output=True,
            env=environment,
        )
        if result.returncode != 0:
            raise SystemExit(result.stderr.decode(errors="replace"))
        candidate = (output / source.name).read_bytes()
        if len(candidate) >= len(source_bytes):
            raise SystemExit("OptiPNG did not produce the required real size reduction")
        if b"tEXt" in candidate:
            raise SystemExit("OptiPNG did not strip PNG metadata when requested")
        if source.read_bytes() != source_bytes or len(list(output.iterdir())) != 1:
            raise SystemExit("OptiPNG integration changed the source or created extra output")
        stdout = result.stdout.decode(errors="replace")
        stderr = result.stderr.decode(errors="replace")
        if "-> optipng" not in stdout or "using optipng provider at" not in stderr:
            raise SystemExit("OptiPNG discovery or winner diagnostics are missing")

        all_source = root / "all-strategies.png"
        all_source.write_bytes(source_bytes)
        all_output = root / "all-out"
        all_output.mkdir()
        all_result = subprocess.run(
            [
                binary,
                "--jobs",
                "3",
                "--require-strategy",
                "optipng",
                "--provider",
                "optipng",
                discovered_provider,
                "--output",
                all_output,
                all_source,
            ],
            check=False,
            capture_output=True,
            env=environment,
        )
        if all_result.returncode != 0:
            raise SystemExit(all_result.stderr.decode(errors="replace"))
        all_stdout = all_result.stdout.decode(errors="replace")
        for strategy in (
            "oxipng-libdeflate",
            "oxipng-zopfli",
            "optipng",
        ):
            if strategy not in all_stdout:
                raise SystemExit(f"complete registry output is missing {strategy}")
        if any(state in all_stdout for state in ("disabled", "unavailable", "not run")):
            raise SystemExit("an available strategy was not executed")
        if all_source.read_bytes() != source_bytes or not (all_output / all_source.name).is_file():
            raise SystemExit("combined strategy execution changed its source or missed output")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
