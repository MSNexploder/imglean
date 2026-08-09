# Version 0.2 Provider Execution Contract

> [!IMPORTANT]
> Provider revisions, strategy identifiers, options, and limits are versioned
> beside the code and recorded in release artifacts.

## Registry and controls

The stable default order is `oxipng-libdeflate-v1`,
`oxipng-zopfli-v1`, then `optipng-v1`. The source baseline precedes all three.
Every enabled strategy is attempted once per validated input. Strictly smaller
accepted output replaces the current winner; stable order breaks equal sizes.

Compatible embedded strategies are enabled by default. `--disable-strategy ID`
removes a strategy. `--require-strategy ID` requires it to resolve during
preflight. `--provider optipng PATH` selects that executable and implicitly
requires `optipng-v1`. Duplicate, unknown, or disabled-and-required controls are
usage errors.

## Execution boundary

Each strategy receives a fresh private input containing the controller's exact
validated source capture and a reserved absent candidate path. Neither the
source path nor requested destination is provided. The controller verifies that
the private input remained unchanged, bounds and reads any candidate, validates
it independently, and exclusively owns selection and publication.

Embedded OxiPNG runs in a short-lived private role of the current executable.
The private protocol contains its version, strategy ID, limits version, private
input, and candidate path. It is not a public command or plugin interface.

External OptiPNG is itself the supervised worker process. It receives the same
private paths through its pinned command adapter. Process separation isolates
ordinary provider crashes and hangs but is not a security sandbox.

## External discovery

ImgLean supports exactly OptiPNG 7.9.1 for `optipng-v1`. Preflight first uses a
configured path when present; otherwise it searches `PATH` for `optipng` (or
`optipng.exe` on Windows). It canonicalizes an executable regular file, invokes
`-version` under the discovery deadline, and requires the exact supported
version. The resolved path and reported version are retained and reported once.

Automatic absence, a failed probe, or incompatibility skips the optional
strategy. The same condition fails preflight when the user required or
configured it. ImgLean never searches again during the invocation and never
downloads, installs, updates, or repairs provider software.

## Supervision and result handling

Standard input is null. Standard output and error are drained concurrently,
bounded independently, and escaped before diagnostics. The controller polls the
process deadline, kills and reaps an overdue process, and cleans only private
artifacts tracked for the current invocation.

Start failure, nonzero or abnormal exit, timeout, excessive diagnostics,
unreadable or oversized output, or candidate rejection produces one strategy
warning and no candidate. Other strategies and the baseline continue. A
successful provider that writes no candidate is the explicit no-improvement
case and is normal, as is an accepted candidate that does not improve the
winner. Private-input mutation or cleanup failure is a per-input failure because
controller-owned state can no longer be trusted.

Exact byte and time controls and their enforcement classification are in
[LIMITS.md](LIMITS.md). Memory is bounded indirectly by format and artifact
limits; no portable hard provider address-space limit is claimed.

## Coverage contract

Registry tests enumerate every stable strategy ID and prove one ordered attempt
per applicable strategy, baseline fallback, warning continuation, deterministic
winner selection, and equal-size tie behavior. Every embedded strategy runs
directly and through native controller tests. External tests cover absence,
required/configured failure, incompatible versions, process failures, bounded
diagnostics, missing and invalid candidates, larger candidates, and process
timeouts. Native CI downloads a checksum-pinned OptiPNG 7.9.1 for each release
target and proves discovery plus a real external-only reduction.

This registry is an explicit product surface, not a generic executable plugin
mechanism.
