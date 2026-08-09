#!/usr/bin/env python3
"""Build and exercise every supported external provider for one CI target."""

from __future__ import annotations

import argparse
import hashlib
import os
import platform
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from collections.abc import Callable, Sequence
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OPTIPNG_SOURCE_SHA256 = "c2579be58c2c66dae9d63154edcb3d427fef64cb00ec0aff079c9d156ec46f29"
OPTIPNG_WINDOWS_SHA256 = "cdc632c21e11b2e0ba6f87a0df632f810827267f263b655002a849d3f87b06b2"
LIBWEBP_SHA256 = "e4ab7009bf0629fd11982d4c2aa83964cf244cffba7347ecd39019a9e38c4564"


def run(arguments: Sequence[os.PathLike[str] | str], *, cwd: Path = ROOT) -> None:
    subprocess.run([os.fspath(argument) for argument in arguments], cwd=cwd, check=True)


def read_pin(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8").strip()


def download(url: str, destination: Path, expected_sha256: str) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "ImgLean-CI"})
    with urllib.request.urlopen(request) as response, destination.open("wb") as output:
        while chunk := response.read(1024 * 1024):
            output.write(chunk)
    actual = hashlib.sha256(destination.read_bytes()).hexdigest()
    if actual != expected_sha256:
        raise RuntimeError(f"checksum mismatch for {url}: expected {expected_sha256}, got {actual}")


def clone(repository: str, revision_file: str, destination: Path, *, submodules: bool = False) -> None:
    run(["git", "clone", "--filter=blob:none", "--no-checkout", repository, destination])
    run(
        ["git", "-c", "advice.detachedHead=false", "checkout", read_pin(revision_file)],
        cwd=destination,
    )
    if submodules:
        run(["git", "submodule", "update", "--init", "--recursive", "--depth", "1"], cwd=destination)


def cmake_configure(source: Path, build: Path, options: Sequence[str]) -> None:
    run(["cmake", "-S", source, "-B", build, "-DCMAKE_BUILD_TYPE=Release", *options])


def cmake_build(build: Path, *targets: str) -> None:
    arguments: list[os.PathLike[str] | str] = [
        "cmake",
        "--build",
        build,
        "--config",
        "Release",
    ]
    if targets:
        arguments.extend(["--target", *targets])
    arguments.extend(["--parallel", "2"])
    run(arguments)


def executable(path: Path) -> Path:
    resolved = path.with_suffix(".exe") if os.name == "nt" else path
    if not resolved.is_file():
        raise RuntimeError(f"provider executable was not built: {resolved}")
    return resolved.resolve()


def helper(script: str, *arguments: os.PathLike[str] | str) -> None:
    run([sys.executable, ROOT / "tools" / script, *arguments])


def build_optipng(directory: Path, binary: Path) -> None:
    directory.mkdir(parents=True)
    version = read_pin("ci/optipng-version.txt")
    if os.name == "nt":
        archive = directory / "optipng.zip"
        download(
            f"https://downloads.sourceforge.net/project/optipng/OptiPNG/optipng-{version}/optipng-{version}-win64.zip",
            archive,
            OPTIPNG_WINDOWS_SHA256,
        )
        extracted = directory / "source"
        with zipfile.ZipFile(archive) as contents:
            contents.extractall(extracted)
        provider = next(extracted.rglob("optipng.exe"), None)
        if provider is None:
            raise RuntimeError("OptiPNG archive does not contain optipng.exe")
    else:
        archive = directory / "optipng.tar.gz"
        download(
            f"https://downloads.sourceforge.net/project/optipng/OptiPNG/optipng-{version}/optipng-{version}.tar.gz",
            archive,
            OPTIPNG_SOURCE_SHA256,
        )
        with tarfile.open(archive) as contents:
            contents.extractall(directory, filter="data")
        source = directory / f"optipng-{version}"
        build = source / "build"
        cmake_configure(source, build, ["-DOPTIPNG_BUILD_TESTS=OFF"])
        cmake_build(build, "optipng")
        provider = executable(build / "optipng")
    helper("test_external_provider.py", "--binary", binary, "--provider", provider)


def build_pngquant(directory: Path, binary: Path, version: str) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    root = directory / version
    run(["cargo", "install", "--locked", "--version", version, "--root", root, "pngquant"])
    provider = executable(root / "bin/pngquant")
    helper("test_pngquant_provider.py", "--binary", binary, "--provider", provider)


def build_jpegtran(directory: Path, binary: Path) -> Path:
    directory.mkdir(parents=True)
    source = directory / "source"
    build = source / "build"
    prefix = directory / "install"
    clone("https://github.com/libjpeg-turbo/libjpeg-turbo.git", "ci/libjpeg-turbo-revision.txt", source)
    cmake_configure(
        source,
        build,
        [
            "-DCMAKE_POLICY_VERSION_MINIMUM=3.5",
            f"-DCMAKE_INSTALL_PREFIX={prefix}",
            "-DENABLE_SHARED=OFF",
            "-DENABLE_STATIC=ON",
            "-DWITH_SIMD=OFF",
            "-DWITH_TESTS=OFF",
            "-DWITH_TURBOJPEG=OFF",
        ],
    )
    cmake_build(build)
    run(["cmake", "--install", build, "--config", "Release"])
    provider = executable(prefix / "bin/jpegtran")
    helper("test_jpegtran_provider.py", "--binary", binary, "--provider", provider)
    return prefix


