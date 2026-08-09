#!/usr/bin/env python3
"""Generate Cargo notices and verify separately licensed native sources."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
LIBWEBP_VERSION = (ROOT / "ci/libwebp-version.txt").read_text().strip()


def main() -> int:
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout
    )
    packages = {item["name"]: item for item in metadata["packages"]}
    libdeflate = packages["libdeflate-sys"]
    mozjpeg = packages["mozjpeg-sys"]
    jpegli = packages["jpegli-sys"]
    libwebp = packages["libwebp-sys"]
    libaom = packages["libaom-sys"]
    native_notices = [
        (
            Path(libdeflate["manifest_path"]).parent / "libdeflate" / "COPYING",
            ROOT / "licenses" / "libdeflate-COPYING",
            f"libdeflate {libdeflate['version']} native code (MIT)",
            "Used through libdeflate-sys.",
        ),
        (
            Path(mozjpeg["manifest_path"]).parent / "LICENSE",
            ROOT / "licenses" / "mozjpeg-LICENSE",
            f"MozJPEG native code from mozjpeg-sys {mozjpeg['version']}",
            "Used by bundled mozjpeg-v1 and jpegtran-v1.",
        ),
        (
            Path(jpegli["manifest_path"]).parent / "libjxl" / "LICENSE",
            ROOT / "licenses" / "jpegli-LICENSE",
            f"Jpegli native code from jpegli-sys {jpegli['version']}",
            "Used by bundled jpegli-v1.",
        ),
        (
            Path(jpegli["manifest_path"]).parent / "libjxl" / "PATENTS",
            ROOT / "licenses" / "jpegli-PATENTS",
            "Jpegli additional patent grant",
            "Applies to the bundled Jpegli source.",
        ),
        (
            Path(jpegli["manifest_path"]).parent
            / "libjxl"
            / "third_party"
            / "highway"
            / "LICENSE",
            ROOT / "licenses" / "highway-LICENSE",
            "Highway native code (Apache-2.0)",
            "Statically linked by jpegli-sys.",
        ),
        (
            Path(libwebp["manifest_path"]).parent / "vendor" / "COPYING",
            ROOT / "licenses" / "libwebp-COPYING",
            f"libwebp {LIBWEBP_VERSION} native code from libwebp-sys {libwebp['version']} (BSD-3-Clause)",
            "Used by bundled libwebp-v1.",
        ),
        (
            Path(libwebp["manifest_path"]).parent / "vendor" / "PATENTS",
            ROOT / "licenses" / "libwebp-PATENTS",
            "libwebp additional patent grant",
            "Applies to the bundled libwebp source.",
        ),
        (
            Path(libaom["manifest_path"]).parent / "vendor" / "PATENTS",
            ROOT / "licenses" / "libaom-PATENTS",
            "Alliance for Open Media Patent License 1.0",
            "Applies to the bundled libaom AV1 implementation.",
        ),
        (
            ROOT / "crates" / "imglean-codecs" / "vendor" / "optipng" / "LICENSE.txt",
            ROOT / "licenses" / "optipng-LICENSE",
            "OptiPNG 7.9.1 PNG optimization engine (Zlib)",
            "Used by bundled optipng-v1.",
        ),
        (
            ROOT
            / "crates"
            / "imglean-codecs"
            / "vendor"
            / "optipng"
            / "third_party"
            / "cexcept"
            / "LICENSE.md",
            ROOT / "licenses" / "cexcept-LICENSE",
            "Cexcept used by OptiPNG (Zlib)",
            "Used by bundled optipng-v1.",
        ),
    ]
    for vendored, checked, _, _ in native_notices:
        if vendored.read_bytes().rstrip(b"\n") != checked.read_bytes().rstrip(b"\n"):
            raise RuntimeError(f"{checked.relative_to(ROOT)} differs from {vendored}")

    cargo_notices = subprocess.run(
        ["cargo", "about", "generate", "--locked", "about.hbs"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.rstrip()
    sections = [cargo_notices]
    for _, checked, heading, usage in native_notices:
        notice = checked.read_text(encoding="utf-8").rstrip()
        sections.append(f"## {heading}\n\n{usage}\n\n```text\n{notice}\n```")
    complete = "\n".join(line.rstrip() for line in "\n\n".join(sections).splitlines()) + "\n"
    (ROOT / "THIRD_PARTY_NOTICES.md").write_text(complete, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
