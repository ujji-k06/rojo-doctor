use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Project file, or a directory containing default.project.json(.c).
    #[arg(value_name = "PROJECT")]
    pub project: Option<PathBuf>,
}
