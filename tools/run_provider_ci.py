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
PNGQUANT_SHA256 = {
    "3.0.2": "33f8501d8b81f34cb6f028a5d06772b9d7940e0bc2b15a5d0bce138cb74233cb",
    "3.0.3": "68a12bdd8825f9989f4ee9a6ab0b42727dae57728b939ef63453366697a07232",
}
PNGQUANT_RGB_VERSION = "0.8.52"
PNGQUANT_BYTEMUCK_VERSION = "1.25.2"


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


def replace_exact(
    path: Path, old: str, new: str, description: str, *, expected_count: int = 1
) -> None:
    contents = path.read_text(encoding="utf-8")
    if contents.count(old) != expected_count:
        raise RuntimeError(f"the pinned {description} changed")
    path.write_text(contents.replace(old, new), encoding="utf-8")


def cmake_configure(source: Path, build: Path, options: Sequence[str]) -> None:
    arguments: list[os.PathLike[str] | str] = ["cmake", "-S", source, "-B", build]
    if os.name == "nt":
        arguments.extend(["-G", "Ninja"])
    if toolchain := os.environ.get("CMAKE_TOOLCHAIN_FILE"):
        arguments.append(f"-DCMAKE_TOOLCHAIN_FILE={toolchain}")
    arguments.append("-DCMAKE_BUILD_TYPE=Release")
    arguments.extend(options)
    run(arguments)


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
        libpng_cmake = source / "third_party/libpng/CMakeLists.txt"
        replace_exact(libpng_cmake, "    pngpread.c\n", "", "OptiPNG progressive-read source list")
        replace_exact(libpng_cmake, "    pngwtran.c\n", "", "OptiPNG write-transform source list")
        build = source / "build"
        cmake_configure(source, build, ["-DOPTIPNG_BUILD_TESTS=OFF"])
        cmake_build(build, "optipng")
        provider = executable(build / "optipng")
    helper("test_external_provider.py", "--binary", binary, "--provider", provider)


def build_pngquant(directory: Path, binary: Path, version: str) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / f"pngquant-{version}.crate"
    download(
        f"https://crates.io/api/v1/crates/pngquant/{version}/download",
        archive,
        PNGQUANT_SHA256[version],
    )
    with tarfile.open(archive) as contents:
        contents.extractall(directory, filter="data")
    source = directory / f"pngquant-{version}"
    stale_sse_check = """if target_arch == "x86_64" ||
       (target_arch == "x86" && cfg!(feature = "sse")) {"""
    replace_exact(
        source / "rust/build.rs",
        stale_sse_check,
        'if target_arch == "x86_64" {',
        "pngquant SSE feature check",
    )
    manifest = source / "Cargo.toml"
    run(
        [
            "cargo",
            "update",
            "--manifest-path",
            manifest,
            "--package",
            "rgb",
            "--precise",
            PNGQUANT_RGB_VERSION,
        ]
    )
    run(
        [
            "cargo",
            "update",
            "--manifest-path",
            manifest,
            "--package",
            "bytemuck",
            "--precise",
            PNGQUANT_BYTEMUCK_VERSION,
        ]
    )
    run(["cargo", "build", "--locked", "--release", "--manifest-path", manifest])
    provider = executable(source / "target/release/pngquant")
    helper("test_pngquant_provider.py", "--binary", binary, "--provider", provider)


def build_jpegtran(directory: Path, binary: Path) -> Path:
    directory.mkdir(parents=True)
    source = directory / "source"
    build = source / "build"
    prefix = directory / "install"
    clone("https://github.com/libjpeg-turbo/libjpeg-turbo.git", "ci/libjpeg-turbo-revision.txt", source)
    replace_exact(
        source / "CMakeLists.txt",
        "cmake_minimum_required(VERSION 3.15...3.28)",
        "cmake_minimum_required(VERSION 3.15...3.28)\n"
        "if(POLICY CMP0219)\n"
        "  cmake_policy(SET CMP0219 NEW)\n"
        "endif()",
        "libjpeg-turbo CMP0219 policy declaration",
    )
    cmake_configure(
        source,
        build,
        [
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
    replace_exact(
        source / "CMakeLists.txt",
        "cmake_minimum_required(VERSION 2.8.12)",
        "cmake_minimum_required(VERSION 3.10)\n"
        "if(POLICY CMP0219)\n"
        "  cmake_policy(SET CMP0219 NEW)\n"
        "endif()",
        "MozJPEG CMake minimum version",
    )
    replace_exact(
        source / "rdtarga.c",
        "for (i = 0; i < sinfo->pixel_size; i++) {",
        "for (i = 0; i < sinfo->pixel_size && i < 4; i++) {",
        "MozJPEG Targa pixel bounds",
        expected_count=2,
    )
    cmake_configure(
        source,
        build,
        [
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
            for library_directory in (jpeg_prefix / "lib", jpeg_prefix / "lib64")
            for pattern in ("*.a", "*.lib")
            for path in library_directory.glob(pattern)
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
    old_policy = "cmake_policy(SET CMP0111 OLD)"
    replace_exact(
        source / "third_party/highway/CMakeLists.txt",
        old_policy,
        "cmake_policy(SET CMP0111 NEW)",
        "Highway CMP0111 policy declaration",
    )
    reproducible_definitions = '''  add_definitions(
    # Avoid changing the binary based on the current time and date.
    -D__DATE__="redacted"
    -D__TIMESTAMP__="redacted"
    -D__TIME__="redacted"
  )'''
    replace_exact(
        source / "CMakeLists.txt",
        reproducible_definitions,
        '''  add_compile_definitions(
    # Resource compilers do not accept the C/C++ warning control below.
    "$<$<COMPILE_LANGUAGE:C,CXX>:__DATE__=\\\"redacted\\\">"
    "$<$<COMPILE_LANGUAGE:C,CXX>:__TIMESTAMP__=\\\"redacted\\\">"
    "$<$<COMPILE_LANGUAGE:C,CXX>:__TIME__=\\\"redacted\\\">"
  )''',
        "Jpegli reproducible compiler definitions",
    )
    cmake_configure(
        source,
        build,
        [
            "-DBUILD_SHARED_LIBS=OFF",
            "-DBUILD_TESTING=OFF",
            "-DCMAKE_LINK_LIBRARIES_STRATEGY=REORDER_FREELY",
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
            "-DCMAKE_DISABLE_FIND_PACKAGE_GIF=TRUE",
            "-DCMAKE_DISABLE_FIND_PACKAGE_JPEG=TRUE",
            "-DCMAKE_DISABLE_FIND_PACKAGE_PNG=TRUE",
            "-DCMAKE_LINK_LIBRARIES_STRATEGY=REORDER_FREELY",
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
