use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::limits::MAX_INPUTS;
use crate::strategy::{ProviderId, ProviderPath, Selection, StrategyId};

const HELP: &str = "ImgLean 0.2.0\n\
Make supported PNG images lean without replacing source files.\n\n\
Usage: imglean --output OUTPUT_DIRECTORY INPUT...\n\n\
Options:\n\
  --output DIRECTORY       Existing directory for separate output files\n\
  --disable-strategy ID    Disable a strategy; may be repeated\n\
  --require-strategy ID    Require an available strategy; may be repeated\n\
  --provider NAME PATH     Use and require a supported provider executable\n\
  --help                   Print help\n\
  --version                Print version\n\
\n\
Strategy IDs (default order):\n\
  oxipng-libdeflate-v1, oxipng-zopfli-v1, optipng-v1\n\
Supported external provider:\n\
  optipng 7.9.1\n";

const VERSION: &str = concat!("imglean ", env!("CARGO_PKG_VERSION"), "\n");

#[derive(Debug)]
pub struct Arguments {
    pub output_directory: PathBuf,
    pub inputs: Vec<PathBuf>,
    pub strategies: Selection,
}

#[derive(Debug)]
pub enum Parsed {
    Run(Arguments),
    Print(&'static str),
}

#[derive(Debug, Eq, PartialEq)]
pub struct UsageError {
    message: String,
}

impl UsageError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn parse<I>(arguments: I) -> Result<Parsed, UsageError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let mut output_directory = None;
    let mut inputs = Vec::new();
    let mut strategies = Selection::default();
    let mut options = true;

    while let Some(argument) = arguments.next() {
        if options && argument == OsStr::new("--") {
            options = false;
            continue;
        }

        if options && argument == OsStr::new("--help") {
            if output_directory.is_some()
                || !inputs.is_empty()
                || strategies != Selection::default()
                || arguments.next().is_some()
            {
                return usage("--help cannot be combined with other arguments");
            }
            return Ok(Parsed::Print(HELP));
        }

        if options && argument == OsStr::new("--version") {
            if output_directory.is_some()
                || !inputs.is_empty()
                || strategies != Selection::default()
                || arguments.next().is_some()
            {
                return usage("--version cannot be combined with other arguments");
            }
            return Ok(Parsed::Print(VERSION));
        }

        if options && argument == OsStr::new("--output") {
            if output_directory.is_some() {
                return usage("--output may be specified only once");
            }
            let Some(directory) = arguments.next() else {
                return usage("--output requires a directory");
            };
            if directory.is_empty() {
                return usage("--output requires a nonempty directory");
            }
            output_directory = Some(PathBuf::from(directory));
            continue;
        }

        if options && argument == OsStr::new("--disable-strategy") {
            let strategy = parse_strategy(arguments.next(), "--disable-strategy")?;
            if strategies.disabled.contains(&strategy) {
                return usage("--disable-strategy contains a duplicate strategy");
            }
            strategies.disabled.push(strategy);
            continue;
        }

        if options && argument == OsStr::new("--require-strategy") {
            let strategy = parse_strategy(arguments.next(), "--require-strategy")?;
            if strategies.required.contains(&strategy) {
                return usage("--require-strategy contains a duplicate strategy");
            }
            strategies.required.push(strategy);
            continue;
        }

        if options && argument == OsStr::new("--provider") {
            let provider = parse_provider(arguments.next())?;
            let Some(path) = arguments.next() else {
                return usage("--provider requires a name and executable path");
            };
            if path.is_empty() {
                return usage("--provider requires a nonempty executable path");
            }
            if strategies
                .provider_paths
                .iter()
                .any(|configured| configured.provider == provider)
            {
                return usage("--provider contains a duplicate provider");
            }
            strategies.provider_paths.push(ProviderPath {
                provider,
                path: PathBuf::from(path),
            });
            let strategy = provider.strategy();
            if !strategies.required.contains(&strategy) {
                strategies.required.push(strategy);
            }
            continue;
        }

        if options && is_option(&argument) {
            return usage("unknown option");
        }

        inputs.push(PathBuf::from(argument));
        if inputs.len() > MAX_INPUTS {
            return usage("too many input files");
        }
    }

    let Some(output_directory) = output_directory else {
        return usage("--output is required");
    };
    if inputs.is_empty() {
        return usage("at least one input file is required");
    }
    if strategies
        .disabled
        .iter()
        .any(|strategy| strategies.required.contains(strategy))
    {
        return usage("a strategy cannot be both disabled and required");
    }

    Ok(Parsed::Run(Arguments {
        output_directory,
        inputs,
        strategies,
    }))
}

fn parse_strategy(argument: Option<OsString>, option: &str) -> Result<StrategyId, UsageError> {
    let Some(argument) = argument else {
        return usage(&format!("{option} requires a strategy ID"));
    };
    let Some(value) = argument.to_str() else {
        return usage("strategy IDs must be ASCII");
    };
    StrategyId::parse(value).ok_or_else(|| UsageError {
        message: format!("unknown strategy ID: {value}"),
    })
}

