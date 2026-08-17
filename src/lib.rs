pub mod cli;
pub mod inspection;
pub mod project;

use std::process::ExitCode;

use cli::{CheckArgs, Cli, Command};

pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Check(args) => run_check(args),
    }
}

fn run_check(args: CheckArgs) -> ExitCode {
    match project::load_project(args.project.as_deref()) {
        Ok(loaded) => {
            let display_path = loaded
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .map_or_else(|| loaded.path.display().to_string(), str::to_owned);

            println!("Checking {display_path}...");
            println!();
            println!("✓ project loaded");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error[project-load]");
            eprintln!("  {error}");
            ExitCode::from(2)
        }
    }
}
