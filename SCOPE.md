# ImgLean Scope

> [!IMPORTANT]
> Version 0.6 is implemented in source. Target-specific release artifacts remain
> unpublished and unqualified until their native gates pass.

## Product definition

ImgLean is a focused offline primitive for people, scripts, CI, and coding
agents that need trustworthy local PNG, JPEG, WebP, and AVIF size reduction
without assembling an optimizer toolchain. For each input, it runs every enabled
applicable strategy and selects the smallest candidate accepted under one common
policy. The validated original participates as the first candidate, so every
successful output is no larger than its source.

The bundled workflow is offline and ships in one target-specific executable.
Supported external executables may augment it when already installed; ImgLean
does not download or manage them. The default distribution is permissively
licensed. This document is the authoritative product boundary.

## Vocabulary

- **Acceptance policy:** output requirements applied to every candidate for one input
- **Baseline:** the validated source capture offered unchanged
- **Provider:** an optimizer implementation, such as OxiPNG
- **Strategy:** one provider with an explicit, pinned option set
- **Candidate:** the baseline or a strategy result
- **Winner:** the first smallest accepted candidate in registry order

## Enduring invariants

- Bound and independently validate every source and provider candidate.
- Attempt every enabled applicable strategy once for each processed input.
- Include the source baseline and use stable order to break equal-size ties.
- Never let a provider select, publish, or overwrite a destination.
- Publish only a complete validated result; replace only the requested regular
  output entry and never an input alias.
- Report every input outcome and the invocation outcome.
- Enable compatible bundled strategies by default.
- Keep strategy identifiers and order stable; keep provider capabilities,
  options, and limits explicit, pinned, tested, and recorded in release artifacts.
- Keep the bundled workflow offline and permissively redistributable.
- Require deliberate numeric quality selection before enabling a
  fidelity-reducing strategy.
- Admit a new strategy only when its production-ready independent
  implementation supports same-format output, fits the licensing and release
  boundary, has a reproducible integration on qualified targets, and
  demonstrates useful reductions or distinct wins against the existing set.

## Version 0.6

Version 0.6 supports explicit static PNG, JPEG, WebP, and AVIF inputs in either
separate-output or publication-free check mode on 64-bit macOS, Linux, and Windows
release targets. It accepts every
standard static PNG color-type and bit-depth combination, including Adam7,
within documented byte, dimension, pixel, allocation, chunk, and elapsed-time
limits. APNG, C2PA `caBX`, standard XMP text, and adjacent `.c2pa` sidecars are
refused.

JPEG input is limited to bounded 8-bit baseline, extended-sequential, and
progressive Huffman coding with one frame. Standard XMP APP1, APP11, and adjacent
`.c2pa` sidecars are refused. Other accepted application/comment data is opaque.

WebP input is limited to bounded static lossy or lossless images with optional
alpha, ICC, and Exif. Animation chunks, standard XMP, embedded `C2PA`, and
adjacent `.c2pa` sidecars are refused. AVIF input is limited to bounded static
`avif`-branded images that libavif can fully decode. Image sequences, the C2PA
BMFF UUID, the standard XMP MIME type, and adjacent `.c2pa` sidecars are refused.

The controller performs a format-specific basic candidate gate: bounded
container inspection and complete decode, matching dimensions, and the explicit
C2PA/XMP refusal. PNG additionally checks chunk CRCs and static animation class.
Other ancillary data is opaque. ImgLean does
not compare decoded samples, calculate perceptual quality, or compare ancillary
payload identity; fidelity and metadata behavior are properties of the audited
strategy configuration.

The ordered, format-specific registry is:

1. OxiPNG 10.2.0 with libdeflater level 11, bundled;
2. OxiPNG 10.2.0 with pinned Zopfli settings, bundled;
3. OptiPNG at optimization level 2, bundled;
4. pngquant at numeric quality, external and optional;
5. jpegtran with configurable native marker copying, Huffman optimization, and
   progressive output, bundled;
