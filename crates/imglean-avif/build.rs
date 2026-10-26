#[path = "../native_sources.rs"]
mod native_sources;

fn main() {
    let sources = native_sources::source_root();
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
