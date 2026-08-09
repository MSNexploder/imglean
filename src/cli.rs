use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::limits::MAX_INPUTS;

const HELP: &str = "ImgLean 0.1.0\n\
Make supported PNG images lean without replacing source files.\n\n\
Usage: imglean --output OUTPUT_DIRECTORY INPUT...\n\n\
Options:\n\
  --output DIRECTORY  Existing directory for separate output files\n\
  --help              Print help\n\
  --version           Print version\n";

const VERSION: &str = concat!("imglean ", env!("CARGO_PKG_VERSION"), "\n");

#[derive(Debug)]
pub struct Arguments {
    pub output_directory: PathBuf,
    pub inputs: Vec<PathBuf>,
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
    let mut options = true;

    while let Some(argument) = arguments.next() {
        if options && argument == OsStr::new("--") {
            options = false;
            continue;
        }

        if options && argument == OsStr::new("--help") {
            if output_directory.is_some() || !inputs.is_empty() || arguments.next().is_some() {
                return usage("--help cannot be combined with other arguments");
            }
            return Ok(Parsed::Print(HELP));
        }

        if options && argument == OsStr::new("--version") {
            if output_directory.is_some() || !inputs.is_empty() || arguments.next().is_some() {
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

    Ok(Parsed::Run(Arguments {
        output_directory,
        inputs,
    }))
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
}
