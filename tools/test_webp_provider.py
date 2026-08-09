#!/usr/bin/env python3
"""Exercise a real cwebp override through discovery and the controller."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/corpus/webp/v1/accepted/metadata.webp"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--provider", required=True, type=Path)
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="imglean-libwebp-ci-") as temporary:
        directory = Path(temporary)
        source = directory / "source.webp"
        output = directory / "output"
        output.mkdir()
        original = FIXTURE.read_bytes()
        source.write_bytes(original)
        completed = subprocess.run(
            [
                args.binary.resolve(),
                "--quality",
                "72",
                "--provider",
                "libwebp",
                args.provider.resolve(),
                "--disable-strategy",
                "image-webp",
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
                f"libwebp integration failed ({completed.returncode})\n"
                f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
            )
        candidate = output / source.name
        if not candidate.is_file() or candidate.stat().st_size >= len(original):
            raise SystemExit("libwebp did not produce the required real size reduction")
        if b"imglean-exif-marker" in candidate.read_bytes():
            raise SystemExit("libwebp did not strip WebP Exif when requested")
        if source.read_bytes() != original or sorted(output.iterdir()) != [candidate]:
            raise SystemExit("libwebp integration changed the source or created extra output")
        if "-> libwebp" not in completed.stdout:
            raise SystemExit("libwebp winner diagnostics are missing")
        if "using libwebp provider at" not in completed.stderr:
            raise SystemExit("libwebp capability discovery diagnostics are missing")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
