# ImgLean Architecture

> [!IMPORTANT]
> This document describes the implemented version 0.2 component boundaries.

## System shape

ImgLean has one public controller and two provider execution forms:

- The **controller** parses the CLI, resolves strategies, preflights the complete
  batch, validates sources and candidates, selects winners, and is the only
  component allowed to publish outputs.
- An **embedded worker** is a short-lived private role of the same executable.
  It runs one OxiPNG strategy against one controller-created private input.
- A supported **external provider** is a separately installed executable invoked
  directly with the same private-input/private-candidate boundary.

Process separation isolates crashes and enables bounded diagnostics and elapsed
time supervision. It is not a security sandbox or portable hard memory limit;
providers run with the user's authority.

## Registry and discovery

The versioned registry fixes strategy identity and order:

1. `oxipng-libdeflate-v1` — embedded;
2. `oxipng-zopfli-v1` — embedded;
3. `optipng-v1` — external OptiPNG 7.9.1 when available.

Compatible embedded strategies are enabled unless disabled. External discovery
resolves a configured executable or the first executable named `optipng` on
`PATH`, probes its version once under a short deadline, and retains that
canonical path and version for the invocation. Automatic absence or
incompatibility skips the strategy; an explicitly required provider turns the
same condition into structural preflight failure. Discovery never downloads or
changes provider software.

## Controller responsibilities

The controller owns complete path and destination preflight, bounded source
capture, the source baseline, stable strategy scheduling, process supervision,
candidate validation, winner selection, publication, diagnostics, and
invocation-wide limits. Workers receive neither source paths nor requested
destination paths.

Source and candidate PNG bytes pass through the same bounded format validator.
It verifies the signature, chunk framing and CRCs, complete static image decode,
resource bounds, and the C2PA/XMP refusal. Candidate dimensions must match the
source. The provider's audited lossless and metadata configuration—not a second
pixel or ancillary comparison—establishes transformation semantics.

## Per-input flow

1. Resolve the strategy registry and preflight every input/output mapping.
2. Read the already-open input under portable before/after state checks.
3. Validate the captured source and admit its bytes as the baseline.
4. In registry order, give each enabled strategy a fresh private copy of those
   bytes and a reserved absent candidate path.
5. Supervise the process, bound diagnostics and output, clean its private
   artifacts, and independently validate any candidate.
6. Replace the current winner only when the candidate is strictly smaller.
7. Write and revalidate the winner in the output directory, then publish it by
   non-replacing hard-link creation.
8. Report the winner and continue with the next input.

The baseline is first, so it wins equal-size ties. Sequential execution makes
completion order identical to stable registry order and keeps one provider
process active at a time.

## Failure and trust boundaries

Source files, provider output, paths, and diagnostics are untrusted. Provider
start failure, nonzero exit, timeout, oversized diagnostics, unreadable or
invalid output, and later executable failure warn and exclude only that
candidate. Successful exit with no output is a normal no-improvement result.
Mutation of the controller-owned private input or inability to clean tracked
current-run artifacts fails the input. A required provider's discovery failure
aborts before any destination is created.

The controller bounds bytes, allocations, input count, temporary storage,
diagnostics, and monitored elapsed time. It does not promise hard address-space,
CPU, descendant-process, hostile path-race, crash-cleanup, or machine-wide
resource containment.

## Output and determinism

The original validated basename maps into one required canonical output
directory. The controller never writes a source or replaces a destination. It
publishes only a complete revalidated temporary file through a same-directory
hard link. Outputs receive ordinary new-file filesystem metadata.

Given the same accepted candidate set, encoded sizes and fixed registry order
fully determine the winner. Provider failures and timeouts may alter that set
and are reported. Release manifests record the registry, provider settings,
dependency lock, toolchain, target, native tools, and build flags; bit-for-bit
binary reproducibility is not claimed.

## Focused contracts

- [Input and batch](docs/contracts/INPUT_AND_BATCH.md)
- [PNG validation](docs/contracts/PNG.md)
- [Provider execution](docs/contracts/PROVIDER_EXECUTION.md)
- [Output publication](docs/contracts/OUTPUT.md)
- [Resource limits](docs/contracts/LIMITS.md)

The private worker role and supported external adapters are implementation
contracts, not a general plugin or compatibility interface.
