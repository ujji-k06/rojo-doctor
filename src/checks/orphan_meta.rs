use std::{fs, path::Path};

use crate::{
    checks::CheckError,
    diagnostic::Diagnostic,
    inspection::{collect_mapped_paths, instance_name_from_entry},
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
    let entries = fs::read_dir(dir)
        .map_err(|source| CheckError::InspectMappedPath {
            path: dir.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| CheckError::InspectMappedPath {
            path: dir.to_path_buf(),
            source,
        })?;

    for entry in &entries {
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
            walk(project, &path, diagnostics)?;
            continue;
        }

        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(stem) = file_name
            .strip_suffix(".meta.json")
            .or_else(|| file_name.strip_suffix(".meta.jsonc"))
        else {
            continue;
        };
        if stem == "init" {
            continue;
        }
        let matched = entries.iter().any(|sibling| {
            sibling.path().is_dir() && sibling.file_name().to_str() == Some(stem)
                || sibling
                    .file_name()
                    .to_str()
                    .and_then(instance_name_from_entry)
                    == Some(stem)
        });
        if !matched {
            diagnostics.push(Diagnostic::warning(
                "orphan-meta",
                relative_subject(project, &path),
                "metadata file has no matching instance",
                format!("add a sibling `{stem}` file or folder, or delete this .meta.json"),
            ));
        }
    }
    Ok(())
}

fn relative_subject(project: &LoadedProject, path: &Path) -> String {
    path.strip_prefix(project.root_dir()).map_or_else(
        |_| path.display().to_string(),
        |path| path.display().to_string(),
    )
}

pub fn fix(project: &LoadedProject) -> Result<usize, CheckError> {
    let diagnostics = run(project)?;
    let mut fixed = 0;
    for diag in diagnostics {
        if diag.code == "orphan-meta" {
            let path = project.root_dir().join(&diag.subject);
            if path.is_file() {
                if fs::remove_file(&path).is_ok() {
                    fixed += 1;
                }
            }
        }
    }
    Ok(fixed)
}
