# Version 0.4 Resource Limits

This contract records limits version `v4`. `src/limits.rs` is authoritative;
changing a value requires a limits-version and boundary-test review.

## Hard byte and structure limits

- 128 explicit inputs per invocation.
- 64 MiB source bytes per input and 512 MiB in aggregate.
- 128 MiB per provider candidate.
- 256 MiB maximum controller-owned artifacts per active strategy worker.
- Three strategy workers maximum; two by default on machines exposing at least
  two CPUs and one by default otherwise. Thus tracked worker artifacts are
  bounded to 768 MiB at the explicit maximum.
- 64 KiB retained from each provider diagnostic stream; excess is drained and
  marked truncated.
- 32,768 pixels per dimension, 64 MiPixels, and 256 MiB decoder output storage.
- 64 MiB per PNG chunk, 4,096 chunks, and 16 MiB total ancillary payload.

Reads use inspected lengths or one-byte-over-limit detection. Arithmetic and
allocations are checked. Inputs run sequentially. Up to the selected worker
count of provider processes may run for the current input. Completed candidate
buffers can coexist until all results are restored to registry order. The four
registered strategies can therefore retain at most 512 MiB of encoded candidate
bytes in aggregate, plus the source. Moving one of those buffers into the winner
does not duplicate it. A registry-coupled test protects this effective bound.
Provider address spaces remain outside this controller-owned byte accounting.

## Elapsed-time limits

- PNG validation: 5 seconds, checked around bounded parse/decode stages.
- External-provider discovery probe: 2 seconds.
- OxiPNG's configured internal timeout: 55 seconds.
- Embedded-worker controller deadline: 60 seconds.
- External OptiPNG controller deadline: 60 seconds.
- External pngquant controller deadline: 60 seconds.
- Complete invocation: 15 minutes, checked before each input, before scheduling
  the strategy pool, before each queued strategy begins, and before publication.

These are provider-configured or monitored deadlines, not real-time guarantees.
A blocking system call, scheduler delay, termination, or one bounded operation
may overshoot. ImgLean does not claim a portable hard address-space or CPU limit.

## Invocation-wide failures

An aggregate-source or invocation-time breach fails active work, prevents later
commits, cleans tracked uncommitted artifacts when possible, and reports later
inputs as not processed. Earlier published outputs remain valid.
