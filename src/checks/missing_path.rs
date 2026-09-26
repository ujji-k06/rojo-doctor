use std::path::Path;

use crate::{
    checks::CheckError, diagnostic::Diagnostic, inspection::collect_mapped_paths,
    project::LoadedProject,
};

#[derive(Debug, Default)]
pub struct MissingPathReport {
    pub diagnostics: Vec<Diagnostic>,
    pub resolved_paths: usize,
}

fn help_for_missing_path(project: &LoadedProject, path: &Path) -> &'static str {
    let file_name = path.file_name().and_then(|name| name.to_str());
    let is_package_path = matches!(file_name, Some("Packages" | "DevPackages" | "pesde_packages"))
        || path.components().any(|c| c.as_os_str() == "pesde_packages");

    if is_package_path && project.root_dir().join("pesde.toml").is_file() {
        "run `pesde install` to create this path"
    } else if matches!(file_name, Some("Packages" | "DevPackages"))
        && project.root_dir().join("wally.toml").is_file()
    {
        "run `wally install` to create this path"
    } else {
        "create the mapped path or update the `$path` entry"
    }
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
                help_for_missing_path(project, &mapping.configured_path),
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
