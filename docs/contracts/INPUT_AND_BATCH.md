# Version 0.3 Input and Batch Contract

> [!IMPORTANT]
> This is the implemented version 0.3 input and batch contract.

## Boundary

This document records version 0.3 input handling. Exact constants and their
enforcement classifications are in [LIMITS.md](LIMITS.md). Version 0.3
deliberately uses common path and file operations and does not promise defense
against adversarial replacement of path components while an invocation is
running.

## Invocation and preflight

Version 0.3 accepts explicit regular files and one required existing output directory:

```text
imglean --output OUTPUT_DIRECTORY INPUT...
```

There is no traversal, standard input, or in-place mode. `--` ends option parsing. `--help` and `--version` exit successfully without preflight; missing inputs and invalid options exit `2`.

The final component of each original input argument is retained as that input's output basename. It must be one component made from printable ASCII, with a nonempty stem and a `.png` extension compared without ASCII case. Preflight folds this retained basename's ASCII case for destination-collision detection on every platform. This deliberately rejects names that would be distinct on some filesystems so one invocation has the same mapping on case-sensitive and case-insensitive filesystems. Canonicalization does not change the retained output basename. Ancestor directory names and the output-directory path are not subject to the basename restriction.

The controller captures the initial working directory and preflights the complete argument list before source capture or filesystem mutation. It rejects an input observed as a final-component symlink, resolves ancestor symlinks and the output-directory path to absolute canonical paths, then opens each input and verifies that it is a regular file. This portable symlink check is not claimed to be race-free. Repeated canonical input paths and ASCII-folded destination collisions are rejected. A destination may be absent or an existing regular non-symlink file; directories, symlinks, and special files are rejected. Existing destinations are compared with every input using platform file identity so direct paths and hard-link aliases cannot replace inputs. Distinct input paths to one source may still be processed as distinct explicit inputs. One structural violation aborts the invocation without producing outputs.

The output directory must be an existing directory. The controller creates each complete internal output there so publication is a same-directory rename rather than a cross-filesystem operation.

Preflight retains each opened source file. Later capture reads that open file rather than resolving the user-provided pathname again. Sidecar and output operations remain path-based; concurrent path replacement can make them fail or redirect them, and version 0.3 does not claim otherwise. The entry occupying a requested destination at publication may be replaced even if it appeared or changed after preflight.

## Source capture

When an input's processing begins, the controller records the open file's portable type, length, and modification time when the platform provides it, performs a bounded read from that file, and records the same state again. A metadata lookup failure, a change in type or length, a change in modification-time availability or value, or more bytes than the limit permits is a per-input source-validation failure. A consistently unavailable modification time is omitted from the comparison. Access time is excluded because the controller's own read may update it. This catches ordinary concurrent changes but is not an adversarial snapshot guarantee; changes that preserve all compared state may go undetected.

The captured bytes are the source of truth for validation, the controller-owned baseline, optimization, and reporting. Each optimizing worker receives a private input derived from them, and the controller verifies that private input before accepting the worker result.

## Content Credentials heuristic

Embedded PNG handling is defined in [PNG.md](PNG.md). For an accepted basename, the C2PA-defined external-manifest path replaces the final extension with `.c2pa`; ImgLean's additional conservative heuristic appends `.c2pa` to the complete basename. Thus `photo.png` checks `photo.c2pa` and `photo.png.c2pa`. Names without an unambiguous stem and extension are already rejected during structural preflight.

The sidecar names are derived from the canonical source path's filename and checked beside that canonical source. Immediately before and after source capture, the controller checks both pathnames. Any filesystem entry or lookup failure is a per-input source-validation failure. Later sidecar changes are intentionally ignored because the output represents that capture. ImgLean does not parse the sidecar or access the network. Concurrent renaming or replacement of the source parent can affect these path-based checks and is outside the version 0.3 guarantee.

## Work limits and sequencing

Version 0.3 processes inputs sequentially after batch preflight. Enabled
strategies for the current input run through a bounded worker pool before its
winner is committed and reported and the next input begins. Provider resolution,
including required external-provider checks, completes before input processing.

The implementation bounds and tests input count, per-input and aggregate
captured bytes, worker concurrency, temporary and candidate storage, buffered
diagnostics, and total elapsed time. Exact release values are version-controlled
in [LIMITS.md](LIMITS.md) and `src/limits.rs`.

An invocation-wide limit breach cancels the current work, prevents later commits, cleans current-run uncommitted artifacts, reports later inputs as not processed in the standard-error summary, and exits `1`. Outputs already committed remain valid.

## Diagnostics and status

All user-controlled paths and captured worker text are converted to one physical line: control characters use visible escapes and invalidly encoded path units use hexadecimal escapes. Raw worker output is never forwarded directly.

For each input processed after structural preflight, ImgLean attempts one
standard-output result after its commit or failure decision and before the next
input begins. A successful result names the winning strategy or baseline. A
successful block lists the baseline followed by every registered strategy in
registry order, its encoded byte count or outcome, one marked winner, and the
destination. Disabled, unavailable, and not-run strategies remain visible.
Strategy warnings and candidate rejections appear inline in that block. A
structural abort emits only standard-error diagnostics. Resolved
external-provider records, failure details, and the compact summary go to
standard error. Standard-error writes are best effort; their failure does not
create a separate reporting failure or alter the exit status. If a required
standard-output write fails, ImgLean performs no later commits, cleans
current-run uncommitted artifacts, and exits `1`; earlier committed outputs
remain valid.

Exit statuses are:

- `0`: all outputs succeeded without optimizer warnings;
- `3`: all outputs succeeded, with at least one optimizer warning;
- `1`: structural, per-input, aggregate-limit, or required standard-output reporting failure; and
- `2`: invalid CLI usage.

Exit `1` takes precedence over exit `3` when failures and optimizer warnings coexist.

The wording is not a stable machine-output schema.
