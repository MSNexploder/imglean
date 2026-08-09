# External MozJPEG Strategy

`mozjpeg-v1` uses a separately installed MozJPEG `cjpeg` executable for JPEG
inputs at numeric quality. It is not bundled, downloaded, installed, updated,
or included in ImgLean's SBOM.

Discovery resolves `cjpeg`, requests its help text, and checks every option used
by the adapter plus MozJPEG's distinguishing `-revert` option. No release number
is requested, parsed, or gated. This prevents an unrelated libjpeg `cjpeg` from
silently claiming the strategy while allowing compatible future MozJPEG
releases.

For `--quality Q`, the adapter invokes:

```text
cjpeg -quality Q -progressive -optimize -strict -outfile CANDIDATE PRIVATE_INPUT
```

The native quality value is passed directly. Progressive encoding, optimized
Huffman coding, and strict warning handling are explicit. The controller then
applies the common JPEG candidate gate and keeps the result only when it is
strictly smaller than the current winner.

The CLI's JPEG-input path re-encodes image samples and copies saved application
markers, including Exif, without exposing a compatible marker-removal option.
`--strip-metadata` therefore adds no MozJPEG argument, and `mozjpeg-v1` remains
applicable. CI verifies that it runs and retains a source Exif marker under this
best-effort policy.

MozJPEG exposes a libjpeg-compatible native library and recommends linking for
graphics applications. ImgLean deliberately uses its CLI because the current
worker contract needs process crash/timeout isolation and linking would add
unsafe FFI plus a native build chain to the default binary. MozJPEG is BSD-style
licensed; as separately installed software it remains outside ImgLean's bundled
dependency inventory. CI builds the pinned representative revision recorded in
`ci/mozjpeg-revision.txt` on every release target and requires a real reduction.
