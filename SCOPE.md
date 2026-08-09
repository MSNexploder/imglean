# ImgLean Scope

> [!IMPORTANT]
> Version 0.2 is implemented in source. Target-specific release artifacts remain
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
licensed. ImageOptim motivates the low-friction experience, but this document is
the authoritative boundary.

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
- Publish only a complete validated result and never replace an existing entry.
- Report every input outcome and the invocation outcome.
- Enable compatible embedded strategies by default.
- Keep strategy identifiers, order, provider versions, options, and limits
  explicit, versioned, tested, and recorded in release artifacts.
- Keep the embedded workflow offline and permissively redistributable.
- Require deliberate selection before enabling any future fidelity-reducing
  policy.

## Version 0.2

Version 0.2 supports explicit static PNG inputs and a required separate output
directory on 64-bit macOS, Linux, and Windows release targets. It accepts every
standard static PNG color-type and bit-depth combination, including Adam7,
within documented byte, dimension, pixel, allocation, chunk, and elapsed-time
limits. APNG, C2PA `caBX`, standard XMP text, and adjacent `.c2pa` sidecars are
refused.

The controller performs a basic candidate gate: signature and chunk checksums,
bounded complete decode, permitted static animation class, matching dimensions,
and the explicit C2PA/XMP refusal. Other ancillary data is opaque. ImgLean does
not compare decoded samples or ancillary payload identity; losslessness and
metadata preservation are properties of the audited strategy configuration.

The ordered registry is:

1. OxiPNG 10.1.1 with libdeflater level 11, embedded;
2. OxiPNG 10.1.1 with pinned Zopfli settings, embedded; and
3. OptiPNG 7.9.1 at optimization level 2, external and optional.

Both embedded strategies are enabled by default. OptiPNG is discovered on
`PATH`, or supplied with `--provider optipng PATH`, and is enabled when its exact
supported version is available. Automatic absence or incompatibility is normal;
an explicitly required or configured provider that is unavailable or
incompatible fails preflight before output creation. Provider execution failure
warns, excludes that candidate, and leaves the baseline and other strategies in
the race.

Complete input/output mapping preflight precedes all publication. Inputs are
then processed independently and sequentially. A later per-input failure does
not roll back earlier outputs. Publication uses a non-replacing hard link to a
complete validated temporary file in the output directory.

## Non-goals and future directions

Version 0.2 does not provide directory traversal, standard-input pipelines,
in-place operation, lossy optimization, format conversion, APNG processing, a
general plugin ABI, provider downloads, remote services, or support for output
filesystems without same-directory hard links. Additional formats, providers,
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
