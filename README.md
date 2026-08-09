# ImgLean

ImgLean is a focused, offline CLI for reducing existing PNG, JPEG, WebP, and
AVIF files without changing their format. It runs the applicable optimizers,
independently checks every result, and selects the smallest accepted candidate.
The original always participates, so a successful output is never larger.
Fidelity-reducing strategies run only after an explicit numeric quality choice.

It is intended to be one predictable command for people, scripts, CI, and
coding agents—not an image editor or general asset pipeline.

Version 0.6 is implemented in source. Target-specific 64-bit macOS, Linux, and
Windows artifacts remain unpublished and unqualified until their native release
gates pass.

## Quick start

Write lossless results to an existing separate directory:

```sh
imglean --output ./optimized photo.jpg icon.png hero.webp cover.avif
```

Explicitly allow strategies that use their native numeric quality control:

```sh
imglean --quality 80 --output ./optimized photo.jpg hero.webp
```

Check whether committed assets could be reduced without creating output files:

```sh
imglean --check assets/logo.png assets/hero.jpg
```

`--check` exits with status `4` when at least one input has a smaller accepted
candidate. It performs the same source, provider, and candidate work as output
mode but never publishes a result.

## Guarantees and boundary

- Inputs keep their format and dimensions; ImgLean does not resize, crop,
  rotate, or convert them.
- Sources are never replaced. Output mode writes to a separate directory and
  replaces only a requested regular destination after the result is complete.
- The original remains a candidate, so an output is never larger than its
  source.
- Lossless is the default. Lossy strategies require `--quality 1..100`.
- Strategy order, selection, and reporting are stable for the same accepted
  candidate set.
- The bundled workflow is offline and never downloads providers.
- Candidate acceptance proves the documented basic format gates, not pixel or
  ancillary-payload equivalence and not a globally smallest representation.

ImgLean removes avoidable encoding overhead within the selected format and
fidelity policy. Another format might be smaller, but conversion is
intentionally outside its scope.

## Strategies and controls

In output mode the destination directory must already exist. Requested
destinations may be absent or existing regular files; directories, symbolic
links, special files, and input aliases are rejected.

The default ordered strategy set is:

1. bundled `oxipng-libdeflate`;
2. bundled `oxipng-zopfli`;
3. bundled `optipng` for PNG;
4. external `pngquant` for PNG at numeric quality;
5. bundled `jpegtran` for lossless JPEG optimization;
6. bundled `mozjpeg` for JPEG at numeric quality;
7. bundled `jpegli` for JPEG at numeric quality;
8. bundled `libwebp` for lossless or numeric-quality WebP;
9. bundled `image-webp` for lossless WebP;
10. bundled `avif-aom` for AVIF at numeric quality; and
11. bundled `avif-rav1e` for AVIF at numeric quality.

`--quality lossless|1..100` selects the fidelity policy and defaults to
`lossless`. The two OxiPNG strategies, OptiPNG, jpegtran, and image-webp remain
eligible at every setting because they are lossless. pngquant, MozJPEG,
Jpegli, and both AVIF strategies participate only at numeric quality. libwebp
uses lossless encoding at `lossless` and native lossy quality at numeric
settings. pngquant maps `Q` to its native
`--quality 0-Q` range; lower values permit more color reduction, while 100
still permits palette conversion. MozJPEG and Jpegli receive `Q` as their
native quality value.

All compatible bundled strategies are enabled by default. pngquant remains an
optional external strategy because its GPL/commercial licensing is not
compatible with the default Apache-2.0 binary. Strategy controls are explicit
and repeatable:

```sh
imglean --disable-strategy oxipng-zopfli --output ./optimized photo.png
imglean --require-strategy optipng --output ./optimized photo.png
imglean --provider optipng /absolute/path/to/optipng --output ./optimized photo.png
imglean --quality 80 --output ./optimized photo.png
imglean --quality 80 --provider pngquant /absolute/path/to/pngquant --output ./optimized photo.png
imglean --provider jpegtran /absolute/path/to/jpegtran --output ./optimized photo.jpg
imglean --quality 80 --provider mozjpeg /absolute/path/to/cjpeg --output ./optimized photo.jpg
imglean --quality 80 --provider jpegli /absolute/path/to/cjpegli --output ./optimized photo.jpg
imglean --quality 80 --provider libwebp /absolute/path/to/cwebp --output ./optimized hero.webp
imglean --jobs 1 --output ./optimized photo.png
imglean --strip-metadata --output ./optimized photo.jpg icon.png
```

