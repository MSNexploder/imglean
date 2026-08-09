# ImgLean

ImgLean is a local CLI that runs every applicable PNG optimization strategy and
writes the smallest candidate that passes its bounded validation gate. The
validated source is always the first candidate, so a successful output is never
larger than its source. Sources are never replaced; existing regular output
files are replaced only after the new result is complete and validated.

Version 0.4 is implemented in source. Target-specific 64-bit macOS, Linux, and
Windows artifacts remain unpublished and unqualified until their native release
gates pass.

## Use

The output directory must already exist. Requested destinations may be absent or
existing regular files; directories, symbolic links, special files, and input
aliases are rejected:

```sh
imglean --output ./optimized photo.png icon.png
```

The default ordered strategy set is:

1. embedded `oxipng-libdeflate-v1`;
2. embedded `oxipng-zopfli-v1`;
3. external `optipng-v1` when OptiPNG 7.9.1 is found on `PATH`; and
4. external `pngquant-v1` at numeric quality when pngquant 3.0.2 or 3.0.3 is
   found.

`--quality lossless|1..100` selects the fidelity policy and defaults to
`lossless`. The two OxiPNG strategies and OptiPNG remain eligible at every
setting because they are lossless. pngquant participates only for numeric
quality and maps `Q` to its native `--quality 0-Q` option. Lower values permit
more color reduction; 100 requests pngquant's highest native target, but is
still lossy for images with more than 256 colors.

All compatible embedded strategies are enabled by default. An automatically
missing or incompatible external provider remains visible as `unavailable` but
is not run. Strategy controls are explicit and repeatable:

```sh
imglean --disable-strategy oxipng-zopfli-v1 --output ./optimized photo.png
imglean --require-strategy optipng-v1 --output ./optimized photo.png
imglean --provider optipng /absolute/path/to/optipng --output ./optimized photo.png
imglean --quality 80 --output ./optimized photo.png
imglean --quality 80 --provider pngquant /absolute/path/to/pngquant --output ./optimized photo.png
imglean --jobs 1 --output ./optimized photo.png
```

`--provider` both selects the executable and requires its adapter. ImgLean never
downloads, installs, or updates external providers. Run `imglean --help` for the
complete CLI surface.

pngquant discovery uses `PATH` or an explicit `--provider pngquant PATH` on
every platform. An unavailable numeric-quality provider is reported as
`unavailable`; at lossless quality pngquant is reported as `not applicable` and
is not probed.

Version 0.4 accepts bounded static PNGs in every standard color-type and
bit-depth combination, including Adam7. It verifies container checksums and a
complete decode, requires candidate dimensions to match, and refuses APNG,
`caBX`, and XMP in PNG text chunks. Other accepted ancillary data is opaque.
ImgLean trusts each audited, pinned strategy to honor its fidelity and metadata
configuration; it does not independently compare pixels, calculate perceptual
quality, or compare ancillary payloads. pngquant intentionally changes colors
and strips optional metadata; the basic PNG gate still rejects malformed,
animated, wrong-sized, C2PA, and XMP candidates.
See the [PNG contract](docs/contracts/PNG.md) for the exact boundary.

Exit statuses are `0` for clean success, `3` when all outputs succeed despite
an optimizer warning, `1` for processing or reporting failure, and `2` for
invalid CLI usage. Per-input results, including the winning strategy, go to
standard output. Each block lists the complete strategy registry; `->` marks
the winner, `!` marks a warning or rejected candidate, and strategies that were
disabled, unavailable, not applicable, or not run retain explicit rows:

```text
photo.png
     baseline                 109,592 bytes
     oxipng-libdeflate-v1     109,104 bytes
  -> oxipng-zopfli-v1         108,928 bytes  winner; saved 664 bytes (0.61%)
     optipng-v1               unavailable
     pngquant-v1              not applicable
     output                   /path/to/optimized/photo.png
```

Strategy warnings remain inside the relevant image block. Provider records,
failure details, and the compact invocation summary go to standard error.
Inputs are processed sequentially. For each input, ImgLean runs up to two
strategy workers concurrently by default when the machine exposes at least two
CPUs. `--jobs N` selects one to three workers; reporting and tie-breaking always
follow registry order rather than completion order.

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
release targets. Separate native jobs exercise OptiPNG 7.9.1 and pngquant 3.0.2
and 3.0.3, including automatic discovery and real provider reductions.

## Documentation

- [SCOPE.md](SCOPE.md) defines the product and version 0.4 boundary.
- [ARCHITECTURE.md](ARCHITECTURE.md) defines components and data flow.
- The [input](docs/contracts/INPUT_AND_BATCH.md),
  [PNG](docs/contracts/PNG.md),
  [provider](docs/contracts/PROVIDER_EXECUTION.md),
  [output](docs/contracts/OUTPUT.md), and
  [limits](docs/contracts/LIMITS.md) contracts define exact behavior.
- Provider-specific settings are recorded for
  [OxiPNG](docs/providers/OXIPNG.md),
  [OptiPNG](docs/providers/OPTIPNG.md), and
  [pngquant](docs/providers/PNGQUANT.md).
- [docs/RELEASE.md](docs/RELEASE.md) defines target qualification and artifact
  contents.

## License

ImgLean is licensed under Apache-2.0. See [LICENSE.md](LICENSE.md).
