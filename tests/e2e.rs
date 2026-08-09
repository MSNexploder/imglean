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
    assert!(stdout(&result).contains("ok "));
    assert!(stderr(&result).contains("1 succeeded, 0 failed"));
    assert_eq!(fs::read_dir(&output_directory).unwrap().count(), 1);
}

#[test]
fn invalid_source_does_not_prevent_a_later_valid_input() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let invalid = directory.path.join("bad.png");
    let valid = directory.path.join("good.png");
    fs::write(&invalid, b"not a PNG").unwrap();
    fs::write(&valid, compressible_png()).unwrap();

    let result = run(&output_directory, &[&invalid, &valid]);
    assert_eq!(result.status.code(), Some(1));
    assert!(!output_directory.join("bad.png").exists());
    assert!(output_directory.join("good.png").exists());
    assert!(stdout(&result).contains("failed "));
    assert!(stdout(&result).contains("ok "));
    assert!(stderr(&result).contains("1 succeeded, 1 failed"));
}

#[test]
fn structural_destination_failure_aborts_the_complete_batch() {
    let directory = TestDirectory::new();
    let output_directory = directory.create_directory("out");
    let first = directory.path.join("first.png");
    let second = directory.path.join("second.png");
    fs::write(&first, compressible_png()).unwrap();
    fs::write(&second, compressible_png()).unwrap();
    fs::write(output_directory.join("first.png"), b"existing").unwrap();

    let result = run(&output_directory, &[&first, &second]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(
        fs::read(output_directory.join("first.png")).unwrap(),
        b"existing"
    );
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
