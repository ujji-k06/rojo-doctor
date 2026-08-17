use crate::{
    checks::CheckError, diagnostic::Diagnostic, inspection::collect_mapped_paths,
    project::LoadedProject,
};

#[derive(Debug, Default)]
pub struct MissingPathReport {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved_paths: usize,
}

pub fn run(project: &LoadedProject) -> Result<MissingPathReport, CheckError> {
    let mut report = MissingPathReport::default();

    for mapping in collect_mapped_paths(project) {
        match mapping.resolved_path.try_exists() {
            Ok(true) => report.resolved_paths += 1,
            Ok(false) if mapping.optional => {}
            Ok(false) => report.diagnostics.push(Diagnostic::warning(
                "missing-path",
                mapping.configured_path.display().to_string(),
                format!(
                    "referenced by {} but does not exist, so Rojo cannot resolve this mapping",
                    mapping.instance_path
                ),
                "create the mapped path or update the `$path` entry",
            )),
            Err(source) => {
                return Err(CheckError::InspectMappedPath {
                    path: mapping.resolved_path,
                    source,
                });
            }
        }
    }

    Ok(report)
}
