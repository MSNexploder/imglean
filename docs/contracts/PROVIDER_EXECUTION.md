# Version 0.4 Provider Execution Contract

> [!IMPORTANT]
> Provider revisions, strategy identifiers, options, and limits are versioned
> beside the code and recorded in release artifacts.

## Registry and controls

The stable default order is `oxipng-libdeflate-v1`,
`oxipng-zopfli-v1`, `optipng-v1`, then `pngquant-v1`. The source baseline
precedes all four. Every enabled applicable strategy is attempted once per
validated input. Strictly smaller accepted output replaces the current winner;
stable order breaks equal sizes.

Compatible embedded strategies are enabled by default. `--disable-strategy ID`
removes a strategy. `--require-strategy ID` requires it to resolve during
preflight. `--provider NAME PATH` selects a supported executable and implicitly
requires its strategy. Duplicate, unknown, or disabled-and-required controls
are usage errors.

Resolution retains one entry for every stable strategy. Runnable entries are
submitted to the worker pool; disabled, unavailable, and not-applicable entries
are not run and do not warn. Per-input reporting always follows registry order.
Optional discovery failure remains normal, while a required or configured
unavailable provider fails structural preflight.

## Quality capability

`--quality lossless|1..100` defaults to `lossless`. Numeric values run lossless
strategies too; they allow fidelity reduction rather than require it. Each
strategy declares applicability and owns its versioned native mapping:

- `oxipng-libdeflate-v1` and `oxipng-zopfli-v1` use OxiPNG's lossless
  recompression path. Transparent-color optimization and forced 16-to-8-bit
  conversion are disabled. OxiPNG has no general native numeric quality
  control.
- `optipng-v1` uses OptiPNG's lossless `-o2` optimization. Its optimization
  levels control effort and trial count, not image quality.
- `pngquant-v1` is not applicable to `lossless`. For numeric `Q`, it passes
  pngquant `--quality 0-Q`: `Q` is the provider's maximum native quality target,
  with 1 lowest and 100 highest. pngquant still limits output to a palette, so
  100 is not a lossless promise.

The common number is intentionally not an ImgLean-calculated quality score.
The controller applies its basic PNG gate and trusts the audited provider
mapping. Baseline participation guarantees only that the selected file is not
larger, not that a numeric-quality result is lossless.

## Execution boundary

Each strategy receives a fresh private input containing the controller's exact
validated source capture and a reserved absent candidate path. Neither the
source path nor requested destination is provided. The controller verifies that
the private input remained unchanged, bounds and reads any candidate, validates
it independently, and exclusively owns selection and publication.

Embedded OxiPNG runs in a short-lived private role of the current executable.
The private protocol contains its version, strategy ID, limits version, private
input, and candidate path. It is not a public command or plugin interface.

External OptiPNG and pngquant are each the supervised worker process. They
receive the same private paths through pinned command adapters. Process
separation isolates ordinary provider crashes and hangs but is not a security
sandbox.

## External discovery

ImgLean supports exactly OptiPNG 7.9.1 for `optipng-v1` and pngquant 3.0.2 or
3.0.3 for `pngquant-v1`. Preflight first uses a configured path when present;
otherwise it searches `PATH` for `optipng`/`pngquant` (with `.exe` on Windows).
ImgLean canonicalizes an executable regular file, invokes the provider's version
command under the discovery deadline, and requires an exact supported version.
Resolved paths and reported versions are retained and reported once.

Automatic absence, a failed probe, or incompatibility marks the optional
strategy unavailable without running it. The same condition fails preflight
when the user required or configured it. A strategy explicitly required at an
inapplicable quality also fails preflight. ImgLean never searches again during
the invocation and never downloads, installs, updates, or repairs provider
software.

## Supervision and result handling

Standard input is null. Standard output and error are drained concurrently,
bounded independently, and escaped before diagnostics. The controller polls the
process deadline, kills and reaps an overdue process, and cleans only private
artifacts tracked for the current invocation.

For one input, ImgLean runs up to the selected `--jobs` count concurrently.
Every worker owns a separate artifact tracker. Results are collected completely
and reordered by registry position before candidate validation, winner
selection, and reporting. A fatal strategy result fails the input after all
bounded work is collected; it cannot make another strategy's result disappear.

Start failure, nonzero or abnormal exit, timeout, excessive diagnostics,
unreadable or oversized output, or candidate rejection produces one strategy
warning and no candidate. Other strategies and the baseline continue. A
successful provider that writes no candidate is a normal no-improvement result,
as is an accepted candidate that does not improve the winner. pngquant exit
status 99 is also a normal no-candidate result because it means its native
quality requirement could not be met. Private-input mutation or cleanup failure
is a per-input failure because controller-owned state can no longer be trusted.

Exact byte and time controls and their enforcement classification are in
[LIMITS.md](LIMITS.md). Memory is bounded indirectly by format and artifact
limits; no portable hard provider address-space limit is claimed.

## Coverage contract

Registry tests enumerate every stable strategy ID and prove one attempt per
applicable strategy, explicit disabled, unavailable, and not-applicable states,
bounded worker concurrency, baseline fallback, warning continuation,
deterministic winner selection, and equal-size tie behavior. Every embedded
strategy runs directly and through native controller tests. External tests cover
absence, required/configured failure, incompatible versions, process failures,
bounded diagnostics, missing and invalid candidates, larger candidates, and
process timeouts. Native CI tests OptiPNG 7.9.1 and both supported pngquant
versions on each release target and proves discovery plus real external-only
reductions.

This registry is an explicit product surface, not a generic executable plugin
mechanism.
