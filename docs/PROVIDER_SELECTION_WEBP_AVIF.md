# WebP and AVIF Provider Selection

This record captures the implementation review performed on 2026-08-08 for
the version 0.6 WebP and AVIF extension. The selection criteria were production
use, active maintenance, permissive default-binary licensing, static builds on
the three release targets, a direct byte-to-byte integration, and a genuinely
different encoder implementation where practical.

## Selected WebP implementations

`libwebp-v1` uses Google's reference libwebp 1.6.0 through
`libwebp-sys` 0.14.4. libwebp is the mature reference encoder/decoder and
supports both lossy and lossless WebP. The strategy uses the advanced API
because libwebp's simple lossless API does not preserve RGB values beneath full
transparency by default. It pins `exact = 1`, method 6, lossless preset 9,
lossless alpha, and one provider thread. `cwebp` is supported as a
capability-probed override because it accepts WebP input and exposes equivalent
controls.

`image-webp-v1` uses image-webp 0.2.4 as an independent, safe-Rust lossless
encoder and as the controller decoder. Its encoder is intentionally simpler
and usually compresses less effectively than libwebp, but it is maintained,
contains no unsafe code, accepts the full static WebP decode surface, and gives
the race a meaningfully independent implementation at little integration cost.

Other Rust wrappers over libwebp were not selected as separate strategies:
they would race the same encoder and some expose only libwebp's simple lossless
API, whose default transparent-RGB behavior does not meet this strategy's
configuration. They add no useful candidate diversity.

## Selected AVIF implementations

`avif-aom-v1` uses libavif with libaom, the reference AV1 implementation.
libavif provides the production AVIF container API and explicitly supports
libaom, rav1e, and SVT-AV1 encoder backends. The locked Rust integration is
`libavif` 0.14.0, which vendors libavif 1.0.4 and libaom 3.11.0. This is older
than upstream libavif 1.4.2, so the exact locked native revisions are recorded
as a release fact and must continue to pass dependency/advisory review.

`avif-rav1e-v1` uses ravif 0.13.0 and rav1e 0.8.1. rav1e is an actively
maintained AV1 encoder with a still-picture mode and weekly prereleases. This
path is a genuinely independent AV1 encoder and stays portable without adding
another C/C++ build system. It encodes decoded RGBA as 8-bit AVIF with native
quality `Q`, alpha quality 100, speed 6, unassociated dirty alpha, and one
thread.
rav1e 0.8.1 still depends on the unmaintained compile-time `paste` macro crate;
because no safe upstream upgrade exists, `deny.toml` records a narrow advisory
exception. This is a tracked supply-chain caveat, not a runtime vulnerability.

Neither selected AVIF strategy claims lossless applicability. AV1 lossless
coding alone does not make the complete RGB-to-YUV/container round trip
sample-identical, and ravif explicitly exposes only its numeric quality model.
At `--quality lossless`, both AVIF strategies are `not applicable` and the
unchanged baseline wins.

## Production-ready alternatives not selected

- SVT-AV1 is actively maintained and describes itself as a production-quality
  AV1 encoder. It is a strong future third AVIF engine, but adding its large
  native stack does not improve the first milestone enough to justify the
  binary size, build matrix, and audit surface when libaom and rav1e already
  provide independent candidates.
- libheif is mature and supports libaom, rav1e, and SVT-AV1, but the library is
  LGPL. Linking it into the default Apache-2.0 executable conflicts with the
  project's permissive-only distribution boundary. Its command-line tools also
  do not provide a direct AVIF-to-AVIF optimizer contract needed by the current
  adapter protocol.
- `avifenc` and `cavif` are production command-line encoders, but their public
  interfaces consume PNG, JPEG, or Y4M rather than AVIF. Supporting them as
  external overrides would require a new decoded-intermediate protocol and
  color-management contract, so they are not callable adapters in this
  milestone.
- Experimental pure-Rust AVIF decoders/containers were not selected for the
  acceptance gate. The current bounded ISO-BMFF inspection plus libavif full
  decode is the smaller production-backed implementation.

## Primary references

- [libwebp repository and 1.6.0 release identity](https://github.com/webmproject/libwebp)
- [libwebp advanced encoding API](https://developers.google.com/speed/webp/docs/api)
- [cwebp option contract](https://developers.google.com/speed/webp/docs/cwebp)
- [image-webp implementation status](https://github.com/image-rs/image-webp)
- [libavif repository and codec model](https://github.com/AOMediaCodec/libavif)
- [libavif releases](https://github.com/AOMediaCodec/libavif/releases)
- [rav1e implementation and release cadence](https://github.com/xiph/rav1e)
- [ravif/cavif integration](https://github.com/kornelski/cavif-rs)
- [SVT-AV1 production scope](https://gitlab.com/AOMediaCodec/SVT-AV1)
- [libheif codec and license boundaries](https://github.com/strukturag/libheif)
