use std::{fs, path::Path};

use crate::{
    checks::CheckError,
    diagnostic::Diagnostic,
    inspection::{collect_mapped_paths, INIT_SCRIPT_NAMES},
    project::LoadedProject,
};

pub fn run(project: &LoadedProject) -> Result<Vec<Diagnostic>, CheckError> {
    let mut diagnostics = Vec::new();
    for mapping in collect_mapped_paths(project) {
        if mapping.resolved_path.is_dir() {
            walk(project, &mapping.resolved_path, &mut diagnostics)?;
        }
    }
    Ok(diagnostics)
}

fn walk(
    project: &LoadedProject,
    dir: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), CheckError> {
    let entries = fs::read_dir(dir).map_err(|source| CheckError::InspectMappedPath {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut init_names = Vec::new();
    let mut subdirectories = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| CheckError::InspectMappedPath {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|source| CheckError::InspectMappedPath {
                path: path.clone(),
                source,
            })?
            .is_dir()
        {
            if path.join("default.project.json").is_file()
                || path.join("default.project.jsonc").is_file()
            {
                continue;
            }
            subdirectories.push(path);
        } else if let Some(name) = entry.file_name().to_str() {
            if INIT_SCRIPT_NAMES.contains(&name) {
                init_names.push(name.to_owned());
            }
        }
    }
    if init_names.len() >= 2 {
        init_names.sort_by_key(|name| INIT_SCRIPT_NAMES.iter().position(|item| item == name));
        diagnostics.push(Diagnostic::warning(
            "ambiguous-init",
            relative_subject(project, dir),
            format!(
                "contains multiple init scripts ({}); Rojo will only use the highest-priority one",
                init_names.join(", ")
            ),
            "keep a single init script (init.luau beats init.lua, then init.server.*, then init.client.*, then init.plugin.*, then init.csv)",
        ));
    }
    for subdirectory in subdirectories {
        walk(project, &subdirectory, diagnostics)?;
    }
    Ok(())
}

fn relative_subject(project: &LoadedProject, path: &Path) -> String {
    path.strip_prefix(project.root_dir()).map_or_else(
        |_| path.display().to_string(),
        |path| path.display().to_string(),
    )
}
