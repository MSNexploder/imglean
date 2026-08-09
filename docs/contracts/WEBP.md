# Version 0.6 WebP Contract

ImgLean accepts bounded static lossy, lossless, simple, and extended WebP files
with optional alpha, ICC, and Exif data. The `.webp` suffix is a path
requirement, not format evidence. The validator checks the RIFF declaration,
complete chunk framing and padding, absence of trailing bytes, resource limits,
and a complete image-webp decode.

`ANIM` or `ANMF` refuses animation. `XMP ` refuses standard WebP XMP and `C2PA`
refuses an embedded C2PA manifest. Adjacent `.c2pa` files are refused by the
input contract. Other accepted chunks, including ICC and Exif, are opaque.

Every candidate must independently pass the same gate and match the source
width and height. ImgLean does not compare decoded samples, transparent RGB,
ancillary identity, or perceptual quality. The two lossless strategies are
configured to preserve samples, including RGB beneath full transparency;
numeric libwebp intentionally permits fidelity reduction.

`libwebp` and `image-webp` preserve ICC and Exif by default and omit them
when `--strip-metadata` is requested. The controller does not independently
verify their removal, and the unchanged baseline remains eligible.

The checked-in v1 corpus covers a static alpha image, Exif, changed dimensions,
animation markers, XMP, C2PA, truncation, trailing bytes, and provider
reductions. Exact shared limits are in [LIMITS.md](LIMITS.md).

References: [WebP container specification](https://developers.google.com/speed/webp/docs/riff_container),
[WebP API](https://developers.google.com/speed/webp/docs/api), and
[C2PA WebP embedding](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html#_webp).
