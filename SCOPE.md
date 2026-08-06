# ImgLean Scope

> [!IMPORTANT]
> ImgLean is currently project scaffolding. This document defines planned product behavior; it does not describe an implemented release.

## Product definition

ImgLean is for command-line users who want trustworthy local image-size reduction without installing separate optimizer tools. Its built-in workflow attempts to reduce the encoded byte length of supported images without violating the active acceptance policy or trusting optimizer claims, and no component of that workflow initiates network requests or remote-service calls. For each input, the validated original participates as the baseline, and ImgLean writes the candidate with the smallest encoded byte length among the baseline and results actually produced by enabled strategies applicable to that input. Because the baseline competes, every output ImgLean commits has an encoded byte length no larger than the validated source.

Candidates may compete for one input only when evaluated under the same declared acceptance policy.

Given an accepted candidate set, ImgLean selects the winner deterministically. Each release artifact is a target-specific executable requiring no separately installed optimizer or runtime. The default distribution is permissively licensed. ImageOptim motivates the desired low-friction experience, but the independently stated behavior in this document is authoritative.

## Vocabulary

- **Acceptance policy:** the equivalence or quality requirements applied to every candidate competing for one input
- **Baseline:** the validated source capture offered as the unchanged candidate
- **Provider:** an optimizer implementation, such as OxiPNG
- **Strategy:** one provider with an explicit option set
- **Candidate:** the baseline or a result produced by a strategy
- **Winner:** the accepted candidate with the smallest encoded byte length in one input's candidate set

## Enduring product invariants

- Bound and validate every source, and independently validate every provider-produced candidate before comparing sizes.
- For each processed input, attempt every enabled applicable strategy within declared resource limits.
- Evaluate every candidate for one input under the same active acceptance policy.
- Include the validated source as the baseline so a failed optimizer or invalid candidate cannot eliminate an otherwise valid result.
- Given an accepted candidate set, select the candidate with the smallest encoded byte length deterministically.
- Require deliberate user selection of any acceptance policy that may reduce image fidelity.
- Publish only complete validated results through an explicit output mode; never replace an existing destination.
- Clearly report each input's outcome and the overall invocation outcome.
- Keep built-in strategy definitions explicit, versioned, and auditable.
- Keep the complete built-in workflow offline: no component initiates network requests or remote-service calls.
- Keep each default workflow release in one target-specific executable requiring no separately installed optimizer or runtime, with a permissively licensed distribution.

These are product outcomes. Architecture, format, input, output, provider, reporting, filesystem, testing, and release mechanisms belong in their owning documents and implementations.

## Planned version 0.1

Version 0.1 is planned as a supported, releasable CLI for a deliberately narrow slice of the core optimization race:

- 64-bit macOS, Linux, and Windows release targets using common filesystem operations;
- a limited, non-animated PNG subset under strict lossless equivalence;
- the validated source baseline and one fixed built-in OxiPNG strategy; and
- explicit input files with separate, non-overwriting outputs.

Version 0.1 never writes source contents or replaces source directory entries. It checks the complete input/output mapping before publishing any output. After that check succeeds, each input is processed and committed independently, so a later per-input failure does not roll back earlier outputs. Each complete output is published by creating the destination as a hard link to a prepared temporary file in the output directory. A destination that exists at the publication point is never replaced.

The portable filesystem contract protects normal local CLI operation and detects ordinary concurrent source changes. It does not claim protection against an adversary that replaces path components while ImgLean runs, identical behavior across filesystem implementations, or support for output filesystems without hard links. Such failures are reported without publishing a partial destination.

Strict lossless equivalence preserves decoded image content and embedded payloads accepted by the PNG format policy. Encoding bytes may change, and source filesystem metadata is outside the guarantee. Every supported valid input enters the fixed baseline-versus-OxiPNG race. For each successfully processed input, ImgLean commits the smallest accepted candidate; when no smaller valid optimizer result exists, it commits the validated source image bytes unchanged.

The milestone is complete when:

- the complete validated baseline-versus-OxiPNG race works end to end;
- every enduring product invariant applicable to the fixed strict-lossless, built-in-provider workflow is demonstrated; and
- at least one supported input produces a validated OxiPNG size reduction.

## Possible future directions—not commitments

- Additional architectures, release targets, and filesystems without hard-link support
- Additional image formats and format-specific equivalence rules
- Explicit lossy quality policies for same-format optimization
- Format conversion
- Additional presets, strategies, and user-configurable resource limits
- Directory traversal, stream and pipeline workflows, dry-run, and machine-readable reporting
- Explicitly selected in-place operation and broader path handling
- Optional external providers or extensions

## Document ownership

[ARCHITECTURE.md](ARCHITECTURE.md) and the focused [input and batch](docs/contracts/INPUT_AND_BATCH.md), [PNG](docs/contracts/PNG.md), and [output](docs/contracts/OUTPUT.md) contracts are approved planned version 0.1 design commitments, not descriptions of implemented behavior. They may change when reviewed feasibility or implementation evidence requires it. Provider and release contracts are added beside their implementations when that work begins.

## Non-goals

ImgLean is not a general image editor, image CDN, hosted service, package manager, or replacement for ImageMagick's broad conversion surface. It does not download third-party providers or circumvent their licenses.

## Licensing boundary

ImgLean is licensed under Apache-2.0. The default binary may include only dependencies approved for permissive redistribution after auditing direct, transitive, vendored, and native code. GPL, AGPL, proprietary, and redistribution-restricted providers are never linked into the default binary. Any future commercially licensed integration must use a separate distribution or an external-provider boundary.
