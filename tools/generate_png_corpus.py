#!/usr/bin/env python3
"""Generate the bounded version 0.1 PNG validation corpus."""

from __future__ import annotations

import binascii
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "tests" / "corpus" / "png" / "v1"


def chunk(name: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + name + data + struct.pack(">I", binascii.crc32(name + data) & 0xFFFFFFFF)


def png(
    width: int,
    height: int,
    depth: int,
    color: int,
    filtered: bytes,
    *,
    palette: bytes | None = None,
    before: tuple[tuple[bytes, bytes], ...] = (),
    after_palette: tuple[tuple[bytes, bytes], ...] = (),
    after: tuple[tuple[bytes, bytes], ...] = (),
    level: int = 6,
    interlace: int = 0,
    split_idat: bool = False,
) -> bytes:
    result = bytearray(b"\x89PNG\r\n\x1a\n")
    result += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, depth, color, 0, 0, interlace))
    for name, data in before:
        result += chunk(name, data)
    if palette is not None:
        result += chunk(b"PLTE", palette)
    for name, data in after_palette:
        result += chunk(name, data)
    compressed = zlib.compress(filtered, level)
    if split_idat:
        midpoint = len(compressed) // 2
        result += chunk(b"IDAT", compressed[:midpoint])
        result += chunk(b"IDAT", compressed[midpoint:])
    else:
        result += chunk(b"IDAT", compressed)
    for name, data in after:
        result += chunk(name, data)
    result += chunk(b"IEND", b"")
    return bytes(result)


def write(group: str, name: str, data: bytes) -> None:
    destination = ROOT / group / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(data)


def main() -> None:
    palette2 = bytes((0, 0, 0, 255, 255, 255))
    palette4 = bytes(value for index in range(4) for value in (index * 60,) * 3)
    palette16 = bytes(value for index in range(16) for value in (index * 16,) * 3)
    palette256 = bytes(value for index in range(256) for value in (index,) * 3)

    write("accepted", "grayscale8.png", png(1, 1, 8, 0, b"\x00\x2a"))
    write("accepted", "truecolor8.png", png(1, 1, 8, 2, b"\x00\x01\x02\x03"))
    write("accepted", "indexed1.png", png(1, 1, 1, 3, b"\x00\x00", palette=palette2))
    write("accepted", "indexed2.png", png(1, 1, 2, 3, b"\x00\x40", palette=palette4))
    write("accepted", "indexed4.png", png(1, 1, 4, 3, b"\x00\x20", palette=palette16))
    write("accepted", "indexed8.png", png(1, 1, 8, 3, b"\x00\x7f", palette=palette256))
    write("accepted", "grayscale-alpha8.png", png(1, 1, 8, 4, b"\x00\x2a\xff"))
    write("accepted", "truecolor-alpha8.png", png(1, 1, 8, 6, b"\x00\x01\x02\x03\xff"))
    write("accepted", "transparent-nonzero-color.png", png(1, 1, 8, 6, b"\x00\x05\x06\x07\x00"))

    srgb_chrm = (31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000)
    ancillary = (
        (b"cHRM", b"".join(struct.pack(">I", value) for value in srgb_chrm)),
        (b"gAMA", struct.pack(">I", 45455)),
        (b"sBIT", b"\x08"),
        (b"sRGB", b"\x00"),
        (b"tRNS", b"\x00\x00"),
        (b"bKGD", b"\x00\x2a"),
        (b"pHYs", struct.pack(">IIB", 1, 1, 1)),
        (b"tEXt", b"Key\x00opaque payload"),
    )
    write(
        "accepted",
        "ancillary-before-after.png",
        png(1, 1, 8, 0, b"\x00\x2a", before=ancillary, after=((b"tIME", b"\x07\xe8\x02\x1d\x17\x3b\x3c"),)),
    )
    write(
        "accepted",
        "indexed-transparency.png",
        png(1, 1, 1, 3, b"\x00\x80", palette=palette2, after_palette=((b"tRNS", b"\x00\xff"),)),
    )

    filtered = b"".join(b"\x00" + bytes(64 * 4) for _ in range(64))
    write("accepted", "oxipng-reduction.png", png(64, 64, 8, 6, filtered, level=0))

    equivalent_source = png(2, 1, 8, 0, b"\x00\x0a\x14", level=0)
    equivalent_candidate = png(2, 1, 8, 0, b"\x01\x0a\x0a", level=9, split_idat=True)
    write("equivalent", "source.png", equivalent_source)
    write("equivalent", "candidate.png", equivalent_candidate)
    write("equivalent", "unchanged.png", equivalent_source)
    write("changed", "source.png", png(2, 1, 8, 0, b"\x00\x0a\x14"))
    write("changed", "candidate.png", png(2, 1, 8, 0, b"\x00\x0a\x15"))

    base = png(1, 1, 8, 0, b"\x00\x2a")
    bad_crc = bytearray(base)
    bad_crc[20] ^= 1
    write("rejected", "bad-crc.png", bytes(bad_crc))
    write("rejected", "truncated.png", base[:-3])
    write("rejected", "trailing.png", base + b"trailing")
    write("rejected", "apng.png", png(1, 1, 8, 0, b"\x00\x2a", before=((b"acTL", struct.pack(">II", 1, 0)),)))
    write("rejected", "xmp.png", png(1, 1, 8, 0, b"\x00\x2a", before=((b"tEXt", b"XML:com.adobe.xmp\x00payload"),)))
    write("rejected", "cabx.png", png(1, 1, 8, 0, b"\x00\x2a", before=((b"caBX", b"manifest"),)))
    write("rejected", "unknown.png", png(1, 1, 8, 0, b"\x00\x2a", before=((b"vpAg", b"unknown"),)))
    write("rejected", "interlaced.png", png(1, 1, 8, 0, b"\x00\x2a", interlace=1))
    write("rejected", "sixteen-bit.png", png(1, 1, 16, 0, b"\x00\x00\x2a"))
    write("rejected", "oversized-dimensions.png", png(32769, 1, 8, 0, b"\x00"))
    write("rejected", "invalid-filter.png", png(1, 1, 8, 0, b"\x05\x2a"))
    write("rejected", "palette-out-of-range.png", png(1, 1, 1, 3, b"\x00\x80", palette=bytes((0, 0, 0))))


if __name__ == "__main__":
    main()
