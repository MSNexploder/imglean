use std::fmt::Write as _;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use crate::artifacts::Artifacts;
use crate::cli::Arguments;
use crate::diagnostics::escape_path;
use crate::input::{self, CaptureError, PreflightError, PreflightInput};
use crate::limits::{INVOCATION_TIMEOUT, MAX_AGGREGATE_SOURCE_BYTES};
use crate::output::{self, OutputError};
use crate::png;
use crate::strategy::{self, Execution, RegistryEntry, RegistryState, Strategy, StrategyId};
use crate::worker::{self, StrategyResult};

pub fn run(arguments: Arguments, stdout: impl Write, mut stderr: impl Write) -> i32 {
    let quality = arguments.strategies.quality;
    let registry = match strategy::resolve(&arguments.strategies) {
        Ok(registry) => registry,
        Err(error) => {
            let mut stderr = stderr;
            write_stderr(
                &mut stderr,
                &format!("imglean: provider preflight failed: {}\n", error.message()),
            );
            write_stderr(
                &mut stderr,
                "imglean: structural preflight failed; no outputs were created\n",
            );
            return 1;
        }
    };
    for entry in &registry {
        if let RegistryState::Runnable(Execution::External {
            executable,
            version,
        }) = &entry.state
        {
            write_stderr(
                &mut stderr,
                &format!(
                    "imglean: using {} provider version {version} at {}\n",
                    entry.id,
                    escape_path(executable.as_os_str())
                ),
            );
        }
    }
    run_with_strategies(
        arguments,
        stdout,
        stderr,
        MAX_AGGREGATE_SOURCE_BYTES,
        INVOCATION_TIMEOUT,
        registry,
        move |artifacts, source, strategy| {
            worker::run_strategy(artifacts, source, strategy, quality)
        },
    )
}

