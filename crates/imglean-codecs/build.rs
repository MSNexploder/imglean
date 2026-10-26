use std::env;
use std::path::Path;

fn main() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("native crate must be inside the workspace")
        .to_path_buf();
    let manifest = root.join("ci/native-sources.json");
    let sources = root.join("target/native-sources");
    assert_eq!(
        std::fs::read(sources.join(".source-manifest.json")).ok(),
        Some(std::fs::read(&manifest).expect("native source manifest is missing")),
        "native sources are missing or stale; run mise run native:prepare"
    );
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", sources.display());
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

    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let aom = cmake::Config::new(sources.join("libaom"))
        .out_dir(output.join("aom"))
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("ENABLE_DOCS", "OFF")
        .define("ENABLE_APPS", "OFF")
        .define("ENABLE_EXAMPLES", "OFF")
        .define("ENABLE_TESTDATA", "OFF")
        .define("ENABLE_TESTS", "OFF")
        .define("ENABLE_TOOLS", "OFF")
        .define("CONFIG_SIZE_LIMIT", "1")
        .define("DECODE_WIDTH_LIMIT", "8192")
        .define("DECODE_HEIGHT_LIMIT", "8192")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();
    let aom_library = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        "aom.lib"
    } else {
        "libaom.a"
    };
    let avif = cmake::Config::new(sources.join("libavif"))
        .out_dir(output.join("avif"))
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("AVIF_BUILD_APPS", "OFF")
        .define("AVIF_BUILD_TESTS", "OFF")
        .define("AVIF_CODEC_AOM", "SYSTEM")
        .define("AVIF_LIBYUV", "OFF")
        .define("AVIF_LIBSHARPYUV", "OFF")
        .define("AVIF_ENABLE_WERROR", "OFF")
        .define("CMAKE_PREFIX_PATH", &aom)
        .define("AOM_INCLUDE_DIR", aom.join("include"))
        .define("AOM_LIBRARY", aom.join("lib").join(aom_library))
        .define("CMAKE_DISABLE_FIND_PACKAGE_PkgConfig", "TRUE")
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .build();
    println!(
        "cargo:rustc-link-search=native={}",
        avif.join("lib").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        aom.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=avif");
    println!("cargo:rustc-link-lib=static=aom");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
}