`--provider` overrides the bundled implementation for OptiPNG, jpegtran,
MozJPEG, Jpegli, or libwebp; for pngquant it selects the external implementation. It
also requires that adapter to pass its capability probe. ImgLean never
downloads, installs, or updates external providers. Run `imglean --help` for
the complete CLI surface.

Automatic `PATH` discovery is used for the unbundled pngquant strategy. Explicit
`--provider NAME PATH` overrides are supported on every platform for all six
provider names. ImgLean verifies required CLI capabilities instead of accepting
or rejecting release-number strings. CI pins representative external revisions
for reproducibility, but runtime compatibility is capability-based. At lossless
quality pngquant, MozJPEG, Jpegli, and both AVIF strategies are `not
applicable`; jpegtran, both WebP strategies, and the PNG lossless strategies
remain applicable.

`--strip-metadata` allows metadata removal and asks strategies with a native
control to use it. OxiPNG uses its safe strip mode, OptiPNG strips all PNG
metadata it recognizes, bundled jpegtran copies no extra JPEG markers, and
pngquant already strips optional metadata. Bundled MozJPEG and Jpegli preserve
opaque JPEG application/comment markers by default, carry JFIF density forward,
keep a single JFIF marker, and avoid replaying source Adobe markers onto the new
encoding. They omit saved markers when stripping is requested.
Both WebP strategies preserve ICC and Exif by default and omit them when
stripping is requested. The selected AVIF APIs expose no metadata-removal
control, so both AVIF strategies remain eligible and emit their normal output.
External overrides retain their own documented native behavior. No
otherwise-applicable strategy is excluded solely because it cannot strip
metadata. ImgLean does not
implement a separate metadata stripper or verify that all metadata is gone.
The unchanged source baseline still participates and can win, so this option is
best effort rather than a guarantee that the selected output contains no
metadata.

Version 0.6 accepts bounded static PNGs in every standard color-type and
bit-depth combination, including Adam7. It verifies container checksums and a
complete decode, requires candidate dimensions to match, and refuses APNG,
`caBX`, and XMP in PNG text chunks. Other accepted ancillary data is opaque.
ImgLean trusts each audited, pinned strategy to honor its fidelity and metadata
configuration; it does not independently compare pixels, calculate perceptual
quality, compare ancillary payloads, or verify metadata stripping. pngquant
intentionally changes colors and strips optional metadata; the basic PNG gate
still rejects malformed, animated, wrong-sized, C2PA, and XMP candidates.
It also accepts bounded 8-bit baseline, extended sequential, and progressive
Huffman JPEGs, requires a complete decode and matching candidate dimensions,
and refuses standard XMP plus APP11. Other accepted JPEG application and comment
segments are opaque. Bundled numeric JPEG strategies preserve their payloads by
default while safely handling structural encoding markers, but external overrides
may drop Exif orientation and other application
metadata. Use the default lossless policy when exact sample preservation is
required. Static WebP accepts lossy and lossless bitstreams plus alpha, ICC,
and Exif while refusing animation, XMP, and `C2PA`. Static AVIF requires an
`avif` brand and refuses sequences, the C2PA BMFF UUID, and the standard XMP
MIME type. See the [PNG](docs/contracts/PNG.md),
[JPEG](docs/contracts/JPEG.md), [WebP](docs/contracts/WEBP.md), and
[AVIF](docs/contracts/AVIF.md) contracts for the exact boundaries.

Exit statuses are `0` for clean success, `1` for processing or reporting
failure, `2` for invalid CLI usage, `3` when processing succeeds with optimizer
warnings, and `4` when `--check` finds at least one smaller accepted candidate.
Per-input results, including the winning strategy, go to
standard output. Each block lists the registry rows for that input's format;
`->` marks the winner, `!` marks a warning or rejected candidate, and strategies
that were disabled, unavailable, not applicable at the selected quality, or not
run retain explicit rows:

```text
photo.png
     baseline                 109,592 bytes
     oxipng-libdeflate        109,104 bytes
  -> oxipng-zopfli            108,928 bytes  winner; saved 664 bytes (0.61%)
     optipng                  109,088 bytes
     pngquant                 not applicable
     output                   /path/to/optimized/photo.png
```