fn run_with_strategies(
    arguments: Arguments,
    mut stdout: impl Write,
    mut stderr: impl Write,
    maximum_aggregate_bytes: u64,
    invocation_timeout: std::time::Duration,
    registry: Vec<RegistryEntry>,
    execute: impl Fn(&mut Artifacts, &[u8], &Strategy) -> StrategyResult + Sync,
) -> i32 {
    let jobs = arguments.jobs;
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
            &registry,
            jobs,
            &execute,
        );
        let mut stop_after_reporting = false;
        let result_line = match outcome {
            InputOutcome::Success {
                source_bytes,
                output_bytes,
                winner,
                attempts,
            } => {
                succeeded += 1;
                warnings += warning_count(&attempts);
                format_success(input, source_bytes, output_bytes, winner, &attempts)
            }
            InputOutcome::Failure { reason, attempts } => {
                failed += 1;
                warnings += warning_count(&attempts);
                write_stderr(
                    &mut stderr,
                    &format!(
                        "imglean: {}: {reason}\n",
                        escape_path(input.canonical_source.as_os_str())
                    ),
                );
                format_failure(input, &attempts)
            }
            InputOutcome::InvocationFailure { reason, attempts } => {
                failed += 1;
                warnings += warning_count(&attempts);
                stop_after_reporting = true;
                write_stderr(
                    &mut stderr,
                    &format!(
                        "imglean: {}: {reason}\n",
                        escape_path(input.canonical_source.as_os_str())
                    ),
                );
                format_failure(input, &attempts)
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
        &format_summary(succeeded, failed, warnings, not_processed),
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
    registry: &[RegistryEntry],
    jobs: usize,
    execute: &(impl Fn(&mut Artifacts, &[u8], &Strategy) -> StrategyResult + Sync),
) -> InputOutcome {
    let mut attempts = registry.iter().map(Attempt::from).collect::<Vec<_>>();
    let source_bytes =
        match input.capture(&mut budget.aggregate_bytes, budget.maximum_aggregate_bytes) {
            Ok(bytes) => bytes,
            Err(CaptureError::AggregateLimit) => {
                return InputOutcome::InvocationFailure {
                    reason: "invocation aggregate source-byte limit exceeded",
                    attempts,
                };
            }
            Err(error) => {
                return InputOutcome::Failure {
                    reason: capture_reason(&error),
                    attempts,
                };
            }
        };
    let source = match png::validate_source(&source_bytes) {
        Ok(source) => source,
        Err(error) => {
            return InputOutcome::Failure {
                reason: error.message(),
                attempts,
            };
        }
    };

    let mut winner = None::<Vec<u8>>;
    let mut winner_strategy = None;
    let mut output_bytes = source.encoded_bytes();
    if budget.elapsed() {
        return InputOutcome::InvocationFailure {
            reason: "invocation elapsed-time limit exceeded",
            attempts,
        };
    }
    let runnable = registry
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match &entry.state {
            RegistryState::Runnable(execution) => Some((
                index,
                Strategy {
                    id: entry.id,
                    execution: execution.clone(),
                },
            )),
            RegistryState::Disabled | RegistryState::Unavailable | RegistryState::NotApplicable => {
                None
            }
        })
        .collect::<Vec<_>>();
    let (scheduled, spawn_failed) = execute_parallel(
        artifacts.directory(),
        &source_bytes,
        runnable,
        jobs,
        budget.deadline(),
        execute,
    );
    let mut failure = None;
    let mut elapsed = false;
    for (index, result) in scheduled {
        let Some(result) = result else {
            elapsed = true;
            continue;
        };
        let strategy = attempts[index].strategy;
        let outcome = match result {
            StrategyResult::Candidate(bytes) => match png::validate_candidate(&source, &bytes) {
                Ok(validated) if validated.encoded_bytes() < output_bytes => {
                    output_bytes = validated.encoded_bytes();
                    winner = Some(bytes);
                    winner_strategy = Some(strategy);
                    AttemptOutcome::Candidate(validated.encoded_bytes())
                }
                Ok(validated) => AttemptOutcome::Candidate(validated.encoded_bytes()),
                Err(error) => AttemptOutcome::Rejected(error.message()),
            },
            StrategyResult::NoCandidate => AttemptOutcome::NoCandidate,
            StrategyResult::Warning(message) => AttemptOutcome::Warning(message),
            StrategyResult::Failure(reason) => {
                failure.get_or_insert(reason);
                AttemptOutcome::Failed
            }
        };
        attempts[index].outcome = outcome;
    }

    if elapsed || budget.elapsed() {
        return InputOutcome::InvocationFailure {
            reason: "invocation elapsed-time limit exceeded",
            attempts,
        };
    }
    if spawn_failed {
        return InputOutcome::Failure {
            reason: "cannot create a strategy worker thread",
            attempts,
        };
    }
    if let Some(reason) = failure {
        return InputOutcome::Failure { reason, attempts };
    }

    let winner = winner.as_deref().unwrap_or(&source_bytes);
    if budget.elapsed() {
        return InputOutcome::InvocationFailure {
            reason: "invocation elapsed-time limit exceeded",
            attempts,
        };
    }
    let prepared = match output::prepare(artifacts, &source, winner) {
        Ok(prepared) => prepared,
        Err(error) => {
            return InputOutcome::Failure {
                reason: output_reason(&error),
                attempts,
            };
        }
    };
    if let Err(error) = output::publish(artifacts, prepared, &input.destination) {
        return InputOutcome::Failure {
            reason: output_reason(&error),
            attempts,
        };
    }
    InputOutcome::Success {
        source_bytes: source.encoded_bytes(),
        output_bytes,
        winner: winner_strategy,
        attempts,
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

    fn deadline(&self) -> InvocationDeadline {
        InvocationDeadline {
            started: self.started,
            timeout: self.timeout,
        }
    }
}

#[derive(Clone, Copy)]
struct InvocationDeadline {
    started: Instant,
    timeout: std::time::Duration,
}

impl InvocationDeadline {
    fn elapsed(self) -> bool {
        self.started.elapsed() > self.timeout
    }
}

fn execute_parallel(
    artifact_directory: &std::path::Path,
    source: &[u8],
    runnable: Vec<(usize, Strategy)>,
    jobs: usize,
    deadline: InvocationDeadline,
    execute: &(impl Fn(&mut Artifacts, &[u8], &Strategy) -> StrategyResult + Sync),
) -> (Vec<(usize, Option<StrategyResult>)>, bool) {
    if runnable.is_empty() {
        return (Vec::new(), false);
    }
    let worker_count = jobs.min(runnable.len());
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        let mut spawn_failed = false;
        for _ in 0..worker_count {
            let sender = sender.clone();
            let next = &next;
            let runnable = &runnable;
            let spawned = std::thread::Builder::new().spawn_scoped(scope, move || {
                loop {
                    let job = next.fetch_add(1, Ordering::Relaxed);
                    let Some((registry_index, strategy)) = runnable.get(job) else {
                        break;
                    };
                    let result = if deadline.elapsed() {
                        None
                    } else {
                        let mut artifacts = Artifacts::new(artifact_directory.to_path_buf());
                        Some(execute(&mut artifacts, source, strategy))
                    };
                    if sender.send((*registry_index, result)).is_err() {
                        break;
                    }
                }
            });
            if spawned.is_err() {
                spawn_failed = true;
                break;
            }
        }
        drop(sender);
        let mut results = receiver.into_iter().collect::<Vec<_>>();
        results.sort_by_key(|(index, _)| *index);
        (results, spawn_failed)
    })
}