6. MozJPEG at numeric quality, bundled;
7. Jpegli at numeric quality, bundled;
8. libwebp at lossless or numeric quality, bundled with a `cwebp` override;
9. image-webp lossless encoding, bundled;
10. libavif/libaom at numeric quality, bundled; and
11. ravif/rav1e at numeric quality, bundled.

`--quality lossless|1..100` defaults to `lossless`. jpegtran and the PNG
lossless strategies remain applicable at numeric quality. `pngquant`,
`mozjpeg`, `jpegli`, `avif-aom`, and `avif-rav1e` are not
applicable at lossless quality. Numeric `Q` maps to each provider's native
quality control; pngquant receives `0-Q`, while MozJPEG and Jpegli receive `Q`.
ImgLean does not emulate lossy transformations or independently score quality.
libwebp maps lossless to its native lossless preset and numeric `Q` to its
native lossy quality. image-webp always contributes a lossless candidate.

`--strip-metadata` allows metadata removal and requests native removal behavior
where a strategy exposes it. It does not exclude an otherwise-applicable
strategy that lacks such a control. ImgLean does not transform the source
baseline, implement a controller-side stripper, or independently verify that
metadata was removed. Because the baseline remains eligible and can win, the
option is explicitly best effort and does not guarantee a metadata-free output.

All bundled strategies are enabled by default when applicable. The unbundled
pngquant provider is discovered on `PATH`. `--provider NAME PATH` can instead
select an external executable for any provider-backed strategy, including as an
override of a bundled implementation. The executable must pass its adapter's
bounded CLI capability probe; reported release numbers are neither required nor
gated. Automatic absence or capability mismatch is normal;
an explicitly required or configured provider that is unavailable,
incompatible, or not applicable fails preflight before output creation.
Provider execution failure warns, excludes that candidate, and leaves the
baseline and other strategies in the race. Every registered strategy for the
input's format remains visible in per-input reporting as a result, disabled,
unavailable, not applicable at the selected quality, or not-run row.

Complete input/output mapping preflight precedes all publication. Inputs are
then processed independently and sequentially. Enabled strategies for one input
run through a bounded worker pool; completion order cannot change registry-order
tie-breaking or reporting. The per-strategy timeout defaults to 60 seconds and
is configurable within the documented bounded range without extending the
invocation deadline. A later per-input failure does not roll back earlier
outputs. Publication renames a complete validated temporary file within the
output directory and replaces an existing regular destination when present.

`--check` requires no output directory. It applies the same source validation,
provider execution, candidate validation, and winner selection using an
invocation-owned temporary directory, but publishes nothing. It exits `4` when
at least one input has a smaller accepted candidate and `0` when none do;
processing failure, invalid usage, and warning-only completion retain statuses
`1`, `2`, and `3` respectively.

## Non-goals and future directions

Version 0.6 does not provide directory traversal, standard-input pipelines,
in-place operation, format conversion, APNG processing, a general plugin ABI,
provider downloads, remote services, transactional rollback, backup creation,
or independent perceptual quality measurement. Additional formats, providers,
architectures, and explicit fidelity policies require their own reviewed
contracts and evidence that they materially improve the applicable candidate
set. ImgLean does not promise the globally smallest representation across
formats; same-format optimization is an intentional product boundary.

## Document ownership

[ARCHITECTURE.md](ARCHITECTURE.md) owns stable component boundaries. Detailed
[input](docs/contracts/INPUT_AND_BATCH.md), [PNG](docs/contracts/PNG.md),
[JPEG](docs/contracts/JPEG.md), [WebP](docs/contracts/WEBP.md),
[AVIF](docs/contracts/AVIF.md),
[provider](docs/contracts/PROVIDER_EXECUTION.md),
[output](docs/contracts/OUTPUT.md), and [limits](docs/contracts/LIMITS.md)
contracts own implemented behavior. [docs/RELEASE.md](docs/RELEASE.md) owns
target qualification and artifact contents.

## Licensing boundary

ImgLean is Apache-2.0. The default binary may include only dependencies approved
for permissive redistribution. GPL, AGPL, proprietary, source-incompatible, and
redistribution-restricted providers are not linked into it. External providers
are separately installed software and are excluded from the bundled dependency
inventory and SBOM.
