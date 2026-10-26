#!/usr/bin/env python3
"""Prepare checksum-pinned native build inputs; never called by the CLI."""

from __future__ import annotations

import hashlib
import json
import shutil
import tarfile
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "ci/native-sources.json"
SOURCES = ROOT / "target/native-sources"


def unpack(archive: Path, destination: Path) -> None:
    with tarfile.open(archive) as contents:
        members = contents.getmembers()
        if sum(member.size for member in members) > 1024 * 1024 * 1024:
            raise RuntimeError("native source archive exceeds 1 GiB")
        contents.extractall(destination, members=members, filter="data")
    roots = list(destination.iterdir())
    if len(roots) != 1 or not roots[0].is_dir():
        raise RuntimeError("native source archive must contain one root directory")
    root = roots[0]
    for child in root.iterdir():
        shutil.move(child, destination / child.name)
    root.rmdir()


def prepare() -> None:
    manifest = MANIFEST.read_bytes()
    stamp = SOURCES / ".source-manifest.json"
    if stamp.is_file() and stamp.read_bytes() == manifest:
        return
    SOURCES.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="imglean-native-", dir=SOURCES.parent) as temporary:
        staging = Path(temporary)
        for source in json.loads(manifest):
            archive = staging / "source.tar.gz"
            request = urllib.request.Request(source["url"], headers={"User-Agent": "ImgLean-build"})
            digest = hashlib.sha256()
            length = 0
            with urllib.request.urlopen(request, timeout=60) as response, archive.open("wb") as output:
                while chunk := response.read(1024 * 1024):
                    length += len(chunk)
                    if length > 128 * 1024 * 1024:
                        raise RuntimeError("native source download exceeds 128 MiB")
                    digest.update(chunk)
                    output.write(chunk)
            if digest.hexdigest() != source["sha256"]:
                raise RuntimeError(f"checksum mismatch for {source['name']}")
            destination = staging / "sources" / source["path"]
            destination.mkdir(parents=True, exist_ok=True)
            unpack(archive, destination)
            archive.unlink()
        prepared = staging / "sources"
        # libjpeg-turbo 3.2 moved its public headers into src/. Jpegli's
        # pinned CMake lists still configure them from the old root paths.
        jpeg_headers = prepared / "jpegli/third_party/libjpeg-turbo"
        for name in ["jconfig.h.in", "jpeglib.h", "jmorecfg.h"]:
            shutil.copy2(jpeg_headers / "src" / name, jpeg_headers / name)
        # The old reproducible-date macros are also sent to the Windows resource
        # compiler. Restrict them to C/C++ as required by current CMake.
        cmake = prepared / "jpegli/CMakeLists.txt"
        text = cmake.read_text()
        old = '''  add_definitions(
    # Avoid changing the binary based on the current time and date.
    -D__DATE__="redacted"
    -D__TIMESTAMP__="redacted"
    -D__TIME__="redacted"
  )'''
        new = '''  add_compile_definitions(
    "$<$<COMPILE_LANGUAGE:C,CXX>:__DATE__=\\\"redacted\\\">"
    "$<$<COMPILE_LANGUAGE:C,CXX>:__TIMESTAMP__=\\\"redacted\\\">"
    "$<$<COMPILE_LANGUAGE:C,CXX>:__TIME__=\\\"redacted\\\">"
  )'''
        if text.count(old) != 1:
            raise RuntimeError("pinned Jpegli CMake date definitions changed")
        cmake.write_text(text.replace(old, new))
        (prepared / ".source-manifest.json").write_bytes(manifest)
        if SOURCES.exists():
            shutil.rmtree(SOURCES)
        shutil.move(prepared, SOURCES)


if __name__ == "__main__":
    prepare()