enum InputOutcome {
    Success {
        source_bytes: usize,
        output_bytes: usize,
        winner: Option<StrategyId>,
        attempts: Vec<Attempt>,
    },
    Failure {
        reason: &'static str,
        attempts: Vec<Attempt>,
    },
    InvocationFailure {
        reason: &'static str,
        attempts: Vec<Attempt>,
    },
}

struct Attempt {
    strategy: StrategyId,
    outcome: AttemptOutcome,
}

enum AttemptOutcome {
    Candidate(usize),
    NoCandidate,
    Warning(String),
    Rejected(&'static str),
    Failed,
    Disabled,
    Unavailable,
    NotApplicable,
    NotRun,
}

impl From<&RegistryEntry> for Attempt {
    fn from(entry: &RegistryEntry) -> Self {
        let outcome = match entry.state {
            RegistryState::Runnable(_) => AttemptOutcome::NotRun,
            RegistryState::Disabled => AttemptOutcome::Disabled,
            RegistryState::Unavailable => AttemptOutcome::Unavailable,
            RegistryState::NotApplicable => AttemptOutcome::NotApplicable,
        };
        Self {
            strategy: entry.id,
            outcome,
        }
    }
}

fn capture_reason(error: &CaptureError) -> &'static str {
    match error {
        CaptureError::Source { reason, .. } => reason,
        CaptureError::AggregateLimit => "invocation aggregate source-byte limit exceeded",
    }
}

fn output_reason(error: &OutputError) -> &'static str {
    match error {
        OutputError::BeforePublication(reason) => reason,
    }
}

fn format_success(
    input: &PreflightInput,
    source_bytes: usize,
    output_bytes: usize,
    winner: Option<StrategyId>,
    attempts: &[Attempt],
) -> String {
    let mut report = format!("{}\n", input_label(input));
    push_candidate_line(
        &mut report,
        "baseline",
        source_bytes,
        winner.is_none().then_some((source_bytes, output_bytes)),
    );
    for attempt in attempts {
        push_attempt_line(
            &mut report,
            attempt,
            (winner == Some(attempt.strategy)).then_some((source_bytes, output_bytes)),
        );
    }
    let _ = writeln!(
        report,
        "     {:<24} {}\n",
        "output",
        escape_path(input.destination.as_os_str())
    );
    report
}

