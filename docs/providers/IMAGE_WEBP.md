# Bundled image-webp Strategy

`image-webp-v1` uses image-webp 0.2.4's safe-Rust lossless encoder. It remains
eligible at numeric quality as an additional lossless candidate and does not
consume the numeric value. ICC and Exif are preserved by default and omitted
when `--strip-metadata` is requested. image-webp has no lossy encoder and no
external override.

The same crate performs controller-side full WebP decoding, but the controller
still performs its own bounded RIFF walk before decode. The crate forbids
unsafe code and is dual MIT/Apache-2.0 licensed.
