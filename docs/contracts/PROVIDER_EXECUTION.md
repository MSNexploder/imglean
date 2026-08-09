# Version 0.6 Provider Execution Contract

## Registry and controls

The source baseline precedes this stable order:

1. `oxipng-libdeflate-v1` (PNG, bundled, lossless)
2. `oxipng-zopfli-v1` (PNG, bundled, lossless)
3. `optipng-v1` (PNG, bundled, lossless)
4. `pngquant-v1` (PNG, external, numeric quality)
5. `jpegtran-v1` (JPEG, bundled, lossless)
6. `mozjpeg-v1` (JPEG, bundled, numeric quality)
7. `jpegli-v1` (JPEG, bundled, numeric quality)
8. `libwebp-v1` (WebP, bundled with `cwebp` override, lossless or numeric quality)
9. `image-webp-v1` (WebP, bundled, lossless)
10. `avif-aom-v1` (AVIF, bundled, numeric quality)
11. `avif-rav1e-v1` (AVIF, bundled, numeric quality)

Every per-input report retains the rows for that input's format. Strategies for
other formats are omitted. A strategy that does not support the selected
quality is `not applicable`; a missing compatible executable is `unavailable`.
Runnable strategies are attempted once. A strictly smaller accepted candidate
replaces the winner, and registry order breaks equal sizes.

`--disable-strategy ID` disables a row. `--require-strategy ID` requires the
strategy to be runnable at the selected quality. `--provider NAME PATH` selects
an executable override and implicitly requires its strategy. Duplicate,
unknown, or disabled-and-required controls are usage errors.

## Quality mapping

`--quality lossless|1..100` defaults to `lossless`. Numeric quality permits
fidelity reduction but still runs applicable lossless strategies.

- Both OxiPNG strategies use pinned lossless recompression settings.
- OptiPNG uses lossless optimization level 2.
- pngquant receives `--quality 0-Q`; it may reduce the image to a palette even
  at 100 and explicitly strips optional metadata.
- jpegtran copies all extra markers, optimizes Huffman tables, and writes a
  progressive JPEG without decoding and requantizing coefficients.
- MozJPEG receives native `-quality Q`, progressive encoding, optimized Huffman
  coding, and strict input handling.
- Bundled Jpegli receives native quality `Q` and progressive scans; the
  external override additionally pins progressive level 2.
- libwebp uses lossless preset 9 at lossless quality or native lossy `Q`, with
  method 6, exact transparent RGB, and lossless alpha.
- image-webp always contributes its native lossless encoding.
- libavif/libaom receives native quality `Q`, alpha quality 100, and speed 6.
- ravif/rav1e receives native quality `Q`, alpha quality 100, speed 6, and
  8-bit still-image output.

The common number is a provider-native control, not an ImgLean-calculated
quality score. The controller does not compare pixels or measure perceptual
quality. Baseline participation guarantees only that a successful selected file
is no larger than the source.

## Metadata mapping

`--strip-metadata` allows metadata removal and requests it through a provider's
native control when available. It never excludes an otherwise-applicable
strategy. The mapping is part of each versioned adapter:

- OxiPNG selects `StripChunks::Safe` instead of `StripChunks::None`.
- OptiPNG adds `-strip all`.
- pngquant already always receives `--strip`, so the flag does not change its
  command.
- jpegtran changes `-copy all` to `-copy none`.
- Bundled MozJPEG and Jpegli preserve opaque JPEG application/comment markers by
  default, carry JFIF density forward, and regenerate structural JFIF/Adobe
  markers as needed to match the new encoding without duplicates or blind
  replay. They omit saved markers when the flag is set. External overrides use
  the behavior of their documented CLI mappings.
- Both WebP strategies preserve ICC and Exif by default and omit them when
  stripping is requested.
- The selected libavif/libaom and ravif APIs expose no metadata-removal control;
  each emits its normal container under either policy.