fn push_attempt_line(
    report: &mut String,
    attempt: &Attempt,
    winner_savings: Option<(usize, usize)>,
) {
    let label = attempt.strategy.as_str();
    match &attempt.outcome {
        AttemptOutcome::Candidate(bytes) => {
            push_candidate_line(report, label, *bytes, winner_savings)
        }
        AttemptOutcome::NoCandidate => {
            let _ = writeln!(report, "     {label:<24} no candidate");
        }
        AttemptOutcome::Warning(message) => {
            let _ = writeln!(report, "  !  {label:<24} warning: {message}");
        }
        AttemptOutcome::Rejected(reason) => {
            let _ = writeln!(report, "  !  {label:<24} rejected: {reason}");
        }
        AttemptOutcome::Failed => {
            let _ = writeln!(report, "  !! {label:<24} failed");
        }
        AttemptOutcome::Disabled => {
            let _ = writeln!(report, "     {label:<24} disabled");
        }
        AttemptOutcome::Unavailable => {
            let _ = writeln!(report, "     {label:<24} unavailable");
        }
        AttemptOutcome::NotApplicable => {
            let _ = writeln!(report, "     {label:<24} not applicable");
        }
        AttemptOutcome::NotRun => {
            let _ = writeln!(report, "     {label:<24} not run");
        }
    }
}

fn push_candidate_line(
    report: &mut String,
    label: &str,
    bytes: usize,
    winner_savings: Option<(usize, usize)>,
) {
    let marker = if winner_savings.is_some() {
        "  -> "
    } else {
        "     "
    };
    let _ = write!(report, "{marker}{label:<24} {} bytes", format_bytes(bytes));
    if let Some((source_bytes, output_bytes)) = winner_savings {
        let saved = source_bytes - output_bytes;
        if saved == 0 {
            report.push_str("  winner");
        } else {
            let percent = saved as f64 * 100.0 / source_bytes as f64;
            let _ = write!(
                report,
                "  winner; saved {} bytes ({percent:.2}%)",
                format_bytes(saved)
            );
        }
    }
    report.push('\n');
}

fn format_failure(input: &PreflightInput, attempts: &[Attempt]) -> String {
    let mut report = format!("{}\n", input_label(input));
    for attempt in attempts {
        push_attempt_line(&mut report, attempt, None);
    }
    report.push_str("  !! failed\n\n");
    report
}

fn warning_count(attempts: &[Attempt]) -> usize {
    attempts
        .iter()
        .filter(|attempt| {
            matches!(
                attempt.outcome,
                AttemptOutcome::Warning(_) | AttemptOutcome::Rejected(_)
            )
        })
        .count()
}

fn input_label(input: &PreflightInput) -> String {
    escape_path(
        input
            .destination
            .file_name()
            .unwrap_or(input.destination.as_os_str()),
    )
}

fn format_bytes(bytes: usize) -> String {
    let digits = bytes.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.bytes().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(char::from(digit));
    }
    formatted
}

