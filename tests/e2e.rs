use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::Compression;
use flate2::write::ZlibEncoder;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[test]
fn complete_worker_race_publishes_a_smaller_valid_result() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let source = directory.path.join("photo.png");
    let source_bytes = compressible_png();
    fs::write(&source, &source_bytes).unwrap();
    let before = fs::metadata(&source).unwrap();

    let result = run(&output_directory, &[&source]);
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    let destination = output_directory.join("photo.png");
    let output_bytes = fs::read(&destination).unwrap();
    assert!(output_bytes.len() < source_bytes.len());
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    let after = fs::metadata(&source).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert!(stdout(&result).contains("photo.png\n"));
    assert!(stdout(&result).contains("-> "));
    assert!(stdout(&result).contains("winner; saved"));
    assert!(stderr(&result).contains("Summary: 1 succeeded"));
    assert_eq!(fs::read_dir(&output_directory).unwrap().count(), 1);
}

#[test]
fn bundled_jpeg_strategies_run_through_the_controller() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let source = directory.path.join("photo.jpg");
    fs::write(
        &source,
        include_bytes!("corpus/jpeg/v1/accepted/provider-reduction.jpg"),
    )
    .unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--quality")
        .arg("80")
        .arg("--output")
        .arg(&output_directory)
        .arg(&source)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    let report = stdout(&result);
    for strategy in ["jpegtran-v1", "mozjpeg-v1", "jpegli-v1"] {
        assert!(
            report.lines().any(|line| {
                line.contains(strategy) && line.contains(" bytes") && !line.contains("warning:")
            }),
            "bundled strategy did not produce a candidate: {strategy}\n{report}"
        );
    }
    assert!(output_directory.join("photo.jpg").is_file());
}

#[test]
fn metadata_stripping_is_delegated_to_the_winning_strategy() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let source = directory.path.join("metadata.png");
    let source_bytes = png_with_text_metadata();
    fs::write(&source, &source_bytes).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--strip-metadata")
        .arg("--disable-strategy")
        .arg("oxipng-zopfli-v1")
        .arg("--disable-strategy")
        .arg("optipng-v1")
        .arg("--output")
        .arg(&output_directory)
        .arg(&source)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    let candidate = fs::read(output_directory.join("metadata.png")).unwrap();
    assert!(!candidate.windows(4).any(|bytes| bytes == b"tEXt"));
    assert!(stdout(&result).contains("-> oxipng-libdeflate-v1"));
    assert_eq!(fs::read(source).unwrap(), source_bytes);
}

#[test]
fn metadata_stripping_request_does_not_transform_the_baseline() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let source = directory.path.join("metadata.png");
    let source_bytes = png_with_text_metadata();
    fs::write(&source, &source_bytes).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--strip-metadata")
        .arg("--disable-strategy")
        .arg("oxipng-libdeflate-v1")
        .arg("--disable-strategy")
        .arg("oxipng-zopfli-v1")
        .arg("--disable-strategy")
        .arg("optipng-v1")
        .arg("--output")
        .arg(&output_directory)
        .arg(&source)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert_eq!(
        fs::read(output_directory.join("metadata.png")).unwrap(),
        source_bytes
    );
    assert!(stdout(&result).contains("-> baseline"));
}

#[test]
fn jpeg_baseline_uses_format_specific_registry_and_replaces_output() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let source = directory.path.join("photo.JPEG");
    let source_bytes = include_bytes!("corpus/jpeg/v1/accepted/baseline.jpg");
    fs::write(&source, source_bytes).unwrap();
    fs::write(output_directory.join("photo.JPEG"), b"existing").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--disable-strategy")
        .arg("jpegtran-v1")
        .arg("--output")
        .arg(&output_directory)
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert_eq!(
        fs::read(output_directory.join("photo.JPEG")).unwrap(),
        source_bytes
    );
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    let report = stdout(&result);
    assert!(report.contains("-> baseline"));
    for strategy in ["jpegtran-v1", "mozjpeg-v1", "jpegli-v1"] {
        assert!(report.contains(strategy), "missing registry row {strategy}");
    }
    for strategy in [
        "oxipng-libdeflate-v1",
        "oxipng-zopfli-v1",
        "optipng-v1",
        "pngquant-v1",
    ] {
        assert!(
            !report.contains(strategy),
            "unexpected registry row {strategy}"
        );
    }
    for strategy in ["mozjpeg-v1", "jpegli-v1"] {
        assert!(
            report
                .lines()
                .any(|line| line.contains(strategy) && line.contains("not applicable"))
        );
    }
    assert!(
        report
            .lines()
            .any(|line| line.contains("jpegtran-v1") && line.contains("disabled"))
    );
}

