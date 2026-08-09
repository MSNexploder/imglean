# Version 0.6 Release Contract

> [!IMPORTANT]
> Release automation is implemented, but the x86-64 target artifacts and
> linux/amd64 container remain unqualified and unpublished until their native
> workflow gates pass.

## Targets

Version 0.6 release qualification begins with these target-specific artifacts:

- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-gnu`; and
- `x86_64-pc-windows-msvc`.

The checked-in native matrix uses `macos-15-intel`, `ubuntu-24.04`, and
`windows-2025`. Until lower runtime boundaries are separately tested and
recorded, the qualified claims are limited to macOS 15 on x86-64, Ubuntu 24.04
on x86-64, and Windows Server 2025 on x86-64. A successful build on these
runners does not imply compatibility with earlier releases.

The container target is `linux/amd64`. Its final image uses the pinned
distroless Debian 13 C/C++ runtime, runs as a non-root user, and contains no
shell or package manager. It contains the executable, Apache-2.0 license, and
third-party notices.

## Build inputs

At release start, the moving stable Rust channel is resolved to one exact
toolchain and used for all builds. Each artifact records:

- Rust and Cargo versions;
- the complete dependency lock;
- target, deployment settings, enabled Cargo features, and build flags;
- OxiPNG, libdeflater, and other bundled revisions;
- native compiler, linker, SDK, and build-tool versions; and
- the source commit.

This record supports audit and reconstruction. Version 0.6 does not promise
bit-for-bit reproducible binaries.

The development configuration currently pins cargo-deny `0.20.2`, cargo-about
`0.9.1` with its CLI feature, and cargo-sbom `0.10.0`. Their versions are part
of the release workflow and change only through a reviewed configuration
update. The release workflow resolves the stable Rust channel once in its
compliance job, passes that exact version to every native build, and records it
in each artifact rather than copying it into this contract. Python `3.13` runs
the standard-library-only packager and is also recorded.

## Qualification

Every target runs the canonical locked formatting, lint, and test gate plus the
target-native CLI, every bundled strategy, bounded parallel workers,
replacing rename publication, metadata, and packaged-artifact smoke tests.
Windows builds pin CMake's Ninja generator because the bundled Jpegli wrapper
expects a single-configuration native-library layout.
Separate CI jobs build pinned representative OptiPNG, pngquant, MozJPEG,
libjpeg-turbo jpegtran, Jpegli, and libwebp revisions, then prove capability
discovery and real execution on each target. These pins make CI reproducible; runtime
adapters do not gate provider release numbers.
Representative native filesystems must demonstrate complete replacing rename
publication. Cross-compilation alone does not qualify a target.

The container job builds with the exact Rust version resolved by the compliance
job, smoke-tests the packaged CLI and publication-free check path inside the image,
and saves the qualified container image archive for the publication gate. It
resolves the exact builder-image digest before the build and records that digest,
the pinned runtime image, Docker versions, and source commit with the release
assets.

The canonical source gate is `mise run check`. Release qualification also runs
`mise run audit`, regenerates and verifies `THIRD_PARTY_NOTICES.md`, generates
and parses an SPDX 2.3 SBOM, builds the target's release executable natively,
and smoke-tests that exact executable through the complete worker and replacing
publication flow before archiving it.

Manual dispatch runs the complete qualification workflow and retains native
archives plus the container image as workflow artifacts without publishing
them. Pushing a tag exactly matching `v` plus the Cargo package version runs the
same gates. The release workflow calls the complete CI workflow, including all
bundled and representative external-provider integrations. Only after that
workflow, every native package, compliance, and container job succeeds does the
publication job create a draft GitHub release, publish the container to GHCR as
the version tag and `latest`, and make the GitHub release visible. The workflow
never creates or pushes a tag.

The dependency gate checks licenses, advisories, source provenance, enabled
features, direct and transitive Rust code, vendored code, build scripts, and
native libraries. GPL, AGPL, proprietary, source-incompatible, and
redistribution-restricted code blocks release.

## Artifact contents

Each platform archive contains:

- the target-specific `imglean` executable;
- `LICENSE.md`;
- third-party notices;
- a dependency inventory;
- an SPDX or CycloneDX software bill of materials;
- the release-input manifest; and
- SHA-256 checksums.

The manifest records operation modes and exit statuses, stable strategy order,
identifiers, quality and metadata policies, worker limits, bundled settings,
external-override capability
contracts, invocation arguments, representative CI revisions, and exact
bundled OxiPNG, OptiPNG, Cexcept, MozJPEG, Jpegli, Highway, libpng,
libdeflater, Zopfli, libwebp, image-webp, libavif, libaom, ravif, and rav1e
dependency versions. pngquant and explicitly selected
provider executables are not bundled or included in the dependency inventory
or SBOM.

Archives do not include an installer, separately installed optimizer, or runtime.
Release notes state the exact platform boundary and do not imply that one binary
runs across operating systems.

The GHCR image is a separate target-specific distribution, not a portable
replacement for the native archives. Its tag follows the Git tag, including the
leading `v`.

`tools/package_release.py` refuses a dirty source tree for normal packaging. It
also accepts only the three declared version 0.6 targets, requires the declared
target to equal the native Rust host, and requires the release-binary smoke
corpus to produce a strict size reduction. It
creates `.tar.gz` archives for macOS and Linux and `.zip` archives for Windows,
verifies the expected archive members, and writes a sibling SHA-256 checksum.
`--allow-dirty` exists only for local pre-commit testing and records that state
in the manifest; such an archive is not releasable.
