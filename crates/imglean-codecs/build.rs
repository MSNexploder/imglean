use std::env;
use std::path::Path;

fn main() {
    let vendor = Path::new("vendor/optipng");
    let mut build = cc::Build::new();
    build
        .warnings(false)
        .include(vendor.join("src/optipng"))
        .include(vendor.join("src/opngreduc"))
        .include(vendor.join("src/pngxtern"))
        .include(vendor.join("third_party/cexcept"));

    let png_includes =
        env::var_os("DEP_PNG_INCLUDE").expect("libpng-sys did not expose its include paths");
    for include in env::split_paths(&png_includes) {
        build.include(include);
    }

    for source in [
        "native/optipng_wrapper.c",
        "vendor/optipng/src/optipng/optim.c",
        "vendor/optipng/src/optipng/bitset.c",
        "vendor/optipng/src/optipng/ioutil.c",
        "vendor/optipng/src/optipng/ratio.c",
        "vendor/optipng/src/opngreduc/opngreduc.c",
        "vendor/optipng/src/pngxtern/pngxmem.c",
        "vendor/optipng/src/pngxtern/pngxset.c",
        "native/pngxread_png_only.c",
    ] {
        build.file(source);
        println!("cargo:rerun-if-changed={source}");
    }
    build.compile("imglean_optipng");

    println!("cargo:rerun-if-changed={}", vendor.display());
    println!("cargo:rerun-if-env-changed=DEP_PNG_INCLUDE");
}
