use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    str::Utf8Error,
};

use serde::{Deserialize, Deserializer};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_PROJECT_FILES: [&str; 2] = ["default.project.json", "default.project.jsonc"];

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub name: Option<String>,
    pub tree: ProjectNode,
}

/// The subset of a Rojo instance description needed by rojo-doctor today.
///
/// Rojo reserves keys beginning with `$` for node metadata. We parse the two
/// fields needed for path analysis and intentionally ignore other reserved
/// fields so that properties/attributes do not get mistaken for child nodes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectNode {
    pub class_name: Option<String>,
    pub path: Option<PathMapping>,
    pub children: BTreeMap<String, ProjectNode>,
}

impl<'de> Deserialize<'de> for ProjectNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut fields = BTreeMap::<String, Value>::deserialize(deserializer)?;

        let class_name = fields
            .remove("$className")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?;

        let path = fields
            .remove("$path")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?;

        let mut children = BTreeMap::new();
        for (name, value) in fields {
            if name.starts_with('$') {
                continue;
            }

            let child = serde_json::from_value(value).map_err(|error| {
                serde::de::Error::custom(format!("invalid child instance `{name}`: {error}"))
            })?;
            children.insert(name, child);
        }

        Ok(Self {
            class_name,
            path,
            children,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum PathMapping {
    Required(PathBuf),
    Optional { optional: PathBuf },
}

impl PathMapping {
    pub fn path(&self) -> &Path {
        match self {
            Self::Required(path) => path,
            Self::Optional { optional } => optional,
        }
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Optional { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedProject {
    pub path: PathBuf,
    pub project: Project,
}

impl LoadedProject {
    pub fn root_dir(&self) -> &Path {
        self.path
            .parent()
            .expect("a discovered project file always has a parent directory")
    }
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("could not determine the current directory: {source}")]
    CurrentDirectory {
        #[source]
        source: std::io::Error,
    },

    #[error("could not inspect `{path}`: {source}")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no Rojo project file found in `{path}`")]
    NoProjectFound { path: PathBuf },

    #[error("`{path}` is not a .project.json or .project.jsonc file")]
    NotProjectFile { path: PathBuf },

    #[error("could not read Rojo project `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Rojo project `{path}` is not valid UTF-8: {source}")]
    InvalidUtf8 {
        path: PathBuf,
        #[source]
        source: Utf8Error,
    },

    #[error("could not parse Rojo project `{path}` as JSON/JSONC: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("Rojo project `{path}` has an unsupported structure: {source}")]
    Deserialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub fn load_project(input: Option<&Path>) -> Result<LoadedProject, ProjectError> {
    let requested_path = match input {
        Some(path) => path.to_path_buf(),
        None => env::current_dir().map_err(|source| ProjectError::CurrentDirectory { source })?,
    };

    let path = discover_project_file(&requested_path)?;
    let bytes = fs::read(&path).map_err(|source| ProjectError::Read {
        path: path.clone(),
        source,
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|source| ProjectError::InvalidUtf8 {
        path: path.clone(),
        source,
    })?;

    let value = jsonc_parser::parse_to_serde_value(text, &Default::default())
        .map_err(|source| ProjectError::Parse {
            path: path.clone(),
            message: source.to_string(),
        })?
        .ok_or_else(|| ProjectError::Parse {
            path: path.clone(),
            message: "file contains no JSON value".to_string(),
        })?;

    let project = serde_json::from_value(value).map_err(|source| ProjectError::Deserialize {
        path: path.clone(),
        source,
    })?;

    Ok(LoadedProject { path, project })
}

pub fn discover_project_file(path: &Path) -> Result<PathBuf, ProjectError> {
    let metadata = fs::metadata(path).map_err(|source| ProjectError::InspectPath {
        path: path.to_path_buf(),
        source,
    })?;

    if metadata.is_file() {
        if is_project_file(path) {
            return Ok(path.to_path_buf());
        }

        return Err(ProjectError::NotProjectFile {
            path: path.to_path_buf(),
        });
    }

    if metadata.is_dir() {
        for file_name in DEFAULT_PROJECT_FILES {
            let candidate = path.join(file_name);
            match fs::metadata(&candidate) {
                Ok(candidate_metadata) if candidate_metadata.is_file() => return Ok(candidate),
                Ok(_) => continue,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(ProjectError::InspectPath {
                        path: candidate,
                        source,
                    });
                }
            }
        }

        return Err(ProjectError::NoProjectFound {
            path: path.to_path_buf(),
        });
    }

    Err(ProjectError::NoProjectFound {
        path: path.to_path_buf(),
    })
}

fn is_project_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".project.json") || name.ends_with(".project.jsonc"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_node_ignores_reserved_fields_instead_of_treating_them_as_children() {
        let node: ProjectNode = serde_json::from_str(
            r#"{
                "$className": "Folder",
                "$properties": { "Name": "Example" },
                "$attributes": { "Enabled": true },
                "Child": { "$path": "src/child" }
            }"#,
        )
        .expect("node should deserialize");

        assert_eq!(node.class_name.as_deref(), Some("Folder"));
        assert_eq!(node.children.len(), 1);
        assert!(node.children.contains_key("Child"));
    }

    #[test]
    fn optional_path_mapping_matches_rojo_shape() {
        let node: ProjectNode = serde_json::from_str(
            r#"{ "$path": { "optional": "src/generated" } }"#,
        )
        .expect("node should deserialize");

        let mapping = node.path.expect("path should exist");
        assert_eq!(mapping.path(), Path::new("src/generated"));
        assert!(mapping.is_optional());
    }
}
