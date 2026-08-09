# ImgLean Agent Guidelines

## Mission

Build the smallest reliable CLI that runs applicable image-optimization strategies and, for each input, keeps the smallest candidate accepted under that input's active policy. Treat [SCOPE.md](SCOPE.md) as the product boundary and [ARCHITECTURE.md](ARCHITECTURE.md) as the structural design.

## Engineering defaults

- Use mise to install and select the latest stable Rust release; do not rely on an ambient Rust toolchain. Prefer standard-library facilities where practical.
- Resolve the stable Rust channel to one exact toolchain before a release build and record it with the dependency lock, target and deployment settings, bundled-provider revisions, native compiler and linker, SDK, build tools, features, and build flags.
- Choose the simplest implementation that satisfies the current milestone.
- Do not add a dependency, format, provider, abstraction, or compatibility path for later scope.
- Keep public APIs near the top of a module and private helpers below them.
- Prefer safe Rust. Any `unsafe` or foreign-function boundary requires a documented safety contract and focused tests.
- Avoid global mutable state and keep candidate validation independent from providers.

## Correctness and safety

- Treat source files and provider output as untrusted.
- Bound source and candidate parsing, decoding, allocation, and validation work, plus invocation-wide input count, captured bytes, concurrency, temporary storage, diagnostics, and elapsed time. Choose and version exact limits from implementation evidence rather than copying guesses into product documentation.
- Abort the entire invocation before mutation when structural input or destination preflight fails; only later per-input failures may allow the batch to continue.
- Validate the source before running strategies and independently validate every candidate before considering its size.
- Open each canonical input during structural preflight. Compare portable source state before and after a bounded controller-owned read through that open file, reject detected ordinary changes, and give the optimizing strategy its own private input derived from those captured bytes. Do not re-resolve the input pathname for capture or overstate capture as an adversarially coherent snapshot.
- Treat outputs as results of the validated source capture; later source changes do not invalidate them.
- Never let a provider overwrite the source directly.
- Apply the version 0.6 basic candidate gates and provider-trust boundary defined in `SCOPE.md`; do not silently claim independently proven pixel or ancillary-payload equivalence.
- Treat accepted ancillary payloads as opaque. Refuse `caBX`, the standard XMP keyword in PNG text chunks, and the documented conservative adjacent `.c2pa` heuristics. Do not add ICC, Exif, XML, or language-tag parsers or retrieve remote manifests.
- Retain the validated final component of each original input argument for destination naming and collision detection; source canonicalization must not change it. Reject inputs observed as final-component symlinks, then resolve ancestor symlinks and output paths to absolute canonical paths during preflight without claiming a race-free symlink check. Reject repeated canonical input paths before optimization. Require printable-ASCII basenames ending in `.png` without ASCII case sensitivity, and fold ASCII case when detecting destination collisions on every platform. Distinct hard links to one source may be processed as distinct explicit inputs.
- Require an explicit output directory and reject every destination that aliases an input. Version 0.2 never writes or replaces sources, intentionally changes their metadata, or mixes output modes; filesystem-managed access-time updates caused by reads are outside that guarantee.
- Use the canonical output-directory path for later operations without claiming protection against concurrent path-component replacement. Require destination absence at preflight and publish only by creating a non-replacing hard link to a complete validated temporary file in that directory.
- Prepare and verify the complete result before atomic output creation.
- Leave the source untouched and create no destination after source-validation or commit failure. Never expose a partial output around the commit point.
- Give temporary artifacts collision-resistant names and create them atomically without claiming confidentiality from their names or permissions. Clean artifacts owned by the current invocation after handled completion or failure; do not delete artifacts from an earlier invocation.
- Give outputs the metadata produced by ordinary new-file creation and hard linking on the current platform and filesystem without copying or normalizing input filesystem metadata. Do not intentionally make an output executable or read-only.
- Define exact format, filesystem, and release-target contracts before implementing their dependent behavior and encode them in focused tests; do not expand `SCOPE.md` into an implementation specification.

