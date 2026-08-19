use crate::{
    diagnostic::Diagnostic,
    inspection::{collect_untyped_nodes, DATA_MODEL_SERVICES},
    project::LoadedProject,
};

pub fn run(project: &LoadedProject) -> Vec<Diagnostic> {
    collect_untyped_nodes(project)
        .into_iter()
        .filter(|node| {
            let name = node.instance_path.segments().last().map(String::as_str);
            !((node.parent_class_name.as_deref() == Some("DataModel")
                && name.is_some_and(|name| DATA_MODEL_SERVICES.contains(&name)))
                || (node.parent_class_name.as_deref() == Some("StarterPlayer")
                    && name.is_some_and(|name| {
                        matches!(name, "StarterPlayerScripts" | "StarterCharacterScripts")
                    }))
                || (node.parent_class_name.as_deref() == Some("Workspace")
                    && name == Some("Terrain")))
        })
        .map(|node| {
            Diagnostic::error(
                "missing-class",
                node.instance_path.to_string(),
                "has neither $className nor $path, so Rojo cannot snapshot this instance",
                "set `$className` or `$path`",
            )
        })
        .collect()
}
