# ImgLean Scope

> [!IMPORTANT]
> Version 0.4 is implemented in source. Target-specific release artifacts remain
> unpublished and unqualified until their native gates pass.

## Product definition

ImgLean is for command-line users who want trustworthy local PNG size reduction
without assembling an optimizer toolchain. For each input, it runs every enabled
applicable strategy and writes the smallest candidate accepted under one common
policy. The validated original participates as the first candidate, so every
successful output is no larger than its source.

The embedded workflow is offline and ships in one target-specific executable.
Supported external executables may augment it when already installed; ImgLean
does not download or manage them. The default distribution is permissively
licensed. This document is the authoritative product boundary.

## Vocabulary

- **Acceptance policy:** output requirements applied to every candidate for one input
- **Baseline:** the validated source capture offered unchanged
- **Provider:** an optimizer implementation, such as OxiPNG
- **Strategy:** one provider with a versioned explicit option set
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
- Enable compatible embedded strategies by default.
- Keep strategy identifiers, order, provider versions, options, and limits
  explicit, versioned, tested, and recorded in release artifacts.
- Keep the embedded workflow offline and permissively redistributable.
- Require deliberate numeric quality selection before enabling a
  fidelity-reducing strategy.

## Version 0.4

Version 0.4 supports explicit static PNG inputs and a required separate output
directory on 64-bit macOS, Linux, and Windows release targets. It accepts every
standard static PNG color-type and bit-depth combination, including Adam7,
within documented byte, dimension, pixel, allocation, chunk, and elapsed-time
limits. APNG, C2PA `caBX`, standard XMP text, and adjacent `.c2pa` sidecars are
refused.

The controller performs a basic candidate gate: signature and chunk checksums,
bounded complete decode, permitted static animation class, matching dimensions,
and the explicit C2PA/XMP refusal. Other ancillary data is opaque. ImgLean does
not compare decoded samples, calculate perceptual quality, or compare ancillary
payload identity; fidelity and metadata behavior are properties of the audited
strategy configuration.

The ordered registry is:

1. OxiPNG 10.1.1 with libdeflater level 11, embedded;
2. OxiPNG 10.1.1 with pinned Zopfli settings, embedded;
3. OptiPNG 7.9.1 at optimization level 2, external and optional; and
4. pngquant 3.0.2 or 3.0.3 at numeric quality, external and optional.

`--quality lossless|1..100` defaults to `lossless`. Lossless strategies remain
applicable at numeric quality. `pngquant-v1` is not applicable at lossless
quality and maps numeric `Q` to pngquant's native `--quality 0-Q` setting.
ImgLean does not emulate lossy transformations or independently score quality.

Both embedded strategies are enabled by default. External providers are
discovered on `PATH` or supplied with `--provider NAME PATH` on every platform.
An exact supported version is required. Automatic absence or incompatibility is normal;
an explicitly required or configured provider that is unavailable,
incompatible, or not applicable fails preflight before output creation.
Provider execution failure warns, excludes that candidate, and leaves the
baseline and other strategies in the race. Every registered strategy remains
visible in per-input reporting as a result, disabled, unavailable, not
applicable, or not-run row.

Complete input/output mapping preflight precedes all publication. Inputs are
then processed independently and sequentially. Enabled strategies for one input
run through a bounded worker pool; completion order cannot change registry-order
tie-breaking or reporting. A later per-input failure does not roll back earlier
outputs. Publication renames a complete validated temporary file within the
output directory and replaces an existing regular destination when present.

## Non-goals and future directions

Version 0.4 does not provide directory traversal, standard-input pipelines,
in-place operation, format conversion, APNG processing, a general plugin ABI,
provider downloads, remote services, transactional rollback, backup creation,
or independent perceptual quality measurement. Additional formats, providers,
architectures, and explicit fidelity policies require their own reviewed
contracts.

## Document ownership

[ARCHITECTURE.md](ARCHITECTURE.md) owns stable component boundaries. Detailed
[input](docs/contracts/INPUT_AND_BATCH.md), [PNG](docs/contracts/PNG.md),
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
