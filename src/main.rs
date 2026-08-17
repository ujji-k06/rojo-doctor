use std::process::ExitCode;

use clap::Parser;
use rojo_doctor::cli::Cli;

fn main() -> ExitCode {
    rojo_doctor::run(Cli::parse())
}
