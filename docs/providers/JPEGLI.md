# Bundled Jpegli Strategy

`jpegli-v1` is enabled by default for JPEG inputs at numeric quality. ImgLean
bundles the BSD-3-Clause `jpegli` 0.1.0 wrapper, whose `jpegli-sys` package
contains the namespaced Jpegli 0.10.2 native source. Namespaced symbols allow it
to coexist with MozJPEG in one executable. The validated JPEG is decoded to
grayscale or RGB samples and re-encoded with the native quality value,
progressive scans, and optimized entropy coding.

Opaque application and comment markers are copied by default. The existing
JFIF marker is kept once, while a source Adobe marker is not replayed onto the
new encoding. With `--strip-metadata`, the bundled strategy omits saved markers.
This remains provider-native best effort, and the common JPEG candidate gate
independently validates the result. Native code runs only inside the provider
worker.

The Rust wrapper is older than current upstream Jpegli, so it is deliberately
pinned as part of this versioned strategy rather than presented as a floating
latest release. An explicit `--provider jpegli PATH` can test or use a newer
compatible `cjpegli` without rebuilding ImgLean. The override is capability-
probed and invoked as:

```text
cjpegli --quality Q --progressive_level 2 PRIVATE_INPUT CANDIDATE
```

Provider release text is not a compatibility gate. Jpegli and its wrapper are
BSD-3-Clause; the bundled Highway dependency is Apache-2.0. Exact revisions,
the JPEG XL patent grant, and required notices are included in release records.
Native CI executes the bundled strategy directly and through the controller and
tests a pinned representative external override on every release target.
