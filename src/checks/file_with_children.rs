use crate::{diagnostic::Diagnostic, inspection::collect_mapped_paths, project::LoadedProject};

pub fn run(project: &LoadedProject) -> Vec<Diagnostic> {
    collect_mapped_paths(project)
        .into_iter()
        .filter(|mapping| mapping.has_children)
        .filter(|mapping| mapping.resolved_path.is_file())
        .map(|mapping| {
            Diagnostic::error(
                "file-with-children",
                mapping.configured_path.display().to_string(),
                format!(
                    "{} maps to a file but also lists child instances, which Rojo cannot merge",
                    mapping.instance_path
                ),
                "remove the child instances or point `$path` at a directory",
            )
        })
        .collect()
}
