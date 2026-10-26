# Bundled libavif/libaom Strategy

`avif-aom` is enabled for AVIF only at numeric quality. It uses libavif
through a focused local adapter with libavif 1.4.2 and libaom 3.15.2 AV1
codec. The strategy pins native quality `Q`, alpha quality 100, speed 6, and one
thread. Color and alpha tuning stay explicitly SSIM to preserve the previous
quality mapping.

The decoded libavif image is re-encoded directly. The selected API exposes no
metadata-removal control, so the strategy emits its normal output and remains
eligible when `--strip-metadata` is requested. The flag is best effort and does
not guarantee metadata removal.

No external override is exposed: `avifenc` does not accept AVIF input, and a
decoded-intermediate adapter is outside the current provider protocol. The
BSD-2-Clause licenses and the Alliance for Open Media patent terms are included
in the release audit and notices.
