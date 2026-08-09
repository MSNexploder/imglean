# Version 0.2 Output Contract

> [!IMPORTANT]
> This is the implemented version 0.2 output contract.

## Boundary

This document defines the portable output guarantees for version 0.2. It uses common file and hard-link operations rather than an operating-system-specific filesystem backend. It protects normal local CLI operation but does not promise that paths remain bound to the same directories when another process concurrently renames or replaces path components.

## Required filesystem behavior

The user supplies an existing output directory. Structural preflight resolves it to an absolute canonical path, verifies that it is a directory, checks the complete destination mapping, and requires every destination pathname to contain no filesystem entry, including a dangling symlink.

For each processed input, the output directory must support creating a regular temporary file and a second hard link to that file within the same directory. ImgLean does not perform a mutating capability probe during structural preflight. If the required hard-link operation is unsupported or fails, that input fails without an ImgLean-created destination; earlier committed outputs remain valid and later inputs continue.

Version 0.2 does not claim identical behavior on every filesystem, protection against hostile concurrent path manipulation, or support for filesystems without hard links. Release testing covers the documented release targets and representative native filesystems.

## Publication guarantee

Each input maps to the validated basename retained from its original command-line argument within the canonical output-directory path. Canonicalizing the source does not change that destination name. ImgLean creates a unique internal file in the output directory using atomic create-new semantics, writes the complete selected bytes, and independently validates the completed file before publication.

ImgLean publishes the result by creating the requested destination as a hard link to that internal file. Hard-link creation must fail when any filesystem entry already occupies the destination. ImgLean never implements publication as an existence check followed by an overwriting rename. A destination that appears after structural preflight is therefore not replaced.

After successful hard-link creation, the requested destination refers to the complete validated file. ImgLean then removes the internal name. Failure to remove that name after publication does not invalidate the destination, but it is a handled filesystem failure: ImgLean reports it, exits `1`, and does not claim successful cleanup.

Hard-link publication is atomic visibility, not crash durability. ImgLean neither controls nor claims knowledge of destination entries created, removed, or changed by other processes after publication.

## Internal artifacts and metadata

Internal files use collision-resistant names distinct from every requested destination and are opened with create-new semantics. Their names and permissions are not a confidentiality boundary. Outputs receive the ownership, permissions, access-control entries, timestamps, flags, and other metadata produced by ordinary new-file creation on the current platform and filesystem. ImgLean does not copy or normalize source filesystem metadata.

The destination hard link refers to the same filesystem object as the prepared internal file and therefore has that object's metadata. Exact metadata can differ across operating systems, filesystems, user configuration, inherited access-control rules, and process umask. Version 0.2 promises only that ImgLean does not intentionally make an output executable or read-only; release tests record observed behavior on supported targets.

The controller tracks every internal pathname created by the current invocation and removes it after handled success or failure when possible. It never deletes an earlier artifact merely because its name resembles an ImgLean temporary name.

## Failure boundary

Before successful hard-link creation, ImgLean has not published a requested destination for that input. A write, validation, metadata, hard-link, or pre-publication cleanup failure leaves no ImgLean-created destination and is a per-input failure. A successful hard-link operation exposes the complete winner, never a partially written candidate.

Abnormal termination before publication may leave an internal file but no requested destination. Abnormal termination after publication may leave both the complete destination and its internal hard link. Crash cleanup and crash durability are not promised. Later inputs may continue after an ordinary per-input failure; invocation-wide and required standard-output reporting failures stop later commits as defined by [INPUT_AND_BATCH.md](INPUT_AND_BATCH.md).
