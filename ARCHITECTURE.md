# ImgLean Architecture

> [!IMPORTANT]
> This document describes the implemented version 0.6 component boundaries.

## System shape

ImgLean has one public controller and two provider execution forms:

- The **controller** parses the CLI, resolves strategies, preflights the complete
  batch, validates sources and candidates, selects winners, and is the only
  component allowed to publish outputs.
- A **bundled worker** is a short-lived private role of the same executable. It
  runs one linked OxiPNG, OptiPNG, jpegtran, MozJPEG, Jpegli, libwebp,
  image-webp, libavif/libaom, or ravif/rav1e strategy against
  one controller-created private input.
- A supported **external provider** is a separately installed executable used
  for pngquant or as an explicit override, with the same private-input/private-
  candidate boundary.

Process separation isolates crashes and enables bounded diagnostics and elapsed
time supervision. It is not a security sandbox or portable hard memory limit;
providers run with the user's authority.

Integration form is assessed in the order safely embeddable, linkable, then
callable. OxiPNG uses its safe Rust API. OptiPNG uses a narrow PNG-only wrapper
around its vendored engine. MozJPEG and Jpegli use Rust-facing native wrappers,
and jpegtran uses MozJPEG's coefficient API behind one audited FFI boundary.
libwebp uses a narrow audited FFI boundary, image-webp is safe Rust,
libavif/libaom uses its Rust-facing native wrapper, and ravif supplies the
independent Rust AV1 encoder.
All linked code runs only in the disposable worker process, so linking does not
remove crash and timeout isolation. pngquant remains external because of its
GPL/commercial licensing boundary.

## Registry and discovery

The versioned registry fixes strategy identity and order:

1. `oxipng-libdeflate-v1` — bundled;
2. `oxipng-zopfli-v1` — bundled;
3. `optipng-v1` — bundled OptiPNG for PNG;
4. `pngquant-v1` — external pngquant for PNG at numeric quality;
5. `jpegtran-v1` — bundled lossless JPEG coefficient optimization;
6. `mozjpeg-v1` — bundled MozJPEG for JPEG at numeric quality;
7. `jpegli-v1` — bundled Jpegli for JPEG at numeric quality;
8. `libwebp-v1` — bundled libwebp for lossless or numeric-quality WebP;
9. `image-webp-v1` — bundled lossless image-webp;
10. `avif-aom-v1` — bundled libavif/libaom at numeric quality; and
11. `avif-rav1e-v1` — bundled ravif/rav1e at numeric quality.

Compatible bundled strategies are enabled unless disabled. A configured
external executable overrides its bundled implementation. Without an override,
only the unbundled pngquant strategy searches `PATH`. libwebp additionally
supports an explicit capability-probed `cwebp` override. External providers are
probed once under a short deadline and retained by canonical path for the
invocation. Probes check CLI identity and behavior-affecting capabilities;
provider version text is not a compatibility boundary. Numeric quality leaves all lossless strategies
applicable and enables the lossy providers; lossless quality marks them not
applicable without probing. Automatic absence or capability mismatch skips a
strategy; an explicitly required provider turns the same
condition into structural preflight failure. Discovery never downloads or
changes provider software.

## Controller responsibilities

The controller owns complete path preflight and, in output mode, destination
preflight, bounded source
capture, the source baseline, stable strategy scheduling, process supervision,
candidate validation, winner selection, publication, diagnostics, and
invocation-wide limits. Workers receive neither source paths nor requested
destination paths.

Source and candidate bytes pass through the same bounded validator for their
declared format. PNG, JPEG, WebP, and AVIF validators check their documented container
subset, complete decode, resource bounds, and C2PA/XMP refusal. Candidate
dimensions must match the source. The provider's audited fidelity and metadata
configuration—not a second pixel, perceptual-quality, or ancillary
comparison—establishes transformation semantics.

The controller passes the common `--strip-metadata` request into each strategy
mapping. A strategy uses its native removal control when one exists and remains
eligible when one does not. The controller does not rewrite a candidate or
inspect it for successful metadata removal. The baseline remains the captured
source bytes, so it can still win unchanged when every candidate is larger,
equal, absent, or rejected.

## Per-input flow

1. Resolve the strategy registry and preflight every input plus any requested
   output mapping.
2. Read the already-open input under portable before/after state checks.
3. Validate the captured source and admit its bytes as the baseline.
4. Submit enabled strategies in registry order to the bounded per-input worker
   pool. Each receives a fresh private copy of those bytes and a reserved absent
   candidate path.
5. Supervise each process, bound diagnostics and output, clean its separately
   owned private artifacts, collect every result, and independently validate
   candidates in registry order.
6. Replace the current winner only when the candidate is strictly smaller.
7. In output mode, write and revalidate the winner in the output directory,
   then publish it by same-directory rename, replacing an existing regular
   destination. In check mode, record whether the winner is smaller and publish
   nothing.
8. Report the winner and the registry rows for that input's format, then
   continue with the next input.

The baseline is first, so it wins equal-size ties. Worker completion order is
discarded before validation and selection; stable registry order remains the
only strategy tie-breaker. Inputs, winner publication, and reporting remain
sequential even though multiple provider processes may run for one input.

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

The CLI may override the bounded per-strategy worker deadline. Bundled OxiPNG
receives a shorter internal deadline so the controller retains cleanup time;
validation, discovery, and invocation-wide deadlines remain separate.

## Output and determinism

In output mode the original validated basename maps into one required canonical
output directory. The controller never writes a source or permits an output to
alias an input. It publishes only a complete revalidated temporary file through
a same-directory replacing rename. Outputs receive ordinary new-file filesystem
metadata rather than metadata copied from the replaced destination. Check mode
uses an invocation-owned temporary directory for private provider artifacts and
attempts to remove it without invoking the output publication layer.

Given the same accepted candidate set, encoded sizes and fixed registry order
fully determine the winner. Provider failures and timeouts may alter that set
and are reported. Release manifests record the registry, provider settings,
dependency lock, toolchain, target, native tools, and build flags; bit-for-bit
binary reproducibility is not claimed.

## Focused contracts

- [Input and batch](docs/contracts/INPUT_AND_BATCH.md)
- [PNG validation](docs/contracts/PNG.md)
- [JPEG validation](docs/contracts/JPEG.md)
- [WebP validation](docs/contracts/WEBP.md)
- [AVIF validation](docs/contracts/AVIF.md)
- [Provider execution](docs/contracts/PROVIDER_EXECUTION.md)
- [Output publication](docs/contracts/OUTPUT.md)
- [Resource limits](docs/contracts/LIMITS.md)

The private worker role and supported external adapters are implementation
contracts, not a general plugin or compatibility interface.