fn parse_provider(argument: Option<OsString>) -> Result<ProviderId, UsageError> {
    let Some(argument) = argument else {
        return usage("--provider requires a name and executable path");
    };
    let Some(value) = argument.to_str() else {
        return usage("provider names must be ASCII");
    };
    ProviderId::parse(value).ok_or_else(|| UsageError {
        message: format!("unknown provider: {value}"),
    })
}

fn is_option(argument: &OsStr) -> bool {
    argument
        .to_str()
        .is_some_and(|value| value.starts_with('-'))
}

fn usage<T>(message: &str) -> Result<T, UsageError> {
    Err(UsageError {
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strings(arguments: &[&str]) -> Result<Parsed, UsageError> {
        parse(arguments.iter().map(OsString::from))
    }

    #[test]
    fn parses_required_output_and_inputs() {
        let Parsed::Run(arguments) =
            parse_strings(&["imglean", "--output", "out", "a.png", "b.png"]).unwrap()
        else {
            panic!("expected runnable arguments");
        };
        assert_eq!(arguments.output_directory, PathBuf::from("out"));
        assert_eq!(
            arguments.inputs,
            [PathBuf::from("a.png"), PathBuf::from("b.png")]
        );
        assert_eq!(arguments.strategies, Selection::default());
    }

    #[test]
    fn double_dash_allows_option_like_input() {
        let Parsed::Run(arguments) =
            parse_strings(&["imglean", "--output", "out", "--", "--help"]).unwrap()
        else {
            panic!("expected runnable arguments");
        };
        assert_eq!(arguments.inputs, [PathBuf::from("--help")]);
    }

    #[test]
    fn help_is_standalone() {
        assert!(matches!(
            parse_strings(&["imglean", "--help"]),
            Ok(Parsed::Print(HELP))
        ));
        assert_eq!(
            parse_strings(&[
                "imglean",
                "--disable-strategy",
                "oxipng-zopfli-v1",
                "--help"
            ])
            .unwrap_err()
            .message(),
            "--help cannot be combined with other arguments"
        );
        assert_eq!(
            parse_strings(&["imglean", "--help", "a.png"])
                .unwrap_err()
                .message(),
            "--help cannot be combined with other arguments"
        );
    }

    #[test]
    fn rejects_missing_output_or_input() {
        assert_eq!(
            parse_strings(&["imglean", "a.png"]).unwrap_err().message(),
            "--output is required"
        );
        assert_eq!(
            parse_strings(&["imglean", "--output", "out"])
                .unwrap_err()
                .message(),
            "at least one input file is required"
        );
    }

    #[test]
    fn rejects_duplicate_and_unknown_options() {
        assert_eq!(
            parse_strings(&["imglean", "--output", "a", "--output", "b", "x.png"])
                .unwrap_err()
                .message(),
            "--output may be specified only once"
        );
        assert_eq!(
            parse_strings(&["imglean", "--wat"]).unwrap_err().message(),
            "unknown option"
        );
    }

    #[test]
    fn rejects_more_than_the_invocation_input_limit() {
        let mut arguments = vec![OsString::from("imglean"), OsString::from("--output")];
        arguments.push(OsString::from("out"));
        arguments.extend((0..=MAX_INPUTS).map(|index| OsString::from(format!("{index}.png"))));

        assert_eq!(
            parse(arguments).unwrap_err().message(),
            "too many input files"
        );
    }

    #[test]
    fn parses_strategy_controls_and_provider_path() {
        let Parsed::Run(arguments) = parse_strings(&[
            "imglean",
            "--output",
            "out",
            "--disable-strategy",
            "oxipng-zopfli-v1",
            "--provider",
            "optipng",
            "/tools/optipng",
            "a.png",
        ])
        .unwrap() else {
            panic!("expected runnable arguments");
        };
        assert_eq!(arguments.strategies.disabled, [StrategyId::OxipngZopfliV1]);
        assert_eq!(arguments.strategies.required, [StrategyId::OptipngV1]);
        assert_eq!(
            arguments.strategies.provider_paths,
            [ProviderPath {
                provider: ProviderId::Optipng,
                path: PathBuf::from("/tools/optipng"),
            }]
        );
    }

    #[test]
    fn rejects_unknown_duplicate_and_conflicting_strategy_controls() {
        assert_eq!(
            parse_strings(&[
                "imglean",
                "--output",
                "out",
                "--disable-strategy",
                "unknown",
                "a.png",
            ])
            .unwrap_err()
            .message(),
            "unknown strategy ID: unknown"
        );
        assert_eq!(
            parse_strings(&[
                "imglean",
                "--output",
                "out",
                "--require-strategy",
                "optipng-v1",
                "--require-strategy",
                "optipng-v1",
                "a.png",
            ])
            .unwrap_err()
            .message(),
            "--require-strategy contains a duplicate strategy"
        );
        assert_eq!(
            parse_strings(&[
                "imglean",
                "--output",
                "out",
                "--disable-strategy",
                "optipng-v1",
                "--require-strategy",
                "optipng-v1",
                "a.png",
            ])
            .unwrap_err()
            .message(),
            "a strategy cannot be both disabled and required"
        );
    }
}
