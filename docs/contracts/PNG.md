# Version 0.3 PNG Contract

> [!IMPORTANT]
> This is the implemented static-PNG validation and acceptance contract.

## Accepted encoding surface

ImgLean accepts bounded, non-animated PNG using the standard compression and
filter methods. All PNG-defined static color-type and bit-depth combinations
are eligible: packed and 8/16-bit grayscale, 8/16-bit truecolor, 1/2/4/8-bit
indexed color, 8/16-bit grayscale-alpha, and 8/16-bit truecolor-alpha. Both
non-interlaced and Adam7 images are supported.

The `.png` input suffix is a path requirement, not format evidence. The
validator checks the PNG signature, chunk framing and CRCs, structure needed by
the decoder, image-data decompression and filtering, a complete decoded frame,
IEND, and absence of trailing bytes. Repairable errors are rejected rather than
repaired.

## Explicit policy refusals

`acTL`, `fcTL`, or `fdAT` rejects the file because version 0.3 does not process
APNG. `caBX` rejects C2PA-bearing PNG. A `tEXt`, `zTXt`, or `iTXt` chunk whose
keyword is `XML:com.adobe.xmp` rejects standard XMP. Adjacent external C2PA
manifests are refused by the input contract. No remote lookup occurs.

Other ancillary payloads are opaque after bounded container checks. In
particular, ImgLean does not parse or normalize ICC, Exif, text, XML, language
tags, or private ancillary semantics. The PNG decoder is instructed not to
inflate text or ICC payloads; the image data itself must still decode fully.

## Candidate gate

The source and every provider result are validated independently. A candidate
may compete only when it:

- is within the candidate encoded-byte limit;
- passes the complete static-PNG gate above;
- has exactly the source width and height; and
- is strictly smaller than the current winner.

The first three conditions determine acceptance; the last determines whether an
accepted candidate replaces the winner. A valid larger or equal-size candidate
is normal and does not warn.

ImgLean does not compare decoded samples, RGB beneath full transparency, chunk
order, or ancillary payload identity. Those transformations remain opaque
provider output. Losslessness and metadata preservation are therefore claims of
the audited, versioned provider configuration. No version 0.3 strategy is
configured to repair errors, reduce fidelity, deliberately strip metadata,
alter fully transparent RGB, or force an interlace change. Lossless
representation reductions performed by an audited provider are permitted.

## Bounds and corpus

Encoded size, dimensions, total pixels, decoder allocation, chunk size/count,
total ancillary payload, and validation time are bounded as documented in
[LIMITS.md](LIMITS.md). The checked-in v2 corpus explicitly covers every static
color-type/bit-depth class, Adam7, common and private ancillary chunks,
malformed/truncated files, APNG, XMP, `caBX`, changed pixels and dimensions, and
provider reductions.

## References

- [PNG Specification, Third Edition](https://www.w3.org/TR/png-3/)
- [C2PA Technical Specification 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
