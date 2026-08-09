use std::io::Write;
use std::time::Instant;

use crate::artifacts::Artifacts;
use crate::cli::Arguments;
use crate::diagnostics::escape_path;
use crate::input::{self, CaptureError, PreflightError, PreflightInput};
use crate::limits::{INVOCATION_TIMEOUT, MAX_AGGREGATE_SOURCE_BYTES};
use crate::output::{self, OutputError};
use crate::png::{self, ValidatedPng};
use crate::worker::{self, StrategyResult};

pub fn run(arguments: Arguments, stdout: impl Write, stderr: impl Write) -> i32 {
    run_with_strategy(
        arguments,
        stdout,
        stderr,
        MAX_AGGREGATE_SOURCE_BYTES,
        INVOCATION_TIMEOUT,
        worker::run_strategy,
    )
}

fn run_with_strategy(
    arguments: Arguments,
    mut stdout: impl Write,
    mut stderr: impl Write,
    maximum_aggregate_bytes: u64,
    invocation_timeout: std::time::Duration,
    mut strategy: impl FnMut(&mut Artifacts, &[u8]) -> StrategyResult,
) -> i32 {
    let mut budget = InvocationBudget {
        started: Instant::now(),
        timeout: invocation_timeout,
        maximum_aggregate_bytes,
        aggregate_bytes: 0,
    };
    let mut batch = match input::preflight(arguments) {
        Ok(batch) => batch,
        Err(error) => {
            write_stderr(&mut stderr, &format_preflight_error(&error));
            write_stderr(
                &mut stderr,
                "imglean: structural preflight failed; no outputs were created\n",
            );
            return 1;
        }
    };

    let mut artifacts = Artifacts::new(batch.output_directory.clone());
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut warnings = 0usize;
    let mut processed = 0usize;
    let mut stopped = false;

    for input in &mut batch.inputs {
        if budget.elapsed() {
            write_stderr(
                &mut stderr,
                "imglean: invocation elapsed-time limit exceeded\n",
            );
            stopped = true;
            break;
        }

        let outcome = process_input(
            input,
            &mut artifacts,
            &mut budget,
            &mut stderr,
            &mut strategy,
        );
        let mut stop_after_reporting = false;
        let result_line = match outcome {
            InputOutcome::Success {
                source_bytes,
                output_bytes,
                optimizer_warning,
            } => {
                succeeded += 1;
                warnings += usize::from(optimizer_warning);
                format!(
                    "ok {} -> {} ({source_bytes} -> {output_bytes} bytes)\n",
                    escape_path(input.canonical_source.as_os_str()),
                    escape_path(input.destination.as_os_str())
                )
            }
            InputOutcome::Failure(reason) => {
                failed += 1;
                write_stderr(
                    &mut stderr,
                    &format!(
                        "imglean: {}: {reason}\n",
                        escape_path(input.canonical_source.as_os_str())
                    ),
                );
                format!(
                    "failed {}\n",
                    escape_path(input.canonical_source.as_os_str())
                )
            }
            InputOutcome::InvocationFailure(reason) => {
                failed += 1;
                stop_after_reporting = true;
                write_stderr(
                    &mut stderr,
                    &format!(
                        "imglean: {}: {reason}\n",
                        escape_path(input.canonical_source.as_os_str())
                    ),
                );
                format!(
                    "failed {}\n",
                    escape_path(input.canonical_source.as_os_str())
                )
            }
        };
        processed += 1;
        if write_required(&mut stdout, result_line.as_bytes()).is_err() {
            write_stderr(
                &mut stderr,
                "imglean: required standard-output reporting failed\n",
            );
            failed += 1;
            stopped = true;
            break;
        }
        if stop_after_reporting {
            stopped = true;
            break;
        }
    }

    let not_processed = batch.inputs.len().saturating_sub(processed);
    write_stderr(
        &mut stderr,
        &format!(
            "imglean: {succeeded} succeeded, {failed} failed, {warnings} optimizer warnings, {not_processed} not processed\n"
        ),
    );

    if failed > 0 || stopped {
        1
    } else if warnings > 0 {
        3
    } else {
        0
    }
}

