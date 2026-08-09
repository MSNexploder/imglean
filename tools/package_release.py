#!/usr/bin/env python3
"""Build a self-describing ImgLean release archive from validated inputs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
RELEASE_TARGETS = {
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--sbom", required=True, type=Path)
    parser.add_argument("--notices", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--allow-dirty", action="store_true")
    args = parser.parse_args()
    if args.target not in RELEASE_TARGETS:
        parser.error(f"unsupported version 0.6 release target: {args.target}")

    required = [
        args.binary,
        args.sbom,
        args.notices,
        ROOT / "Cargo.lock",
        ROOT / "LICENSE.md",
    ]
    for path in required:
        if not path.is_file():
            parser.error(f"required release input is missing: {path}")
    source_dirty = bool(run(["git", "status", "--porcelain", "--untracked-files=all"]))
    if source_dirty and not args.allow_dirty:
        parser.error("release packaging requires a clean source tree")
    rustc = run(["rustc", "-vV"])
    rustc_host = next(
        (line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: ")),
        None,
    )
    if rustc_host != args.target:
        parser.error(f"release target {args.target} is not the native Rust host {rustc_host}")
    binary = args.binary.resolve()
    smoke_release_binary(binary)

    package = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]
    archive_stem = f"{package['name']}-{package['version']}-{args.target}"
    args.output.mkdir(parents=True, exist_ok=True)

    metadata = cargo_metadata(args.target)
    packages = reachable_packages(metadata)

    with tempfile.TemporaryDirectory(prefix="imglean-release-") as temporary:
        staging = Path(temporary) / archive_stem
        staging.mkdir()
        binary_name = "imglean.exe" if args.target.endswith("windows-msvc") else "imglean"
        staged_binary = staging / binary_name
        shutil.copy2(binary, staged_binary)
        shutil.copy2(ROOT / "LICENSE.md", staging / "LICENSE.md")
        shutil.copy2(args.notices, staging / "THIRD_PARTY_NOTICES.md")
        shutil.copy2(args.sbom, staging / "SBOM.spdx.json")

        inventory_path = staging / "DEPENDENCIES.tsv"
        inventory_path.write_text(dependency_inventory(packages), encoding="utf-8")
        manifest = release_manifest(
            args.target,
            staged_binary,
            metadata,
            packages,
            source_dirty,
            rustc,
        )
        (staging / "RELEASE_MANIFEST.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        if args.target.endswith("windows-msvc"):
            archive = args.output / f"{archive_stem}.zip"
            write_zip(archive, staging, archive_stem)
        else:
            archive = args.output / f"{archive_stem}.tar.gz"
            with tarfile.open(archive, "w:gz") as bundle:
                bundle.add(staging, arcname=archive_stem)

    verify_archive(archive, archive_stem, binary_name)

    checksum = sha256(archive)
    archive.with_suffix(archive.suffix + ".sha256").write_text(
        f"{checksum}  {archive.name}\n", encoding="ascii"
    )
    print(archive)
    return 0


def cargo_metadata(target: str) -> dict[str, Any]:
    command = [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
        "--filter-platform",
        target,
    ]
    return json.loads(run(command))


def reachable_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [metadata["resolve"]["root"]]
    reachable: set[str] = set()
    while pending:
        package_id = pending.pop()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        pending.extend(nodes[package_id]["dependencies"])
    packages = [package for package in metadata["packages"] if package["id"] in reachable]
    return sorted(packages, key=lambda item: (item["name"], item["version"]))


def dependency_inventory(packages: list[dict[str, Any]]) -> str:
    lines = []
    for package in packages:
        lines.append(
            "\t".join(
                [
                    package["name"],
                    package["version"],
                    package.get("license") or "UNKNOWN",
                    package.get("source") or "workspace",
                ]
            )
        )
    lines.extend(
        [
            "cexcept\t2.99-optipng\tZlib\tvendored",
            "highway\t1.1.0\tApache-2.0\tvendored by jpegli-sys",
            "libwebp\t"
            f"{(ROOT / 'ci/libwebp-version.txt').read_text().strip()}"
            "\tBSD-3-Clause\tvendored by libwebp-sys",
            "optipng\t"
            f"{(ROOT / 'ci/optipng-version.txt').read_text().strip()}"
            "\tZlib AND Libpng\tvendored",
        ]
    )
    return "name\tversion\tlicense\tsource\n" + "\n".join(sorted(lines)) + "\n"


def release_manifest(
    target: str,
    binary: Path,
    metadata: dict[str, Any],
    packages: list[dict[str, Any]],
    source_dirty: bool,
    rustc: str,
) -> dict[str, Any]:
    versions = {package["name"]: package["version"] for package in packages}
    identities = {
        package["id"]: f"{package['name']} {package['version']}" for package in packages
    }
    features = {
        identities[node["id"]]: sorted(node["features"])
        for node in metadata["resolve"]["nodes"]
        if node["id"] in identities
    }
    return {
        "schema_version": 9,
        "package": "imglean",
        "version": tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"],
        "source_commit": run(["git", "rev-parse", "HEAD"]),
        "source_dirty": source_dirty,
        "target": target,
        "rustc": rustc,
        "cargo": run(["cargo", "-V"]),
        "mise": run_optional(["mise", "--version"]),
        "cargo_features": dict(sorted(features.items())),
        "cargo_lock_sha256": sha256(ROOT / "Cargo.lock"),
        "binary_sha256": sha256(binary),
        "bundled_providers": {
            "oxipng": versions.get("oxipng"),
            "libdeflater": versions.get("libdeflater"),
            "libdeflate-sys": versions.get("libdeflate-sys"),
            "zopfli": versions.get("zopfli"),
            "optipng": (ROOT / "ci/optipng-version.txt").read_text().strip(),
            "cexcept": "2.99-optipng",
            "highway": "1.1.0",
            "mozjpeg": versions.get("mozjpeg"),
            "mozjpeg-sys": versions.get("mozjpeg-sys"),
            "jpegli": versions.get("jpegli"),
            "jpegli-sys": versions.get("jpegli-sys"),
            "libpng-sys": versions.get("libpng-sys"),
            "image-webp": versions.get("image-webp"),
            "libwebp": (ROOT / "ci/libwebp-version.txt").read_text().strip(),
            "libwebp-sys": versions.get("libwebp-sys"),
            "libavif": versions.get("libavif"),
            "libavif-sys": versions.get("libavif-sys"),
            "libaom-sys": versions.get("libaom-sys"),
            "ravif": versions.get("ravif"),
            "rav1e": versions.get("rav1e"),
        },
        "representative_external_overrides": [
            {
                "strategies": ["optipng-v1"],
                "implementation": "OptiPNG",
                "revision": (ROOT / "ci/optipng-version.txt").read_text().strip(),
            },
            *[
                {
                    "strategies": ["pngquant-v1"],
                    "implementation": "pngquant",
                    "revision": revision,
                }
                for revision in (ROOT / "ci/pngquant-versions.txt").read_text().splitlines()
                if revision
            ],
            {
                "strategies": ["jpegtran-v1", "mozjpeg-v1"],
                "implementation": "MozJPEG",
                "revision": (ROOT / "ci/mozjpeg-revision.txt").read_text().strip(),
            },
            {
                "strategies": ["jpegtran-v1"],
                "implementation": "libjpeg-turbo",
                "revision": (ROOT / "ci/libjpeg-turbo-revision.txt").read_text().strip(),
            },
            {
                "strategies": ["jpegli-v1"],
                "implementation": "Jpegli",
                "revision": (ROOT / "ci/jpegli-revision.txt").read_text().strip(),
            },
            {
                "strategies": ["libwebp-v1"],
                "implementation": "libwebp cwebp",
                "revision": (ROOT / "ci/libwebp-version.txt").read_text().strip(),
            },
        ],
        "strategy_registry": [
            {
                "id": "oxipng-libdeflate-v1",
                "execution": "bundled",
                "format": "PNG",
                "settings": "OxiPNG 10.1.1, pinned filters, libdeflater level 11",
                "metadata": "preserve by default; OxiPNG Safe with --strip-metadata",
            },
            {
                "id": "oxipng-zopfli-v1",
                "execution": "bundled",
                "format": "PNG",
                "settings": "OxiPNG 10.1.1, pinned filters, Zopfli 15 iterations",
                "metadata": "preserve by default; OxiPNG Safe with --strip-metadata",
            },
            {
                "id": "optipng-v1",
                "execution": "bundled-with-external-override",
                "provider": "optipng",
                "format": "PNG",
                "bundled_settings": "OptiPNG 7.9.1 PNG-only engine, optimization level 2",
                "override_discovery": "CLI capability probe; provider version is not gated",
                "arguments": ["-quiet", "-o2", "-out", "CANDIDATE", "--", "INPUT"],
                "strip_metadata_arguments": [
                    "-quiet",
                    "-o2",
                    "-strip",
                    "all",
                    "-out",
                    "CANDIDATE",
                    "--",
                    "INPUT",
                ],
            },
            {
                "id": "pngquant-v1",
                "execution": "external-optional",
                "provider": "pngquant",
                "format": "PNG",
                "discovery": "CLI capability probe; provider version is not gated",
                "applicability": "numeric quality only",
                "arguments": [
                    "--force",
                    "--quality",
                    "0-QUALITY",
                    "--speed",
                    "4",
                    "--strip",
                    "--output",
                    "CANDIDATE",
                    "--",
                    "INPUT",
                ],
                "metadata": "always requests provider-native --strip",
            },
            {
                "id": "jpegtran-v1",
                "execution": "bundled-with-external-override",
                "provider": "jpegtran",
                "format": "JPEG",
                "bundled_settings": "MozJPEG coefficient transcode, optimized progressive coding",
                "override_discovery": "CLI capability probe; provider version is not gated",
                "applicability": "lossless and numeric quality",
                "arguments": [
                    "-copy",
                    "all",
                    "-optimize",
                    "-progressive",
                    "-strict",
                    "-outfile",
                    "CANDIDATE",
                    "INPUT",
                ],
                "strip_metadata_arguments": [
                    "-copy",
                    "none",
                    "-optimize",
                    "-progressive",
                    "-strict",
                    "-outfile",
                    "CANDIDATE",
                    "INPUT",
                ],
            },
            {
                "id": "mozjpeg-v1",
                "execution": "bundled-with-external-override",
                "provider": "mozjpeg",
                "format": "JPEG",
                "bundled_settings": "MozJPEG RGB or grayscale re-encode, optimized progressive coding",
                "override_discovery": "CLI capability probe; provider version is not gated",
                "applicability": "numeric quality only",
                "arguments": [
                    "-quality",
                    "QUALITY",
                    "-progressive",
                    "-optimize",
                    "-strict",
                    "-outfile",
                    "CANDIDATE",
                    "INPUT",
                ],
                "metadata": "bundled implementation preserves opaque markers and handles structural JFIF/Adobe markers without duplicates or blind replay; --strip-metadata omits saved markers; external override uses its native CLI behavior",
            },
            {
                "id": "jpegli-v1",
                "execution": "bundled-with-external-override",
                "provider": "jpegli",
                "format": "JPEG",
                "bundled_settings": "Jpegli RGB or grayscale re-encode, optimized progressive coding",
                "override_discovery": "CLI capability probe; provider version is not gated",
                "applicability": "numeric quality only",
                "arguments": [
                    "--quality",
                    "QUALITY",
                    "--progressive_level",
                    "2",
                    "INPUT",
                    "CANDIDATE",
                ],
                "metadata": "bundled implementation preserves opaque markers and handles structural JFIF/Adobe markers without duplicates or blind replay; --strip-metadata omits saved markers; external override uses its native CLI behavior",
            },
            {
                "id": "libwebp-v1",
                "execution": "bundled-with-external-override",
                "provider": "libwebp",
                "format": "WebP",
                "bundled_settings": "libwebp lossless preset 9 or native quality Q, method 6, exact transparent RGB, lossless alpha, one thread",
                "override_discovery": "cwebp CLI capability probe; provider version is not gated",
                "applicability": "lossless and numeric quality",
                "metadata": "preserves ICC and Exif by default; --strip-metadata omits both",
            },
            {
                "id": "image-webp-v1",
                "execution": "bundled",
                "format": "WebP",
                "settings": "image-webp 0.2.4 lossless encoder",
                "applicability": "lossless and numeric quality",
                "metadata": "preserves ICC and Exif by default; --strip-metadata omits both",
            },
            {
                "id": "avif-aom-v1",
                "execution": "bundled",
                "format": "AVIF",
                "settings": "libavif with libaom, native quality Q, alpha quality 100, speed 6, one thread",
                "applicability": "numeric quality only",
                "metadata": "selected API exposes no metadata-removal control; normal output under either policy",
            },
            {
                "id": "avif-rav1e-v1",
                "execution": "bundled",
                "format": "AVIF",
                "settings": "ravif/rav1e native quality Q, alpha quality 100, speed 6, 8-bit, one thread",
                "applicability": "numeric quality only",
                "metadata": "ravif does not expose metadata preservation or stripping controls",
            },
        ],
        "quality_policy": {
            "accepted": "lossless or an integer from 1 through 100",
            "default": "lossless",
            "numeric_mapping": "provider-native and strategy-versioned",
            "candidate_trust": "format-specific basic candidate gate plus pinned adapter settings",
        },
        "metadata_policy": {
            "control": "--strip-metadata",
            "mapping": "provider-native and strategy-versioned",
            "guarantee": "best effort; unchanged baseline remains eligible",
            "controller_transform": False,
            "controller_verification": False,
        },
        "strategy_workers": {
            "default_cap": 2,
            "maximum": 3,
            "selection": "minimum of available parallelism and default cap",
            "timeout_seconds": {
                "default": 60,
                "minimum": 6,
                "maximum": 600,
                "oxipng_internal_reserve": 5,
                "oxipng_internal_minimum": 1,
            },
        },
        "limits_version": "v9",
        "build_environment": {
            "platform": platform.platform(),
            "packager": f"Python {platform.python_version()}",
            "native_compiler": native_compiler(),
            "linker": native_linker(),
            "sdk": sdk_details(),
            "build_flags": selected_environment(),
        },
    }


def selected_environment() -> dict[str, str]:
    names = [
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CFLAGS",
        "CXXFLAGS",
        "LDFLAGS",
        "MACOSX_DEPLOYMENT_TARGET",
        "RUSTFLAGS",
        "WindowsSDKVersion",
    ]
    return {name: os.environ[name] for name in names if name in os.environ}


def native_compiler() -> str | None:
    candidates = [["cl.exe"], ["cc", "--version"], ["clang", "--version"]]
    return first_available(candidates)


def native_linker() -> str | None:
    if platform.system() == "Darwin":
        candidates = [["ld", "-v"]]
    else:
        candidates = [["link.exe", "/?"], ["ld", "--version"]]
    return first_available(candidates)


def sdk_details() -> dict[str, str]:
    details: dict[str, str] = {}
    for name, command in [
        ("macos_path", ["xcrun", "--show-sdk-path"]),
        ("macos_version", ["xcrun", "--show-sdk-version"]),
        ("glibc", ["ldd", "--version"]),
    ]:
        value = run_optional(command)
        if value:
            details[name] = value
    if "WindowsSdkDir" in os.environ:
        details["windows_path"] = os.environ["WindowsSdkDir"]
    if "WindowsSDKVersion" in os.environ:
        details["windows_version"] = os.environ["WindowsSDKVersion"]
    return details


def first_available(commands: list[list[str]]) -> str | None:
    for command in commands:
        if shutil.which(command[0]):
            value = run_optional(command)
            if value:
                return value
    return None


def run(command: list[str]) -> str:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return completed.stdout.strip()


def run_optional(command: list[str]) -> str | None:
    if not shutil.which(command[0]):
        return None
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    value = completed.stdout.strip()
    return value or None


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_zip(archive: Path, staging: Path, archive_stem: str) -> None:
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for path in sorted(staging.rglob("*")):
            if path.is_file():
                bundle.write(path, arcname=Path(archive_stem) / path.relative_to(staging))


def smoke_release_binary(binary: Path) -> None:
    for argument in ["--help", "--version"]:
        subprocess.run(
            [binary, argument],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    with tempfile.TemporaryDirectory(prefix="imglean-release-smoke-") as temporary:
        directory = Path(temporary)
        source = directory / "source.png"
        output = directory / "output"
        output.mkdir()
        destination = output / source.name
        destination.write_bytes(b"existing")
        shutil.copyfile(
            ROOT / "tests/corpus/png/v2/accepted/oxipng-reduction.png",
            source,
        )
        subprocess.run(
            [binary, "--output", output, source],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if not destination.is_file() or destination.stat().st_size >= source.stat().st_size:
            raise RuntimeError("release executable failed the packaged-workflow smoke test")


def verify_archive(archive: Path, archive_stem: str, binary_name: str) -> None:
    required = {
        f"{archive_stem}/{binary_name}",
        f"{archive_stem}/DEPENDENCIES.tsv",
        f"{archive_stem}/LICENSE.md",
        f"{archive_stem}/RELEASE_MANIFEST.json",
        f"{archive_stem}/SBOM.spdx.json",
        f"{archive_stem}/THIRD_PARTY_NOTICES.md",
    }
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as bundle:
            names = set(bundle.namelist())
    else:
        with tarfile.open(archive, "r:gz") as bundle:
            names = set(bundle.getnames())
    missing = required - names
    if missing:
        raise RuntimeError(f"release archive is incomplete: {sorted(missing)}")


if __name__ == "__main__":
    raise SystemExit(main())
