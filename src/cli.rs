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
