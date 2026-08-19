use crate::{
    diagnostic::Diagnostic,
    inspection::{collect_mapped_paths, dir_snapshot_is_folder},
    project::LoadedProject,
};

pub fn run(project: &LoadedProject) -> Vec<Diagnostic> {
    collect_mapped_paths(project)
        .into_iter()
        .filter_map(|mapping| {
            let class_name = mapping.class_name.as_deref()?;
            let is_non_folder = mapping.resolved_path.is_file()
                || (mapping.resolved_path.is_dir()
                    && !dir_snapshot_is_folder(&mapping.resolved_path));
            is_non_folder.then(|| {
                Diagnostic::error(
                    "class-on-non-folder",
                    mapping.configured_path.display().to_string(),
                    format!(
                        "{} sets $className to `{class_name}` but $path is not a Folder",
                        mapping.instance_path
                    ),
                    "remove `$className` or point `$path` at a directory without an init script",
                )
            })
        })
        .collect()
}