fn process_input(
    input: &mut PreflightInput,
    artifacts: &mut Artifacts,
    budget: &mut InvocationBudget,
    stderr: &mut impl Write,
    strategy: &mut impl FnMut(&mut Artifacts, &[u8]) -> StrategyResult,
) -> InputOutcome {
    let source_bytes =
        match input.capture(&mut budget.aggregate_bytes, budget.maximum_aggregate_bytes) {
            Ok(bytes) => bytes,
            Err(CaptureError::AggregateLimit) => {
                return InputOutcome::InvocationFailure(
                    "invocation aggregate source-byte limit exceeded",
                );
            }
            Err(error) => return InputOutcome::Failure(capture_reason(&error)),
        };
    let source = match png::validate_source(&source_bytes) {
        Ok(source) => source,
        Err(error) => return InputOutcome::Failure(error.message()),
    };

    let mut optimizer_warning = false;
    let candidate = match strategy(artifacts, &source_bytes) {
        StrategyResult::Candidate(bytes) => match png::validate_candidate(&source, &bytes) {
            Ok(validated) => Some((bytes, validated)),
            Err(error) => {
                optimizer_warning = true;
                write_stderr(
                    stderr,
                    &format!(
                        "imglean: warning: OxiPNG candidate rejected for {}: {}\n",
                        escape_path(input.canonical_source.as_os_str()),
                        error.message()
                    ),
                );
                None
            }
        },
        StrategyResult::Warning(message) => {
            optimizer_warning = true;
            write_stderr(
                stderr,
                &format!(
                    "imglean: warning: {}: {message}\n",
                    escape_path(input.canonical_source.as_os_str())
                ),
            );
            None
        }
        StrategyResult::Failure(reason) => return InputOutcome::Failure(reason),
    };

    let (winner, output_bytes) = select_winner(&source_bytes, &source, candidate.as_ref());
    if budget.elapsed() {
        return InputOutcome::InvocationFailure("invocation elapsed-time limit exceeded");
    }
    let prepared = match output::prepare(artifacts, &source, winner) {
        Ok(prepared) => prepared,
        Err(error) => return InputOutcome::Failure(output_reason(&error)),
    };
    if let Err(error) = output::publish(artifacts, prepared, &input.destination) {
        return InputOutcome::Failure(output_reason(&error));
    }
    InputOutcome::Success {
        source_bytes: source.encoded_bytes(),
        output_bytes,
        optimizer_warning,
    }
}

struct InvocationBudget {
    started: Instant,
    timeout: std::time::Duration,
    maximum_aggregate_bytes: u64,
    aggregate_bytes: u64,
}

impl InvocationBudget {
    fn elapsed(&self) -> bool {
        self.started.elapsed() > self.timeout
    }
}

fn select_winner<'a>(
    source_bytes: &'a [u8],
    source: &ValidatedPng,
    candidate: Option<&'a (Vec<u8>, ValidatedPng)>,
) -> (&'a [u8], usize) {
    if let Some((bytes, validated)) = candidate
        && validated.encoded_bytes() < source.encoded_bytes()
    {
        (bytes, validated.encoded_bytes())
    } else {
        (source_bytes, source.encoded_bytes())
    }
}

enum InputOutcome {
    Success {
        source_bytes: usize,
        output_bytes: usize,
        optimizer_warning: bool,
    },
    Failure(&'static str),
    InvocationFailure(&'static str),
}

fn capture_reason(error: &CaptureError) -> &'static str {
    match error {
        CaptureError::Source { reason, .. } => reason,
        CaptureError::AggregateLimit => "invocation aggregate source-byte limit exceeded",
    }
}

fn output_reason(error: &OutputError) -> &'static str {
    match error {
        OutputError::BeforePublication(reason) | OutputError::AfterPublication(reason) => reason,
    }
}

fn format_preflight_error(error: &PreflightError) -> String {
    match error {
        PreflightError::WorkingDirectory => {
            "imglean: cannot capture the initial working directory\n".to_owned()
        }
        PreflightError::OutputDirectory(reason) => format!("imglean: output: {reason}\n"),
        PreflightError::Input { path, reason } => {
            format!(
                "imglean: input {}: {reason}\n",
                escape_path(path.as_os_str())
            )
        }
        PreflightError::Destination { path, reason } => format!(
            "imglean: destination {}: {reason}\n",
            escape_path(path.as_os_str())
        ),
    }
}

fn write_required(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)?;
    writer.flush()
}

