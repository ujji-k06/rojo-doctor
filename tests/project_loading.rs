use std::path::{Path, PathBuf};

use rojo_doctor::project::{load_project, PathMapping, ProjectError};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn discovers_and_loads_default_project_json_from_directory() {
    let loaded = load_project(Some(&fixture("valid-json"))).expect("fixture should load");

    assert_eq!(loaded.project.name.as_deref(), Some("FixtureGame"));
    assert_eq!(
        loaded.path.file_name().and_then(|name| name.to_str()),
        Some("default.project.json")
    );
    assert!(loaded
        .project
        .tree
        .children
        .contains_key("ReplicatedStorage"));
}

#[test]
fn discovers_and_loads_default_project_jsonc_from_directory() {
    let loaded = load_project(Some(&fixture("valid-jsonc"))).expect("fixture should load");

    let starter_player = loaded
        .project
        .tree
        .children
        .get("StarterPlayer")
        .expect("StarterPlayer should exist");
    let scripts = starter_player
        .children
        .get("StarterPlayerScripts")
        .expect("StarterPlayerScripts should exist");

    assert!(matches!(scripts.path, Some(PathMapping::Optional { .. })));
}

#[test]
fn accepts_an_explicit_project_file() {
    let project_file = fixture("valid-json").join("default.project.json");
    let loaded = load_project(Some(&project_file)).expect("explicit project should load");

    assert_eq!(loaded.path, project_file);
}

#[test]
fn reports_when_a_directory_has_no_default_project() {
    let error = load_project(Some(&fixture("no-project"))).expect_err("load should fail");

    assert!(matches!(error, ProjectError::NoProjectFound { .. }));
}

#[test]
fn rejects_an_invalid_path_mapping_shape() {
    let error = load_project(Some(&fixture("invalid-json"))).expect_err("load should fail");

    assert!(matches!(error, ProjectError::Deserialize { .. }));
}