#[test]
fn invalid_source_does_not_prevent_a_later_valid_input() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let invalid = directory.path.join("bad.png");
    let valid = directory.path.join("good.png");
    fs::write(&invalid, b"not a PNG").unwrap();
    fs::write(&valid, compressible_png()).unwrap();
    fs::write(output_directory.join("bad.png"), b"existing").unwrap();

    let result = run(&output_directory, &[&invalid, &valid]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        fs::read(output_directory.join("bad.png")).unwrap(),
        b"existing"
    );
    assert!(output_directory.join("good.png").exists());
    assert!(stdout(&result).contains("bad.png\n"));
    assert!(stdout(&result).contains("oxipng-libdeflate-v1     not run"));
    assert!(stdout(&result).contains("  !! failed"));
    assert!(stdout(&result).contains("good.png\n"));
    assert!(stderr(&result).contains("Summary: 1 succeeded, 1 failed"));
}

#[test]
fn existing_destinations_are_replaced() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let first = directory.path.join("first.png");
    let second = directory.path.join("second.png");
    fs::write(&first, compressible_png()).unwrap();
    fs::write(&second, compressible_png()).unwrap();
    fs::write(output_directory.join("first.png"), b"existing").unwrap();

    let result = run(&output_directory, &[&first, &second]);
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));
    assert_ne!(
        fs::read(output_directory.join("first.png")).unwrap(),
        b"existing"
    );
    assert!(output_directory.join("second.png").exists());
}

#[test]
fn non_regular_destination_aborts_the_complete_batch() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let first = directory.path.join("first.png");
    let second = directory.path.join("second.png");
    fs::write(&first, compressible_png()).unwrap();
    fs::write(&second, compressible_png()).unwrap();
    fs::create_dir(output_directory.join("first.png")).unwrap();

    let result = run(&output_directory, &[&first, &second]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(output_directory.join("first.png").is_dir());
    assert!(!output_directory.join("second.png").exists());
    assert!(stderr(&result).contains("structural preflight failed"));
}

#[test]
fn invalid_cli_uses_status_two() {
    let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--unknown")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}

#[test]
fn help_and_version_use_status_zero() {
    for argument in ["--help", "--version"] {
        let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
            .arg(argument)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(0));
        assert!(!result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn required_missing_provider_fails_before_output_creation() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let source = directory.path.join("source.png");
    fs::write(&source, compressible_png()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--provider")
        .arg("optipng")
        .arg(directory.path.join("missing-optipng"))
        .arg("--output")
        .arg(&output_directory)
        .arg(&source)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read_dir(output_directory).unwrap().count(), 0);
    assert!(stderr(&result).contains("provider preflight failed"));
}

fn run(output_directory: &Path, inputs: &[&Path]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_imglean"))
        .arg("--output")
        .arg(output_directory)
        .args(inputs)
        .output()
        .unwrap()
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn compressible_png() -> Vec<u8> {
    let width = 64u32;
    let height = 64u32;
    let mut filtered = Vec::new();
    for _ in 0..height {
        filtered.push(0);
        filtered.extend(std::iter::repeat_n(0, width as usize * 4));
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_chunk(&mut png, b"IHDR", &ihdr);
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::none());
    encoder.write_all(&filtered).unwrap();
    push_chunk(&mut png, b"IDAT", &encoder.finish().unwrap());
    push_chunk(&mut png, b"IEND", &[]);
    png
}

fn png_with_text_metadata() -> Vec<u8> {
    let mut png = compressible_png();
    let iend = png.split_off(png.len() - 12);
    let mut metadata = b"Comment\0".to_vec();
    metadata.extend(std::iter::repeat_n(b'x', 8_192));
    push_chunk(&mut png, b"tEXt", &metadata);
    png.extend_from_slice(&iend);
    png
}

fn push_chunk(png: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
    png.extend_from_slice(name);
    png.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(name);
    crc.update(data);
    png.extend_from_slice(&crc.finalize().to_be_bytes());
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("imglean-e2e-test-{}-{unique}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn create_directory(&self, name: &str) -> PathBuf {
        let path = self.path.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
