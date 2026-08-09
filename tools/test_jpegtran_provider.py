#!/usr/bin/env python3
"""Exercise the real lossless jpegtran adapter through the controller."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/corpus/jpeg/v1/accepted/provider-reduction.jpg"
EXIF = (
    b"Exif\x00\x00II*\x00\x08\x00\x00\x00\x01\x00"
    b"\x12\x01\x03\x00\x01\x00\x00\x00\x06\x00\x00\x00\x00\x00\x00\x00"
)


def with_exif(jpeg: bytes) -> tuple[bytes, bytes]:
    segment = b"\xff\xe1" + (len(EXIF) + 2).to_bytes(2, "big") + EXIF
    return jpeg[:2] + segment + jpeg[2:], segment


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--provider", required=True, type=Path)
    args = parser.parse_args()

    binary = args.binary.resolve()
    provider = args.provider.resolve()
    original, exif_segment = with_exif(FIXTURE.read_bytes())
    with tempfile.TemporaryDirectory(prefix="imglean-jpegtran-ci-") as temporary:
        directory = Path(temporary)
        source = directory / "source.jpg"
        output = directory / "output"
        output.mkdir()
        source.write_bytes(original)

        completed = subprocess.run(
            [
                binary,
                "--provider",
                "jpegtran",
                provider,
                "--output",
                output,
                source,
            ],
            cwd=ROOT,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if completed.returncode != 0:
            raise SystemExit(
                f"jpegtran integration failed ({completed.returncode})\n"
                f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
            )
        candidate = output / source.name
        if not candidate.is_file() or candidate.stat().st_size >= len(original):
            raise SystemExit("jpegtran did not produce the required real size reduction")
        if exif_segment not in candidate.read_bytes():
            raise SystemExit("jpegtran did not preserve the source Exif marker")
        if source.read_bytes() != original or sorted(output.iterdir()) != [candidate]:
            raise SystemExit("jpegtran changed the source or created extra output")
        if "-> jpegtran-v1" not in completed.stdout:
            raise SystemExit("jpegtran winner diagnostics are missing")
        if "using jpegtran-v1 provider at" not in completed.stderr:
            raise SystemExit("jpegtran capability discovery diagnostics are missing")

        stripped_source = directory / "stripped.jpg"
        stripped_output = directory / "stripped-output"
        stripped_output.mkdir()
        stripped_source.write_bytes(original)
        stripped = subprocess.run(
            [
                binary,
                "--provider",
                "jpegtran",
                provider,
                "--strip-metadata",
                "--output",
                stripped_output,
                stripped_source,
            ],
            cwd=ROOT,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if stripped.returncode != 0:
            raise SystemExit(
                f"jpegtran metadata stripping failed ({stripped.returncode})\n"
                f"stdout:\n{stripped.stdout}\nstderr:\n{stripped.stderr}"
            )
        stripped_candidate = stripped_output / stripped_source.name
        if exif_segment in stripped_candidate.read_bytes():
            raise SystemExit("jpegtran did not strip the source Exif marker")
        if stripped_source.read_bytes() != original:
            raise SystemExit("jpegtran metadata stripping changed the source")
        if "-> jpegtran-v1" not in stripped.stdout:
            raise SystemExit("jpegtran metadata-stripping winner diagnostics are missing")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