fn write_stderr(writer: &mut impl Write, message: &str) {
    let _ = writer.write_all(message.as_bytes());
    let _ = writer.flush();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{self, Write as _};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn baseline_wins_equal_size_tie() {
        let source_bytes = valid_png();
        let source = png::validate_source(&source_bytes).unwrap();
        let candidate = png::validate_candidate(&source, &source_bytes).unwrap();
        let candidate_pair = (source_bytes.clone(), candidate);
        let (winner, size) = select_winner(&source_bytes, &source, Some(&candidate_pair));
        assert_eq!(winner, source_bytes);
        assert_eq!(size, source.encoded_bytes());
    }

    #[test]
    fn structural_failure_writes_no_stdout() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_with_strategy(
            Arguments {
                output_directory: PathBuf::from("missing"),
                inputs: vec![PathBuf::from("missing.png")],
            },
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            |_, _| panic!("strategy must not run"),
        );
        assert_eq!(status, 1);
        assert!(stdout.is_empty());
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("structural preflight failed")
        );
    }

    #[test]
    fn stdout_failure_stops_later_commits() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let first = directory.path.join("first.png");
        let second = directory.path.join("second.png");
        let bytes = valid_png();
        fs::write(&first, &bytes).unwrap();
        fs::write(&second, &bytes).unwrap();
        let mut stderr = Vec::new();
        let status = run_with_strategy(
            Arguments {
                output_directory: output.clone(),
                inputs: vec![first, second],
            },
            FailingWriter,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            |_, source| StrategyResult::Candidate(source.to_vec()),
        );
        assert_eq!(status, 1);
        assert!(output.join("first.png").exists());
        assert!(!output.join("second.png").exists());
    }

    #[test]
    fn aggregate_limit_stops_later_inputs() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let first = directory.path.join("first.png");
        let second = directory.path.join("second.png");
        let bytes = valid_png();
        fs::write(&first, &bytes).unwrap();
        fs::write(&second, &bytes).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategy(
            Arguments {
                output_directory: output.clone(),
                inputs: vec![first, second],
            },
            &mut stdout,
            &mut stderr,
            0,
            INVOCATION_TIMEOUT,
            |_, _| panic!("strategy must not run"),
        );

        assert_eq!(status, 1);
        assert_eq!(String::from_utf8(stdout).unwrap().lines().count(), 1);
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("aggregate source-byte limit exceeded"));
        assert!(stderr.contains("1 not processed"));
        assert!(!output.join("first.png").exists());
        assert!(!output.join("second.png").exists());
    }

    #[test]
    fn elapsed_limit_before_processing_commits_nothing() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let first = directory.path.join("first.png");
        let second = directory.path.join("second.png");
        let bytes = valid_png();
        fs::write(&first, &bytes).unwrap();
        fs::write(&second, &bytes).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategy(
            Arguments {
                output_directory: output.clone(),
                inputs: vec![first, second],
            },
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            std::time::Duration::ZERO,
            |_, _| panic!("strategy must not run"),
        );

        assert_eq!(status, 1);
        assert!(stdout.is_empty());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("elapsed-time limit exceeded"));
        assert!(stderr.contains("2 not processed"));
        assert!(!output.join("first.png").exists());
        assert!(!output.join("second.png").exists());
    }

    #[test]
    fn larger_valid_candidate_keeps_baseline_without_warning() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let bytes = valid_png();
        fs::write(&source, &bytes).unwrap();
        let larger = larger_valid_candidate(&bytes);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategy(
            Arguments {
                output_directory: output.clone(),
                inputs: vec![source],
            },
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            |_, _| StrategyResult::Candidate(larger.clone()),
        );

        assert_eq!(status, 0);
        assert_eq!(fs::read(output.join("source.png")).unwrap(), bytes);
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("0 optimizer warnings")
        );
    }

    #[test]
    fn optimizing_strategy_warning_keeps_baseline_and_returns_three() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let bytes = valid_png();
        fs::write(&source, &bytes).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategy(
            Arguments {
                output_directory: output.clone(),
                inputs: vec![source],
            },
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            |_, _| StrategyResult::Warning("injected provider failure".to_owned()),
        );

        assert_eq!(status, 3);
        assert_eq!(fs::read(output.join("source.png")).unwrap(), bytes);
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("injected provider failure"));
        assert!(stderr.contains("1 optimizer warnings"));
    }

    struct FailingWriter;

    impl io::Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
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

    fn larger_valid_candidate(source: &[u8]) -> Vec<u8> {
        let first_idat = 8 + 12 + 13;
        let mut candidate = source[..first_idat].to_vec();
        push_chunk(&mut candidate, b"IDAT", &[]);
        candidate.extend_from_slice(&source[first_idat..]);
        candidate
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
            let path = std::env::temp_dir().join(format!(
                "imglean-controller-test-{}-{unique}",
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
