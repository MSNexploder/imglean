# Bundled jpegtran Strategy

`jpegtran` is enabled by default for JPEG input at lossless and numeric
quality. It uses the coefficient-transcoding API bundled by `mozjpeg-sys`: the
source coefficients are copied without pixel decoding or requantization,
Huffman coding is optimized, and progressive scans are written. This preserves
the strategy's lossless sample semantics while allowing a smaller byte stream.

By default the bundled implementation copies recognized application and comment
markers. With `--strip-metadata`, it requests no marker copying. All native
calls live behind the audited `imglean-codecs` FFI boundary and run only in the
short-lived provider worker; fatal codec errors therefore cannot crash the
controller. The controller still applies the common JPEG candidate gate.

An explicit `--provider jpegtran PATH` replaces the bundled implementation. The
external adapter capability-probes the executable and invokes:

```text
jpegtran -copy all -optimize -progressive -strict -outfile CANDIDATE PRIVATE_INPUT
```

With metadata stripping, `-copy none` replaces `-copy all`. Provider release
text is not a compatibility gate.

The bundled implementation shares MozJPEG's permissive IJG, BSD-3-Clause, and
zlib licensing. Release notices and inventories include its native source.
Native CI executes the bundled strategy directly and through the controller and
tests pinned MozJPEG and libjpeg-turbo external overrides on every release
target.
