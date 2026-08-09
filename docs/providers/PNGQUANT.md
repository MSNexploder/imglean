# External pngquant Strategy

`pngquant-v1` supports separately installed pngquant 3.0.2 and 3.0.3. It is not
bundled, downloaded, installed, updated, or included in ImgLean's SBOM. The
strategy participates only when the user selects numeric quality. Discovery
uses `PATH` or an explicit `--provider pngquant PATH` on every platform.

For `--quality Q`, the adapter invokes:

```text
pngquant --force --quality 0-Q --speed 4 --strip --output CANDIDATE -- PRIVATE_INPUT
```

This mapping uses pngquant's native 1–100 scale: lower values permit stronger
color reduction and higher values request higher fidelity. Even quality 100 can
reduce an image to a palette and is not lossless. `--speed 4` fixes the
speed/quality tradeoff, and `--strip` intentionally removes optional metadata.
The baseline and every lossless strategy remain in the race.

ImgLean does not independently score perceptual quality or compare pixels. It
trusts this audited adapter mapping, then independently enforces the same basic
bounded PNG gate used for every candidate. pngquant exit status 99 is treated
as a normal no-candidate result.

pngquant is GPL-licensed with a separately available commercial license.
Because the user supplies it as a separate executable, it does not change the
license or dependency inventory of the Apache-2.0 ImgLean binary. Native CI
builds and exercises both supported versions on every supported release target.
