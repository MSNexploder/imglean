mod artifacts;
mod cli;
mod controller;
mod diagnostics;
mod input;
mod limits;
mod output;
mod png;
mod worker;

use std::io::{self, Write};

fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if let Some(status) = worker::try_run(&arguments) {
        std::process::exit(status);
    }

    let status = match cli::parse(arguments) {
        Ok(cli::Parsed::Print(message)) => write_message(io::stdout().lock(), message, 0),
        Ok(cli::Parsed::Run(arguments)) => {
            controller::run(arguments, io::stdout().lock(), io::stderr().lock())
        }
        Err(error) => {
            let message = format!(
                "imglean: {}\nTry 'imglean --help' for usage.\n",
                error.message()
            );
            write_message(io::stderr().lock(), &message, 2)
        }
    };

    std::process::exit(status);
}

fn write_message(mut writer: impl Write, message: &str, success_status: i32) -> i32 {
    if writer
        .write_all(message.as_bytes())
        .and_then(|()| writer.flush())
        .is_ok()
    {
        success_status
    } else {
        1
    }
}
