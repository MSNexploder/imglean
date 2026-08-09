# Bundled OptiPNG Strategy

`optipng` bundles the OptiPNG 7.9.1 optimization engine and is enabled by
default for PNG input. The native source is pinned in `imglean-codecs`, limited
to the PNG read path required by ImgLean, and linked with a pinned bundled
libpng/zlib. It runs only inside the short-lived provider worker.

The wrapper initializes OptiPNG with optimization level 2, preserves the input
interlace mode, writes only to the private candidate path, and does not request
error repair. With `--strip-metadata`, it enables OptiPNG's native `strip_all`
setting. The controller validates the private source before execution and
independently applies the common PNG candidate gate afterward.

An explicit `--provider optipng PATH` replaces the bundled engine for that
invocation. The external adapter capability-probes the executable and invokes:

```text
optipng -quiet -o2 -out CANDIDATE -- PRIVATE_INPUT
```

With metadata stripping it adds `-strip all`. Provider release text is not a
compatibility gate.

OptiPNG and its Cexcept helper are zlib-licensed; libpng and zlib use their
permissive licenses. Exact sources and required notices are recorded in the
dependency lock, release manifest, SBOM, and third-party notices. Native CI
executes the bundled strategy directly and through the controller, and also
tests a pinned representative external override on every release target.
