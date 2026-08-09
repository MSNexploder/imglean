# Version 0.6 Provider Execution Contract

## Registry and controls

The source baseline precedes this stable order:

1. `oxipng-libdeflate-v1` (PNG, embedded, lossless)
2. `oxipng-zopfli-v1` (PNG, embedded, lossless)
3. `optipng-v1` (PNG, external, lossless)
4. `pngquant-v1` (PNG, external, numeric quality)
5. `jpegtran-v1` (JPEG, external, lossless)
6. `mozjpeg-v1` (JPEG, external, numeric quality)
7. `jpegli-v1` (JPEG, external, numeric quality)

Every per-input report retains the rows for that input's format. Strategies for
other formats are omitted. A strategy that does not support the selected
quality is `not applicable`; a missing compatible executable is `unavailable`.
Runnable strategies are attempted once. A strictly smaller accepted candidate
replaces the winner, and registry order breaks equal sizes.

`--disable-strategy ID` disables a row. `--require-strategy ID` requires its
provider to resolve. `--provider NAME PATH` selects an executable and implicitly
requires its strategy. Duplicate, unknown, or disabled-and-required controls are
usage errors.

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
- Jpegli receives native `--quality Q` and progressive level 2.

The common number is a provider-native control, not an ImgLean-calculated
quality score. The controller does not compare pixels or measure perceptual
quality. Baseline participation guarantees only that a successful selected file
is no larger than the source.

## Integration and execution boundary

Integration form is evaluated in the order embedded, linked, then callable.
OxiPNG is embedded through a maintained safe Rust API. jpegtran, MozJPEG, and
Jpegli provide linkable native APIs, but linking them would introduce unsafe
FFI, native build dependencies, and in-process crash behavior. Their maintained
CLI front ends meet the current contract with the existing worker isolation, so
version 0.6 uses them as callable providers. OptiPNG and pngquant use the same
external boundary. No ImageOptim-specific discovery or execution path exists.

Every strategy receives a fresh private input containing the validated source
capture and an absent private candidate path. It never receives the source path
or requested destination. The controller verifies the private input remained
unchanged, bounds and reads the candidate, validates it independently, and
exclusively owns winner selection and publication.

Embedded OxiPNG runs in a short-lived private role of the current executable.
External providers are supervised worker processes. Process separation isolates
ordinary crashes and hangs; it is not a security sandbox or portable hard
memory limit.

`--timeout SECONDS` sets each strategy worker's controller deadline from 6
through 600 seconds and defaults to 60. Each worker receives the smaller of that
setting and the invocation time remaining when it starts. OxiPNG's internal
timeout is five seconds shorter than its effective controller deadline, with a
one-second floor for workers starting near the invocation deadline. Provider
discovery and image validation retain separate fixed limits.

## Capability-based discovery

Preflight uses a configured executable when present, otherwise the first named
executable on `PATH`: `optipng`, `pngquant`, `jpegtran`, `cjpeg` for MozJPEG,
or `cjpegli` (with `.exe` on Windows). The path is canonicalized and probed once
under the discovery deadline.

Each probe requires bounded execution plus provider-specific CLI identity and
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

Native CI executes both embedded strategies directly and through the controller
on each release target. Separate jobs build pinned representative OptiPNG,
pngquant, MozJPEG, libjpeg-turbo jpegtran, and Jpegli sources and require a real
reduction through discovery and the full controller path. Unit tests cover
absence, bad identity, required failure, process failure, timeout, malformed
output, larger output, quality mapping, and baseline fallback.

Exact bounds are in [LIMITS.md](LIMITS.md). The registry is an explicit product
surface, not a general executable plugin mechanism.
