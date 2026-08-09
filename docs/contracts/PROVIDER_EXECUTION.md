# Version 0.1 Provider Execution Contract

> [!IMPORTANT]
> This is an approved version 0.1 implementation contract. Exact provider
> revisions, options, and numeric limits are versioned beside the code and
> release artifacts.

## Boundary

ImgLean runs each built-in optimizing strategy in a short-lived worker process
created from the same target-specific executable. The worker boundary isolates
provider crashes and hangs from the controller. It is not a security sandbox or
a portable hard memory-containment boundary.

The private worker role is an implementation detail, not a supported command,
plugin interface, or compatibility promise. The controller remains the only
component allowed to select a winner or publish a requested destination.

## Protocol

The controller invokes the current executable with a versioned private role,
the fixed strategy identifier, a controller-created private input path, and a
controller-reserved candidate path. Both paths identify current-invocation
artifacts in the canonical output directory. The original source path and the
requested destination path are never worker inputs.

The private input contains the controller's complete validated source capture.
The worker opens and bounded-reads it, runs the configured provider in memory,
and creates the candidate path with create-new semantics. It never modifies the
private input. The controller treats the candidate and all worker output as
untrusted and validates them independently.

The worker communicates success or failure through its exit status. Standard
output is reserved for future private protocol diagnostics and is empty in
version 0.1. Standard error may contain a human-readable diagnostic, but the
controller never forwards it directly.

Protocol-version, strategy, argument-count, path, and limit mismatches are
worker failures. They never alter the public CLI usage status.

## Supervision and limits

The controller captures both worker streams concurrently so neither pipe can
block the worker. It retains at most the configured number of bytes from each
stream while continuing to drain discarded bytes. Captured bytes are rendered
as one physical escaped line before reporting.

The worker receives the provider-supported decompressed-byte and elapsed-time
limits. The controller separately enforces a wall-clock deadline, terminates a
worker that exceeds it, waits for termination, and cleans current-run private
artifacts. Provider timeouts are configured slightly inside the controller
deadline so normal provider timeout reporting can complete.

Version 0.1 hard-enforces source, private-input, candidate-file,
captured-stream, and temporary-storage byte limits. Provider and controller
elapsed limits are configured or monitored at documented boundaries; blocking
operating-system calls may overshoot before control returns. Provider allocation
and native-code memory use are bounded indirectly by accepted dimensions,
decoded-byte limits, provider configuration, sequential execution, and process
termination; they are not claimed as portable hard address-space limits. Exact
values and enforcement classifications are in [LIMITS.md](LIMITS.md).

The built-in provider does not create descendants. ImgLean does not claim to
terminate descendants of a compromised worker or to bound machine-wide resource
exhaustion.

## Result handling

The following are optimizing-strategy warnings, not source failures:

- provider error or panic;
- abnormal exit or signal termination;
- timeout or failure to terminate cleanly;
- excessive or malformed diagnostics;
- missing, oversized, unreadable, or structurally invalid candidate; and
- a candidate rejected by the active PNG equivalence policy.

The controller excludes the failed candidate and continues deterministic
selection with the baseline and any other accepted candidates. A valid candidate
that is not smaller is a normal no-improvement result and does not warn.

## Cleanup

The controller tracks every private input and candidate path reserved or created
for the current invocation. Handled success and failure remove those entries
when possible. ImgLean never scans for similarly named artifacts and never
deletes an artifact it did not create or reserve in the current invocation.

Abnormal termination may leave private artifacts. Version 0.1 does not promise
crash cleanup.
