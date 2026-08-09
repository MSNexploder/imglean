# Version 0.1 PNG Contract

> [!IMPORTANT]
> This is the implemented version 0.1 PNG contract.

## Boundary

This document defines the deliberately limited PNG surface needed to prove ImgLean's optimization race. It uses the [PNG Third Edition](https://www.w3.org/TR/png-3/) container rules but does not claim semantic validation of standards embedded inside ancillary payloads.

## Accepted image encodings

Version 0.1 accepts non-animated, non-interlaced PNG with compression method 0 and filter method 0. It accepts:

- 8-bit grayscale, truecolor, grayscale-with-alpha, and truecolor-with-alpha; and
- indexed-color at 1, 2, 4, or 8 bits.

Sixteen-bit samples and Adam7 are deferred. The input contract requires a `.png` filename, but the validator independently detects and validates PNG from its signature and structure; the extension alone never establishes the format.

The validator checks the signature, chunk framing and CRCs, required critical chunks, ordering, palette references, IDAT zlib stream, scanline filters, reconstructed samples, and absence of trailing bytes. It rejects repairable errors rather than repairing them.

`acTL`, `fcTL`, or `fdAT` rejects the file as APNG. Any chunk type outside the accepted list, including otherwise standard ancillary chunks, is rejected.

## Accepted ancillary chunks

The accepted standard ancillary chunk types are:

```text
tRNS cHRM gAMA sBIT sRGB
tEXt bKGD pHYs tIME
```

The validator enforces the PNG-defined structure, values, multiplicity, ordering, and cross-chunk dependencies of accepted chunks. The `tEXt` payload is opaque after its PNG keyword and framing checks; ImgLean neither normalizes it nor claims semantic validity for its content.

Under the version 0.1 C2PA 2.4 refusal policy, a `tEXt` chunk with the standard XMP keyword `XML:com.adobe.xmp` or any `caBX` chunk rejects the input. The limited subset also rejects `iCCP`, `zTXt`, `iTXt`, and `eXIf`. Together these rules avoid adding ICC, Exif, XML/RDF, or language-tag parsers solely for the first milestone. ImgLean performs no remote lookup.

## Candidate equivalence

A candidate is accepted only when:

- `IHDR` and `PLTE` are byte-for-byte identical;
- every accepted ancillary chunk is byte-for-byte identical and remains in the same order and on the same side of the complete consecutive IDAT group;
- every reconstructed native sample is identical at the stored bit depth, including color samples under full transparency; and
- no critical structure or trailing data changes except the IDAT representation.

Unused low-order padding bits at the end of packed indexed-color scanlines are not image samples and may change. Otherwise, only filtering, deflate bytes, and segmentation within the consecutive IDAT group may change. A bounded controller validator independent of OxiPNG performs this comparison.

## Bounds

Source and candidate validation bounds encoded bytes, dimensions, total pixels,
reconstructed sample storage, chunk size and count, total ancillary bytes,
allocations, image-data decompression work, and validation time.

Exact values and enforcement classifications are version-controlled in
[LIMITS.md](LIMITS.md) and `src/limits.rs` and covered by boundary tests.

## OxiPNG boundary

Every behavior-affecting OxiPNG option and its exact revision must be pinned before enablement. Error repair, representation reductions, alpha optimization, metadata stripping, and interlace changes remain disabled.

A no-improvement result is a normal baseline-only result, not a warning; absent another warning or failure, it contributes to exit `0`.

## Corpus gate

The checked-in corpus covers every accepted color-type and bit-depth combination, palette and transparency cases, each accepted ancillary family, opaque text payloads, fully transparent nonzero color samples, malformed structures, APNG, XMP-bearing `tEXt`, rejected standard ancillary chunks, `caBX`, unknown chunks, trailing data, semantic changes, and at least one validated OxiPNG reduction.

## References

- [PNG Specification, Third Edition](https://www.w3.org/TR/png-3/)
- [C2PA Technical Specification 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
- [OxiPNG options](https://docs.rs/oxipng/latest/oxipng/struct.Options.html)
