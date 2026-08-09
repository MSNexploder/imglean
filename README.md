# ImgLean

ImgLean is a focused CLI for making existing PNG, JPEG, WebP, and AVIF files
smaller without changing their format or dimensions. Its bundled workflow runs
entirely offline.

Give it one or more images and it runs the applicable optimizers, validates
their results, and keeps the smallest accepted candidate. The original file is
always a candidate, so successful output is never larger than its source.

## Design intent

Image optimization is fragmented across codecs, tools, APIs, and
provider-specific flags. ImgLean provides one stable command and policy across
those implementations for people, scripts, CI, and coding agents.

It is intentionally narrow:

- same-format optimization only;
- lossless by default, with explicit opt-in for numeric quality;
- no resizing, cropping, rotation, or format conversion;
- no source replacement; and
- an offline bundled workflow with no provider downloads.

ImgLean removes avoidable encoding overhead within the selected format and
quality policy. It does not try to become an image editor or asset pipeline.

## Install

On macOS 15 or newer, install the qualified Apple Silicon or Intel binary with
Homebrew:

```sh
brew install MSNexploder/tap/imglean
```

Target-specific archives for macOS, Linux, and Windows are available from
[GitHub Releases](https://github.com/MSNexploder/imglean/releases/latest). The
first release qualifies macOS 15 on Apple Silicon and Intel, Ubuntu 24.04 on
x86-64, and Windows Server 2025 on x86-64.

The minimal `linux/amd64` container is published separately:

```sh
docker pull ghcr.io/msnexploder/imglean:v0.6.0
```

## Use

Create an output directory and write lossless results into it:

```sh
mkdir -p optimized
imglean --output optimized photo.jpg icon.png hero.webp cover.avif
```

The output directory must already exist. A same-named regular file may be
replaced, but only after the complete winner has been validated.

Explicitly allow strategies that use native numeric quality controls:

```sh
imglean --quality 80 --output optimized photo.jpg hero.webp cover.avif
```

Check whether tracked assets could be reduced without writing output:

```sh
imglean --check public/logo.png public/hero.jpg
```

`--check` exits with status `4` when at least one smaller candidate exists.
This makes it suitable for repository checks: the caller decides how files are
discovered and whether optimized output should replace tracked assets.

Exit statuses are `0` for clean success, `1` for processing failure, `2` for
invalid usage, `3` for completed work with optimizer warnings, and `4` for a
reduction found in check mode.

Useful controls include:

- `--quality lossless|1..100` selects the fidelity policy;
- `--strip-metadata` asks providers to remove metadata when they support it;
- `--jobs N` controls strategy concurrency;
- `--timeout SECONDS` controls the per-strategy deadline; and
- `--disable-strategy`, `--require-strategy`, and `--provider` provide
  explicit strategy control.

Metadata removal is best effort: ImgLean delegates it to each strategy and does
not implement a separate metadata stripper. Run `imglean --help` for the
complete CLI and current strategy identifiers.

## What ImgLean guarantees

- Inputs keep their format and dimensions.
- Source files are never replaced.
- Lossless is the default; lossy strategies require a numeric quality.
- The original participates in selection, preventing larger successful output.
- Every source and candidate must pass bounded format validation.
- Selection and reporting use stable strategy order.
- The bundled workflow stays offline.

ImgLean validates structure, complete decoding, dimensions, and the documented
metadata safety gates. It trusts each audited strategy configuration for its
fidelity behavior; it does not independently compare every decoded pixel or
promise the globally smallest representation across formats.

Exact guarantees and accepted format subsets are defined in
[SCOPE.md](SCOPE.md) and the [focused contracts](docs/contracts/).

## Formats and strategies

All applicable bundled strategies are enabled by default:

- PNG uses OxiPNG and OptiPNG for lossless optimization. An installed pngquant
  can add numeric-quality palette reduction.
- JPEG uses jpegtran for lossless optimization and MozJPEG plus Jpegli at
  numeric quality.
- WebP uses libwebp and image-webp, with lossless and numeric-quality behavior
  where supported.
- AVIF uses independent libaom- and rav1e-based strategies at numeric quality.

pngquant is the only automatically discovered external tool because its
licensing prevents bundling it into the Apache-2.0 binary. Supported providers
can also be explicitly overridden for testing or controlled substitution.
External providers are ordinary executables running with the user's authority;
ImgLean does not treat them as sandboxed or control their network behavior.

Provider-specific behavior is documented under
[docs/providers](docs/providers/).

## Development

ImgLean uses mise to install and select its development toolchain. Contributors
can run the complete local gate with:

```sh
mise install
mise run check
```

`mise run check` is the canonical development gate. It checks formatting,
runs warnings-as-errors linting, executes the full test suite, and verifies
Homebrew formula generation.

Before changing product behavior, read [SCOPE.md](SCOPE.md). Before changing
system structure, read [ARCHITECTURE.md](ARCHITECTURE.md). Coding agents should
also follow [AGENTS.md](AGENTS.md).

## Documentation

- [SCOPE.md](SCOPE.md) explains the product boundary and promises.
- [ARCHITECTURE.md](ARCHITECTURE.md) explains the stable system shape.
- [docs/contracts](docs/contracts/) contains exact format, provider, filesystem,
  and limit contracts.
- [docs/providers](docs/providers/) records provider-specific settings.
- [docs/RELEASE.md](docs/RELEASE.md) defines release qualification and
  artifacts.

## License

ImgLean is licensed under Apache-2.0. See [LICENSE.md](LICENSE.md).
