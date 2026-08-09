# ImgLean

ImgLean is a local command-line tool that makes supported PNG images leaner.
Its built-in workflow is offline, requires no separately installed optimizer,
and independently validates every OxiPNG result before it can win. The validated
source is always a candidate, so a successful output is never larger than its
source.

Version 0.1 is implemented in source. The three target-specific release
artifacts are not published or considered qualified until their native CI gates
pass on 64-bit macOS, Linux, and Windows.

## Use

The output directory must already exist, support same-directory hard links, and
not contain any requested destination:

```sh
imglean --output ./optimized photo.png icon.png
```

ImgLean preflights the complete batch before creating anything. It retains each
original basename, rejects ambiguous or unsafe mappings, captures and validates
each source, runs the fixed OxiPNG strategy in an isolated worker process, and
publishes the smallest accepted bytes without replacing a destination. Sources
are never written or replaced.

Version 0.1 accepts a deliberately strict non-animated, non-interlaced PNG
subset: 8-bit grayscale, truecolor, grayscale-alpha, and truecolor-alpha, plus
1/2/4/8-bit indexed color. It preserves accepted ancillary chunks byte-for-byte
around the IDAT group and refuses APNG, 16-bit samples, ICC, Exif, XMP/C2PA,
compressed/international text, and unknown chunks. See the
[PNG contract](docs/contracts/PNG.md) for the exact subset.

Exit statuses are `0` for clean success, `3` when all outputs succeed despite
an optimizer warning, `1` for processing or reporting failure, and `2` for
invalid CLI usage. Human-readable per-input results go to standard output;
warnings, errors, and the invocation summary go to standard error.

## Build and validate

ImgLean uses mise to install the selected stable Rust toolchain and pinned
release-audit tools:

```sh
mise install
mise run check
```

`mise run check` verifies formatting, runs Clippy for every target and feature
with warnings denied, and runs the complete locked test suite. Release work also
runs:

```sh
mise run audit
mise run notices
mise run sbom
```

The release workflows build and test natively on the three documented targets,
generate notices and an SPDX 2.3 SBOM, smoke-test the release executable, and
package it with an input manifest, dependency inventory, license, and checksums.

## Documentation

- [SCOPE.md](SCOPE.md) defines the product outcomes and milestone boundary.
- [ARCHITECTURE.md](ARCHITECTURE.md) defines the component boundaries and flow.
- The [input](docs/contracts/INPUT_AND_BATCH.md),
  [PNG](docs/contracts/PNG.md), [provider](docs/contracts/PROVIDER_EXECUTION.md),
  [output](docs/contracts/OUTPUT.md), and
  [resource-limit](docs/contracts/LIMITS.md) contracts define exact version 0.1
  behavior.
- [docs/RELEASE.md](docs/RELEASE.md) defines target qualification and artifact
  contents.

## License

ImgLean is licensed under Apache-2.0. See [LICENSE.md](LICENSE.md).
