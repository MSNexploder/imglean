# Version 0.6 JPEG Contract

ImgLean accepts files named with a case-insensitive `.jpg` or `.jpeg` extension
when their bytes are one bounded, completely decodable JPEG image in the subset
below. The same validator is applied independently to sources, provider
candidates, and completed internal outputs.

## Accepted subset

- SOI through one terminal EOI, with no trailing bytes.
- 8-bit baseline sequential, extended sequential, or progressive Huffman frames
  (`SOF0`, `SOF1`, or `SOF2`).
- One frame with one, three, or four components.
- Dimensions up to the common per-dimension, pixel-count, and reconstructed-byte
  limits.
- Bounded marker count, segment framing, entropy-coded scans, and complete strict
  decode.
- Opaque application and comment segments within the ancillary-byte limit,
  except for the refusals below.

Arithmetic, lossless, differential, hierarchical, multiple-frame, non-8-bit,
malformed, truncated, trailing-data, and incompletely decodable JPEGs are
rejected.

## Metadata and candidate gate

Standard and extended XMP APP1 identifiers are refused. APP11 is refused in full
as the conservative C2PA/JUMBF boundary, and both documented adjacent `.c2pa`
sidecar forms are refused during source capture. Other accepted application and
comment payloads are opaque; ImgLean does not parse ICC, Exif, JFIF, or vendor
metadata.

A candidate must pass the same subset and have the source dimensions. ImgLean
does not prove pixel equality, metadata identity, color-management equivalence,
or perceptual quality. Numeric-quality transformations are trusted to the
explicit provider adapter. The validated source remains the baseline, so a
larger candidate never wins.

The lossless jpegtran adapter copies all extra markers by default and
transcodes existing JPEG coefficients without requantization. With
`--strip-metadata`, it instead requests no extra-marker copying. Numeric-quality
JPEG adapters re-encode and may drop application metadata. CI exercises these
native behaviors with an Exif-bearing input, while the common candidate gate
intentionally does not parse, compare, or verify removal of Exif payloads.

## Bounded validation

Container inspection checks all segment lengths and scan marker transitions
under the common encoded, marker-count, ancillary, dimension, pixel, decoded
allocation, and elapsed-time limits. A strict `zune-jpeg` decode then proves the
accepted image data can be fully reconstructed within those bounds. The
versioned corpus in `tests/corpus/jpeg/v1` covers accepted encodings, dimension
change, XMP, APP11, truncation, trailing bytes, invalid scan structure, and a
real-provider reduction fixture.
