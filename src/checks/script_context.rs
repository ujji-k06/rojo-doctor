use std::{fs, path::Path};

use crate::{
    checks::CheckError, diagnostic::Diagnostic, inspection::collect_mapped_paths,
    project::LoadedProject,
};

#[derive(Debug, PartialEq, Eq)]
enum ExpectedContext {
    ClientOnly,
    ServerOnly,
}

fn expected_context_for_instance(segments: &[String]) -> Option<ExpectedContext> {
    if segments.is_empty() {
        return None;
    }
    let first = segments[0].as_str();
    if first == "ServerScriptService" || first == "ServerStorage" {
        return Some(ExpectedContext::ServerOnly);
    }
    if first == "ReplicatedFirst" || first == "StarterGui" {
        return Some(ExpectedContext::ClientOnly);
    }
    if first == "StarterPlayer"
        && segments.get(1).map(String::as_str) == Some("StarterPlayerScripts")
    {
        return Some(ExpectedContext::ClientOnly);
    }
    None
}

pub fn run(project: &LoadedProject) -> Result<Vec<Diagnostic>, CheckError> {
    let mut diagnostics = Vec::new();
    for mapping in collect_mapped_paths(project) {
        let Some(expected) = expected_context_for_instance(mapping.instance_path.segments()) else {
            continue;
        };

        if mapping.resolved_path.is_file() {
            check_file(
                project,
                &mapping.resolved_path,
                &mapping.instance_path.to_string(),
                &expected,
                &mut diagnostics,
            );
        } else if mapping.resolved_path.is_dir() {
            walk(
                project,
                &mapping.resolved_path,
                &mapping.instance_path.to_string(),
                &expected,
                &mut diagnostics,
            )?;
        }
    }
    Ok(diagnostics)
}

fn is_client_script(name: &str) -> bool {
    name.ends_with(".client.luau") || name.ends_with(".client.lua")
}

fn is_server_script(name: &str) -> bool {
    name.ends_with(".server.luau") || name.ends_with(".server.lua")
}

fn check_file(
    project: &LoadedProject,
    path: &Path,
    instance_name: &str,
    expected: &ExpectedContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };

    match expected {
        ExpectedContext::ServerOnly if is_client_script(file_name) => {
            diagnostics.push(Diagnostic::warning(
                "script-context",
                relative_subject(project, path),
                format!("client script placed under `{instance_name}` will not execute on the client"),
                "move this script to a client container (e.g. StarterPlayerScripts, StarterGui) or change to a server script",
            ));
        }
        ExpectedContext::ClientOnly if is_server_script(file_name) => {
            diagnostics.push(Diagnostic::warning(
                "script-context",
                relative_subject(project, path),
                format!(
                    "server script placed under `{instance_name}` will not execute on the client"
                ),
                "move this script to ServerScriptService or change to a client script",
            ));
        }
        _ => {}
    }
}

fn walk(
    project: &LoadedProject,
    dir: &Path,
    instance_name: &str,
    expected: &ExpectedContext,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), CheckError> {
    let entries = fs::read_dir(dir).map_err(|source| CheckError::InspectMappedPath {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| CheckError::InspectMappedPath {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| CheckError::InspectMappedPath {
                path: path.clone(),
                source,
            })?;

        if file_type.is_dir() {
            if path.join("default.project.json").is_file()
                || path.join("default.project.jsonc").is_file()
            {
                continue;
            }
            walk(project, &path, instance_name, expected, diagnostics)?;
        } else if file_type.is_file() {
            check_file(project, &path, instance_name, expected, diagnostics);
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
