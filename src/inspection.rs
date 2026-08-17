use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::project::{LoadedProject, ProjectNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstancePath {
    segments: Vec<String>,
}

impl InstancePath {
    fn from_segments(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for InstancePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            formatter.write_str("project root")
        } else {
            formatter.write_str(&self.segments.join("."))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedPath {
    pub instance_path: InstancePath,
    pub configured_path: PathBuf,
    pub resolved_path: PathBuf,
    pub optional: bool,
}

pub fn collect_mapped_paths(project: &LoadedProject) -> Vec<MappedPath> {
    let mut mappings = Vec::new();
    let mut instance_path = Vec::new();

    collect_node_mappings(
        &project.project.tree,
        project.root_dir(),
        &mut instance_path,
        &mut mappings,
    );

    mappings
}

fn collect_node_mappings(
    node: &ProjectNode,
    project_root: &Path,
    instance_path: &mut Vec<String>,
    mappings: &mut Vec<MappedPath>,
) {
    if let Some(mapping) = &node.path {
        let configured_path = mapping.path().to_path_buf();
        let resolved_path = if configured_path.is_absolute() {
            configured_path.clone()
        } else {
            project_root.join(&configured_path)
        };

        mappings.push(MappedPath {
            instance_path: InstancePath::from_segments(instance_path.clone()),
            configured_path,
            resolved_path,
            optional: mapping.is_optional(),
        });
    }

    for (child_name, child) in &node.children {
        instance_path.push(child_name.clone());
        collect_node_mappings(child, project_root, instance_path, mappings);
        instance_path.pop();
    }
}
