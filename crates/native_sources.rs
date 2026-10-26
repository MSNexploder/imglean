use std::path::PathBuf;

pub fn source_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
    sources
}
