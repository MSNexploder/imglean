# ImgLean Architecture

> [!IMPORTANT]
> This document describes the implemented version 0.1 component boundaries.
> Target-specific release qualification is tracked separately.

## Role of this document

This document records the structural decisions needed to implement version 0.1 safely. [SCOPE.md](SCOPE.md) remains authoritative for product behavior. Exact format rules, resource values, provider options, release targets, and command-line details belong in focused contracts and tests when their subsystem is implemented.

## System shape

ImgLean ships as one executable with two internal roles:

- The **controller** handles the public CLI, owns the optimization race, and is the only component allowed to commit an output. Version 0.1 never writes or replaces a source file or intentionally changes its metadata.
- A short-lived **worker** runs one optimizing strategy against one private input derived from the controller's validated source capture. The controller invokes the same ImgLean executable in an internal mode; this is not a public plugin interface.

Built-in provider code runs only inside separate worker processes. This boundary isolates provider crashes, aborts, and native memory faults from the controller and lets the controller detect and terminate hangs. The controller bounds exchanged bytes and elapsed time, and the provider receives explicit internal limits where it supports them. The worker process is not a security sandbox or a hard memory-containment boundary: built-in providers are audited trusted code running with the user's authority.

## Controller responsibilities

The controller owns:

- complete argument-list and output-mapping preflight;
- bounded source inspection and validation;
- capture of validated source content, creation of the baseline candidate, and creation of a private input for the optimizing strategy;
- deterministic strategy selection and ordering;
- worker lifecycle and resource controls;
- independent validation of candidate results;
- winner selection and filesystem commits; and
- per-input diagnostics and final process status.

Controller-wide limits bound the complete invocation, not only individual workers. They cover input count, captured bytes, worker concurrency, temporary storage, diagnostics, and elapsed time. Exact values and enforcement classifications are versioned in [the limits contract](docs/contracts/LIMITS.md) and tested with their implementations.

The worker protocol identifies only the private strategy input and candidate destination needed by a strategy; source and commit paths are not protocol inputs. This reduces accidental misuse but does not deny access: workers run with the user's authority, and process separation alone does not prevent same-user access to other system resources. The mechanism used to capture source content and derive private strategy inputs belongs in the input-and-batch contract rather than this architecture.

## Output model

Version 0.1 requires an existing output directory. Each input maps to the validated final component retained from its original command-line argument; source canonicalization does not change that destination name. Structural preflight rejects destination collisions and every pre-existing destination. Mixed or in-place operation does not exist in version 0.1. The controller resolves the output directory to an absolute canonical path during preflight and uses that path for later operations. Version 0.1 does not claim that those operations remain bound to the same directory if another process renames or replaces path components while ImgLean runs.

The validated source capture is the controller-owned baseline candidate. It enters the same winner-selection model as optimizer-produced candidates without being copied through a worker. It is first in stable order and therefore wins equal-size ties.

A complete temporary file in the output directory receives the selected winner. The controller validates that file and publishes it by creating the requested destination as a hard link. Hard-link creation fails if the destination exists, so a destination that appears after preflight is never replaced. The temporary name is then removed. Filesystems that cannot create the required same-directory hard link produce a per-input output failure.

Outputs represent the validated source capture even if the source changes later.

Embedded image metadata is governed by the format-equivalence contracts. Version 0.1 treats accepted ancillary payloads as opaque and requires their bytes, order, and placement around image data to remain unchanged; anything not explicitly accepted is rejected. Filesystem metadata is not part of image equivalence: outputs receive new-file metadata, and version 0.1 does not add a separate metadata-copying or normalization layer.

## Per-input flow