Strategy warnings remain inside the relevant image block. Provider records for
formats present in the batch, failure details, and the compact invocation
summary go to standard error.
Inputs are processed sequentially. For each input, ImgLean runs up to two
strategy workers concurrently by default when the machine exposes at least two
CPUs. `--jobs N` selects one to three workers; reporting and tie-breaking always
follow registry order rather than completion order. `--timeout SECONDS` sets
the per-strategy worker deadline from 6 through 600 seconds and defaults to 60;
each worker is capped by the remaining invocation time. Provider discovery,
validation, and the invocation-wide deadline are unchanged.

## Automation workflows

Optimize explicitly selected generated assets into a staging directory before
promoting them through the calling workflow:

```sh
mkdir -p .imglean-output
imglean --output .imglean-output generated/logo.png generated/hero.jpg
```

Fail a verification step when tracked assets have avoidable encoding overhead:

```sh
imglean --check public/logo.png public/hero.jpg
status=$?
test "$status" -eq 0
```

Status `4` means an optimization is available; status `1` means processing
failed; status `2` means the command was invalid; and status `3` means checking
completed but at least one optimizer warned. A calling script or agent owns
file discovery and any promotion of staged output. ImgLean intentionally does
not traverse directories or replace sources in place.

## Install on macOS

Once the first release is published, tagged releases will be available through
the project tap for native Apple Silicon and Intel Macs:

```sh
brew install MSNexploder/imglean/imglean
```

The formula installs the same qualified executable published in the GitHub
release archive; it does not compile a separate Homebrew variant.

## Build and validate

ImgLean uses mise to select the Rust toolchain and release-audit tools:

```sh
mise install
mise run check
```

`mise run check` verifies formatting, runs Clippy for all targets and features
with warnings denied, and runs the complete locked test suite. Release work also
runs `mise run audit`, `mise run notices`, and `mise run sbom`. Windows source
builds require CMake and Ninja; set `CMAKE_GENERATOR=Ninja` so the pinned Jpegli
wrapper produces the static libraries in its expected single-configuration
layout. CI and release workflows set this explicitly.

CI executes every bundled strategy directly and through the controller on all
release targets. Separate native jobs build pinned representative executable
overrides for OptiPNG, pngquant, jpegtran, MozJPEG, Jpegli, and libwebp and require real
reductions through capability discovery and the complete controller path.

The release workflow can be dispatched manually to produce unpublished
qualification artifacts. A matching `v<package-version>` tag publishes only
after the complete bundled/external-provider CI suite plus the macOS, Linux,
Windows, compliance, and linux/amd64 container gates all succeed. The container
is a non-root distroless image intended for explicit
file arguments and mounted input/output directories; its package name is the
current repository under `ghcr.io`.

## Documentation

- [SCOPE.md](SCOPE.md) defines the product and version 0.6 boundary.
- [ARCHITECTURE.md](ARCHITECTURE.md) defines components and data flow.
- The [input](docs/contracts/INPUT_AND_BATCH.md),
  [PNG](docs/contracts/PNG.md),
  [JPEG](docs/contracts/JPEG.md),
  [WebP](docs/contracts/WEBP.md),
  [AVIF](docs/contracts/AVIF.md),
  [provider](docs/contracts/PROVIDER_EXECUTION.md),
  [output](docs/contracts/OUTPUT.md), and
  [limits](docs/contracts/LIMITS.md) contracts define exact behavior.
- Provider-specific settings are recorded for
  [OxiPNG](docs/providers/OXIPNG.md),
  [OptiPNG](docs/providers/OPTIPNG.md),
  [pngquant](docs/providers/PNGQUANT.md),
  [jpegtran](docs/providers/JPEGTRAN.md),
  [MozJPEG](docs/providers/MOZJPEG.md),
  [Jpegli](docs/providers/JPEGLI.md),
  [libwebp](docs/providers/LIBWEBP.md),
  [image-webp](docs/providers/IMAGE_WEBP.md),
  [libavif/libaom](docs/providers/AVIF_AOM.md), and
  [ravif/rav1e](docs/providers/AVIF_RAV1E.md).
- [Provider selection](docs/PROVIDER_SELECTION_WEBP_AVIF.md) records the WebP
  and AVIF implementation analysis and rejected alternatives.
- [docs/RELEASE.md](docs/RELEASE.md) defines target qualification and artifact
  contents.

## License

ImgLean is licensed under Apache-2.0. See [LICENSE.md](LICENSE.md).
