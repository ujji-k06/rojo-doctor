pub mod ambiguous_init;
pub mod child_collision;
pub mod class_on_non_folder;
pub mod file_with_children;
pub mod missing_class;
pub mod missing_name;
pub mod missing_path;
pub mod orphan_meta;

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
    diagnostics.extend(missing_class::run(project));
    diagnostics.extend(class_on_non_folder::run(project));
    diagnostics.extend(child_collision::run(project)?);
    diagnostics.extend(ambiguous_init::run(project)?);
    diagnostics.extend(orphan_meta::run(project)?);

    Ok(Report {
        diagnostics,
        resolved_paths: missing.resolved_paths,
    })
}
