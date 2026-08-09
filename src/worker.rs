use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::num::NonZeroU64;
use std::path::Path;
use std::process::Command;

use oxipng::{Deflater, FilterStrategy, Options, StripChunks, ZopfliOptions, indexset};

use crate::artifacts::Artifacts;
use crate::diagnostics::escape_worker_text;
use crate::limits::{
    EMBEDDED_WORKER_TIMEOUT, LIMITS_VERSION, MAX_CANDIDATE_BYTES, MAX_RECONSTRUCTED_BYTES,
    MAX_SOURCE_BYTES, MAX_TEMPORARY_BYTES, OPTIPNG_TIMEOUT, OXIPNG_TIMEOUT, PNGQUANT_TIMEOUT,
};
use crate::png::validate_source;
use crate::process::{self, Capture};
use crate::strategy::{Execution, Quality, Strategy, StrategyId};

const WORKER_ROLE: &str = "--imglean-internal-worker-v2";

#[derive(Debug, Eq, PartialEq)]
pub enum StrategyResult {
    Candidate(Vec<u8>),
    NoCandidate,
    Warning(String),
    Failure(&'static str),
}

pub fn try_run(arguments: &[OsString]) -> Option<i32> {
    if arguments
        .get(1)
        .is_none_or(|argument| argument != OsStr::new(WORKER_ROLE))
    {
        return None;
    }
    Some(run_private(arguments))
}

pub fn run_strategy(
    artifacts: &mut Artifacts,
    source: &[u8],
    strategy: &Strategy,
    quality: Quality,
) -> StrategyResult {
    let maximum_live_bytes = (source.len() as u64)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(MAX_CANDIDATE_BYTES));
    if maximum_live_bytes.is_none_or(|bytes| bytes > MAX_TEMPORARY_BYTES) {
        return StrategyResult::Warning(
            "provider artifacts exceed the temporary-byte limit".to_owned(),
        );
    }
    let (private_input, mut input_file) = match artifacts.create("provider-input") {
        Ok(value) => value,
        Err(_) => {
            return StrategyResult::Warning("cannot create the private provider input".to_owned());
        }
    };
    if input_file
        .write_all(source)
        .and_then(|()| input_file.flush())
        .is_err()
    {
        drop(input_file);
        return cleanup_failure_or(
            artifacts,
            &[&private_input],
            StrategyResult::Warning("cannot write the private provider input".to_owned()),
        );
    }
    drop(input_file);
    let candidate_path = match artifacts.reserve_path("provider-candidate") {
        Ok(path) => path,
        Err(_) => {
            return cleanup_failure_or(
                artifacts,
                &[&private_input],
                StrategyResult::Warning("cannot reserve the provider candidate path".to_owned()),
            );
        }
    };

    let command = match strategy_command(strategy, quality, &private_input, &candidate_path) {
        Ok(command) => command,
        Err(message) => {
            return cleanup_failure_or(
                artifacts,
                &[&private_input, &candidate_path],
                StrategyResult::Warning(message.to_owned()),
            );
        }
    };
    let timeout = match strategy.id {
        StrategyId::OxipngLibdeflateV1 | StrategyId::OxipngZopfliV1 => EMBEDDED_WORKER_TIMEOUT,
        StrategyId::OptipngV1 => OPTIPNG_TIMEOUT,
        StrategyId::PngquantV1 => PNGQUANT_TIMEOUT,
    };
    let output = match process::run(command, timeout) {
        Ok(output) => output,
        Err(()) => {
            return cleanup_failure_or(
                artifacts,
                &[&private_input, &candidate_path],
                StrategyResult::Warning("cannot start worker".to_owned()),
            );
        }
    };

    let mut result = if !private_input_matches(&private_input, source) {
        StrategyResult::Failure("the private provider input changed during execution")
    } else if output.timed_out {
        StrategyResult::Warning("worker timeout exceeded".to_owned())
    } else if output.stderr.truncated || output.stdout.truncated {
        StrategyResult::Warning("worker diagnostics exceeded the byte limit".to_owned())
    } else if strategy.id == StrategyId::PngquantV1
        && output
            .status
            .is_some_and(|status| status.code() == Some(99))
    {
        StrategyResult::NoCandidate
    } else if output.status.is_none_or(|status| !status.success()) {
        let detail = diagnostic_detail(&output.stderr, &output.stdout);
        StrategyResult::Warning(match detail {
            Some(detail) => format!("worker failed: {detail}"),
            None => "worker failed".to_owned(),
        })
    } else if matches!(strategy.execution, Execution::Embedded) && !output.stdout.bytes.is_empty() {
        StrategyResult::Warning("worker produced unexpected standard output".to_owned())
    } else {
        read_candidate(&candidate_path)
    };

