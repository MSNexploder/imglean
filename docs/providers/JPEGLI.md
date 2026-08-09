# External Jpegli Strategy

`jpegli-v1` uses a separately installed Jpegli `cjpegli` executable for JPEG
inputs at numeric quality. It is not bundled, downloaded, installed, updated,
or included in ImgLean's SBOM.

Jpegli does not expose a stable CLI version command. Discovery therefore runs
`cjpegli --help` and requires advertised JPEG input, JPEG output, numeric
quality, and progressive-level capabilities. No release number is parsed or
gated.

For `--quality Q`, the adapter invokes:

```text
cjpegli --quality Q --progressive_level 2 PRIVATE_INPUT CANDIDATE
```

The native quality value is passed directly and progressive level 2 is pinned.
The controller independently applies the common JPEG candidate gate and keeps
the result only when it is strictly smaller than the current winner.

The CLI re-encodes image samples and does not promise to preserve Exif
orientation or other application metadata. Version 0.6 deliberately treats
that metadata as opaque and permits this behavior only after the user selects
numeric JPEG quality.

Jpegli also exposes libjpeg-compatible and native C++ libraries. ImgLean uses the
maintained CLI because it preserves the existing process crash/timeout boundary
without unsafe FFI or a bundled C++ build. Jpegli is BSD-3-Clause licensed; as
separately installed software it remains outside ImgLean's bundled dependency
inventory. CI builds the pinned representative commit recorded in
`ci/jpegli-revision.txt` on every release target and requires a real reduction.
