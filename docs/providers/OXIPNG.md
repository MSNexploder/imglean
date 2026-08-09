# Built-in OxiPNG Strategy

> [!IMPORTANT]
> The controller's independent validator, not OxiPNG, decides candidate
> acceptance.

## Integration

Version 0.1 includes one fixed OxiPNG library strategy. The worker reads the
controller-created private input, calls OxiPNG's in-memory optimization API, and
writes the returned bytes to the reserved candidate path using ImgLean-owned
filesystem code.

The dependency disables OxiPNG's default features. ImgLean does not include the
OxiPNG command-line interface, file-attribute preservation, Rayon parallelism,
or Zopfli in version 0.1. OxiPNG's required libdeflater integration is native
code and is covered by the worker crash boundary, dependency audit, target
builds, and malformed-input corpus.

## Fixed policy constraints

Every behavior-affecting `oxipng::Options` field is assigned explicitly. The
strategy:

- rejects errors rather than repairing them;
- always recodes IDAT so the controller can conduct the race;
- preserves interlace state;
- disables alpha optimization;
- disables bit-depth, color-type, palette, grayscale, and 16-to-8-bit reduction;
- strips no accepted metadata;
- uses one explicit filter set and libdeflater compression level;
- uses the versioned decompressed-size and elapsed-time limits; and
- produces a candidate even when OxiPNG does not consider it an improvement so
  ImgLean, rather than the provider, owns comparison and tie-breaking.

Version 0.1 pins OxiPNG `10.1.1` with default features disabled and assigns:

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
deflater: Libdeflater, compression 11
fast_evaluation: true
timeout: 55 seconds
max_decompressed_size: 256 MiB
```

`force` ensures ImgLean receives the attempted strategy result and owns size
comparison. The baseline still wins an equal-size tie. These choices use the
bounded equivalent of OxiPNG's level-2 filtering and compression behavior while
disabling every representation or metadata transformation outside the version
0.1 equivalence policy.

## Licensing and release record

OxiPNG is MIT-licensed. Release artifacts include its required notice and the
notices for all transitive Rust and native dependencies. The release manifest
records the exact OxiPNG and libdeflater revisions, Cargo features, native
compiler and linker, target, SDK, and build flags.

No built-in provider component initiates a network request or remote-service
call at runtime.