def build_mozjpeg(directory: Path, binary: Path) -> None:
    directory.mkdir(parents=True)
    source = directory / "source"
    build = source / "build"
    clone("https://github.com/mozilla/mozjpeg.git", "ci/mozjpeg-revision.txt", source)
    cmake_configure(
        source,
        build,
        [
            "-DCMAKE_POLICY_VERSION_MINIMUM=3.5",
            "-DENABLE_SHARED=OFF",
            "-DENABLE_STATIC=ON",
            "-DPNG_SUPPORTED=OFF",
            "-DWITH_SIMD=OFF",
            "-DWITH_TURBOJPEG=OFF",
        ],
    )
    cmake_build(build, "cjpeg-static", "jpegtran-static")
    helper(
        "test_jpeg_provider.py",
        "--binary",
        binary,
        "--name",
        "mozjpeg",
        "--provider",
        executable(build / "cjpeg-static"),
    )
    helper(
        "test_jpegtran_provider.py",
        "--binary",
        binary,
        "--provider",
        executable(build / "jpegtran-static"),
    )


def build_jpegli(directory: Path, binary: Path, jpeg_prefix: Path | None) -> None:
    if jpeg_prefix is None:
        raise RuntimeError("the pinned libjpeg-turbo prerequisite failed")
    jpeg_library = next(
        (
            path
            for pattern in ("*.a", "*.lib")
            for path in (jpeg_prefix / "lib").glob(pattern)
            if "jpeg" in path.name.lower() and "turbo" not in path.name.lower()
        ),
        None,
    )
    if jpeg_library is None:
        raise RuntimeError("the pinned libjpeg-turbo installation has no JPEG library")
    directory.mkdir(parents=True)
    source = directory / "source"
    build = source / "build"
    clone("https://github.com/google/jpegli.git", "ci/jpegli-revision.txt", source, submodules=True)
    cmake_configure(
        source,
        build,
        [
            "-DBUILD_TESTING=OFF",
            "-DCMAKE_DISABLE_FIND_PACKAGE_GIF=TRUE",
            "-DCMAKE_DISABLE_FIND_PACKAGE_PNG=TRUE",
            f"-DCMAKE_PREFIX_PATH={jpeg_prefix}",
            f"-DJPEG_INCLUDE_DIR={jpeg_prefix / 'include'}",
            f"-DJPEG_LIBRARY={jpeg_library}",
            "-DJPEGLI_BUNDLE_LIBPNG=OFF",
            "-DJPEGLI_ENABLE_BENCHMARK=OFF",
            "-DJPEGLI_ENABLE_DOXYGEN=OFF",
            "-DJPEGLI_ENABLE_JNI=OFF",
            "-DJPEGLI_ENABLE_MANPAGES=OFF",
            "-DJPEGLI_ENABLE_OPENEXR=OFF",
            "-DJPEGLI_ENABLE_SJPEG=OFF",
        ],
    )
    cmake_build(build, "cjpegli")
    helper("test_jpeg_provider.py", "--binary", binary, "--name", "jpegli", "--provider", executable(build / "tools/cjpegli"))


def build_libwebp(directory: Path, binary: Path) -> None:
    directory.mkdir(parents=True)
    version = read_pin("ci/libwebp-version.txt")
    archive = directory / "libwebp.tar.gz"
    download(
        f"https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-{version}.tar.gz",
        archive,
        LIBWEBP_SHA256,
    )
    with tarfile.open(archive) as contents:
        contents.extractall(directory, filter="data")
    source = directory / f"libwebp-{version}"
    build = source / "build"
    cmake_configure(
        source,
        build,
        [
            "-DBUILD_SHARED_LIBS=OFF",
            "-DWEBP_BUILD_ANIM_UTILS=OFF",
            "-DWEBP_BUILD_CWEBP=ON",
            "-DWEBP_BUILD_DWEBP=OFF",
            "-DWEBP_BUILD_EXTRAS=OFF",
            "-DWEBP_BUILD_GIF2WEBP=OFF",
            "-DWEBP_BUILD_IMG2WEBP=OFF",
            "-DWEBP_BUILD_VWEBP=OFF",
            "-DWEBP_BUILD_WEBPINFO=OFF",
            "-DWEBP_BUILD_WEBPMUX=OFF",
        ],
    )
    cmake_build(build, "cwebp")
    helper("test_webp_provider.py", "--binary", binary, "--provider", executable(build / "cwebp"))


def report_error(name: str, error: Exception) -> None:
    message = str(error).replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print(f"::error title={name} provider integration failed::{message}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        raise SystemExit(f"ImgLean executable does not exist: {binary}")

    failures: list[str] = []
    jpeg_prefix: Path | None = None
    with tempfile.TemporaryDirectory(prefix="imglean-provider-ci-") as temporary:
        root = Path(temporary)

        def check(name: str, operation: Callable[[], None]) -> None:
            print(f"::group::{name}", flush=True)
            try:
                operation()
            except Exception as error:  # Every provider must still be attempted.
                failures.append(name)
                report_error(name, error)
            finally:
                print("::endgroup::", flush=True)

        check("OptiPNG", lambda: build_optipng(root / "optipng", binary))
        versions = (ROOT / "ci/pngquant-versions.txt").read_text(encoding="utf-8").splitlines()
        check(f"pngquant {versions[0]}", lambda: build_pngquant(root / "pngquant", binary, versions[0]))
        if platform.system() == "Linux":
            check(f"pngquant {versions[1]}", lambda: build_pngquant(root / "pngquant", binary, versions[1]))

        def jpegtran() -> None:
            nonlocal jpeg_prefix
            jpeg_prefix = build_jpegtran(root / "jpegtran", binary)

        check("jpegtran", jpegtran)
        check("MozJPEG", lambda: build_mozjpeg(root / "mozjpeg", binary))
        check("Jpegli", lambda: build_jpegli(root / "jpegli", binary, jpeg_prefix))
        check("libwebp", lambda: build_libwebp(root / "libwebp", binary))

    if failures:
        print(f"provider integrations failed: {', '.join(failures)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
