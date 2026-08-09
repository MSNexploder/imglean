# Version 1 JPEG corpus

This generated, bounded corpus defines ImgLean's initial JPEG validator boundary.
It covers baseline, progressive, and grayscale Huffman JPEGs, candidate dimension
changes, truncated and trailing data, invalid scan structure, standard XMP APP1,
and the conservative APP11/C2PA refusal. `provider-reduction.jpg` is also used by
real-provider CI to require a smaller candidate from each JPEG adapter.

Regenerate it with `mise exec -- python tools/generate_jpeg_corpus.py`.
