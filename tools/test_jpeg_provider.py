#!/usr/bin/env python3
"""Exercise one real JPEG adapter through discovery and the controller."""

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
    parser.add_argument("--name", required=True, choices=("mozjpeg", "jpegli"))
    args = parser.parse_args()

    binary = args.binary.resolve()
    provider = args.provider.resolve()
    strategy = args.name
    other = "jpegli" if args.name == "mozjpeg" else "mozjpeg"
    with tempfile.TemporaryDirectory(prefix=f"imglean-{args.name}-ci-") as temporary:
        directory = Path(temporary)
        source = directory / "source.jpg"
        output = directory / "output"
        output.mkdir()
        original, exif_segment = with_exif(FIXTURE.read_bytes())
        source.write_bytes(original)

        completed = subprocess.run(
            [
                binary,
                "--quality",
                "80",
                "--provider",
                args.name,
                provider,
                "--disable-strategy",
                other,
                "--strip-metadata",
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
                f"{args.name} integration failed ({completed.returncode})\n"
                f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
            )
        candidate = output / source.name
        if not candidate.is_file() or candidate.stat().st_size >= len(original):
            raise SystemExit(f"{args.name} did not produce the required real size reduction")
        candidate_bytes = candidate.read_bytes()
        if args.name == "mozjpeg" and exif_segment not in candidate_bytes:
            raise SystemExit("MozJPEG did not preserve the source Exif marker")
        if args.name == "jpegli" and exif_segment in candidate_bytes:
            raise SystemExit("Jpegli did not strip the source Exif marker")
        if source.read_bytes() != original or sorted(output.iterdir()) != [candidate]:
            raise SystemExit(f"{args.name} integration changed the source or created extra output")
        if f"-> {strategy}" not in completed.stdout:
            raise SystemExit(f"{args.name} winner diagnostics are missing")
        if f"using {strategy} provider at" not in completed.stderr:
            raise SystemExit(f"{args.name} capability discovery diagnostics are missing")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
