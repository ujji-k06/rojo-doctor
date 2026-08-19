pub mod file_with_children;
pub mod missing_name;
pub mod missing_path;

use std::path::PathBuf;

use thiserror::Error;

use crate::{diagnostic::Diagnostic, project::LoadedProject};

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("could not inspect mapped path `{path}`: {source}")]
    InspectMappedPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved_paths: usize,
}

pub fn run_all(project: &LoadedProject) -> Result<Report, CheckError> {
    let missing = missing_path::run(project)?;
    let mut diagnostics = missing.diagnostics;
    diagnostics.extend(file_with_children::run(project));
    diagnostics.extend(missing_name::run(project));

    Ok(Report {
        diagnostics,
        resolved_paths: missing.resolved_paths,
    })
}
