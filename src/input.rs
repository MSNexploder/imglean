use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::{self, File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::cli::Arguments;
use crate::limits::MAX_SOURCE_BYTES;

#[derive(Debug)]
pub struct Batch {
    pub output_directory: PathBuf,
    pub inputs: Vec<PreflightInput>,
}

#[derive(Debug)]
pub struct PreflightInput {
    pub canonical_source: PathBuf,
    pub destination: PathBuf,
    source: File,
    sidecars: [PathBuf; 2],
}

#[derive(Debug, Eq, PartialEq)]
pub enum PreflightError {
    WorkingDirectory,
    OutputDirectory(&'static str),
    Input { path: PathBuf, reason: &'static str },
    Destination { path: PathBuf, reason: &'static str },
}

#[derive(Debug, Eq, PartialEq)]
pub enum CaptureError {
    Source { path: PathBuf, reason: &'static str },
    AggregateLimit,
}

pub fn preflight(arguments: Arguments) -> Result<Batch, PreflightError> {
    let working_directory =
        std::env::current_dir().map_err(|_| PreflightError::WorkingDirectory)?;
    let requested_output = absolute_from(&working_directory, &arguments.output_directory);
    let output_directory = fs::canonicalize(&requested_output)
        .map_err(|_| PreflightError::OutputDirectory("cannot resolve the output directory"))?;
    let output_metadata = fs::metadata(&output_directory)
        .map_err(|_| PreflightError::OutputDirectory("cannot inspect the output directory"))?;
    if !output_metadata.is_dir() {
        return Err(PreflightError::OutputDirectory(
            "the output path is not a directory",
        ));
    }

    let mut canonical_sources = HashSet::new();
    let mut folded_destinations = HashSet::new();
    let mut inputs = Vec::with_capacity(arguments.inputs.len());

    for argument in arguments.inputs {
        let basename = validate_basename(&argument)?;
        let absolute_argument = absolute_from(&working_directory, &argument);
        let observed =
            fs::symlink_metadata(&absolute_argument).map_err(|_| PreflightError::Input {
                path: argument.clone(),
                reason: "cannot inspect the input",
            })?;
        if observed.file_type().is_symlink() {
            return Err(PreflightError::Input {
                path: argument,
                reason: "the final input component is a symbolic link",
            });
        }

        let canonical_source =
            fs::canonicalize(&absolute_argument).map_err(|_| PreflightError::Input {
                path: argument.clone(),
                reason: "cannot resolve the input",
            })?;
        if !canonical_sources.insert(canonical_source.clone()) {
            return Err(PreflightError::Input {
                path: argument,
                reason: "the canonical input is repeated",
            });
        }
        let source = File::open(&canonical_source).map_err(|_| PreflightError::Input {
            path: argument.clone(),
            reason: "cannot open the input",
        })?;
        let metadata = source.metadata().map_err(|_| PreflightError::Input {
            path: argument.clone(),
            reason: "cannot inspect the open input",
        })?;
        if !metadata.is_file() {
            return Err(PreflightError::Input {
                path: argument,
                reason: "the input is not a regular file",
            });
        }

        let folded = basename
            .to_str()
            .ok_or_else(|| PreflightError::Input {
                path: argument.clone(),
                reason: "the input basename is not printable ASCII",
            })?
            .to_ascii_lowercase();
        if !folded_destinations.insert(folded) {
            return Err(PreflightError::Input {
                path: argument,
                reason: "the output basename collides after ASCII case folding",
            });
        }
        let destination = output_directory.join(&basename);
        reject_existing_destination(&destination)?;
        let sidecars = sidecar_paths(&canonical_source).ok_or_else(|| PreflightError::Input {
            path: argument.clone(),
            reason: "the canonical input has no filename",
        })?;
        inputs.push(PreflightInput {
            canonical_source,
            destination,
            source,
            sidecars,
        });
    }

    for input in &inputs {
        if canonical_sources.contains(&input.destination) {
            return Err(PreflightError::Destination {
                path: input.destination.clone(),
                reason: "the destination aliases an input",
            });
        }
    }

    Ok(Batch {
        output_directory,
        inputs,
    })
}

impl PreflightInput {
    pub fn capture(
        &mut self,
        aggregate_bytes: &mut u64,
        maximum_aggregate_bytes: u64,
    ) -> Result<Vec<u8>, CaptureError> {
        self.capture_with_hook(aggregate_bytes, maximum_aggregate_bytes, || {})
    }

    fn capture_with_hook(
        &mut self,
        aggregate_bytes: &mut u64,
        maximum_aggregate_bytes: u64,
        after_initial_state: impl FnOnce(),
    ) -> Result<Vec<u8>, CaptureError> {
        self.check_sidecars()?;
        let before = portable_state(&self.source).map_err(|reason| self.source_error(reason))?;
        if before.length > MAX_SOURCE_BYTES {
            return Err(self.source_error("the source exceeds the encoded-byte limit"));
        }
        after_initial_state();
        self.source
            .seek(SeekFrom::Start(0))
            .map_err(|_| self.source_error("cannot seek the retained source"))?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(before.length)
                .unwrap_or(0)
                .min(MAX_SOURCE_BYTES as usize),
        );
        self.source
            .by_ref()
            .take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| self.source_error("cannot read the retained source"))?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(self.source_error("the source exceeds the encoded-byte limit"));
        }
        let after = portable_state(&self.source).map_err(|reason| self.source_error(reason))?;
        self.check_sidecars()?;
        if before != after || after.length != bytes.len() as u64 {
            return Err(self.source_error("the source changed during capture"));
        }
        let new_aggregate = aggregate_bytes
            .checked_add(bytes.len() as u64)
            .ok_or(CaptureError::AggregateLimit)?;
        if new_aggregate > maximum_aggregate_bytes {
            return Err(CaptureError::AggregateLimit);
        }
        *aggregate_bytes = new_aggregate;
        Ok(bytes)
    }

    fn check_sidecars(&self) -> Result<(), CaptureError> {
        for sidecar in &self.sidecars {
            match fs::symlink_metadata(sidecar) {
                Ok(_) => return Err(self.source_error("a C2PA sidecar entry exists")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) => return Err(self.source_error("cannot inspect a C2PA sidecar path")),
            }
        }
        Ok(())
    }

    fn source_error(&self, reason: &'static str) -> CaptureError {
        CaptureError::Source {
            path: self.canonical_source.clone(),
            reason,
        }
    }
}

#[derive(Eq, PartialEq)]
struct PortableState {
    regular_file: bool,
    length: u64,
    modified: Option<SystemTime>,
}

fn portable_state(file: &File) -> Result<PortableState, &'static str> {
    let metadata = file
        .metadata()
        .map_err(|_| "cannot inspect the retained source")?;
    let modified = portable_modified(&metadata)?;
    Ok(PortableState {
        regular_file: metadata.is_file(),
        length: metadata.len(),
        modified,
    })
}

fn portable_modified(metadata: &Metadata) -> Result<Option<SystemTime>, &'static str> {
    match metadata.modified() {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::Unsupported => Ok(None),
        Err(_) => Err("cannot read the source modification time"),
    }
}

