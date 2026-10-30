use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "rojo-doctor",
    version,
    about = "Diagnose structural problems in Rojo projects"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect a Rojo project.
    Check(CheckArgs),

    /// Verification gate for Roblox/Luau projects.
    Verify(VerifyArgs),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Project file, or a directory containing default.project.json(.c).
    #[arg(value_name = "PROJECT")]
    pub project: Option<PathBuf>,

    /// `text` for rustc-style diagnostics, `json` for CI.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Check every `.project.json` / `.project.jsonc` in the directory.
    #[arg(long)]
    pub all: bool,

    /// Automatically fix safe issues (e.g. remove orphaned .meta.json files).
    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum SolverVersion {
    V1,
    #[default]
    V2,
}

#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Files or directories to verify (default: current directory).
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// Run stylua formatting and safe structural fixes instead of checking.
    #[arg(long)]
    pub fix: bool,

    /// Skip the Lune test stage.
    #[arg(long)]
    pub no_tests: bool,

    /// Luau type solver version (default: v2).
    #[arg(long, value_enum, default_value_t = SolverVersion::V2)]
    pub solver: SolverVersion,

    /// Substring filter on test file names.
    #[arg(long)]
    pub test: Option<String>,

    /// Only verify .luau/.lua files changed vs REV (default: HEAD).
    #[arg(long, num_args = 0..=1, default_missing_value = "HEAD")]
    pub diff: Option<String>,

    /// Output format: text (default) or json.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Engine definitions file override for luau-lsp.
    #[arg(long)]
    pub definitions: Option<PathBuf>,

    /// Accept a plain Luau tree without a Rojo project file.
    #[arg(long)]
    pub no_project: bool,

    /// Skip automatic dependency installation.
    #[arg(long)]
    pub no_install: bool,

    /// Show detailed diagnostic output.
    #[arg(short, long)]
    pub verbose: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verify_defaults() {
        let cli = Cli::try_parse_from(["rojo-doctor", "verify"]).expect("should parse");
        match cli.command {
            Command::Verify(args) => {
                assert!(args.paths.is_empty());
                assert!(!args.fix);
                assert!(!args.no_tests);
                assert_eq!(args.solver, SolverVersion::V2);
                assert_eq!(args.test, None);
                assert_eq!(args.diff, None);
                assert_eq!(args.format, OutputFormat::Text);
                assert_eq!(args.definitions, None);
                assert!(!args.no_project);
                assert!(!args.no_install);
                assert!(!args.verbose);
            }
            _ => panic!("expected verify command"),
        }
    }

    #[test]
    fn parses_verify_diff_flag() {
        // Without REV
        let cli1 = Cli::try_parse_from(["rojo-doctor", "verify", "--diff"]).expect("should parse");
        match cli1.command {
            Command::Verify(args) => assert_eq!(args.diff, Some("HEAD".to_string())),
            _ => panic!("expected verify command"),
        }

        // With REV (space-separated)
        let cli2 =
            Cli::try_parse_from(["rojo-doctor", "verify", "--diff", "main"]).expect("should parse");
        match cli2.command {
            Command::Verify(args) => assert_eq!(args.diff, Some("main".to_string())),
            _ => panic!("expected verify command"),
        }

        // With REV (equals-separated)
        let cli3 =
            Cli::try_parse_from(["rojo-doctor", "verify", "--diff=HEAD~2"]).expect("should parse");
        match cli3.command {
            Command::Verify(args) => assert_eq!(args.diff, Some("HEAD~2".to_string())),
            _ => panic!("expected verify command"),
        }
    }

    #[test]
    fn parses_verify_solver_flag() {
        let cli1 =
            Cli::try_parse_from(["rojo-doctor", "verify", "--solver", "v1"]).expect("should parse");
        match cli1.command {
            Command::Verify(args) => assert_eq!(args.solver, SolverVersion::V1),
            _ => panic!("expected verify command"),
        }

        let cli2 =
            Cli::try_parse_from(["rojo-doctor", "verify", "--solver", "v2"]).expect("should parse");
        match cli2.command {
            Command::Verify(args) => assert_eq!(args.solver, SolverVersion::V2),
            _ => panic!("expected verify command"),
        }
    }

    #[test]
    fn parses_verify_all_flags() {
        let cli = Cli::try_parse_from([
            "rojo-doctor",
            "verify",
            "src/a.luau",
            "src/b.luau",
            "--fix",
            "--no-tests",
            "--solver",
            "v1",
            "--test",
            "spring",
            "--format",
            "json",
            "--definitions",
            "types/globalTypes.d.luau",
            "--no-install",
            "-v",
        ])
        .expect("should parse");

        match cli.command {
            Command::Verify(args) => {
                assert_eq!(
                    args.paths,
                    vec![PathBuf::from("src/a.luau"), PathBuf::from("src/b.luau")]
                );
                assert!(args.fix);
                assert!(args.no_tests);
                assert_eq!(args.solver, SolverVersion::V1);
                assert_eq!(args.test, Some("spring".to_string()));
                assert_eq!(args.format, OutputFormat::Json);
                assert_eq!(
                    args.definitions,
                    Some(PathBuf::from("types/globalTypes.d.luau"))
                );
                assert!(args.no_install);
                assert!(args.verbose);
            }
            _ => panic!("expected verify command"),
        }
    }
}
