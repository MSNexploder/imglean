#!/usr/bin/env python3
"""Regenerate the checked-in WebP and AVIF validation corpora.

Requires ImageMagick, libwebp's cwebp, and libavif's avifenc. The generated
bytes are checked in, so normal builds and tests do not require these tools.
"""

from __future__ import annotations

import shutil
import struct
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1] / "tests" / "corpus"
C2PA_UUID = bytes.fromhex("d8fec3d61b0e483c92975828877ec481")


def run(*arguments: str) -> None:
    subprocess.run(arguments, check=True, stdout=subprocess.DEVNULL)


def riff_chunk(name: bytes, payload: bytes) -> bytes:
    return name + struct.pack("<I", len(payload)) + payload + (b"\0" if len(payload) & 1 else b"")


def append_webp(source: bytes, name: bytes, payload: bytes) -> bytes:
    contents = source + riff_chunk(name, payload)
    return contents[:4] + struct.pack("<I", len(contents) - 8) + contents[8:]


def bmff_box(name: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload) + 8) + name + payload


def write(group: str, kind: str, name: str, contents: bytes) -> None:
    directory = ROOT / kind / "v1" / group
    directory.mkdir(parents=True, exist_ok=True)
    (directory / name).write_bytes(contents)


def main() -> None:
    for executable in ("magick", "cwebp", "webpmux", "avifenc"):
        if shutil.which(executable) is None:
            raise SystemExit(f"missing required corpus generator: {executable}")

    with tempfile.TemporaryDirectory(prefix="imglean-modern-corpus-") as temporary:
        temporary = Path(temporary)
        source = temporary / "source.png"
        changed = temporary / "changed.png"
        webp = temporary / "source.webp"
        changed_webp = temporary / "changed.webp"
        metadata_webp = temporary / "metadata.webp"
        avif = temporary / "source.avif"
        changed_avif = temporary / "changed.avif"
        exif = temporary / "exif.bin"
        run(
            "magick",
            "-size",
            "128x128",
            "gradient:#102040-#f0c060",
            "-fill",
            "#20a0c0aa",
            "-draw",
            "circle 64,64 64,12",
            str(source),
        )
        run("magick", str(source), "-extent", "129x128", str(changed))
        run("cwebp", "-quiet", "-lossless", "-exact", "-m", "0", str(source), "-o", str(webp))
        run("cwebp", "-quiet", "-lossless", "-exact", "-m", "6", str(changed), "-o", str(changed_webp))
        exif.write_bytes(b"imglean-exif-marker")
        run("webpmux", "-set", "exif", str(exif), str(webp), "-o", str(metadata_webp))
        avif_options = ("--codec", "aom", "-q", "72", "--qalpha", "100", "-s", "6", "-j", "1")
        run("avifenc", *avif_options, str(source), str(avif))
        run("avifenc", *avif_options, str(changed), str(changed_avif))

        webp_bytes = webp.read_bytes()
        write("accepted", "webp", "provider-reduction.webp", webp_bytes)
        write("accepted", "webp", "metadata.webp", metadata_webp.read_bytes())
        write("changed", "webp", "dimensions.webp", changed_webp.read_bytes())
        write("rejected", "webp", "trailing.webp", webp_bytes + b"trailing")
        write("rejected", "webp", "truncated.webp", webp_bytes[:-3])
        write("rejected", "webp", "xmp.webp", append_webp(webp_bytes, b"XMP ", b"xmp"))
        write("rejected", "webp", "c2pa.webp", append_webp(webp_bytes, b"C2PA", b"manifest"))
        write("rejected", "webp", "animated.webp", append_webp(webp_bytes, b"ANIM", b"\0" * 6))

        avif_bytes = avif.read_bytes()
        write("accepted", "avif", "provider-reduction.avif", avif_bytes)
        write("changed", "avif", "dimensions.avif", changed_avif.read_bytes())
        write("rejected", "avif", "trailing.avif", avif_bytes + b"trailing")
        write("rejected", "avif", "truncated.avif", avif_bytes[:-3])
        write("rejected", "avif", "xmp.avif", avif_bytes + bmff_box(b"free", b"application/rdf+xml"))
        write("rejected", "avif", "c2pa.avif", avif_bytes + bmff_box(b"uuid", C2PA_UUID + b"manifest"))


if __name__ == "__main__":
    main()
