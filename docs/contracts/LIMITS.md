# Version 0.1 Resource Limits

This contract records limits version `v1`. The constants in `src/limits.rs` are
authoritative; changing one requires a limits-version review and boundary-test
updates.

## Hard byte and structure limits

- 128 explicit inputs per invocation.
- 64 MiB encoded source bytes per input and 512 MiB in aggregate.
- 128 MiB encoded provider candidate bytes.
- 256 MiB maximum live controller-owned provider artifacts per input.
- 64 KiB retained from each worker diagnostic stream; excess bytes are drained
  and marked as truncated.
- 32,768 pixels per dimension, 64 MiPixels, and 256 MiB reconstructed scanline
  storage.
- 64 MiB per PNG chunk, 4,096 chunks, and 16 MiB total accepted ancillary
  payload bytes.

Reads use one-byte-over-limit detection or an already-inspected file length.
Integer arithmetic and allocations are checked before use. Inputs are processed
sequentially, so version 0.1 has one worker at a time.

## Elapsed-time limits

- PNG validation: 5 seconds, checked at bounded parsing, decompression, and row
  reconstruction boundaries.
- OxiPNG's configured provider timeout: 55 seconds.
- Controller worker deadline: 60 seconds, polled every 10 milliseconds; an
  overdue worker is terminated and reaped.
- Complete invocation: 15 minutes, checked before each input and before output
  preparation.

These are monitored or provider-configured elapsed limits, not real-time
deadlines. A blocking operating-system call, process scheduling, termination,
or one bounded validation operation may overshoot before control returns to the
next check. Worker memory is constrained indirectly by the accepted dimensions,
byte limits, provider settings, sequential execution, and process termination;
ImgLean does not claim a portable hard address-space limit.

## Invocation-wide failures

An aggregate-source or invocation-time breach fails the current input if one is
active, prevents later commits, cleans current-run uncommitted artifacts, and
reports remaining inputs as not processed. Earlier published outputs remain
valid.
