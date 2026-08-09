use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::artifacts::Artifacts;
use crate::limits::MAX_SOURCE_BYTES;
use crate::png::{ValidatedPng, validate_candidate};

#[derive(Debug)]
pub struct PreparedOutput {
    path: PathBuf,
}

#[derive(Debug, Eq, PartialEq)]
pub enum OutputError {
    BeforePublication(&'static str),
}

pub fn prepare(
    artifacts: &mut Artifacts,
    source: &ValidatedPng,
    winner: &[u8],
) -> Result<PreparedOutput, OutputError> {
    if winner.len() > MAX_SOURCE_BYTES as usize {
        return Err(OutputError::BeforePublication(
            "the selected output exceeds the output-byte limit",
        ));
    }
    let (path, mut file) = artifacts
        .create("output")
        .map_err(|_| OutputError::BeforePublication("cannot create the internal output"))?;
    let result = write_prepared_file(&mut file, winner);
    drop(file);
    let result = result.and_then(|()| verify_prepared_file(&path, source, winner));
    if let Err(error) = result {
        if artifacts.remove(&path).is_err() {
            return Err(OutputError::BeforePublication(
                "cannot clean the failed internal output",
            ));
        }
        return Err(error);
    }
    Ok(PreparedOutput { path })
}

pub fn publish(
    artifacts: &mut Artifacts,
    prepared: PreparedOutput,
    destination: &Path,
) -> Result<(), OutputError> {
    publish_with_ops(artifacts, prepared, destination, &RealPublishOps)
}

fn write_prepared_file(file: &mut File, winner: &[u8]) -> Result<(), OutputError> {
    file.write_all(winner)
        .and_then(|()| file.flush())
        .map_err(|_| OutputError::BeforePublication("cannot write the internal output"))?;
    let metadata = file
        .metadata()
        .map_err(|_| OutputError::BeforePublication("cannot inspect the internal output"))?;
    if !metadata.is_file() || metadata.len() != winner.len() as u64 {
        return Err(OutputError::BeforePublication(
            "the internal output has unexpected filesystem state",
        ));
    }
    Ok(())
}

fn verify_prepared_file(
    path: &Path,
    source: &ValidatedPng,
    winner: &[u8],
) -> Result<(), OutputError> {
    let completed = read_completed(path)?;
    if completed != winner {
        return Err(OutputError::BeforePublication(
            "the completed internal output differs from the selected bytes",
        ));
    }
    validate_candidate(source, &completed).map_err(|_| {
        OutputError::BeforePublication("the completed internal output failed PNG validation")
    })?;
    Ok(())
}

fn read_completed(path: &Path) -> Result<Vec<u8>, OutputError> {
    let file = File::open(path)
        .map_err(|_| OutputError::BeforePublication("cannot reopen the internal output"))?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| OutputError::BeforePublication("cannot verify the internal output"))?;
    if bytes.len() > MAX_SOURCE_BYTES as usize {
        return Err(OutputError::BeforePublication(
            "the internal output exceeds the output-byte limit",
        ));
    }
    Ok(bytes)
}

trait PublishOps {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
}

struct RealPublishOps;

impl PublishOps for RealPublishOps {
    fn rename(&self, source: &Path, destination: &Path) -> io::Result<()> {
        replace(source, destination)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

#[cfg(not(windows))]
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MoveFileExW};

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();

    // SAFETY: Both pointers refer to live, NUL-terminated UTF-16 buffers for
    // the duration of the call. MoveFileExW only reads those buffers.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn publish_with_ops(
    artifacts: &mut Artifacts,
    prepared: PreparedOutput,
    destination: &Path,
    operations: &impl PublishOps,
) -> Result<(), OutputError> {
    if operations.rename(&prepared.path, destination).is_err() {
        if operations.remove_file(&prepared.path).is_ok() {
            artifacts.forget(&prepared.path);
            return Err(OutputError::BeforePublication(
                "cannot replace the destination with the completed output",
            ));
        }
        return Err(OutputError::BeforePublication(
            "publication failed and the internal output could not be cleaned",
        ));
    }
    artifacts.forget(&prepared.path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;
    use crate::png::validate_source;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn prepares_then_publishes_complete_valid_output() {
        let directory = test_directory();
        let destination = directory.join("output.png");
        let bytes = valid_png();
        let source = validate_source(&bytes).unwrap();
        let mut artifacts = Artifacts::new(directory.clone());
        let prepared = prepare(&mut artifacts, &source, &bytes).unwrap();
        assert!(!destination.exists());
        assert!(prepared.path.exists());
        publish(&mut artifacts, prepared, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn existing_destination_is_replaced() {
        let directory = test_directory();
        let destination = directory.join("output.png");
        let bytes = valid_png();
        let source = validate_source(&bytes).unwrap();
        let mut artifacts = Artifacts::new(directory.clone());
        let prepared = prepare(&mut artifacts, &source, &bytes).unwrap();
        fs::write(&destination, b"existing").unwrap();
        publish(&mut artifacts, prepared, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn publication_failure_preserves_destination_and_cleans_internal_file() {
        let directory = test_directory();
        let destination = directory.join("output.png");
        fs::write(&destination, b"existing").unwrap();
        let bytes = valid_png();
        let source = validate_source(&bytes).unwrap();
        let mut artifacts = Artifacts::new(directory.clone());
        let prepared = prepare(&mut artifacts, &source, &bytes).unwrap();
        assert_eq!(
            publish_with_ops(&mut artifacts, prepared, &destination, &FailingRenameOps),
            Err(OutputError::BeforePublication(
                "cannot replace the destination with the completed output"
            ))
        );
        assert_eq!(fs::read(&destination).unwrap(), b"existing");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn output_uses_ordinary_new_file_metadata() {
        let directory = test_directory();
        let destination = directory.join("output.png");
        let control = directory.join("control");
        fs::write(&control, b"control").unwrap();
        let bytes = valid_png();
        let source = validate_source(&bytes).unwrap();
        let mut artifacts = Artifacts::new(directory.clone());
        let prepared = prepare(&mut artifacts, &source, &bytes).unwrap();
        publish(&mut artifacts, prepared, &destination).unwrap();

        let output_metadata = fs::metadata(&destination).unwrap();
        let control_metadata = fs::metadata(&control).unwrap();
        assert_eq!(
            output_metadata.permissions().readonly(),
            control_metadata.permissions().readonly()
        );
        assert!(!output_metadata.permissions().readonly());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                output_metadata.permissions().mode() & 0o777,
                control_metadata.permissions().mode() & 0o777
            );
        }

        fs::remove_dir_all(directory).unwrap();
    }

    struct FailingRenameOps;

    impl PublishOps for FailingRenameOps {
        fn rename(&self, _source: &Path, _destination: &Path) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "injected rename failure",
            ))
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            fs::remove_file(path)
        }
    }

    fn valid_png() -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
        push_chunk(&mut png, b"IHDR", &ihdr);
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&[0, 42]).unwrap();
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

    fn test_directory() -> PathBuf {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "imglean-output-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
}
