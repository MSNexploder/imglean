# Bundled libavif/libaom Strategy

`avif-aom-v1` is enabled for AVIF only at numeric quality. It uses libavif
0.14.0 with the vendored libavif 1.0.4 container library and libaom 3.11.0 AV1
codec. The strategy pins native quality `Q`, alpha quality 100, speed 6, and one
thread.

The decoded libavif image is re-encoded directly. The selected API exposes no
metadata-removal control, so the strategy emits its normal output and remains
eligible when `--strip-metadata` is requested. The flag is best effort and does
not guarantee metadata removal.

No external override is exposed: `avifenc` does not accept AVIF input, and a
decoded-intermediate adapter is outside the current provider protocol. The
BSD-2-Clause licenses and the Alliance for Open Media patent terms are included
in the release audit and notices.
