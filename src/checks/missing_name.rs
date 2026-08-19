use crate::{diagnostic::Diagnostic, project::LoadedProject};

pub fn run(project: &LoadedProject) -> Vec<Diagnostic> {
    match &project.project.name {
        Some(name) if !name.trim().is_empty() => Vec::new(),
        _ => vec![Diagnostic::warning(
            "missing-name",
            project
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project")
                .to_string(),
            "project has no name, so Rojo will fall back to the file name",
            "set the top-level `name` field",
        )],
    }
}