    result = cleanup_failure_or(artifacts, &[&private_input, &candidate_path], result);
    result
}

fn strategy_command(
    strategy: &Strategy,
    quality: Quality,
    private_input: &Path,
    candidate_path: &Path,
) -> Result<Command, &'static str> {
    match &strategy.execution {
        Execution::Embedded => {
            let executable =
                std::env::current_exe().map_err(|_| "cannot identify the current executable")?;
            let mut command = Command::new(executable);
            command
                .arg(WORKER_ROLE)
                .arg(strategy.id.as_str())
                .arg(LIMITS_VERSION)
                .arg(private_input)
                .arg(candidate_path);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::OptipngV1 => {
            let mut command = Command::new(executable);
            command
                .arg("-quiet")
                .arg("-o2")
                .arg("-out")
                .arg(candidate_path)
                .arg("--")
                .arg(private_input);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::PngquantV1 => {
            let Some(quality) = quality.numeric() else {
                return Err("pngquant requires numeric quality");
            };
            let mut command = Command::new(executable);
            command
                .arg("--force")
                .arg("--quality")
                .arg(format!("0-{quality}"))
                .arg("--speed")
                .arg("4")
                .arg("--strip")
                .arg("--output")
                .arg(candidate_path)
                .arg("--")
                .arg(private_input);
            Ok(command)
        }
        Execution::External { .. } => Err("unsupported external strategy"),
    }
}

fn run_private(arguments: &[OsString]) -> i32 {
    if arguments.len() != 6 || arguments[3] != OsStr::new(LIMITS_VERSION) {
        return private_error("invalid private worker protocol");
    }
    let Some(strategy) = arguments[2]
        .to_str()
        .and_then(StrategyId::parse)
        .filter(|strategy| StrategyId::EMBEDDED.contains(strategy))
    else {
        return private_error("invalid private worker strategy");
    };
    let input = Path::new(&arguments[4]);
    let candidate = Path::new(&arguments[5]);
    let bytes = match read_bounded(input, MAX_SOURCE_BYTES) {
        Ok(bytes) => bytes,
        Err(message) => return private_error(message),
    };
    if validate_source(&bytes).is_err() {
        return private_error("private provider input failed PNG validation");
    }
    let options = oxipng_options(strategy);
    let optimized = match oxipng::optimize_from_memory(&bytes, &options) {
        Ok(bytes) => bytes,
        Err(_) => return private_error("OxiPNG could not optimize the private input"),
    };
    if optimized.len() as u64 > MAX_CANDIDATE_BYTES {
        return private_error("OxiPNG candidate exceeds the candidate-byte limit");
    }
    let mut output = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(candidate)
    {
        Ok(file) => file,
        Err(_) => return private_error("cannot create the private candidate"),
    };
    if output
        .write_all(&optimized)
        .and_then(|()| output.flush())
        .is_err()
    {
        return private_error("cannot write the private candidate");
    }
    0
}

fn oxipng_options(strategy: StrategyId) -> Options {
    let deflater = match strategy {
        StrategyId::OxipngLibdeflateV1 => Deflater::Libdeflater { compression: 11 },
        StrategyId::OxipngZopfliV1 => Deflater::Zopfli(ZopfliOptions {
            iteration_count: NonZeroU64::new(15).expect("15 is nonzero"),
            iterations_without_improvement: NonZeroU64::new(u64::MAX).expect("u64::MAX is nonzero"),
            maximum_block_splits: 15,
        }),
        StrategyId::OptipngV1 | StrategyId::PngquantV1 => {
            unreachable!("external strategies do not use OxiPNG options")
        }
    };
    Options {
        fix_errors: false,
        force: true,
        filters: indexset! {
            FilterStrategy::NONE,
            FilterStrategy::SUB,
            FilterStrategy::Entropy,
            FilterStrategy::Bigrams,
        },
        interlace: None,
        optimize_alpha: false,
        bit_depth_reduction: false,
        color_type_reduction: false,
        palette_reduction: false,
        grayscale_reduction: false,
        idat_recoding: true,
        scale_16: false,
        strip: StripChunks::None,
        deflater,
        fast_evaluation: true,
        timeout: Some(OXIPNG_TIMEOUT),
        max_decompressed_size: Some(MAX_RECONSTRUCTED_BYTES),
    }
}

fn read_candidate(path: &Path) -> StrategyResult {
    match read_bounded(path, MAX_CANDIDATE_BYTES) {
        Ok(bytes) => StrategyResult::Candidate(bytes),
        Err("worker file is missing") => StrategyResult::NoCandidate,
        Err(message) => StrategyResult::Warning(message.to_owned()),
    }
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "worker file is missing"
        } else {
            "cannot inspect the worker file"
        }
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("worker file is not a regular non-symlink file");
    }
    if metadata.len() > maximum {
        return Err("worker file exceeds its byte limit");
    }
    let file = File::open(path).map_err(|_| "cannot open the worker file")?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read the worker file")?;
    if bytes.len() as u64 > maximum {
        return Err("worker file exceeds its byte limit");
    }
    Ok(bytes)
}

