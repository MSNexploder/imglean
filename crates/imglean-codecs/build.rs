use std::env;
use std::path::Path;

#[path = "../native_sources.rs"]
mod native_sources;

fn main() {
    let sources = native_sources::source_root();
    let png = sources.join("libpng");
    let png_config = std::path::PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::copy(
        png.join("scripts/pnglibconf.h.prebuilt"),
        png_config.join("pnglibconf.h"),
    )
    .expect("could not prepare libpng configuration");
    let mut png_build = cc::Build::new();
    png_build.warnings(false).include(&png).include(&png_config);
    let zlib_include = env::var_os("DEP_Z_INCLUDE").expect("libz-sys include path");
    png_build.include(&zlib_include);
    for file in [
        "png", "pngerror", "pngget", "pngmem", "pngpread", "pngread", "pngrio", "pngrtran",
        "pngrutil", "pngset", "pngtrans", "pngwio", "pngwrite", "pngwtran", "pngwutil",
    ] {
        png_build.file(png.join(format!("{file}.c")));
    }
    if matches!(
        env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
        Ok("arm" | "aarch64")
    ) {
        for file in [
            "arm_init",
            "filter_neon_intrinsics",
            "palette_neon_intrinsics",
        ] {
            png_build.file(png.join(format!("arm/{file}.c")));
        }
    }
    // Emit libpng after OptiPNG so static linkers resolve its dependent symbols.
    png_build.cargo_metadata(false).compile("imglean_png");
    let vendor = Path::new("vendor/optipng");
    let mut build = cc::Build::new();
    build
        .warnings(false)
        .include(vendor.join("src/optipng"))
        .include(vendor.join("src/opngreduc"))
        .include(vendor.join("src/pngxtern"))
        .include(vendor.join("third_party/cexcept"));

    build
        .include(&png)
        .include(&png_config)
        .include(&zlib_include);

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
    println!("cargo:rustc-link-lib=static=imglean_png");

    println!("cargo:rerun-if-changed={}", vendor.display());
    println!("cargo:rerun-if-env-changed=DEP_Z_INCLUDE");

    // Retain the old wrapper's optimized native dev/test build and MSVC CRT.
    let profile = if env::var("PROFILE").as_deref() == Ok("release") {
        "Release"
    } else {
        "RelWithDebInfo"
    };
    let prefix = cmake::Config::new(sources.join("jpegli"))
        .out_dir(std::path::PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("jpegli"))
        .profile(profile)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("BUILD_TESTING", "OFF")
        .define("JPEGLI_ENABLE_TOOLS", "OFF")
        .define("JPEGLI_ENABLE_SKCMS", "ON")
        .define("JPEGLI_ENABLE_DEVTOOLS", "OFF")
        .define("JPEGLI_ENABLE_DOXYGEN", "OFF")
        .define("JPEGLI_ENABLE_MANPAGES", "OFF")
        .define("JPEGLI_ENABLE_BENCHMARK", "OFF")
        .define("JPEGLI_ENABLE_EXAMPLES", "OFF")
        .define("JPEGLI_ENABLE_JNI", "OFF")
        .define("JPEGLI_ENABLE_SJPEG", "OFF")
        .define("JPEGLI_ENABLE_OPENEXR", "OFF")
        .define("JPEGLI_ENABLE_JPEGLI_LIBJPEG", "OFF")
        .define("CMAKE_DISABLE_FIND_PACKAGE_JPEG", "TRUE")
        .define("JPEGLI_LIBJPEG_LIBRARY_SOVERSION", "8")
        .define("JPEGLI_LIBJPEG_LIBRARY_VERSION", "8.2.2")
        .build_target("jpegli-static")
        .build()
        .join("build");
    println!(
        "cargo:rustc-link-search=native={}",
        prefix.join("lib").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        prefix.join("third_party/highway").display()
    );
    println!("cargo:rustc-link-lib=static=jpegli-static");
    println!("cargo:rustc-link-lib=static=hwy");
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => println!("cargo:rustc-link-lib=c++"),
        Ok("windows") => {}
        _ => println!("cargo:rustc-link-lib=stdc++"),
    }
}