1. Canonicalize every input and the output directory, open every input, map every destination, and preflight the complete invocation. Any structural violation aborts the entire invocation before any path is changed.
2. Through the already-open input file, record portable source state, perform a bounded read under controller ownership, and compare the state again. Reject detected ordinary changes; the captured bytes are the source of truth but are not claimed to be an adversarially coherent filesystem snapshot.
3. Validate the source capture within the documented source limits.
4. Admit the validated source capture as the baseline and schedule the single bundled OxiPNG strategy.
5. Derive a private input from the validated capture and run the strategy in a separate worker process.
6. Independently validate every optimizer-produced candidate. An optimizing-strategy failure or rejected candidate produces a warning and is excluded from selection.
7. Choose the smallest candidate accepted under the version 0.1 strict-lossless policy. The baseline wins equal-size ties through stable order; worker completion order never affects the result.
8. Write and validate the result under a unique temporary name in the output directory, publish it through non-replacing hard-link creation, and remove the temporary name.

Inputs become independent only after structural batch preflight succeeds. Version 0.1 then processes them sequentially: it completes and reports one input before beginning the next. An ordinary per-input failure does not stop subsequent inputs.

## Trust and failure boundaries

Source files, candidate files, and metadata are untrusted. Built-in providers are trusted dependencies but are treated as fallible. Parsing, decoding, provider execution, allocation, output, and temporary storage receive explicit resource controls.

The controller treats worker errors, abnormal exits, timeouts, bounded-I/O violations, and malformed results as untrusted outcomes. It configures provider concurrency and provider-supported limits, bounds exchanged data, and terminates a worker that exceeds the controller's elapsed-time limit. Version 0.1 does not promise portable hard limits for worker address space, CPU consumption below the wall timeout, or descendants created by a compromised provider. The built-in provider does not spawn child processes. Exact mechanisms and permitted overshoot belong in the provider-execution contract. An optimizing-strategy failure is reported but does not veto the baseline or another valid candidate.

Temporary artifacts use unique internal names created atomically and never occupy a requested destination path before publication. They receive the platform and filesystem metadata produced by ordinary new-file creation; their names and permissions are not a confidentiality boundary. Handled completion and failure remove artifacts tracked by the current invocation; version 0.1 never infers that an artifact from another invocation is safe to delete.

Before a successful atomic commit, ImgLean has not published data at the destination. A successful commit exposes the complete validated result, never a partial candidate. ImgLean does not promise crash durability, control over external destination changes, or protection from machine-wide resource exhaustion.

## Determinism

Strategy definitions, order, and behavior-affecting provider options are version-controlled. Given the accepted candidate set, selection depends on validated content, byte size, and stable strategy order—not scheduling or worker completion timing. Timeouts and resource failures may change which candidates are accepted and are reported explicitly.

At the start of a release build, the moving stable Rust channel is resolved to one exact version and used throughout that build. Release artifacts record that toolchain, their dependency lock, target and deployment settings, bundled-provider revisions, native compiler and linker, SDK, build tools, features, and build flags. This makes release inputs auditable and reconstructible; version 0.1 does not claim bit-for-bit reproducible binaries.

## Focused implementation contracts

Version 0.1 behavior is further defined by focused contracts:

- **[Input and batch](docs/contracts/INPUT_AND_BATCH.md):** path representation and mapping, structural preflight, portable source capture, invocation-wide limits, C2PA sidecar checks, diagnostics, and CLI edge cases.
- **[Provider execution](docs/contracts/PROVIDER_EXECUTION.md):** worker protocol, safe candidate acquisition, portable byte and elapsed-time limits, diagnostic sanitization, result handling, termination, and cleanup.
- **[PNG](docs/contracts/PNG.md):** accepted encoding classes, equivalence, metadata, validation limits, and OxiPNG option boundaries.
- **[Output](docs/contracts/OUTPUT.md):** path handling, required hard-link behavior, temporary paths and cleanup, non-replacing publication, new-file metadata, and filesystem failures.

The input-and-batch and output contracts precede implementation of structural preflight, output mapping, and commit behavior. The PNG contract precedes OxiPNG enablement.

Provider-specific notes should record only the exact integration: revision, enabled features, every behavior-affecting option, build inputs, license obligations, and known limitations. Provider integrations must not rely on upstream defaults.

## Intentionally deferred

Version 0.1 does not establish an in-place mode, public worker protocol, plugin ABI, machine-report schema, external-provider contract, or generalized filesystem abstraction. Those decisions remain deferred until their corresponding product scope is active.
