# ImgLean

ImgLean is a local CLI that runs every applicable PNG or JPEG optimization strategy and
writes the smallest candidate that passes its bounded validation gate. The
validated source is always the first candidate, so a successful output is never
larger than its source. Sources are never replaced; existing regular output
files are replaced only after the new result is complete and validated.

Version 0.6 is implemented in source. Target-specific 64-bit macOS, Linux, and
Windows artifacts remain unpublished and unqualified until their native release
gates pass.

## Use

The output directory must already exist. Requested destinations may be absent or
existing regular files; directories, symbolic links, special files, and input
aliases are rejected:

```sh
imglean --output ./optimized photo.jpg icon.png
```

The default ordered strategy set is:

1. embedded `oxipng-libdeflate-v1`;
2. embedded `oxipng-zopfli-v1`;
3. external `optipng-v1` for PNG;
4. external `pngquant-v1` for PNG at numeric quality;
5. external `jpegtran-v1` for lossless JPEG optimization;
6. external `mozjpeg-v1` for JPEG at numeric quality; and
7. external `jpegli-v1` for JPEG at numeric quality.

`--quality lossless|1..100` selects the fidelity policy and defaults to
`lossless`. The two OxiPNG strategies, OptiPNG, and jpegtran remain eligible at
every setting because they are lossless. pngquant, MozJPEG, and Jpegli
participate only at numeric quality. pngquant maps `Q` to its native
`--quality 0-Q` range; lower values permit more color reduction, while 100
still permits palette conversion. MozJPEG and Jpegli receive `Q` as their
native quality value.

All compatible embedded strategies are enabled by default. An automatically
missing or incompatible external provider remains visible as `unavailable` but
is not run. Strategy controls are explicit and repeatable:

```sh
imglean --disable-strategy oxipng-zopfli-v1 --output ./optimized photo.png
imglean --require-strategy optipng-v1 --output ./optimized photo.png
imglean --provider optipng /absolute/path/to/optipng --output ./optimized photo.png
imglean --quality 80 --output ./optimized photo.png
imglean --quality 80 --provider pngquant /absolute/path/to/pngquant --output ./optimized photo.png
imglean --provider jpegtran /absolute/path/to/jpegtran --output ./optimized photo.jpg
imglean --quality 80 --provider mozjpeg /absolute/path/to/cjpeg --output ./optimized photo.jpg
imglean --quality 80 --provider jpegli /absolute/path/to/cjpegli --output ./optimized photo.jpg
imglean --jobs 1 --output ./optimized photo.png
imglean --strip-metadata --output ./optimized photo.jpg icon.png
```

`--provider` both selects the executable and requires its adapter. ImgLean never
downloads, installs, or updates external providers. Run `imglean --help` for the
complete CLI surface.

External discovery uses `PATH` or an explicit `--provider NAME PATH` on every
platform. ImgLean verifies the required CLI capabilities instead of accepting
or rejecting release-number strings. CI pins representative upstream revisions
for reproducibility, but runtime compatibility is capability-based. An
unavailable provider is reported as `unavailable`; at lossless quality pngquant,
MozJPEG, and Jpegli are `not applicable` and are not probed. jpegtran remains
applicable at lossless and numeric quality.

`--strip-metadata` allows metadata removal and asks strategies with a native
control to use it. OxiPNG uses its safe strip mode, OptiPNG strips all PNG
metadata it recognizes, jpegtran copies no extra JPEG markers, and pngquant
already strips optional metadata. Jpegli's native decode/re-encode path does
not copy source application markers. MozJPEG exposes no compatible removal
control and may preserve them, but remains eligible. No otherwise-applicable
strategy is excluded solely because it cannot strip metadata. ImgLean does not
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
segments are opaque. Numeric JPEG strategies may therefore drop Exif
orientation and other application metadata; use the default lossless policy
when that metadata must be preserved. See the [PNG](docs/contracts/PNG.md) and
[JPEG](docs/contracts/JPEG.md) contracts for the exact boundaries.

Exit statuses are `0` for clean success, `3` when all outputs succeed despite
an optimizer warning, `1` for processing or reporting failure, and `2` for
invalid CLI usage. Per-input results, including the winning strategy, go to
standard output. Each block lists the registry rows for that input's format;
`->` marks the winner, `!` marks a warning or rejected candidate, and strategies
that were disabled, unavailable, not applicable at the selected quality, or not
run retain explicit rows:

```text
photo.png
     baseline                 109,592 bytes
     oxipng-libdeflate-v1     109,104 bytes
  -> oxipng-zopfli-v1         108,928 bytes  winner; saved 664 bytes (0.61%)
     optipng-v1               unavailable
     pngquant-v1              not applicable
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

## Build and validate

ImgLean uses mise to select the Rust toolchain and release-audit tools:

```sh
mise install
mise run check
```

`mise run check` verifies formatting, runs Clippy for all targets and features
with warnings denied, and runs the complete locked test suite. Release work also
runs `mise run audit`, `mise run notices`, and `mise run sbom`.

CI executes both embedded strategies directly and through the controller on all
release targets. Separate native jobs build pinned representative revisions of
all five external providers and require real reductions through capability
discovery and the complete controller path.

## Documentation

- [SCOPE.md](SCOPE.md) defines the product and version 0.6 boundary.
- [ARCHITECTURE.md](ARCHITECTURE.md) defines components and data flow.
- The [input](docs/contracts/INPUT_AND_BATCH.md),
  [PNG](docs/contracts/PNG.md),
  [JPEG](docs/contracts/JPEG.md),
  [provider](docs/contracts/PROVIDER_EXECUTION.md),
  [output](docs/contracts/OUTPUT.md), and
  [limits](docs/contracts/LIMITS.md) contracts define exact behavior.
- Provider-specific settings are recorded for
  [OxiPNG](docs/providers/OXIPNG.md),
  [OptiPNG](docs/providers/OPTIPNG.md),
  [pngquant](docs/providers/PNGQUANT.md),
  [jpegtran](docs/providers/JPEGTRAN.md),
  [MozJPEG](docs/providers/MOZJPEG.md), and
  [Jpegli](docs/providers/JPEGLI.md).
- [docs/RELEASE.md](docs/RELEASE.md) defines target qualification and artifact
  contents.

## License

ImgLean is licensed under Apache-2.0. See [LICENSE.md](LICENSE.md).
