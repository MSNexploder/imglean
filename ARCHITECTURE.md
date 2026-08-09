# ImgLean Architecture

This document describes the stable system shape of version 0.6. Exact format,
provider, filesystem, and resource behavior belongs to the
[focused contracts](docs/contracts/).

## System shape

ImgLean is organized around five responsibilities:

- The **controller** owns the public CLI and the complete invocation.
- The **strategy registry** defines applicable optimizers and stable order.
- Short-lived **workers** run one strategy against one private input.
- **Validators** inspect sources and candidates independently from providers.
- The **output layer** is the only component allowed to publish a winner.

Bundled strategies run through a private worker role of the ImgLean executable.
Supported external executables use the same private-input and private-candidate
boundary. Providers never receive the requested source or destination paths.

Worker processes isolate provider crashes and let the controller supervise
elapsed time and diagnostics. They are not a security sandbox or a hard memory
containment boundary; providers still run with the user's authority.

## Data flow

For one invocation, the controller:

1. Parses the CLI and resolves the stable strategy registry.
2. Preflights the complete input and output mapping before publication.
3. Captures and validates each source under configured resource limits.
4. Adds the validated source bytes as the baseline candidate.
5. Runs enabled applicable strategies against private copies of that capture.
6. Independently validates candidates in registry order.
7. Selects a new winner only when a candidate is strictly smaller.
8. Publishes the complete winner in output mode, or reports possible savings in
   check mode.
9. Reports the per-input strategy results and invocation summary.

Inputs are processed sequentially. Strategies for one input may run in
parallel, but completion order never controls validation, reporting, or
tie-breaking.

## Ownership boundaries

The controller owns:

- input and destination preflight;
- bounded source capture;
- the baseline candidate;
- strategy scheduling and process supervision;
- candidate validation and winner selection;
- temporary artifact cleanup;
- output publication; and
- user-facing diagnostics and exit status.

Workers own only one strategy attempt and its private artifacts. A provider may
produce bytes or fail; it cannot accept its own result, select the winner, or
publish output.

Validators are format-specific but provider-independent. They enforce the
documented structural, decode, dimension, resource, C2PA, and XMP gates. The
audited provider configuration establishes transformation semantics such as
lossless behavior and metadata handling.

## Failure model

Sources, paths, provider output, and provider diagnostics are untrusted.
Structural preflight failures stop the invocation before publication. Later
per-input failures may allow already completed inputs to remain published.

An optional optimizer failure normally excludes only that candidate and emits a
warning; the baseline and other strategies stay in the race. A provider that
the user explicitly requires must pass discovery before any output is created.

The controller bounds parsing, decoding, captured bytes, candidate bytes,
temporary storage, diagnostics, concurrency, input count, and monitored elapsed
time. The exact enforcement model and acknowledged limits are defined in
[docs/contracts/LIMITS.md](docs/contracts/LIMITS.md).

## Output and determinism

The validated original basename maps into one explicit output directory. The
controller refuses input aliases and publishes only a complete revalidated
temporary file through the platform-specific output contract. Source files are
never replaced.

The baseline appears first and wins equal-size ties. Given the same accepted
candidate set, encoded sizes and registry order fully determine the winner.
Provider failure or timeout can change that candidate set and is always
reported.

Release artifacts record the inputs needed to audit and reconstruct a build.
Bit-for-bit binary reproducibility is not claimed.

## Detailed contracts

- [Input and batch](docs/contracts/INPUT_AND_BATCH.md)
- [PNG validation](docs/contracts/PNG.md)
- [JPEG validation](docs/contracts/JPEG.md)
- [WebP validation](docs/contracts/WEBP.md)
- [AVIF validation](docs/contracts/AVIF.md)
- [Provider execution](docs/contracts/PROVIDER_EXECUTION.md)
- [Output publication](docs/contracts/OUTPUT.md)
- [Resource limits](docs/contracts/LIMITS.md)

Provider-specific integrations are documented under
[docs/providers](docs/providers/). The private worker protocol and supported
external adapters are implementation contracts, not a public plugin API.
