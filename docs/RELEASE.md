# Version 0.6 Release Contract

> [!IMPORTANT]
> Release automation is implemented, but the native target artifacts and
> multi-platform Linux container remain unqualified and unpublished until
> their workflow gates pass.

## Targets

Version 0.6 release qualification begins with these target-specific artifacts:

- `aarch64-apple-darwin`;
- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-musl`; and
- `x86_64-pc-windows-msvc`.

The macOS archives encode a macOS 15 deployment target. The Linux archive is
built natively for musl in a pinned Alpine builder and has no ELF interpreter
or shared-library dependencies. The Windows archive uses the
`x86_64-pc-windows-msvc` target contract: Windows 10 or newer for client
systems and Windows Server 2016 or newer for servers. CI runner versions are
build and test inputs, not user-facing runtime boundaries.

The container targets are `linux/amd64` and `linux/arm64`. Each is built and
smoke-tested on a native runner, and the published version and `latest` tags
are OCI indexes containing both variants. Their final images use the pinned
distroless Debian 13 C/C++ runtime, run as a non-root user, and contain no shell
or package manager. They contain the executable, Apache-2.0 license, and
third-party notices. Per-architecture build-tool records beside the release
assets capture the exact Debian packages, compiler, linker, CMake, Ninja, and
NASM used for each image.

## Build inputs

At release start, the moving stable Rust channel is resolved to one exact
toolchain and used for all builds. Each artifact records:

- Rust and Cargo versions;
- the complete dependency lock;
- target, deployment settings, enabled Cargo features, and build flags;
- OxiPNG, libdeflater, and other bundled revisions;
- native compiler, linker, SDK, and build-tool versions;
- system-package versions where applicable; and
- the source commit.

This record supports audit and reconstruction. Version 0.6 does not promise
bit-for-bit reproducible binaries.

The development configuration currently pins cargo-deny `0.20.2`, cargo-about
`0.9.1` with its CLI feature, and cargo-sbom `0.10.0`. Their versions are part
of the release workflow and change only through a reviewed configuration
update. The release workflow resolves the stable Rust channel once in its
compliance job, passes that exact version to every native build, and records it
in each artifact rather than copying it into this contract. The packager's
Python version is recorded per artifact. Validator fuzzing and native sanitizer
tests use cargo-fuzz `0.13.2` with the exact nightly recorded in
`ci/rust-nightly-version.txt`; that nightly is not used for release binaries.

## Qualification

The shared source runs the canonical locked formatting, lint, and test gate on
GNU/Linux, both macOS architectures, and Windows. Each release executable then
runs target-native tests covering the CLI, every bundled strategy, bounded
parallel workers, replacing rename publication, metadata, and the packaged
workflow. The musl job additionally rejects an ELF interpreter or any
`DT_NEEDED` shared library before archiving the executable.
Windows builds pin CMake's Ninja generator because the bundled Jpegli wrapper
expects a single-configuration native-library layout.
Pinned representative OptiPNG, pngquant, MozJPEG, libjpeg-turbo jpegtran,
Jpegli, and libwebp revisions prove capability discovery and real execution on
every release target, including the static musl executable. These pins make CI
reproducible; runtime adapters do not gate provider release numbers.
Representative native filesystems must demonstrate complete replacing rename
publication. Cross-compilation alone does not qualify a target.

CI also regenerates the checked-in PNG, JPEG, WebP, and AVIF corpora and rejects
drift. A Linux hardening job runs the complete test suite with AddressSanitizer,
including instrumented native C and C++ builds, then fuzzes every format
validator from private copies of its checked-in corpus. These checks complement
the bounded corpus; they do not turn provider workers into a security sandbox.

The container matrix builds both architectures with the exact Rust version
resolved by the compliance job, smoke-tests the packaged CLI and
publication-free check path inside each image, and saves both qualified image
archives for the publication gate. It resolves the exact builder-image digest
before each build and records that digest, the pinned runtime image, Docker
versions, source commit, target platform, and build-tool record extracted from
each qualified image with the release assets.

The canonical source gate is `mise run check`. Release qualification also runs
`mise run audit`, regenerates and verifies `THIRD_PARTY_NOTICES.md`, generates
and parses an SPDX 2.3 SBOM, and builds each release executable natively. The
Linux job resolves its exact Rust-on-Alpine builder digest, records every
installed Alpine package version, runs the locked test suite and representative
external providers inside that builder, and smoke-tests the exact static
executable before archiving it.

Manual dispatch runs the complete qualification workflow and retains native
archives plus the container image as workflow artifacts without publishing
them. Pushing a tag exactly matching `v` plus the Cargo package version runs the
same gates. The release workflow calls the complete CI workflow, including all
bundled and representative external-provider integrations. Only after that
workflow, every native package, compliance, and container job succeeds does the
publication job create a draft GitHub release, publish the AMD64 and ARM64
containers behind multi-platform GHCR version and `latest` tags, and make the
GitHub release visible. The workflow never creates or pushes a tag.

The dependency gate checks licenses, advisories, source provenance, enabled
features, direct and transitive Rust code, vendored code, build scripts, and
native libraries. GPL, AGPL, proprietary, source-incompatible, and
redistribution-restricted code blocks release.
An independent weekly workflow refreshes the advisory database and reruns the
same locked dependency policy even when no source change triggers normal CI.

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

The GHCR image is a separate multi-platform distribution, not a portable
replacement for the native archives. Its tag follows the Git tag, including
the leading `v`.

## Homebrew tap

The public `MSNexploder/homebrew-tap` repository distributes ImgLean's
macOS-only formula. The formula selects the qualified `aarch64-apple-darwin` or
`x86_64-apple-darwin` GitHub release archive for the current Mac and installs
its executable unchanged. It requires macOS 15 because that is the qualified
runtime floor for both archives. Homebrew therefore does not introduce a second
build or dependency-resolution path.

After the GitHub release becomes public, the release workflow generates the
formula from the two archive checksum files, prepares its tap commit locally,
runs `brew style`, strict online audit, installation, and the formula test on an
Apple Silicon runner and an Intel runner, then pushes the commit only after both
architectures pass. The final publisher retries concurrent tap updates against
the newest tap commit, while the generator and publisher both refuse to replace
a newer formula when an older release workflow is rerun. The source repository
must define a fine-grained
`HOMEBREW_TAP_TOKEN` secret with contents write access only to
`MSNexploder/homebrew-tap`. The GitHub release remains the authoritative binary
and checksum source.

`tools/package_release.py` refuses a dirty source tree for normal packaging. It
also accepts only the four declared version 0.6 targets, requires the declared
target to equal the native Rust host, enforces the target runtime contract
(including static linkage for Linux), and requires the release-binary smoke
corpus to produce a strict size reduction. It
creates `.tar.gz` archives for macOS and Linux and `.zip` archives for Windows,
verifies the expected archive members, and writes a sibling SHA-256 checksum.
`--allow-dirty` exists only for local pre-commit testing and records that state
in the manifest; such an archive is not releasable.
