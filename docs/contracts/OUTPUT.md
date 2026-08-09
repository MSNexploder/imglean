# Version 0.4 Output Contract

> [!IMPORTANT]
> This is the implemented version 0.4 output contract.

## Boundary

This document defines the portable output guarantees for version 0.4. It uses
common file creation and the target's same-directory replacing-rename operation
(`rename` on Unix and `MoveFileExW` with replacement on Windows). It protects
normal local CLI operation but does not promise that paths remain bound to the
same directory entries when another process concurrently renames or replaces
path components.

## Destination preflight

The user supplies an existing output directory. Structural preflight resolves
it to an absolute canonical path and verifies the complete destination mapping.
Each destination may be absent or an existing regular non-symlink file.
Directories, symbolic links, and special files are rejected.

Every existing destination is compared with every retained input using platform
file identity. Direct source paths and hard-link aliases are rejected before any
output mutation. These checks are portable normal-operation checks, not a
race-free adversarial guarantee.

## Publication guarantee

Each input maps to the validated basename retained from its original argument
inside the canonical output directory. The controller creates a unique internal
file there with create-new semantics, writes the complete selected bytes, and
independently validates the completed file.

Publication renames that internal file to the requested destination. Because
both paths are in the same directory, publication does not cross filesystems.
The operation creates an absent destination or replaces the entry occupying the
destination at that moment. A destination that appears or changes after
preflight may therefore be replaced. ImgLean does not follow or modify the
contents of a destination symlink; it authorizes the requested pathname and does
not claim protection against hostile concurrent path manipulation.

If preparation or rename fails, ImgLean removes its internal file when possible
and reports a per-input failure. An existing destination remains the applicable
output unless the rename succeeds. After successful rename, the requested path
refers to the complete validated winner and the former destination has been
replaced. No partial candidate is exposed around the publication point.

Publication is atomic visibility on the qualified target filesystems, not crash
durability. ImgLean does not create backups, synchronize file or directory data,
or roll back outputs replaced earlier in the invocation.

## Internal artifacts and metadata

Internal files use collision-resistant names distinct from requested
destinations and are opened with create-new semantics. Their names and
permissions are not a confidentiality boundary. The controller tracks every
internal pathname created by the current invocation and removes it after
handled failure when possible. It never deletes an earlier artifact merely
because its name resembles an ImgLean temporary name.

Outputs receive the ownership, permissions, access-control entries, timestamps,
flags, and other metadata produced by ordinary new-file creation on the current
platform and filesystem. ImgLean does not copy metadata from the source or the
replaced destination and does not intentionally make an output executable or
read-only.

## Failure boundary

Before successful rename, ImgLean has not changed the requested destination for
that input. Source validation, provider, preparation, or publication failure
therefore preserves an existing output. After successful rename, the new output
remains committed even if required reporting fails or a later input fails.
Abnormal termination may leave a complete internal file before publication;
crash cleanup and crash durability are not promised.
