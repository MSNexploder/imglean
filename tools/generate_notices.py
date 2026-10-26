#!/usr/bin/env python3
"""Generate Cargo notices and verify separately licensed native sources."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tomllib
from pathlib import Path

from prepare_native import MANIFEST, SOURCES


ROOT = Path(__file__).resolve().parent.parent
LIBWEBP_VERSION = (ROOT / "ci/libwebp-version.txt").read_text().strip()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-native", action="store_true")
    arguments = parser.parse_args()
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
    libwebp = packages["libwebp-sys"]
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
            "Used by bundled mozjpeg and jpegtran.",
        ),
        (
            Path(libwebp["manifest_path"]).parent / "vendor" / "COPYING",
            ROOT / "licenses" / "libwebp-COPYING",
            f"libwebp {LIBWEBP_VERSION} native code from libwebp-sys {libwebp['version']} (BSD-3-Clause)",
            "Used by bundled libwebp.",
        ),
        (
            Path(libwebp["manifest_path"]).parent / "vendor" / "PATENTS",
            ROOT / "licenses" / "libwebp-PATENTS",
            "libwebp additional patent grant",
            "Applies to the bundled libwebp source.",
        ),
        (
            ROOT / "crates" / "imglean-codecs" / "vendor" / "optipng" / "LICENSE.txt",
            ROOT / "licenses" / "optipng-LICENSE",
            "OptiPNG 7.9.1 PNG optimization engine (Zlib)",
            "Used by bundled optipng.",
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
            "Used by bundled optipng.",
        ),
    ]
    allowed = set(tomllib.loads((ROOT / "deny.toml").read_text())["licenses"]["allow"])
    for source in json.loads(MANIFEST.read_text()):
        declared = set(re.findall(r"[A-Za-z0-9.+-]+", source["license"])) - {"AND", "OR"}
        if not declared <= allowed:
            raise RuntimeError(f"unapproved native license for {source['name']}: {source['license']}")
        native_notices.append(
            (
                SOURCES / source["path"] / source["license_file"],
                ROOT / "licenses" / f"{source['name']}-LICENSE",
                f"{source['name']} {source['version']} native source ({source['license']})",
                "Checksum-pinned build input recorded in ci/native-sources.json.",
            )
        )
    for name in ["libaom", "jpegli"]:
        native_notices.append(
            (
                SOURCES / name / "PATENTS", ROOT / "licenses" / f"{name}-PATENTS",
                f"{name} additional patent grant", "Applies to the pinned native source.",
            )
        )
    for name, relative in [
        ("libjpeg-turbo-IJG", "jpegli/third_party/libjpeg-turbo/README.ijg"),
        ("libaom-fastfeat", "libaom/third_party/fastfeat/LICENSE"),
        ("libaom-vector", "libaom/third_party/vector/LICENSE"),
        ("libaom-libyuv", "libaom/third_party/libyuv/LICENSE"),
        ("libaom-libwebm", "libaom/third_party/libwebm/LICENSE.TXT"),
        ("libaom-libwebm-patents", "libaom/third_party/libwebm/PATENTS.TXT"),
        ("libaom-highway", "libaom/third_party/highway/LICENSE-BSD3"),
        ("libaom-x86inc", "libaom/third_party/x86inc/LICENSE"),
    ]:
        native_notices.append(
            (
                SOURCES / relative, ROOT / "licenses" / f"{name}-LICENSE",
                f"{name} native source license", "Applies to the pinned native build inputs.",
            )
        )
    for vendored, checked, _, _ in native_notices:
        if vendored.read_bytes().rstrip(b"\n") != checked.read_bytes().rstrip(b"\n"):
            raise RuntimeError(f"{checked.relative_to(ROOT)} differs from {vendored}")

    if arguments.check_native:
        return 0

    cargo_notices = subprocess.run(
        ["cargo", "about", "generate", "--locked", "about.hbs"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.rstrip()
    sections = [cargo_notices]
    for name, version in [("libavif", "0.14.0"), ("jpegli", "0.1.0")]:
        notice = (ROOT / "licenses" / f"{name}-rs-LICENSE").read_text().rstrip()
        sections.append(
            f"## Adapted {name} {version} Rust wrapper portions\n\n"
            f"Only the operations used by ImgLean are retained from https://crates.io/crates/{name}/{version}.\n\n"
            f"```text\n{notice}\n```"
        )
    for _, checked, heading, usage in native_notices:
        notice = checked.read_text(encoding="utf-8").rstrip()
        sections.append(f"## {heading}\n\n{usage}\n\n```text\n{notice}\n```")
    complete = "\n".join(line.rstrip() for line in "\n\n".join(sections).splitlines()) + "\n"
    (ROOT / "THIRD_PARTY_NOTICES.md").write_text(complete, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