fn format_summary(
    succeeded: usize,
    failed: usize,
    warnings: usize,
    not_processed: usize,
) -> String {
    let mut parts = vec![format!("{succeeded} succeeded")];
    if failed > 0 {
        parts.push(format!("{failed} failed"));
    }
    if warnings > 0 {
        let label = if warnings == 1 { "warning" } else { "warnings" };
        parts.push(format!("{warnings} {label}"));
    }
    if not_processed > 0 {
        parts.push(format!("{not_processed} not processed"));
    }
    format!("Summary: {}\n", parts.join(", "))
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
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Barrier, Mutex};

    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn baseline_wins_equal_size_tie() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let bytes = valid_png();
        fs::write(&source, &bytes).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            test_registry(),
            |_, source, _| StrategyResult::Candidate(source.to_vec()),
        );
        assert_eq!(status, 0);
        assert_eq!(fs::read(output.join("source.png")).unwrap(), bytes);
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("-> baseline                 67 bytes  winner")
        );
    }

    #[test]
    fn attempts_every_strategy_once_and_keeps_the_smallest_candidate() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let base = valid_png();
        let source_bytes = with_empty_idats(&base, 3);
        let first_candidate = with_empty_idats(&base, 2);
        let winner = base.clone();
        fs::write(&source, source_bytes).unwrap();
        let strategies = StrategyId::ALL
            .into_iter()
            .map(|id| RegistryEntry {
                id,
                state: RegistryState::Runnable(
                    if matches!(id, StrategyId::OptipngV1 | StrategyId::PngquantV1) {
                        Execution::External {
                            executable: PathBuf::from("unused"),
                            version: "7.9.1".to_owned(),
                        }
                    } else {
                        Execution::Embedded
                    },
                ),
            })
            .collect::<Vec<_>>();
        let attempted = Mutex::new(Vec::new());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            strategies,
            |_, _, strategy| {
                attempted.lock().unwrap().push(strategy.id);
                match strategy.id {
                    StrategyId::OxipngLibdeflateV1 => {
                        StrategyResult::Candidate(first_candidate.clone())
                    }
                    StrategyId::OxipngZopfliV1 => {
                        StrategyResult::Warning("injected failure".to_owned())
                    }
                    StrategyId::OptipngV1 => StrategyResult::Candidate(winner.clone()),
                    StrategyId::PngquantV1 => StrategyResult::NoCandidate,
                }
            },
        );

        assert_eq!(status, 3);
        let attempted = attempted.into_inner().unwrap();
        assert_eq!(attempted.len(), StrategyId::ALL.len());
        assert!(StrategyId::ALL.iter().all(|id| attempted.contains(id)));
        assert_eq!(fs::read(output.join("source.png")).unwrap(), base);
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.contains("baseline                 103 bytes"));
        assert!(stdout.contains("oxipng-libdeflate-v1     91 bytes"));
        assert!(stdout.contains("oxipng-zopfli-v1         warning: injected failure"));
        let winner_line = stdout
            .lines()
            .find(|line| line.contains("-> optipng-v1"))
            .expect("winner row");
        assert!(winner_line.contains("67 bytes  winner; saved 36 bytes"));
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "Summary: 1 succeeded, 1 warning\n"
        );
    }

    #[test]
    fn reports_runnable_disabled_and_unavailable_registry_entries() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let bytes = valid_png();
        fs::write(&source, &bytes).unwrap();
        let registry = vec![
            RegistryEntry {
                id: StrategyId::OxipngLibdeflateV1,
                state: RegistryState::Runnable(Execution::Embedded),
            },
            RegistryEntry {
                id: StrategyId::OxipngZopfliV1,
                state: RegistryState::Disabled,
            },
            RegistryEntry {
                id: StrategyId::OptipngV1,
                state: RegistryState::Unavailable,
            },
            RegistryEntry {
                id: StrategyId::PngquantV1,
                state: RegistryState::NotApplicable,
            },
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategies(
            arguments(output, vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            registry,
            |_, source, _| StrategyResult::Candidate(source.to_vec()),
        );

        assert_eq!(status, 0);
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.contains("oxipng-libdeflate-v1     67 bytes"));
        assert!(stdout.contains("oxipng-zopfli-v1         disabled"));
        assert!(stdout.contains("optipng-v1               unavailable"));
        assert!(stdout.contains("pngquant-v1              not applicable"));
    }

    #[test]
    fn bounds_parallel_strategy_execution_to_the_requested_jobs() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        fs::write(&source, valid_png()).unwrap();
        let registry = StrategyId::ALL
            .into_iter()
            .map(|id| RegistryEntry {
                id,
                state: RegistryState::Runnable(Execution::Embedded),
            })
            .collect();
        let barrier = Barrier::new(2);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let counts = [
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
            AtomicUsize::new(0),
        ];
        let mut arguments = arguments(output, vec![source]);
        arguments.jobs = 2;

        let status = run_with_strategies(
            arguments,
            Vec::new(),
            Vec::new(),
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            registry,
            |_, _, strategy| {
                let index = StrategyId::ALL
                    .iter()
                    .position(|id| *id == strategy.id)
                    .unwrap();
                counts[index].fetch_add(1, Ordering::Relaxed);
                let current = active.fetch_add(1, Ordering::Relaxed) + 1;
                maximum.fetch_max(current, Ordering::Relaxed);
                if matches!(
                    strategy.id,
                    StrategyId::OxipngLibdeflateV1 | StrategyId::OxipngZopfliV1
                ) {
                    barrier.wait();
                }
                active.fetch_sub(1, Ordering::Relaxed);
                StrategyResult::NoCandidate
            },
        );

        assert_eq!(status, 0);
        assert_eq!(maximum.load(Ordering::Relaxed), 2);
        assert!(
            counts
                .iter()
                .all(|count| count.load(Ordering::Relaxed) == 1)
        );
    }

    #[test]
    fn parallel_equal_size_candidates_keep_registry_order() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let winner = valid_png();
        fs::write(&source, with_empty_idats(&winner, 2)).unwrap();
        let registry = StrategyId::EMBEDDED
            .into_iter()
            .map(|id| RegistryEntry {
                id,
                state: RegistryState::Runnable(Execution::Embedded),
            })
            .collect();
        let mut arguments = arguments(output, vec![source]);
        arguments.jobs = 2;
        let mut stdout = Vec::new();

        let status = run_with_strategies(
            arguments,
            &mut stdout,
            Vec::new(),
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            registry,
            |_, _, _| StrategyResult::Candidate(winner.clone()),
        );

        assert_eq!(status, 0);
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("-> oxipng-libdeflate-v1")
        );
    }

    #[test]
    fn structural_failure_writes_no_stdout() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_with_strategies(
            arguments(PathBuf::from("missing"), vec![PathBuf::from("missing.png")]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            test_registry(),
            |_, _, _| panic!("strategy must not run"),
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
        let status = run_with_strategies(
            arguments(output.clone(), vec![first, second]),
            FailingWriter,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            test_registry(),
            |_, source, _| StrategyResult::Candidate(source.to_vec()),
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

        let status = run_with_strategies(
            arguments(output.clone(), vec![first, second]),
            &mut stdout,
            &mut stderr,
            0,
            INVOCATION_TIMEOUT,
            test_registry(),
            |_, _, _| panic!("strategy must not run"),
        );

        assert_eq!(status, 1);
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.starts_with("first.png\n"));
        assert!(stdout.contains("oxipng-libdeflate-v1     not run"));
        assert!(stdout.contains("  !! failed"));
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

        let status = run_with_strategies(
            arguments(output.clone(), vec![first, second]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            std::time::Duration::ZERO,
            test_registry(),
            |_, _, _| panic!("strategy must not run"),
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
    fn elapsed_limit_stops_before_the_next_strategy() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        fs::write(&source, valid_png()).unwrap();
        let strategies = StrategyId::EMBEDDED
            .into_iter()
            .map(|id| RegistryEntry {
                id,
                state: RegistryState::Runnable(Execution::Embedded),
            })
            .collect();
        let attempts = AtomicUsize::new(0);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            std::time::Duration::from_secs(1),
            strategies,
            |_, source, _| {
                attempts.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(std::time::Duration::from_millis(1_100));
                StrategyResult::Candidate(source.to_vec())
            },
        );

        assert_eq!(status, 1);
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
        assert!(!output.join("source.png").exists());
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("elapsed-time limit exceeded")
        );
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

        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            test_registry(),
            |_, _, _| StrategyResult::Candidate(larger.clone()),
        );

        assert_eq!(status, 0);
        assert_eq!(fs::read(output.join("source.png")).unwrap(), bytes);
        assert_eq!(String::from_utf8(stderr).unwrap(), "Summary: 1 succeeded\n");
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

        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            test_registry(),
            |_, _, _| StrategyResult::Warning("injected provider failure".to_owned()),
        );

        assert_eq!(status, 3);
        assert_eq!(fs::read(output.join("source.png")).unwrap(), bytes);
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("warning: injected provider failure")
        );
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "Summary: 1 succeeded, 1 warning\n"
        );
    }

    #[test]
    fn later_strategy_failure_preserves_earlier_warning() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        fs::write(&source, valid_png()).unwrap();
        let strategies = StrategyId::EMBEDDED
            .into_iter()
            .map(|id| RegistryEntry {
                id,
                state: RegistryState::Runnable(Execution::Embedded),
            })
            .collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            strategies,
            |_, _, strategy| match strategy.id {
                StrategyId::OxipngLibdeflateV1 => {
                    StrategyResult::Warning("injected warning".to_owned())
                }
                StrategyId::OxipngZopfliV1 => StrategyResult::Failure("injected fatal failure"),
                StrategyId::OptipngV1 | StrategyId::PngquantV1 => unreachable!(),
            },
        );

        assert_eq!(status, 1);
        assert!(!output.join("source.png").exists());
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.contains("warning: injected warning"));
        assert!(stdout.contains("oxipng-zopfli-v1         failed"));
        assert!(stdout.contains("!! failed"));
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("injected fatal failure"));
        assert!(stderr.contains("Summary: 0 succeeded, 1 failed, 1 warning"));
    }

    #[test]
    fn missing_candidate_is_normal_but_malformed_candidate_warns() {
        let directory = TestDirectory::new();
        let output = directory.create_directory("out");
        let source = directory.path.join("source.png");
        let bytes = valid_png();
        fs::write(&source, &bytes).unwrap();
        let strategies = StrategyId::EMBEDDED
            .into_iter()
            .map(|id| RegistryEntry {
                id,
                state: RegistryState::Runnable(Execution::Embedded),
            })
            .collect();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let status = run_with_strategies(
            arguments(output.clone(), vec![source]),
            &mut stdout,
            &mut stderr,
            MAX_AGGREGATE_SOURCE_BYTES,
            INVOCATION_TIMEOUT,
            strategies,
            |_, _, strategy| match strategy.id {
                StrategyId::OxipngLibdeflateV1 => StrategyResult::NoCandidate,
                StrategyId::OxipngZopfliV1 => StrategyResult::Candidate(b"not a PNG".to_vec()),
                StrategyId::OptipngV1 | StrategyId::PngquantV1 => unreachable!(),
            },
        );

        assert_eq!(status, 3);
        assert_eq!(fs::read(output.join("source.png")).unwrap(), bytes);
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.contains("-> baseline                 67 bytes  winner"));
        assert!(stdout.contains("oxipng-libdeflate-v1     no candidate"));
        assert!(stdout.contains("oxipng-zopfli-v1         rejected: invalid PNG signature"));
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "Summary: 1 succeeded, 1 warning\n"
        );
    }

    #[test]
    fn formats_grouped_bytes_and_compact_summaries() {
        assert_eq!(format_bytes(0), "0");
        assert_eq!(format_bytes(999), "999");
        assert_eq!(format_bytes(1_234), "1,234");
        assert_eq!(format_bytes(12_345_678), "12,345,678");
        assert_eq!(format_summary(8, 0, 0, 0), "Summary: 8 succeeded\n");
        assert_eq!(
            format_summary(8, 1, 2, 3),
            "Summary: 8 succeeded, 1 failed, 2 warnings, 3 not processed\n"
        );
    }

    #[test]
    fn complete_registry_candidate_results_have_a_versioned_memory_bound() {
        assert_eq!(
            crate::limits::MAX_CANDIDATE_BYTES * StrategyId::ALL.len() as u64,
            512 * 1024 * 1024
        );
    }

    struct FailingWriter;

    fn arguments(output_directory: PathBuf, inputs: Vec<PathBuf>) -> Arguments {
        Arguments {
            output_directory,
            inputs,
            strategies: crate::strategy::Selection::default(),
            jobs: 1,
        }
    }

    fn test_registry() -> Vec<RegistryEntry> {
        vec![RegistryEntry {
            id: crate::strategy::StrategyId::OxipngLibdeflateV1,
            state: RegistryState::Runnable(crate::strategy::Execution::Embedded),
        }]
    }

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
        with_empty_idats(source, 1)
    }

    fn with_empty_idats(source: &[u8], count: usize) -> Vec<u8> {
        let first_idat = 8 + 12 + 13;
        let mut candidate = source[..first_idat].to_vec();
        for _ in 0..count {
            push_chunk(&mut candidate, b"IDAT", &[]);
        }
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
