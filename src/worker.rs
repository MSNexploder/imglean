use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::num::NonZeroU64;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use oxipng::{Deflater, FilterStrategy, Options, StripChunks, ZopfliOptions, indexset};

use crate::artifacts::Artifacts;
use crate::avif::validate_source as validate_avif_source;
use crate::diagnostics::escape_worker_text;
use crate::jpeg::validate_source as validate_jpeg_source;
use crate::limits::{
    LIMITS_VERSION, MAX_CANDIDATE_BYTES, MAX_RECONSTRUCTED_BYTES, MAX_SOURCE_BYTES,
    MAX_STRATEGY_TIMEOUT_SECONDS, MAX_TEMPORARY_BYTES, MIN_OXIPNG_TIMEOUT, OXIPNG_CLEANUP_RESERVE,
};
use crate::png::validate_source as validate_png_source;
use crate::process::{self, Capture};
use crate::strategy::{Execution, Quality, Strategy, StrategyId};
use crate::webp::validate_source as validate_webp_source;

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
    strip_metadata: bool,
    strategy_timeout: Duration,
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

    let command = match strategy_command(
        strategy,
        quality,
        strip_metadata,
        strategy_timeout,
        &private_input,
        &candidate_path,
    ) {
        Ok(command) => command,
        Err(message) => {
            return cleanup_failure_or(
                artifacts,
                &[&private_input, &candidate_path],
                StrategyResult::Warning(message.to_owned()),
            );
        }
    };
    let output = match process::run(command, strategy_timeout) {
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
    } else if strategy.id == StrategyId::Pngquant
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
    } else if matches!(strategy.execution, Execution::Bundled) && !output.stdout.bytes.is_empty() {
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
    strip_metadata: bool,
    strategy_timeout: Duration,
    private_input: &Path,
    candidate_path: &Path,
) -> Result<Command, &'static str> {
    match &strategy.execution {
        Execution::Bundled => {
            let executable =
                std::env::current_exe().map_err(|_| "cannot identify the current executable")?;
            let directory = private_input
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            if candidate_path.parent() != private_input.parent() {
                return Err("bundled provider paths do not share a directory");
            }
            let input_name = private_input
                .file_name()
                .ok_or("private input has no file name")?;
            let candidate_name = candidate_path
                .file_name()
                .ok_or("private candidate has no file name")?;
            let mut command = Command::new(executable);
            command.current_dir(directory);
            command
                .arg(WORKER_ROLE)
                .arg(strategy.id.as_str())
                .arg(LIMITS_VERSION)
                .arg(
                    strategy_timeout
                        .saturating_sub(OXIPNG_CLEANUP_RESERVE)
                        .max(MIN_OXIPNG_TIMEOUT)
                        .as_secs()
                        .to_string(),
                )
                .arg(if strip_metadata { "strip" } else { "preserve" })
                .arg(match quality {
                    Quality::Lossless => "lossless".to_owned(),
                    Quality::Numeric(quality) => quality.to_string(),
                })
                .arg(input_name)
                .arg(candidate_name);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::Optipng => {
            let mut command = Command::new(executable);
            command.arg("-quiet").arg("-o2");
            if strip_metadata {
                command.arg("-strip").arg("all");
            }
            command
                .arg("-out")
                .arg(candidate_path)
                .arg("--")
                .arg(private_input);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::Pngquant => {
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
        Execution::External { executable, .. } if strategy.id == StrategyId::Jpegtran => {
            let mut command = Command::new(executable);
            command
                .arg("-copy")
                .arg(if strip_metadata { "none" } else { "all" })
                .arg("-optimize")
                .arg("-progressive")
                .arg("-strict")
                .arg("-outfile")
                .arg(candidate_path)
                .arg(private_input);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::Mozjpeg => {
            let Some(quality) = quality.numeric() else {
                return Err("MozJPEG requires numeric quality");
            };
            let mut command = Command::new(executable);
            command
                .arg("-quality")
                .arg(quality.to_string())
                .arg("-progressive")
                .arg("-optimize")
                .arg("-strict")
                .arg("-outfile")
                .arg(candidate_path)
                .arg(private_input);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::Jpegli => {
            let Some(quality) = quality.numeric() else {
                return Err("Jpegli requires numeric quality");
            };
            let mut command = Command::new(executable);
            command
                .arg("--quality")
                .arg(quality.to_string())
                .arg("--progressive_level")
                .arg("2")
                .arg(private_input)
                .arg(candidate_path);
            Ok(command)
        }
        Execution::External { executable, .. } if strategy.id == StrategyId::Libwebp => {
            let mut command = Command::new(executable);
            command.arg("-quiet");
            match quality {
                Quality::Lossless => {
                    command.arg("-lossless").arg("-exact").arg("-q").arg("100");
                }
                Quality::Numeric(quality) => {
                    command.arg("-q").arg(quality.to_string());
                }
            }
            command
                .arg("-m")
                .arg("6")
                .arg("-alpha_q")
                .arg("100")
                .arg("-metadata")
                .arg(if strip_metadata { "none" } else { "all" })
                .arg("-o")
                .arg(candidate_path)
                .arg("--")
                .arg(private_input);
            Ok(command)
        }
        Execution::External { .. } => Err("unsupported external strategy"),
    }
}

fn run_private(arguments: &[OsString]) -> i32 {
    if arguments.len() != 9 || arguments[3] != OsStr::new(LIMITS_VERSION) {
        return private_error("invalid private worker protocol");
    }
    let Some(strategy) = arguments[2]
        .to_str()
        .and_then(StrategyId::parse)
        .filter(|strategy| StrategyId::BUNDLED.contains(strategy))
    else {
        return private_error("invalid private worker strategy");
    };
    let Some(timeout_seconds) = arguments[4]
        .to_str()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| {
            (MIN_OXIPNG_TIMEOUT.as_secs()
                ..=MAX_STRATEGY_TIMEOUT_SECONDS - OXIPNG_CLEANUP_RESERVE.as_secs())
                .contains(seconds)
        })
    else {
        return private_error("invalid private worker timeout");
    };
    let strip_metadata = match arguments[5].to_str() {
        Some("strip") => true,
        Some("preserve") => false,
        _ => return private_error("invalid private worker metadata policy"),
    };
    let quality = match arguments[6].to_str() {
        Some("lossless") => Quality::Lossless,
        Some(value) => match value.parse::<u8>() {
            Ok(value @ 1..=100) => Quality::Numeric(value),
            _ => return private_error("invalid private worker quality"),
        },
        None => return private_error("invalid private worker quality"),
    };
    let input = Path::new(&arguments[7]);
    let candidate = Path::new(&arguments[8]);
    let bytes = match read_bounded(input, MAX_SOURCE_BYTES) {
        Ok(bytes) => bytes,
        Err(message) => return private_error(message),
    };
    let source_is_valid = match strategy.format() {
        crate::image::ImageFormat::Png => validate_png_source(&bytes).is_ok(),
        crate::image::ImageFormat::Jpeg => validate_jpeg_source(&bytes).is_ok(),
        crate::image::ImageFormat::Webp => validate_webp_source(&bytes).is_ok(),
        crate::image::ImageFormat::Avif => validate_avif_source(&bytes).is_ok(),
    };
    if !source_is_valid {
        return private_error("private provider input failed format validation");
    }
    if strategy == StrategyId::Optipng {
        return match imglean_codecs::optimize_optipng(input, candidate, strip_metadata) {
            Ok(()) => 0,
            Err(()) => private_error("OptiPNG could not optimize the private input"),
        };
    }
    let optimized = match strategy {
        StrategyId::OxipngLibdeflate | StrategyId::OxipngZopfli => {
            let options = oxipng_options(
                strategy,
                Duration::from_secs(timeout_seconds),
                strip_metadata,
            );
            match oxipng::optimize_from_memory(&bytes, &options) {
                Ok(bytes) => bytes,
                Err(_) => return private_error("OxiPNG could not optimize the private input"),
            }
        }
        StrategyId::Jpegtran => match imglean_codecs::optimize_jpegtran(&bytes, strip_metadata) {
            Ok(bytes) => bytes,
            Err(()) => return private_error("jpegtran could not optimize the private input"),
        },
        StrategyId::Mozjpeg => {
            let Some(quality) = quality.numeric() else {
                return private_error("MozJPEG requires numeric quality");
            };
            match imglean_codecs::optimize_mozjpeg(&bytes, quality, strip_metadata) {
                Ok(bytes) => bytes,
                Err(()) => return private_error("MozJPEG could not optimize the private input"),
            }
        }
        StrategyId::Jpegli => {
            let Some(quality) = quality.numeric() else {
                return private_error("Jpegli requires numeric quality");
            };
            match imglean_codecs::optimize_jpegli(&bytes, quality, strip_metadata) {
                Ok(bytes) => bytes,
                Err(()) => return private_error("Jpegli could not optimize the private input"),
            }
        }
        StrategyId::Libwebp => {
            match imglean_codecs::optimize_libwebp(&bytes, quality.numeric(), strip_metadata) {
                Ok(bytes) => bytes,
                Err(()) => return private_error("libwebp could not optimize the private input"),
            }
        }
        StrategyId::ImageWebp => {
            match imglean_codecs::optimize_image_webp(&bytes, strip_metadata) {
                Ok(bytes) => bytes,
                Err(()) => return private_error("image-webp could not optimize the private input"),
            }
        }
        StrategyId::AvifAom => {
            let Some(quality) = quality.numeric() else {
                return private_error("libavif/libaom requires numeric quality");
            };
            match imglean_codecs::optimize_avif_aom(&bytes, quality) {
                Ok(bytes) => bytes,
                Err(()) => {
                    return private_error("libavif/libaom could not optimize the private input");
                }
            }
        }
        StrategyId::AvifRav1e => {
            let Some(quality) = quality.numeric() else {
                return private_error("ravif requires numeric quality");
            };
            match imglean_codecs::optimize_avif_rav1e(&bytes, quality) {
                Ok(bytes) => bytes,
                Err(()) => return private_error("ravif could not optimize the private input"),
            }
        }
        StrategyId::Optipng | StrategyId::Pngquant => {
            unreachable!("handled or external strategy")
        }
    };
    if optimized.len() as u64 > MAX_CANDIDATE_BYTES {
        return private_error("provider candidate exceeds the candidate-byte limit");
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

fn oxipng_options(strategy: StrategyId, timeout: Duration, strip_metadata: bool) -> Options {
    let deflater = match strategy {
        StrategyId::OxipngLibdeflate => Deflater::Libdeflater { compression: 11 },
        StrategyId::OxipngZopfli => Deflater::Zopfli(ZopfliOptions {
            iteration_count: NonZeroU64::new(15).expect("15 is nonzero"),
            iterations_without_improvement: NonZeroU64::new(u64::MAX).expect("u64::MAX is nonzero"),
            maximum_block_splits: 15,
        }),
        StrategyId::Optipng
        | StrategyId::Pngquant
        | StrategyId::Jpegtran
        | StrategyId::Mozjpeg
        | StrategyId::Jpegli
        | StrategyId::Libwebp
        | StrategyId::ImageWebp
        | StrategyId::AvifAom
        | StrategyId::AvifRav1e => {
            unreachable!("other strategies do not use OxiPNG options")
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
        strip: if strip_metadata {
            StripChunks::Safe
        } else {
            StripChunks::None
        },
        deflater,
        fast_evaluation: true,
        timeout: Some(timeout),
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
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;
    use crate::limits::DEFAULT_STRATEGY_TIMEOUT;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    static BUNDLED_PROVIDER_TEST: Mutex<()> = Mutex::new(());

    #[test]
    fn options_pin_every_policy_boundary() {
        let timeout = DEFAULT_STRATEGY_TIMEOUT.saturating_sub(OXIPNG_CLEANUP_RESERVE);
        let options = oxipng_options(StrategyId::OxipngLibdeflate, timeout, false);
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
        assert_eq!(options.timeout, Some(timeout));
        assert_eq!(options.max_decompressed_size, Some(MAX_RECONSTRUCTED_BYTES));
        assert_eq!(options.deflater, Deflater::Libdeflater { compression: 11 });

        let stripped = oxipng_options(StrategyId::OxipngLibdeflate, timeout, true);
        assert_eq!(stripped.strip, StripChunks::Safe);

        let zopfli = oxipng_options(StrategyId::OxipngZopfli, timeout, false);
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
    fn every_bundled_strategy_produces_a_candidate() {
        let _provider_guard = BUNDLED_PROVIDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = test_directory();
        for strategy in StrategyId::BUNDLED {
            let (source, extension) = match strategy.format() {
                crate::image::ImageFormat::Png => (compressible_png(), "png"),
                crate::image::ImageFormat::Jpeg => (
                    include_bytes!("../tests/corpus/jpeg/v1/accepted/provider-reduction.jpg")
                        .to_vec(),
                    "jpg",
                ),
                crate::image::ImageFormat::Webp => (
                    include_bytes!("../tests/corpus/webp/v1/accepted/provider-reduction.webp")
                        .to_vec(),
                    "webp",
                ),
                crate::image::ImageFormat::Avif => (
                    include_bytes!("../tests/corpus/avif/v1/accepted/provider-reduction.avif")
                        .to_vec(),
                    "avif",
                ),
            };
            let input = directory.join(format!("{}-input.{extension}", strategy.as_str()));
            let candidate_path =
                directory.join(format!("{}-candidate.{extension}", strategy.as_str()));
            fs::write(&input, &source).unwrap();
            let arguments = [
                OsString::from("imglean"),
                OsString::from(WORKER_ROLE),
                OsString::from(strategy.as_str()),
                OsString::from(LIMITS_VERSION),
                OsString::from("55"),
                OsString::from("preserve"),
                OsString::from("80"),
                input.into_os_string(),
                candidate_path.clone().into_os_string(),
            ];
            assert_eq!(run_private(&arguments), 0);
            let candidate = fs::read(candidate_path).unwrap();
            assert!(!candidate.is_empty());
            match strategy.format() {
                crate::image::ImageFormat::Png => {
                    crate::png::validate_source(&candidate).unwrap();
                }
                crate::image::ImageFormat::Jpeg => {
                    crate::jpeg::validate_source(&candidate).unwrap();
                }
                crate::image::ImageFormat::Webp => {
                    crate::webp::validate_source(&candidate).unwrap();
                }
                crate::image::ImageFormat::Avif => {
                    crate::avif::validate_source(&candidate).unwrap();
                }
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bundled_png_strategies_strip_metadata_when_requested() {
        let _provider_guard = BUNDLED_PROVIDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = test_directory();
        let source = png_with_text_metadata();
        for strategy in StrategyId::BUNDLED
            .into_iter()
            .filter(|strategy| strategy.format() == crate::image::ImageFormat::Png)
        {
            let input = directory.join(format!("{}-strip-input.png", strategy.as_str()));
            let candidate_path =
                directory.join(format!("{}-strip-candidate.png", strategy.as_str()));
            fs::write(&input, &source).unwrap();
            let arguments = [
                OsString::from("imglean"),
                OsString::from(WORKER_ROLE),
                OsString::from(strategy.as_str()),
                OsString::from(LIMITS_VERSION),
                OsString::from("55"),
                OsString::from("strip"),
                OsString::from("80"),
                input.into_os_string(),
                candidate_path.clone().into_os_string(),
            ];
            assert_eq!(run_private(&arguments), 0);
            let candidate = fs::read(candidate_path).unwrap();
            assert!(!candidate.windows(4).any(|bytes| bytes == b"tEXt"));
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bundled_jpeg_strategies_preserve_or_strip_exif_as_requested() {
        let _provider_guard = BUNDLED_PROVIDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = test_directory();
        let source = jpeg_with_exif();
        for strategy in [
            StrategyId::Jpegtran,
            StrategyId::Mozjpeg,
            StrategyId::Jpegli,
        ] {
            for (policy, should_preserve) in [("preserve", true), ("strip", false)] {
                let input = directory.join(format!("{}-{policy}-input.jpg", strategy.as_str()));
                let candidate =
                    directory.join(format!("{}-{policy}-candidate.jpg", strategy.as_str()));
                fs::write(&input, &source).unwrap();
                let arguments = [
                    OsString::from("imglean"),
                    OsString::from(WORKER_ROLE),
                    OsString::from(strategy.as_str()),
                    OsString::from(LIMITS_VERSION),
                    OsString::from("55"),
                    OsString::from(policy),
                    OsString::from("80"),
                    input.into_os_string(),
                    candidate.clone().into_os_string(),
                ];
                assert_eq!(run_private(&arguments), 0);
                let candidate = fs::read(candidate).unwrap();
                assert_eq!(
                    candidate.windows(6).any(|bytes| bytes == b"Exif\0\0"),
                    should_preserve
                );
                assert_eq!(
                    candidate
                        .windows(b"imglean-app15".len())
                        .any(|bytes| bytes == b"imglean-app15"),
                    should_preserve
                );
                if should_preserve && strategy != StrategyId::Jpegtran {
                    assert_eq!(
                        candidate
                            .windows(b"JFIF\0".len())
                            .filter(|bytes| *bytes == b"JFIF\0")
                            .count(),
                        1
                    );
                    assert!(
                        !candidate
                            .windows(b"Adobe\0d\0\0\0\0\0".len())
                            .any(|bytes| bytes == b"Adobe\0d\0\0\0\0\0")
                    );
                }
                crate::jpeg::validate_source(&candidate).unwrap();
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bundled_webp_strategies_preserve_or_strip_exif_as_requested() {
        let _provider_guard = BUNDLED_PROVIDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = test_directory();
        let source = include_bytes!("../tests/corpus/webp/v1/accepted/metadata.webp");
        for strategy in [StrategyId::Libwebp, StrategyId::ImageWebp] {
            for (policy, should_preserve) in [("preserve", true), ("strip", false)] {
                let input = directory.join(format!("{}-{policy}-input.webp", strategy.as_str()));
                let candidate =
                    directory.join(format!("{}-{policy}-candidate.webp", strategy.as_str()));
                fs::write(&input, source).unwrap();
                let arguments = [
                    OsString::from("imglean"),
                    OsString::from(WORKER_ROLE),
                    OsString::from(strategy.as_str()),
                    OsString::from(LIMITS_VERSION),
                    OsString::from("55"),
                    OsString::from(policy),
                    OsString::from("80"),
                    input.into_os_string(),
                    candidate.clone().into_os_string(),
                ];
                assert_eq!(run_private(&arguments), 0);
                let candidate = fs::read(candidate).unwrap();
                assert_eq!(
                    candidate
                        .windows(b"imglean-exif-marker".len())
                        .any(|bytes| bytes == b"imglean-exif-marker"),
                    should_preserve
                );
                crate::webp::validate_source(&candidate).unwrap();
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn bundled_worker_receives_the_strategy_timeout_minus_cleanup_reserve() {
        let strategy = Strategy {
            id: StrategyId::OxipngLibdeflate,
            execution: Execution::Bundled,
        };
        let command = strategy_command(
            &strategy,
            Quality::Lossless,
            true,
            Duration::from_secs(90),
            Path::new("non-ASCII-ä/private-input.png"),
            Path::new("non-ASCII-ä/candidate.png"),
        )
        .unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new(WORKER_ROLE),
                OsStr::new("oxipng-libdeflate"),
                OsStr::new(LIMITS_VERSION),
                OsStr::new("85"),
                OsStr::new("strip"),
                OsStr::new("lossless"),
                OsStr::new("private-input.png"),
                OsStr::new("candidate.png"),
            ]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("non-ASCII-ä")));
    }

    #[test]
    fn optipng_command_pins_every_adapter_argument() {
        let strategy = external_strategy(PathBuf::from("provider"));
        let command = strategy_command(
            &strategy,
            Quality::Lossless,
            false,
            DEFAULT_STRATEGY_TIMEOUT,
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

        let command = strategy_command(
            &strategy,
            Quality::Lossless,
            true,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.png"),
            Path::new("candidate.png"),
        )
        .unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-quiet"),
                OsStr::new("-o2"),
                OsStr::new("-strip"),
                OsStr::new("all"),
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
            true,
            DEFAULT_STRATEGY_TIMEOUT,
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

    #[test]
    fn jpeg_commands_map_common_quality_and_pin_adapter_arguments() {
        let jpegtran = Strategy {
            id: StrategyId::Jpegtran,
            execution: Execution::External {
                executable: PathBuf::from("jpegtran"),
            },
        };
        let command = strategy_command(
            &jpegtran,
            Quality::Lossless,
            false,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.jpg"),
            Path::new("candidate.jpg"),
        )
        .unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-copy"),
                OsStr::new("all"),
                OsStr::new("-optimize"),
                OsStr::new("-progressive"),
                OsStr::new("-strict"),
                OsStr::new("-outfile"),
                OsStr::new("candidate.jpg"),
                OsStr::new("private-input.jpg"),
            ]
        );

        let command = strategy_command(
            &jpegtran,
            Quality::Lossless,
            true,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.jpg"),
            Path::new("candidate.jpg"),
        )
        .unwrap();
        assert_eq!(command.get_args().nth(1), Some(OsStr::new("none")));

        let mozjpeg = Strategy {
            id: StrategyId::Mozjpeg,
            execution: Execution::External {
                executable: PathBuf::from("mozjpeg"),
            },
        };
        let command = strategy_command(
            &mozjpeg,
            Quality::Numeric(82),
            true,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.jpg"),
            Path::new("candidate.jpg"),
        )
        .unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-quality"),
                OsStr::new("82"),
                OsStr::new("-progressive"),
                OsStr::new("-optimize"),
                OsStr::new("-strict"),
                OsStr::new("-outfile"),
                OsStr::new("candidate.jpg"),
                OsStr::new("private-input.jpg"),
            ]
        );

        let jpegli = Strategy {
            id: StrategyId::Jpegli,
            execution: Execution::External {
                executable: PathBuf::from("jpegli"),
            },
        };
        let command = strategy_command(
            &jpegli,
            Quality::Numeric(82),
            true,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.jpg"),
            Path::new("candidate.jpg"),
        )
        .unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("--quality"),
                OsStr::new("82"),
                OsStr::new("--progressive_level"),
                OsStr::new("2"),
                OsStr::new("private-input.jpg"),
                OsStr::new("candidate.jpg"),
            ]
        );
    }

    #[test]
    fn libwebp_command_maps_quality_metadata_and_exact_lossless_settings() {
        let strategy = Strategy {
            id: StrategyId::Libwebp,
            execution: Execution::External {
                executable: PathBuf::from("cwebp"),
            },
        };
        let lossless = strategy_command(
            &strategy,
            Quality::Lossless,
            false,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.webp"),
            Path::new("candidate.webp"),
        )
        .unwrap();
        assert_eq!(
            lossless.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-quiet"),
                OsStr::new("-lossless"),
                OsStr::new("-exact"),
                OsStr::new("-q"),
                OsStr::new("100"),
                OsStr::new("-m"),
                OsStr::new("6"),
                OsStr::new("-alpha_q"),
                OsStr::new("100"),
                OsStr::new("-metadata"),
                OsStr::new("all"),
                OsStr::new("-o"),
                OsStr::new("candidate.webp"),
                OsStr::new("--"),
                OsStr::new("private-input.webp"),
            ]
        );

        let numeric = strategy_command(
            &strategy,
            Quality::Numeric(72),
            true,
            DEFAULT_STRATEGY_TIMEOUT,
            Path::new("private-input.webp"),
            Path::new("candidate.webp"),
        )
        .unwrap();
        assert_eq!(
            numeric.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-quiet"),
                OsStr::new("-q"),
                OsStr::new("72"),
                OsStr::new("-m"),
                OsStr::new("6"),
                OsStr::new("-alpha_q"),
                OsStr::new("100"),
                OsStr::new("-metadata"),
                OsStr::new("none"),
                OsStr::new("-o"),
                OsStr::new("candidate.webp"),
                OsStr::new("--"),
                OsStr::new("private-input.webp"),
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
                false,
                DEFAULT_STRATEGY_TIMEOUT,
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
            false,
            DEFAULT_STRATEGY_TIMEOUT,
        );
        assert!(matches!(
            result,
            StrategyResult::Warning(message) if message.contains("provider failed")
        ));

        let timeout = directory.join("timeout-optipng");
        write_executable(&timeout, "#!/bin/sh\nwhile :; do :; done\n");
        assert_eq!(
            run_strategy(
                &mut artifacts,
                &source,
                &external_strategy(timeout),
                Quality::Lossless,
                false,
                Duration::from_millis(20),
            ),
            StrategyResult::Warning("worker timeout exceeded".to_owned())
        );

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
                false,
                DEFAULT_STRATEGY_TIMEOUT,
            ),
            StrategyResult::Failure("the private provider input changed during execution")
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 4);
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
                false,
                DEFAULT_STRATEGY_TIMEOUT,
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

    fn png_with_text_metadata() -> Vec<u8> {
        let mut png = compressible_png();
        let iend = png.split_off(png.len() - 12);
        push_chunk(&mut png, b"tEXt", b"Comment\0worker integration");
        png.extend_from_slice(&iend);
        png
    }

    fn jpeg_with_exif() -> Vec<u8> {
        let source = include_bytes!("../tests/corpus/jpeg/v1/accepted/provider-reduction.jpg");
        let mut jpeg = source[..2].to_vec();
        push_jpeg_segment(&mut jpeg, 0xe1, b"Exif\0\0II*\0\x08\0\0\0\0\0\0\0");
        push_jpeg_segment(&mut jpeg, 0xee, b"Adobe\0d\0\0\0\0\0");
        push_jpeg_segment(&mut jpeg, 0xef, b"imglean-app15");
        jpeg.extend_from_slice(&source[2..]);
        jpeg
    }

    fn push_jpeg_segment(jpeg: &mut Vec<u8>, marker: u8, payload: &[u8]) {
        jpeg.extend_from_slice(&[0xff, marker]);
        jpeg.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        jpeg.extend_from_slice(payload);
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
            id: StrategyId::Optipng,
            execution: Execution::External { executable },
        }
    }

    fn pngquant_strategy(executable: PathBuf) -> Strategy {
        Strategy {
            id: StrategyId::Pngquant,
            execution: Execution::External { executable },
        }
    }
}
