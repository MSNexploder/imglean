# External jpegtran Strategy

`jpegtran-v1` uses a separately installed compatible `jpegtran` executable for
JPEG inputs at lossless or numeric quality. It is not bundled, downloaded,
installed, updated, or included in ImgLean's SBOM.

Discovery runs `jpegtran -help` and requires every CLI capability used by the
adapter: copying all or no extra markers, Huffman optimization, progressive
output, strict input handling, and an explicit output path. The historical help path
exits with status 1 after printing valid help; ImgLean accepts that status only
when all required markers are present. No release number is requested, parsed,
or gated.

The adapter invokes:

```text
jpegtran -copy all -optimize -progressive -strict -outfile CANDIDATE PRIVATE_INPUT
```

With `--strip-metadata`, `-copy none` replaces `-copy all`.

jpegtran transcodes JPEG coefficients without decoding and requantizing image
samples. It copies all recognized extra markers by default and omits them when
stripping is requested. It optimizes Huffman tables and writes progressive
scans. The controller independently applies the common JPEG
candidate gate and keeps the result only when it is strictly smaller than the
current winner.

jpegtran-compatible implementations also expose linkable native coefficient
APIs. ImgLean uses the maintained CLI because it preserves the existing process
crash/timeout boundary without unsafe FFI or a native build dependency. The
provider remains separately installed software outside ImgLean's bundled
dependency inventory. CI builds jpegtran from the pinned representative
MozJPEG and libjpeg-turbo revisions in `ci/mozjpeg-revision.txt` and
`ci/libjpeg-turbo-revision.txt` on every release target, requires a real
reduction, and verifies that an Exif marker is retained.
The same real-provider test verifies that the marker is omitted when stripping
is requested.
