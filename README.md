# ImgLean

Make images lean.

ImgLean is a planned local command-line tool whose built-in workflow attempts to make supported images smaller without requiring separately installed optimizer tools. No component of that workflow initiates network requests or remote-service calls. Every optimizer result is independently checked, and no committed output is larger in encoded bytes than its source. On successful processing, when no smaller valid result is found, ImgLean writes the validated source image bytes unchanged.

The project takes inspiration from ImageOptim's low-friction workflow while aiming for permissively licensed, target-specific executables that require no separately installed optimizer or runtime.

> [!IMPORTANT]
> ImgLean is currently project scaffolding. The interface and behavior below are planned and are not implemented yet.

## Planned version 0.1

```sh
imglean --output ./optimized photo.png icon.png
```

Version 0.1 is planned as a supported, releasable CLI for 64-bit macOS, Linux, and Windows, a limited subset of non-animated PNG, and one bundled OxiPNG strategy. It accepts explicit input files, writes separate outputs without overwriting existing files, and never writes source contents or replaces source directory entries. The output directory must support hard links: ImgLean prepares each complete result under a temporary name and publishes it by creating the requested destination as a non-replacing hard link.

The source is validated, and every optimizer-produced candidate is independently checked under the same fixed strict-lossless rules. Those rules preserve decoded image content and embedded payloads accepted by the PNG format policy while allowing encoding bytes to change; source filesystem metadata is outside the guarantee. Invalid optimizer output is rejected, and an optimizer failure does not discard the valid source. Exact format, input, and output boundaries are documented separately so the high-level project definition can remain stable.

Broader platforms, formats, quality policies, workflows, and optional providers may be considered after this core optimization race is demonstrated; they are not committed roadmap items.

## Documentation

- [SCOPE.md](SCOPE.md) defines the product outcomes and milestone boundary.
- [ARCHITECTURE.md](ARCHITECTURE.md) defines the system shape and data flow.
- The [input and batch](docs/contracts/INPUT_AND_BATCH.md), [PNG](docs/contracts/PNG.md), and [output](docs/contracts/OUTPUT.md) contracts define detailed planned version 0.1 behavior.
- [AGENTS.md](AGENTS.md) contains implementation guidance.

## Development

ImgLean uses mise to install the latest stable Rust release and its required components. Install the project toolchain, then run the canonical development gate:

```sh
mise install
mise run check
```

Use `mise run format` to apply formatting and `mise run dev` to run the development CLI. The check task verifies formatting, runs Clippy with warnings denied, uses the committed Cargo lockfile, and runs the complete test suite.

## License

ImgLean is licensed under the Apache License 2.0. See [LICENSE.md](LICENSE.md).
