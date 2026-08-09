# Bundled ravif/rav1e Strategy

`avif-rav1e` is enabled for AVIF only at numeric quality. It decodes to RGBA
through libavif, then uses ravif 0.13.0 and rav1e 0.8.1 with native quality `Q`,
alpha quality 100, speed 6, 8-bit output, unassociated dirty alpha, and one
thread.

ravif does not expose source metadata preservation or a metadata stripping
control, so the strategy emits its normal metadata-free container under both
policies and remains applicable. It is a genuinely independent AV1 encoder
from libaom. No external `cavif` override is exposed because cavif does not
accept AVIF input.
