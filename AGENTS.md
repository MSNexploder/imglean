# ImgLean Agent Guidelines

## Mission and sources of truth

Maintain ImgLean as a focused, reliable CLI that runs every applicable image
optimization strategy and keeps the smallest accepted same-format result.

- [SCOPE.md](SCOPE.md) owns the product boundary and promises.
- [ARCHITECTURE.md](ARCHITECTURE.md) owns stable components and data flow.
- [docs/contracts](docs/contracts/) owns exact implemented behavior.
- [docs/providers](docs/providers/) owns provider-specific configuration.
- [docs/RELEASE.md](docs/RELEASE.md) owns release qualification.

Read the relevant owner before changing its behavior. Do not duplicate exact
contracts in high-level documents or infer new behavior from implementation
accidents.

## Engineering approach

- Make the smallest direct change that fully satisfies the current requirement.
- Do not add a dependency, provider, format, abstraction, fallback, or
  compatibility path for hypothetical future use.
- Use mise to install and select the supported Rust toolchain; do not rely on an
  ambient toolchain.
- Prefer standard-library facilities and safe Rust. Document every FFI or
  `unsafe` safety contract and cover it with focused tests.
- Keep public APIs near the top of modules, private helpers below them, and
  validation independent from provider implementations.
- Resolve and record exact toolchain and native build inputs for releases.

## Safety invariants

- Treat sources, paths, provider output, and diagnostics as untrusted.
- Bound parsing, decoding, allocation, captured bytes, candidate bytes,
  temporary storage, diagnostics, concurrency, input count, and elapsed time.
- Preflight the complete input/output mapping before publication.
- Validate each source before strategies run and every candidate before it can
  participate in selection.
- Give strategies private copies of controller-captured source bytes; never let
  a provider read from or write to requested source or destination paths.
- Always include the validated source baseline and keep registry-order
  tie-breaking deterministic.
- Publish only a complete revalidated winner through the output contract. Never
  replace an input or expose partial output.
- Preserve the exact trust boundaries and guarantees in the focused contracts;
  do not claim independent pixel, metadata, or ancillary-payload equivalence.

## Providers and licensing

- Enable every compatible bundled strategy by default.
- Keep pngquant optional and external because it cannot be linked into the
  Apache-2.0 distribution.
- Never download, install, or update providers at runtime.
- Capability-check supported external executables once per invocation; do not
  gate compatibility on version strings.
- Run strategies in supervised worker processes for crash and elapsed-time
  isolation without describing them as a security sandbox.
- Keep behavior-affecting provider options explicit, pinned, tested, and
  recorded in release artifacts.
- Warn and continue with remaining candidates when an optional strategy fails.
- Audit Rust, native, vendored, and transitive dependencies before distribution
  and preserve required notices, inventory, and SBOM output.

## Validation

Run project tooling through `mise.toml`. The canonical development gate is:

```sh
mise run check
```

Do not bypass locked dependencies, all-target/all-feature linting,
warnings-as-errors, or the complete test suite.

Changes to validators, providers, input handling, output publication, resource
limits, or release targets require focused regression evidence in addition to
the canonical gate. Update the relevant bounded corpus and contract tests.
Every bundled strategy must execute directly and through the controller in CI;
every supported external adapter needs representative execution plus absence,
capability, failure, timeout, malformed-output, and larger-output coverage on
each claimed target.

## Documentation

- Keep `README.md` focused on what ImgLean does and how to use it.
- Keep `SCOPE.md` focused on product decisions, promises, and non-goals.
- Keep `ARCHITECTURE.md` focused on stable ownership and data flow.
- Put exact format, provider, filesystem, protocol, and limit behavior in its
  focused contract.
- Describe implemented behavior directly. Clearly label only genuinely
  unfinished or unqualified work.
- Treat release artifacts as target-specific; never imply that one executable
  runs across operating systems.