fn private_input_matches(path: &Path, expected: &[u8]) -> bool {
    read_bounded(path, MAX_SOURCE_BYTES).is_ok_and(|bytes| bytes == expected)
}

fn cleanup_failure_or(
    artifacts: &mut Artifacts,
    paths: &[&Path],
    result: StrategyResult,
) -> StrategyResult {
    let mut failed = false;
    for path in paths {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                if artifacts.remove(path).is_err() {
                    failed = true;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => artifacts.forget(path),
            Err(_) => failed = true,
        }
    }
    if failed {
        StrategyResult::Failure("cannot clean current-run provider artifacts")
    } else {
        result
    }
}

fn diagnostic_detail(stderr: &Capture, stdout: &Capture) -> Option<String> {
    [stderr, stdout]
        .into_iter()
        .find(|capture| !capture.bytes.is_empty())
        .map(|capture| {
            let mut escaped = escape_worker_text(&capture.bytes);
            if capture.truncated {
                escaped.push_str(" [truncated]");
            }
            escaped
        })
}

fn private_error(message: &str) -> i32 {
    let _ = writeln!(io::stderr().lock(), "{message}");
    1
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn options_pin_every_policy_boundary() {
        let options = oxipng_options(StrategyId::OxipngLibdeflateV1);
        assert!(!options.fix_errors);
        assert!(options.force);
        assert_eq!(
            options.filters,
            indexset! {
                FilterStrategy::NONE,
                FilterStrategy::SUB,
                FilterStrategy::Entropy,
                FilterStrategy::Bigrams,
            }
        );
        assert_eq!(options.interlace, None);
        assert!(!options.optimize_alpha);
        assert!(!options.bit_depth_reduction);
        assert!(!options.color_type_reduction);
        assert!(!options.palette_reduction);
        assert!(!options.grayscale_reduction);
        assert!(options.idat_recoding);
        assert!(!options.scale_16);
        assert_eq!(options.strip, StripChunks::None);
        assert!(options.fast_evaluation);
        assert_eq!(options.timeout, Some(OXIPNG_TIMEOUT));
        assert_eq!(options.max_decompressed_size, Some(MAX_RECONSTRUCTED_BYTES));
        assert_eq!(options.deflater, Deflater::Libdeflater { compression: 11 });

        let zopfli = oxipng_options(StrategyId::OxipngZopfliV1);
        assert_eq!(
            zopfli.deflater,
            Deflater::Zopfli(ZopfliOptions {
                iteration_count: NonZeroU64::new(15).unwrap(),
                iterations_without_improvement: NonZeroU64::new(u64::MAX).unwrap(),
                maximum_block_splits: 15,
            })
        );
    }

    #[test]
    fn embedded_strategies_produce_candidates() {
        let directory = test_directory();
        let source = compressible_png();
        for strategy in StrategyId::EMBEDDED {
            let input = directory.join(format!("{}-input.png", strategy.as_str()));
            let candidate_path = directory.join(format!("{}-candidate.png", strategy.as_str()));
            fs::write(&input, &source).unwrap();
            let arguments = [
                OsString::from("imglean"),
                OsString::from(WORKER_ROLE),
                OsString::from(strategy.as_str()),
                OsString::from(LIMITS_VERSION),
                input.into_os_string(),
                candidate_path.clone().into_os_string(),
            ];
            assert_eq!(run_private(&arguments), 0);
            let candidate = fs::read(candidate_path).unwrap();
            assert!(!candidate.is_empty());
            assert!(candidate.len() <= source.len());
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn optipng_command_pins_every_adapter_argument() {
        let strategy = external_strategy(PathBuf::from("provider"));
        let command = strategy_command(
            &strategy,
            Quality::Lossless,
            Path::new("private-input.png"),
            Path::new("candidate.png"),
        )
        .unwrap();
        assert_eq!(command.get_program(), OsStr::new("provider"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-quiet"),
                OsStr::new("-o2"),
                OsStr::new("-out"),
                OsStr::new("candidate.png"),
                OsStr::new("--"),
                OsStr::new("private-input.png"),
            ]
        );
    }

    #[test]
    fn pngquant_command_pins_quality_and_every_adapter_argument() {
        let strategy = pngquant_strategy(PathBuf::from("provider"));
        let command = strategy_command(
            &strategy,
            Quality::Numeric(80),
            Path::new("private-input.png"),
            Path::new("candidate.png"),
        )
        .unwrap();
        assert_eq!(command.get_program(), OsStr::new("provider"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("--force"),
                OsStr::new("--quality"),
                OsStr::new("0-80"),
                OsStr::new("--speed"),
                OsStr::new("4"),
                OsStr::new("--strip"),
                OsStr::new("--output"),
                OsStr::new("candidate.png"),
                OsStr::new("--"),
                OsStr::new("private-input.png"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_strategy_uses_private_paths_and_captures_failures() {
        let directory = test_directory();
        let source = compressible_png();
        let success = directory.join("success-optipng");
        write_executable(
            &success,
            "#!/bin/sh\nwhile [ \"$1\" != \"-out\" ]; do shift; done\nshift\noutput=$1\nshift 2\ncp \"$1\" \"$output\"\n",
        );
        let mut artifacts = Artifacts::new(directory.clone());
        assert_eq!(
            run_strategy(
                &mut artifacts,
                &source,
                &external_strategy(success),
                Quality::Lossless,
            ),
            StrategyResult::Candidate(source.clone())
        );

        let failure = directory.join("failing-optipng");
        write_executable(
            &failure,
            "#!/bin/sh\nprintf 'provider failed\\n' >&2\nexit 9\n",
        );
        let result = run_strategy(
            &mut artifacts,
            &source,
            &external_strategy(failure),
            Quality::Lossless,
        );
        assert!(matches!(
            result,
            StrategyResult::Warning(message) if message.contains("provider failed")
        ));

        let mutation = directory.join("mutating-optipng");
        write_executable(
            &mutation,
            "#!/bin/sh\nfor input do :; done\nprintf changed > \"$input\"\nexit 9\n",
        );
        assert_eq!(
            run_strategy(
                &mut artifacts,
                &source,
                &external_strategy(mutation),
                Quality::Lossless,
            ),
            StrategyResult::Failure("the private provider input changed during execution")
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 3);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn pngquant_quality_rejection_is_a_normal_missing_candidate() {
        let directory = test_directory();
        let executable = directory.join("pngquant");
        write_executable(&executable, "#!/bin/sh\nexit 99\n");
        let mut artifacts = Artifacts::new(directory.clone());
        assert_eq!(
            run_strategy(
                &mut artifacts,
                &compressible_png(),
                &pngquant_strategy(executable),
                Quality::Numeric(100),
            ),
            StrategyResult::NoCandidate
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bounded_worker_read_rejects_oversized_and_symlink_files() {
        let directory = test_directory();
        let oversized = directory.join("oversized");
        fs::write(&oversized, b"1234").unwrap();
        assert_eq!(
            read_bounded(&oversized, 3),
            Err("worker file exceeds its byte limit")
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = directory.join("link");
            symlink(&oversized, &link).unwrap();
            assert_eq!(
                read_bounded(&link, 8),
                Err("worker file is not a regular non-symlink file")
            );
        }

        fs::remove_dir_all(directory).unwrap();
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
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
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

    fn test_directory() -> PathBuf {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "imglean-worker-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        use std::os::unix::fs::PermissionsExt as _;

        fs::write(path, contents).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn external_strategy(executable: PathBuf) -> Strategy {
        Strategy {
            id: StrategyId::OptipngV1,
            execution: Execution::External {
                executable,
                version: "7.9.1".to_owned(),
            },
        }
    }

    fn pngquant_strategy(executable: PathBuf) -> Strategy {
        Strategy {
            id: StrategyId::PngquantV1,
            execution: Execution::External {
                executable,
                version: "3.0.2".to_owned(),
            },
        }
    }
}
