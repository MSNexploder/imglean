#!/usr/bin/env python3
"""Exercise one real pngquant adapter through discovery and the controller."""

from __future__ import annotations

import argparse
import binascii
import os
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / "tests/corpus/png/v2/accepted/pngquant-reduction.png"


def chunk(name: bytes, data: bytes) -> bytes:
    crc = binascii.crc32(name + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + name + data + struct.pack(">I", crc)


def with_text_metadata(png: bytes) -> bytes:
    return png[:-12] + chunk(b"tEXt", b"Comment\x00provider integration") + png[-12:]


def run(
    binary: Path, arguments: list[object], environment: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), *(str(argument) for argument in arguments)],
        check=False,
        capture_output=True,
        env=environment,
        text=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--provider", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    provider = args.provider.resolve()
    with tempfile.TemporaryDirectory(prefix="imglean-pngquant-ci-") as temporary:
        root = Path(temporary)
        provider_directory = root / "bin"
        provider_directory.mkdir()
        provider_name = "pngquant.exe" if os.name == "nt" else "pngquant"
        discovered_provider = provider_directory / provider_name
        shutil.copy2(provider, discovered_provider)
        environment = os.environ.copy()
        environment["PATH"] = str(provider_directory) + os.pathsep + environment.get("PATH", "")

        source = root / "lossy.png"
        source_bytes = with_text_metadata(FIXTURE.read_bytes())
        source.write_bytes(source_bytes)
        output = root / "lossy-out"
        output.mkdir()
        result = run(
            binary,
            [
                "--quality",
                "80",
                "--disable-strategy",
                "oxipng-libdeflate",
                "--disable-strategy",
                "oxipng-zopfli",
                "--disable-strategy",
                "optipng",
                "--require-strategy",
                "pngquant",
                "--strip-metadata",
                "--output",
                output,
                source,
            ],
            environment,
        )
        if result.returncode != 0:
            raise SystemExit(
                f"pngquant integration failed ({result.returncode})\n"
                f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
            )
        candidate = (output / source.name).read_bytes()
        if len(candidate) >= len(source_bytes) or candidate == source_bytes:
            raise SystemExit("pngquant did not produce the required real lossy reduction")
        if b"tEXt" in candidate:
            raise SystemExit("pngquant did not strip PNG metadata")
        if source.read_bytes() != source_bytes or len(list(output.iterdir())) != 1:
            raise SystemExit("pngquant integration changed the source or created extra output")
        stdout = result.stdout
        stderr = result.stderr
        if "-> pngquant" not in stdout or "using pngquant provider at" not in stderr:
            raise SystemExit("pngquant discovery or winner diagnostics are missing")

        lossless_source = root / "lossless.png"
        lossless_source.write_bytes(source_bytes)
        lossless_output = root / "lossless-out"
        lossless_output.mkdir()
        lossless = run(
            binary,
            [
                "--disable-strategy",
                "oxipng-libdeflate",
                "--disable-strategy",
                "oxipng-zopfli",
                "--disable-strategy",
                "optipng",
                "--output",
                lossless_output,
                lossless_source,
            ],
            environment,
        )
        if lossless.returncode != 0:
            raise SystemExit(
                f"pngquant lossless-policy integration failed ({lossless.returncode})\n"
                f"stdout:\n{lossless.stdout}\nstderr:\n{lossless.stderr}"
            )
        if (lossless_output / lossless_source.name).read_bytes() != source_bytes:
            raise SystemExit("lossless mode did not preserve the baseline")
        lossless_stdout = lossless.stdout
        lossless_stderr = lossless.stderr
        if "pngquant                 not applicable" not in lossless_stdout:
            raise SystemExit("lossless mode did not report pngquant as not applicable")
        if "using pngquant" in lossless_stderr:
            raise SystemExit("lossless mode unexpectedly discovered or executed pngquant")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
