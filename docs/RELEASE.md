# Version 0.6 Release Contract

> [!IMPORTANT]
> Release automation is implemented, but the x86-64 target artifacts are not
> yet qualified or published. Qualification requires the native workflow runs
> below and recorded minimum operating-system boundaries.

## Targets

Version 0.6 release qualification begins with these target-specific artifacts:

- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-gnu`; and
- `x86_64-pc-windows-msvc`.

Before the first release candidate, native test results define and record the
minimum macOS deployment version, minimum supported glibc version, and minimum
supported Windows version. If a target cannot satisfy the documented workflow,
the scope and this contract are revised before release rather than silently
shipping an unqualified artifact.

The checked-in native matrix uses `macos-15-intel`, `ubuntu-24.04`, and
`windows-2025`. These runner images are build environments, not minimum runtime
version claims.

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

The canonical source gate is `mise run check`. Release qualification also runs
`mise run audit`, regenerates and verifies `THIRD_PARTY_NOTICES.md`, generates
and parses an SPDX 2.3 SBOM, builds the target's release executable natively,
and smoke-tests that exact executable through the complete worker and replacing
publication flow before archiving it.

The release-candidate workflow is manually dispatched and retains its archives
as workflow artifacts. It does not create a tag or publish a GitHub release;
those external publication actions require a separate explicit decision after
the target qualifications and minimum runtime boundaries are recorded.

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

The manifest records stable strategy order, identifiers, quality and metadata
policies, worker limits, bundled settings, external-override capability
contracts, invocation arguments, representative CI revisions, and exact
bundled OxiPNG, OptiPNG, Cexcept, MozJPEG, Jpegli, Highway, libpng,
libdeflater, Zopfli, libwebp, image-webp, libavif, libaom, ravif, and rav1e
dependency versions. pngquant and explicitly selected
provider executables are not bundled or included in the dependency inventory
or SBOM.

Archives do not include an installer, separately installed optimizer, or runtime.
Release notes state the exact platform boundary and do not imply that one binary
runs across operating systems.

`tools/package_release.py` refuses a dirty source tree for normal packaging. It
also accepts only the three declared version 0.6 targets, requires the declared
target to equal the native Rust host, and requires the release-binary smoke
corpus to produce a strict size reduction. It
creates `.tar.gz` archives for macOS and Linux and `.zip` archives for Windows,
verifies the expected archive members, and writes a sibling SHA-256 checksum.
`--allow-dirty` exists only for local pre-commit testing and records that state
in the manifest; such an archive is not releasable.
