use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;

mod cli;
mod commands;
mod project_root;

use cli::Cli;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Usage errors exit with 1: clap's default (2) would collide with
            // ctx-sync's "context conflict" exit code.
            let code = match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => 1,
            };
            let _ = e.print();
            return ExitCode::from(code);
        }
    };
    match commands::dispatch(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}
