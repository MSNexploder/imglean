# ImgLean Scope

> **Status:** ImgLean 0.6 is the complete first version in source.
> Target-specific release artifacts remain unpublished until their native
> qualification gates pass.

This document is the authoritative product boundary: what ImgLean is, what it
promises, and what it intentionally does not do.

## Product

ImgLean is a command-line tool for reducing existing PNG, JPEG, WebP, and AVIF
files without changing their format or dimensions. Its bundled workflow is
offline.

For each input, ImgLean runs every enabled strategy that applies to the selected
quality policy. It independently validates the results and chooses the first
smallest accepted candidate. The validated source is always the baseline
candidate, so successful output cannot be larger than the input.

The goal is one reliable optimization command for people, scripts, CI, and
coding agents—not a general image-processing suite.

## Why it exists

Image optimization is fragmented across codecs, executables, APIs, flags, and
licensing models. ImgLean provides one stable policy above those differences:

- safe lossless defaults for unattended use;
- explicit quality selection before lossy work;
- predictable strategy diagnostics and exit statuses;
- independent basic validation of provider output;
- deterministic winner selection; and
- a bundled, offline workflow with optional controlled provider overrides.

ImgLean's purpose is to prevent avoidable encoding overhead without requiring
every caller to assemble and understand an optimizer toolchain.

## Product promises

- Treat input and provider output as untrusted and enforce every documented
  controller-owned limit.
- Validate the source before optimization and every candidate before selection.
- Attempt each enabled applicable strategy once per processed input.
- Always include the source baseline and use stable order for equal-size ties.
- Never let a provider choose or publish a destination.
- Never replace an input or expose a partial output.
- Publish only a complete validated winner into the requested output directory.
- Keep lossless as the default and require numeric quality for fidelity-reducing
  strategies.
- Keep ImgLean-controlled work offline and the bundled distribution
  permissively redistributable.
- Report each input outcome and the overall invocation outcome.

Candidate validation proves the documented container, decoding, dimension, and
metadata safety gates. ImgLean trusts audited strategy configuration for
lossless or lossy transformation behavior; it does not independently compare
decoded pixels, ancillary payload identity, or perceptual quality.

## Version 0.6

Version 0.6 provides:

- explicit static PNG, JPEG, WebP, and AVIF inputs;
- separate-output optimization and publication-free `--check` mode;
- lossless defaults plus `--quality 1..100`;
- best-effort provider-native metadata stripping;
- bounded parallel strategies with stable selection and reporting;
- replacement of an existing regular destination after complete validation;
- bundled PNG, JPEG, WebP, and AVIF strategies;
- optional external pngquant discovery; and
- explicit executable overrides for supported providers.

Release qualification covers 64-bit Apple Silicon and Intel macOS, x86-64
Linux, x86-64 Windows, and a minimal linux/amd64 container. Each artifact is
qualified separately; one executable is not claimed to run across platforms.

All compatible bundled strategies are enabled by default. Lossy strategies
participate only after numeric quality is selected. External providers are
capability-checked, never downloaded, and either add the optional pngquant
strategy or replace a corresponding bundled implementation for controlled
testing.

The exact accepted subsets, limits, option mappings, filesystem behavior,
provider failure handling, and exit statuses belong to the
[focused contracts](docs/contracts/), not this high-level scope.

## Vocabulary

- **Provider:** an optimizer implementation, such as OxiPNG
- **Strategy:** one provider with an explicit option set
- **Candidate:** the validated source or one strategy result
- **Baseline:** the validated source candidate
- **Winner:** the first smallest accepted candidate in stable registry order
- **Acceptance policy:** the requirements every candidate must satisfy

## Non-goals

ImgLean does not provide:

- resizing, cropping, rotation, or format conversion;
- directory traversal, standard-input pipelines, or in-place operation;
- animated-image optimization;
- provider downloads or remote optimization services;
- a general plugin API;
- transactional rollback or backup management;
- independent perceptual quality scoring; or
- a guarantee of the globally smallest representation across formats.

New formats or providers must demonstrate a useful addition to the candidate
set, fit the licensing and offline boundary, and have reproducible integration
coverage on every claimed target.

## Document ownership

- [README.md](README.md) owns user-facing introduction and common usage.
- [ARCHITECTURE.md](ARCHITECTURE.md) owns stable components and data flow.
- [docs/contracts](docs/contracts/) owns exact implemented behavior.
- [docs/providers](docs/providers/) owns provider-specific configuration.
- [docs/RELEASE.md](docs/RELEASE.md) owns qualification and artifact contents.

## Licensing

ImgLean is Apache-2.0. The default binary may include only dependencies approved
for permissive redistribution. Providers with incompatible licenses remain
separately installed external software and are excluded from the bundled
dependency inventory and SBOM.
