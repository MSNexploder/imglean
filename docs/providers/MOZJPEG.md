# Bundled MozJPEG Strategy

`mozjpeg` is enabled by default for JPEG inputs at numeric quality. ImgLean
bundles the `mozjpeg` 0.10.13 Rust wrapper and `mozjpeg-sys` 2.2.3 native source.
The validated JPEG is decoded to grayscale or RGB samples and re-encoded with
the native quality value, progressive scans, optimized Huffman coding, and
MozJPEG scan optimization.

Opaque application and comment markers are copied by default. JFIF density is
carried forward, while JFIF and Adobe structural markers are regenerated to
describe the new encoding instead of being replayed. With `--strip-metadata`,
the bundled strategy omits saved markers. This is provider-native best effort:
the controller does not parse or verify metadata removal. The native codec runs
only inside the provider worker, so codec panics or crashes remain isolated from
the controller.

An explicit `--provider mozjpeg PATH` replaces the bundled implementation. The
external adapter requires MozJPEG's distinguishing CLI capabilities and invokes:

```text
cjpeg -quality Q -progressive -optimize -strict -outfile CANDIDATE PRIVATE_INPUT
```

The external CLI exposes no compatible marker-removal switch, so
`--strip-metadata` does not change that command. Provider release text is not a
compatibility gate.

MozJPEG uses permissive IJG, BSD-3-Clause, and zlib licensing. Its exact wrapper
and native source versions are recorded in the dependency lock, release
manifest, SBOM, and notices. Native CI executes the bundled strategy directly
and through the controller and tests a pinned representative external override
on every release target.
