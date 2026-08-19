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
    pub has_children: bool,
    pub class_name: Option<String>,
    pub child_names: Vec<String>,
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
            has_children: !node.children.is_empty(),
            class_name: node.class_name.clone(),
            child_names: node.children.keys().cloned().collect(),
        });
    }

    for (child_name, child) in &node.children {
        instance_path.push(child_name.clone());
        collect_node_mappings(child, project_root, instance_path, mappings);
        instance_path.pop();
    }
}

pub const INIT_SCRIPT_NAMES: [&str; 9] = [
    "init.luau",
    "init.lua",
    "init.server.luau",
    "init.server.lua",
    "init.client.luau",
    "init.client.lua",
    "init.plugin.luau",
    "init.plugin.lua",
    "init.csv",
];

pub fn instance_name_from_entry(file_name: &str) -> Option<&str> {
    if file_name.starts_with("init.")
        || file_name.ends_with(".meta.json")
        || file_name.ends_with(".meta.jsonc")
    {
        return None;
    }

    for suffix in [
        ".server.luau",
        ".server.lua",
        ".client.luau",
        ".client.lua",
        ".plugin.luau",
        ".plugin.lua",
        ".project.jsonc",
        ".project.json",
        ".model.jsonc",
        ".model.json",
        ".luau",
        ".lua",
        ".jsonc",
        ".json",
        ".toml",
        ".csv",
        ".txt",
        ".rbxmx",
        ".rbxm",
        ".yaml",
        ".yml",
    ] {
        if let Some(stem) = file_name.strip_suffix(suffix) {
            return (!stem.is_empty()).then_some(stem);
        }
    }

    None
}

pub fn dir_snapshot_is_folder(dir: &Path) -> bool {
    dir.is_dir()
        && !INIT_SCRIPT_NAMES
            .iter()
            .any(|name| dir.join(name).is_file())
        && !dir.join("default.project.json").is_file()
        && !dir.join("default.project.jsonc").is_file()
}

pub const DATA_MODEL_SERVICES: &[&str] = &[
    "Workspace",
    "Players",
    "Lighting",
    "ReplicatedFirst",
    "ReplicatedStorage",
    "ServerScriptService",
    "ServerStorage",
    "StarterGui",
    "StarterPack",
    "StarterPlayer",
    "SoundService",
    "Chat",
    "TextChatService",
    "LocalizationService",
    "TestService",
    "HttpService",
    "Teams",
    "MaterialService",
    "VoiceChatService",
    "ProximityPromptService",
    "SocialService",
    "TeleportService",
    "MarketplaceService",
    "InsertService",
    "Debris",
    "TweenService",
    "UserInputService",
    "RunService",
    "ContextActionService",
    "CollectionService",
    "PhysicsService",
    "PathfindingService",
];

fn inferred_class<'a>(name: &'a str, parent_class: Option<&str>) -> Option<&'a str> {
    if parent_class == Some("DataModel") && DATA_MODEL_SERVICES.contains(&name) {
        Some(name)
    } else if parent_class == Some("StarterPlayer")
        && matches!(name, "StarterPlayerScripts" | "StarterCharacterScripts")
    {
        Some(name)
    } else if parent_class == Some("Workspace") && name == "Terrain" {
        Some(name)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct UntypedNode {
    pub instance_path: InstancePath,
    pub parent_class_name: Option<String>,
}

pub fn collect_untyped_nodes(project: &LoadedProject) -> Vec<UntypedNode> {
    let mut nodes = Vec::new();
    collect_untyped_node(&project.project.tree, &mut Vec::new(), None, &mut nodes);
    nodes
}

fn collect_untyped_node(
    node: &ProjectNode,
    instance_path: &mut Vec<String>,
    parent_class_name: Option<&str>,
    nodes: &mut Vec<UntypedNode>,
) {
    if node.class_name.is_none() && node.path.is_none() {
        nodes.push(UntypedNode {
            instance_path: InstancePath::from_segments(instance_path.clone()),
            parent_class_name: parent_class_name.map(str::to_owned),
        });
    }

    let inferred = instance_path
        .last()
        .and_then(|name| inferred_class(name, parent_class_name))
        .map(str::to_owned);
    let effective_class = node.class_name.clone().or(inferred);

    for (child_name, child) in &node.children {
        instance_path.push(child_name.clone());
        collect_untyped_node(child, instance_path, effective_class.as_deref(), nodes);
        instance_path.pop();
    }
}
