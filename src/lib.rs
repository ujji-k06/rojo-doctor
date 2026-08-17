pub mod checks;
pub mod cli;
pub mod diagnostic;
pub mod inspection;
pub mod project;

use std::process::ExitCode;

use cli::{CheckArgs, Cli, Command};
use diagnostic::Severity;

pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Check(args) => run_check(args),
    }
}

fn run_check(args: CheckArgs) -> ExitCode {
    let loaded = match project::load_project(args.project.as_deref()) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("error[project-load]");
            eprintln!("  {error}");
            return ExitCode::from(2);
        }
    };

    let display_path = loaded
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| loaded.path.display().to_string(), str::to_owned);

    println!("Checking {display_path}...");
    println!();

    let report = match checks::missing_path::run(&loaded) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("error[filesystem]");
            eprintln!("  {error}");
            return ExitCode::from(2);
        }
    };

    let path_label = if report.resolved_paths == 1 {
        "path"
    } else {
        "paths"
    };
    println!("✓ {} mapped {} resolved", report.resolved_paths, path_label);

    for diagnostic in &report.diagnostics {
        println!();
        println!("{diagnostic}");
    }

    let warnings = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .count();
    let errors = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();

    println!();
    println!(
        "{} warning{}, {} error{}",
        warnings,
        plural_suffix(warnings),
        errors,
        plural_suffix(errors)
    );

    if report.diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}