fn validate_basename(path: &Path) -> Result<OsString, PreflightError> {
    let Some(basename) = path.file_name() else {
        return Err(PreflightError::Input {
            path: path.to_path_buf(),
            reason: "the input has no final component",
        });
    };
    let Some(text) = basename.to_str() else {
        return Err(PreflightError::Input {
            path: path.to_path_buf(),
            reason: "the input basename is not printable ASCII",
        });
    };
    if !text.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(PreflightError::Input {
            path: path.to_path_buf(),
            reason: "the input basename is not printable ASCII",
        });
    }
    let bytes = text.as_bytes();
    if bytes.len() <= 4 || !bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".png") {
        return Err(PreflightError::Input {
            path: path.to_path_buf(),
            reason: "the input basename needs a nonempty stem and .png extension",
        });
    }
    Ok(basename.to_os_string())
}

fn reject_existing_destination(destination: &Path) -> Result<(), PreflightError> {
    match fs::symlink_metadata(destination) {
        Ok(_) => Err(PreflightError::Destination {
            path: destination.to_path_buf(),
            reason: "the destination already contains a filesystem entry",
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(PreflightError::Destination {
            path: destination.to_path_buf(),
            reason: "cannot inspect the destination",
        }),
    }
}

fn sidecar_paths(source: &Path) -> Option<[PathBuf; 2]> {
    let mut replaced = source.to_path_buf();
    replaced.set_extension("c2pa");
    let filename = source.file_name()?;
    let mut appended_name = filename.to_os_string();
    appended_name.push(".c2pa");
    let appended = source.with_file_name(appended_name);
    Some([replaced, appended])
}

fn absolute_from(working_directory: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_directory.join(path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn preflight_retains_original_basename_and_canonical_source() {
        let directory = TestDirectory::new();
        let source_directory = directory.path.join("source");
        let output_directory = directory.path.join("out");
        fs::create_dir(&source_directory).unwrap();
        fs::create_dir(&output_directory).unwrap();
        let source = source_directory.join("Photo.PNG");
        fs::write(&source, b"bytes").unwrap();

        let batch = preflight(Arguments {
            output_directory: output_directory.clone(),
            inputs: vec![source.clone()],
            strategies: crate::strategy::Selection::default(),
        })
        .unwrap();
        assert_eq!(
            batch.inputs[0].canonical_source,
            fs::canonicalize(source).unwrap()
        );
        assert_eq!(
            batch.inputs[0].destination,
            fs::canonicalize(output_directory)
                .unwrap()
                .join("Photo.PNG")
        );
    }

    #[test]
    fn rejects_ascii_folded_collisions_and_repeated_sources() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let first_directory = directory.create_directory("one");
        let second_directory = directory.create_directory("two");
        let first = first_directory.join("a.png");
        let second = second_directory.join("A.PNG");
        fs::write(&first, b"a").unwrap();
        fs::write(&second, b"b").unwrap();

        assert!(matches!(
            preflight(Arguments {
                output_directory: output.clone(),
                inputs: vec![first.clone(), second],
                strategies: crate::strategy::Selection::default(),
            }),
            Err(PreflightError::Input { reason, .. })
                if reason == "the output basename collides after ASCII case folding"
        ));
        assert!(matches!(
            preflight(Arguments {
                output_directory: output,
                inputs: vec![first.clone(), first],
                strategies: crate::strategy::Selection::default(),
            }),
            Err(PreflightError::Input { reason, .. }) if reason == "the canonical input is repeated"
        ));
    }

    #[test]
    fn distinct_hard_links_are_distinct_inputs() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let first = directory.path.join("a.png");
        let second = directory.path.join("b.png");
        fs::write(&first, b"same").unwrap();
        fs::hard_link(&first, &second).unwrap();
        let batch = preflight(Arguments {
            output_directory: output,
            inputs: vec![first, second],
            strategies: crate::strategy::Selection::default(),
        })
        .unwrap();
        assert_eq!(batch.inputs.len(), 2);
    }

    #[test]
    fn rejects_existing_destination_and_invalid_basename() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("a.png");
        fs::write(&source, b"source").unwrap();
        fs::write(output.join("a.png"), b"existing").unwrap();
        assert!(matches!(
            preflight(Arguments {
                output_directory: output,
                inputs: vec![source],
                strategies: crate::strategy::Selection::default(),
            }),
            Err(PreflightError::Destination { .. })
        ));

        assert!(matches!(
            validate_basename(Path::new(".png")),
            Err(PreflightError::Input { .. })
        ));
        assert!(matches!(
            validate_basename(Path::new("photo.jpg")),
            Err(PreflightError::Input { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_final_symlink_and_dangling_destination() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let target = directory.path.join("target.png");
        let link = directory.path.join("link.png");
        fs::write(&target, b"source").unwrap();
        symlink(&target, &link).unwrap();
        assert!(matches!(
            preflight(Arguments {
                output_directory: output.clone(),
                inputs: vec![link],
                strategies: crate::strategy::Selection::default(),
            }),
            Err(PreflightError::Input { reason, .. })
                if reason == "the final input component is a symbolic link"
        ));

        symlink(directory.path.join("missing"), output.join("target.png")).unwrap();
        assert!(matches!(
            preflight(Arguments {
                output_directory: output,
                inputs: vec![target],
                strategies: crate::strategy::Selection::default(),
            }),
            Err(PreflightError::Destination { .. })
        ));
    }

    #[test]
    fn capture_detects_source_change_and_sidecars() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("photo.png");
        fs::write(&source, b"source").unwrap();
        let mut batch = preflight(Arguments {
            output_directory: output.clone(),
            inputs: vec![source.clone()],
            strategies: crate::strategy::Selection::default(),
        })
        .unwrap();
        let error = batch.inputs[0]
            .capture_with_hook(&mut 0, u64::MAX, || {
                let mut file = OpenOptions::new().append(true).open(&source).unwrap();
                file.write_all(b" changed").unwrap();
            })
            .unwrap_err();
        assert!(matches!(
            error,
            CaptureError::Source { reason, .. } if reason == "the source changed during capture"
        ));

        fs::write(&source, b"source").unwrap();
        fs::write(directory.path.join("photo.c2pa"), b"manifest").unwrap();
        let mut batch = preflight(Arguments {
            output_directory: output,
            inputs: vec![source],
            strategies: crate::strategy::Selection::default(),
        })
        .unwrap();
        assert!(matches!(
            batch.inputs[0].capture(&mut 0, u64::MAX),
            Err(CaptureError::Source { reason, .. }) if reason == "a C2PA sidecar entry exists"
        ));

        fs::remove_file(directory.path.join("photo.c2pa")).unwrap();
        fs::write(directory.path.join("photo.png.c2pa"), b"manifest").unwrap();
        let mut batch = preflight(Arguments {
            output_directory: directory.create_directory("second-out"),
            inputs: vec![directory.path.join("photo.png")],
            strategies: crate::strategy::Selection::default(),
        })
        .unwrap();
        assert!(matches!(
            batch.inputs[0].capture(&mut 0, u64::MAX),
            Err(CaptureError::Source { reason, .. }) if reason == "a C2PA sidecar entry exists"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn resolves_ancestor_symlinks_without_changing_destination_name() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let real = directory.create_directory("real");
        let output = directory.create_directory("out");
        let source = real.join("Photo.PNG");
        fs::write(&source, b"source").unwrap();
        let linked_directory = directory.path.join("linked");
        symlink(&real, &linked_directory).unwrap();

        let batch = preflight(Arguments {
            output_directory: output.clone(),
            inputs: vec![linked_directory.join("Photo.PNG")],
            strategies: crate::strategy::Selection::default(),
        })
        .unwrap();

        assert_eq!(
            batch.inputs[0].canonical_source,
            fs::canonicalize(source).unwrap()
        );
        assert_eq!(
            batch.inputs[0].destination,
            fs::canonicalize(output).unwrap().join("Photo.PNG")
        );
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "imglean-input-test-{}-{unique}",
                std::process::id()
            ));
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
}
