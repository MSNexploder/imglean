# External OptiPNG Strategy

`optipng-v1` supports exactly the separately installed OptiPNG 7.9.1 executable.
It is not bundled, downloaded, installed, updated, or included in ImgLean's SBOM.
Its resolved canonical path and reported version are recorded for each
invocation in which it is enabled.

The adapter invokes:

```text
optipng -quiet -o2 -out CANDIDATE -- PRIVATE_INPUT
```

Optimization level 2 is explicit. The adapter does not pass `-fix`, `-strip`, an
interlace conversion, or an in-place destination, so error repair, metadata
stripping, forced interlace changes, and source replacement are not requested.
ImgLean validates the private source before invocation and independently applies
its bounded candidate gate afterward.

OptiPNG is zlib-licensed. Because the user supplies it as a separate executable,
its distribution and dependency obligations remain outside the ImgLean binary.
Native CI nevertheless downloads checksum-pinned official 7.9.1 artifacts or
source and tests the complete adapter on every supported release target.
