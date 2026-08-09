# Embedded OxiPNG Strategies

The controller's format validator decides candidate acceptance. OxiPNG 10.1.1
runs only inside the private worker role, with default features disabled except
the explicitly enabled Zopfli backend.

Both embedded strategies share these pinned `oxipng::Options` values:

```text
fix_errors: false
force: true
filters: NONE, SUB, Entropy, Bigrams
interlace: None
optimize_alpha: false
bit_depth_reduction: false
color_type_reduction: false
palette_reduction: false
grayscale_reduction: false
idat_recoding: true
scale_16: false
strip: None
fast_evaluation: true
timeout: 55 seconds
max_decompressed_size: 256 MiB
```

`oxipng-libdeflate-v1` uses libdeflater compression level 11.
`oxipng-zopfli-v1` uses 15 iterations, unlimited iterations without improvement,
and at most 15 block splits. Both are enabled by default and preserve interlace,
representation, alpha values, and ancillary data according to this pinned
configuration. Error repair and metadata stripping are disabled.

`force` makes OxiPNG return its attempted result so ImgLean owns comparison and
tie-breaking. A valid non-improving candidate is normal.

OxiPNG is MIT-licensed, Zopfli is Apache-2.0, and libdeflate is MIT-licensed
native code. Release notices, dependency inventory, SBOM, and manifest record
their exact revisions and enabled Cargo features. No embedded provider
component initiates a network request or remote-service call.