This is deliberately provider-native and best effort. The controller does not
strip metadata itself, compare ancillary payloads, or verify that a provider
removed every metadata form it may understand. The unchanged baseline remains
eligible and can win, so successful execution with this flag does not guarantee
a metadata-free output.

## Integration and execution boundary

Integration form is evaluated in the order safely embeddable, linkable, then
callable. OxiPNG and image-webp use safe Rust APIs. OptiPNG, MozJPEG, jpegtran,
Jpegli, libwebp, and libavif/libaom are linked behind the internal codec
boundary; ravif/rav1e is linked Rust code. Native code is
still called only in the short-lived provider worker. pngquant and explicit
provider overrides use the external boundary. No ImageOptim-specific discovery
or execution path exists.

Every strategy receives a fresh private input containing the validated source
capture and an absent private candidate path. It never receives the source path
or requested destination. The controller verifies the private input remained
unchanged, bounds and reads the candidate, validates it independently, and
exclusively owns winner selection and publication.

Every bundled provider runs in a short-lived private role of the current
executable. External providers are supervised directly. Process separation isolates
ordinary crashes and hangs; it is not a security sandbox or portable hard
memory limit.

`--timeout SECONDS` sets each strategy worker's controller deadline from 6
through 600 seconds and defaults to 60. Each worker receives the smaller of that
setting and the invocation time remaining when it starts. OxiPNG's internal
timeout is five seconds shorter than its effective controller deadline, with a
one-second floor for workers starting near the invocation deadline. Provider
discovery and image validation retain separate fixed limits.

## Capability-based discovery

Preflight uses a configured executable as an override when present. Without an
override, bundled strategies need no discovery and pngquant searches for
`pngquant` (or `pngquant.exe`) on `PATH`. An external path is canonicalized and
probed once under the discovery deadline.

The libwebp override probes `cwebp -longhelp` for lossless, exact, metadata,
method, and alpha-quality controls. Each probe otherwise requires bounded execution plus provider-specific CLI identity and
the options the adapter depends on. jpegtran and MozJPEG's historical help
paths exit with status 1 after printing valid help, which each adapter accepts
only when every required marker is present. A release-number string is neither
requested nor used as a compatibility gate. This is important for Jpegli,
which does not provide a stable version command, and keeps the same rule for
all providers. CI pins representative upstream revisions so the adapter itself
remains reproducibly tested; those revisions do not restrict compatible
installations.

Automatic absence or capability mismatch marks an optional strategy
`unavailable`. The same condition fails structural preflight when explicitly
required or configured. A strategy required at an inapplicable quality also
fails preflight. ImgLean never downloads, installs, updates, or repairs provider
software.

## Supervision and results

Provider output streams are drained concurrently and bounded independently.
The controller kills and reaps an overdue process, cleans only current-run
private artifacts, and restores results to registry order before validation and
selection.

Start failure, nonzero exit, timeout, excessive diagnostics, unreadable or
oversized output, or candidate rejection warns and excludes that candidate.
The baseline and other strategies continue. Successful execution without a
candidate is a normal no-improvement result; pngquant status 99 is also a normal
no-candidate result. Private-input mutation or cleanup failure fails the input.

Native CI executes every bundled strategy directly and through the controller
on each release target. Separate jobs build pinned representative OptiPNG,
pngquant, MozJPEG, libjpeg-turbo jpegtran, Jpegli, and libwebp `cwebp` sources
and require a real reduction through discovery and the full controller path.
Unit tests cover
absence, bad identity, required failure, process failure, timeout, malformed
output, larger output, quality and metadata mappings, and baseline fallback.
Real-provider tests use metadata-bearing inputs and exercise both jpegtran
marker preservation by default and provider-native stripping when requested.

Exact bounds are in [LIMITS.md](LIMITS.md). The registry is an explicit product
surface, not a general executable plugin mechanism.
