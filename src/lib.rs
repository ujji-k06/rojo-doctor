pub mod checks;
pub mod cli;
pub mod diagnostic;
pub mod inspection;
pub mod project;

use std::process::ExitCode;

use cli::{CheckArgs, Cli, Command, OutputFormat};
use diagnostic::Severity;
use serde::Serialize;

pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Check(args) => run_check(args),
    }
}

#[derive(Serialize)]
struct JsonReport {
    project: String,
    resolved_paths: usize,
    warnings: usize,
    errors: usize,
    diagnostics: Vec<diagnostic::Diagnostic>,
}

fn run_check(args: CheckArgs) -> ExitCode {
    if args.all {
        return run_check_all(args);
    }

    let loaded = match project::load_project(args.project.as_deref()) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("error[project-load]");
            eprintln!("  {error}");
            return ExitCode::from(2);
        }
    };

    if args.fix {
        match checks::fix_all(&loaded) {
            Ok(count) if count > 0 && args.format == OutputFormat::Text => {
                println!("fixed: removed {count} orphaned metadata file{}", plural_suffix(count));
                println!();
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!("error[filesystem]");
                eprintln!("  {error}");
                return ExitCode::from(2);
            }
        }
    }

    let report = match checks::run_all(&loaded) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("error[filesystem]");
            eprintln!("  {error}");
            return ExitCode::from(2);
        }
    };

    let payload = make_json_report(&loaded, &report);
    if let Err(code) = print_report(args.format, std::slice::from_ref(&payload), false) {
        return code;
    }

    exit_for_reports(std::slice::from_ref(&payload))
}

fn run_check_all(args: CheckArgs) -> ExitCode {
    let requested = args
        .project
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let directory = match std::fs::metadata(&requested) {
        Ok(metadata) if metadata.is_file() => requested
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
        Ok(metadata) if metadata.is_dir() => requested,
        Ok(_) => {
            eprintln!("error[project-load]");
            eprintln!("  no Rojo project file found");
            return ExitCode::from(2);
        }
        Err(error) => {
            eprintln!("error[project-load]");
            eprintln!("  could not inspect `{}`: {error}", requested.display());
            return ExitCode::from(2);
        }
    };
    let paths = match project::discover_project_files(&directory) {
        Ok(paths) if !paths.is_empty() => paths,
        Ok(_) => {
            eprintln!("error[project-load]");
            eprintln!("  no Rojo project file found");
            return ExitCode::from(2);
        }
        Err(error) => {
            eprintln!("error[project-load]");
            eprintln!("  {error}");
            return ExitCode::from(2);
        }
    };

    let mut reports = Vec::new();
    for path in paths {
        match project::load_project(Some(&path)) {
            Ok(loaded) => {
                if args.fix {
                    match checks::fix_all(&loaded) {
                        Ok(count) if count > 0 && args.format == OutputFormat::Text => {
                            println!("fixed: removed {count} orphaned metadata file{}", plural_suffix(count));
                            println!();
                        }
                        Ok(_) => {}
                        Err(error) => {
                            eprintln!("error[filesystem]");
                            eprintln!("  {error}");
                            return ExitCode::from(2);
                        }
                    }
                }
                match checks::run_all(&loaded) {
                    Ok(report) => reports.push(make_json_report(&loaded, &report)),
                    Err(error) => {
                        eprintln!("error[filesystem]");
                        eprintln!("  {error}");
                        return ExitCode::from(2);
                    }
                }
            }
            Err(error) => {
                eprintln!("error[project-load]");
                eprintln!("  {error}");
                return ExitCode::from(2);
            }
        }
    }

    if let Err(code) = print_report(args.format, &reports, true) {
        return code;
    }
    exit_for_reports(&reports)
}

fn print_report(
    format: OutputFormat,
    reports: &[JsonReport],
    json_array: bool,
) -> Result<(), ExitCode> {
    match format {
        OutputFormat::Json => {
            let payload = if json_array {
                serde_json::to_string_pretty(reports)
            } else {
                serde_json::to_string_pretty(&reports[0])
            };
            match payload {
                Ok(json) => {
                    println!("{json}");
                    Ok(())
                }
                Err(error) => {
                    eprintln!("error[json]");
                    eprintln!("  {error}");
                    Err(ExitCode::from(2))
                }
            }
        }
        OutputFormat::Text => {
            for (index, report) in reports.iter().enumerate() {
                if index > 0 {
                    println!();
                }
                println!("Checking {}...", report.project);
                println!();
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
                println!();
                println!(
                    "{} warning{}, {} error{}",
                    report.warnings,
                    plural_suffix(report.warnings),
                    report.errors,
                    plural_suffix(report.errors)
                );
            }
            Ok(())
        }
    }
}

fn exit_for_reports(reports: &[JsonReport]) -> ExitCode {
    if reports
        .iter()
        .any(|report| report.warnings > 0 || report.errors > 0)
    {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn make_json_report(loaded: &project::LoadedProject, report: &checks::Report) -> JsonReport {
    let project = loaded
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| loaded.path.display().to_string(), str::to_owned);
    JsonReport {
        project,
        resolved_paths: report.resolved_paths,
        warnings: report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .count(),
        errors: report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count(),
        diagnostics: report.diagnostics.clone(),
    }
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}