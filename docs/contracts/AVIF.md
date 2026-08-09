# Version 0.6 AVIF Contract

ImgLean accepts bounded static AVIF files that libavif can fully decode to
RGBA. The `.avif` suffix is a path requirement, not format evidence. The
validator checks complete top-level ISO-BMFF box framing, sizes and counts, an
`ftyp` brand compatible with `avif`, resource limits, and a complete libavif
decode. The bundled libaom decoder is compiled with the same 8,192-pixel width
and height ceilings as the AVIF controller contract so oversized AV1 frames are
refused before their frame-buffer allocation.

The `avis` brand or a top-level `moov` box refuses image sequences. A top-level
`uuid` box beginning with the C2PA UUID
`d8fec3d6-1b0e-483c-9297-5828877ec481` refuses embedded C2PA. The exact AVIF XMP
MIME type `application/rdf+xml` is conservatively refused wherever it appears
in the bounded source. Adjacent `.c2pa` files are refused by the input contract.
Other accepted properties and metadata are opaque.

Every candidate must independently pass the same gate and match the source
width and height. ImgLean does not compare decoded samples, color transforms,
bit depth, chroma subsampling, ancillary identity, or perceptual quality.

Both AVIF strategies require numeric quality. The selected libavif/libaom and
ravif APIs expose no metadata-removal control, so both remain eligible under
either policy and emit their normal containers. The controller does not verify
metadata removal, and the unchanged baseline remains eligible.

The checked-in v1 corpus covers a static alpha image, changed dimensions, XMP,
C2PA, truncation, trailing bytes, and provider reductions. Exact shared limits
are in [LIMITS.md](LIMITS.md).

References: [AV1 Image File Format](https://aomediacodec.github.io/av1-avif/),
[libavif](https://github.com/AOMediaCodec/libavif), and
[C2PA BMFF embedding](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html#_iso_base_media_file_format).
