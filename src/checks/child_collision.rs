use std::fs;

use crate::{
    checks::CheckError,
    diagnostic::Diagnostic,
    inspection::{collect_mapped_paths, dir_snapshot_is_folder, instance_name_from_entry},
    project::LoadedProject,
};

pub fn run(project: &LoadedProject) -> Result<Vec<Diagnostic>, CheckError> {
    let mut diagnostics = Vec::new();
    for mapping in collect_mapped_paths(project)
        .into_iter()
        .filter(|mapping| !mapping.child_names.is_empty())
        .filter(|mapping| {
            mapping.resolved_path.is_dir() && !has_nested_project(&mapping.resolved_path)
        })
    {
        let entries = fs::read_dir(&mapping.resolved_path).map_err(|source| {
            CheckError::InspectMappedPath {
                path: mapping.resolved_path.clone(),
                source,
            }
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| CheckError::InspectMappedPath {
                path: mapping.resolved_path.clone(),
                source,
            })?;
            let name = if entry
                .file_type()
                .map_err(|source| CheckError::InspectMappedPath {
                    path: entry.path(),
                    source,
                })?
                .is_dir()
            {
                entry.file_name().to_str().map(str::to_owned)
            } else {
                entry
                    .file_name()
                    .to_str()
                    .and_then(instance_name_from_entry)
                    .map(str::to_owned)
            };
            if let Some(name) = name.filter(|name| mapping.child_names.contains(name)) {
                diagnostics.push(Diagnostic::error(
                    "child-collision",
                    mapping.configured_path.display().to_string(),
                    format!(
                        "{} lists child `{name}` which $path already provides from the filesystem",
                        mapping.instance_path
                    ),
                    "remove the duplicate child from the project file or rename the filesystem entry",
                ));
            }
        }
    }
    Ok(diagnostics)
}

fn has_nested_project(path: &std::path::Path) -> bool {
    !dir_snapshot_is_folder(path)
        && (path.join("default.project.json").is_file()
            || path.join("default.project.jsonc").is_file())
}