## Providers and licensing

- Use the vocabulary from `SCOPE.md`: provider, strategy, candidate, and winner.
- Version 0.6 bundles two audited OxiPNG strategies, OptiPNG, jpegtran, MozJPEG, and Jpegli. pngquant remains an explicitly supported optional external adapter because its license is incompatible with the default Apache-2.0 binary.
- Enable every bundled strategy compatible with the active policy by default. Discover pngquant during preflight and enable it when available; never download or install providers. An explicit `--provider` path overrides the corresponding bundled implementation for testing or controlled substitution.
- Resolve an external provider once per invocation, validate its required CLI capabilities without gating a reported version, and distinguish normal automatic absence from failure of a provider the user explicitly requires.
- Ensure no component of the built-in workflow initiates network requests or remote-service calls.
- Run providers in separate worker processes for crash isolation and portable byte and elapsed-time control, not as a security or hard memory-containment boundary.
- Use the validated controller-owned source capture as the baseline candidate; do not spawn a worker or create a separate artifact for it before winner selection.
- Warn and continue deterministic selection among the baseline and remaining accepted candidates when an optimizing strategy fails or produces a rejected candidate.
- Document and test which provider limits are hard-enforced, monitored, or configured.
- Complete the provider-execution contract before enabling worker-run provider code.
- Configure behavior-affecting provider options explicitly and record capability contracts plus reproducible CI provider revisions in release artifacts.
- Pin every behavior-affecting OxiPNG option explicitly; do not inherit defaults for interlacing, representation reduction, alpha handling, metadata, error repair, limits, or output behavior.
- ImgLean is licensed under Apache-2.0.
- Audit every direct, transitive, vendored, and native dependency before distribution.
- Do not link GPL, AGPL, proprietary, source-incompatible, or redistribution-restricted code into the default binary.
- Keep any future commercially licensed integration in a separate distribution or behind an external-provider boundary.
- Preserve required notices and generate a dependency inventory and software bill of materials for releases.

## Validation

Run project tooling through the tasks in `mise.toml`. Before a releasable change, run the canonical gate:

```sh
mise run check
```

Use `mise run format` when formatting needs correction. Do not bypass `--locked`, the all-target/all-feature Clippy coverage, warnings-as-errors, or the complete test task with ad hoc Cargo commands.

Provider and validator changes require checked-in, versioned, bounded format corpora covering each accepted subset plus malformed, oversized, metadata-bearing, C2PA/XMP-bearing, unchanged, and semantically changed images. Every provider family needs at least one validated size reduction. Input and output changes require tests for batch-preflight atomicity, repeated canonical inputs, source/destination aliases, ASCII name collisions, C2PA path transformations, destination appearance races, capture-time semantics, interruption on both sides of publication, hard-link publication and unsupported filesystems, platform metadata behavior, larger candidates, optimizing-provider failures, baseline selection, current-run temporary-path cleanup, refusal to delete earlier artifacts, diagnostics escaping and routing, and filesystem failures.

Every registered bundled strategy must execute directly and through the
controller in CI. Every supported external adapter or override must run against a pinned
representative provider revision on every target where support is claimed, with
additional coverage for absence, capability mismatch, failure, timeout,
malformed output, and larger output. `--all-features` compilation alone is not
provider integration coverage.

## Documentation

- Keep `README.md` focused on users and clearly distinguish implemented behavior from planned behavior.
- Keep `SCOPE.md` high level: product boundary, milestone, invariants, non-goals, and outcomes.
- Keep `ARCHITECTURE.md` focused on stable component boundaries and data flow.
- Put detailed format, provider, filesystem, and protocol contracts beside their implementations when that work begins.
- Clearly label planned behavior that is not implemented.
- Do not claim that one executable runs across operating systems; each release artifact is target-specific even though the source and filesystem workflow are portable.
