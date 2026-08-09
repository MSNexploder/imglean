# Bundled libwebp Strategy

`libwebp-v1` is enabled for WebP at lossless and numeric quality. The bundled
strategy uses libwebp 1.6.0's advanced API with method 6, alpha quality 100,
one thread, and exact transparent RGB. Lossless quality selects lossless preset
9; numeric quality passes native `Q` and permits lossy color coding.

The source is decoded by image-webp. ICC and Exif chunks are attached through
libwebpmux by default and omitted with `--strip-metadata`; XMP sources are
already refused. The native API is isolated in the disposable worker behind a
documented FFI safety contract.

`--provider libwebp PATH` accepts a capability-compatible `cwebp` override. It
maps the same lossless/numeric choice, method 6, alpha quality 100, and
`-metadata all|none`; lossless additionally receives `-exact`. Reported version
text is not gated. libwebp's BSD-3-Clause terms and patent grant are included
in release notices.
