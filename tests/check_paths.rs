use std::{
    path::{Path, PathBuf},
    process::Command,
};

use rojo_doctor::{
    checks::missing_path,
    diagnostic::Severity,
    inspection::{collect_mapped_paths, instance_name_from_entry},
    project::load_project,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn collects_nested_mapped_paths_with_instance_locations() {
    let fixture_root = fixture("valid-json");
    let loaded = load_project(Some(&fixture_root)).expect("fixture should load");
    let mappings = collect_mapped_paths(&loaded);

    assert_eq!(mappings.len(), 2);

    let inventory = mappings
        .iter()
        .find(|mapping| mapping.instance_path.to_string() == "ServerScriptService.Inventory")
        .expect("Inventory mapping should exist");

    assert_eq!(inventory.configured_path, Path::new("src/server"));
    assert_eq!(inventory.resolved_path, fixture_root.join("src/server"));
    assert!(!inventory.optional);
}

#[test]
fn missing_path_check_reports_required_paths_and_ignores_missing_optional_paths() {
    let loaded = load_project(Some(&fixture("missing-path"))).expect("fixture should load");
    let report = missing_path::run(&loaded).expect("check should complete");

    assert_eq!(report.resolved_paths, 1);
    assert_eq!(report.diagnostics.len(), 1);

    let diagnostic = &report.diagnostics[0];
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code, "missing-path");
    assert_eq!(
        diagnostic.subject,
        Path::new("src/server/Inventory").display().to_string()
    );
    assert!(diagnostic.message.contains("ServerScriptService.Inventory"));
}

#[test]
fn check_command_returns_one_when_findings_are_present() {
    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .arg("check")
        .arg(fixture("missing-path"))
        .output()
        .expect("rojo-doctor should run");

    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("warning[missing-path]"));
    assert!(stdout.contains("ServerScriptService.Inventory"));
    assert!(stdout.contains("1 warning, 0 errors"));
}

#[test]
fn check_command_returns_zero_when_all_required_paths_exist() {
    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .arg("check")
        .arg(fixture("valid-json"))
        .output()
        .expect("rojo-doctor should run");

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("2 mapped paths resolved"));
    assert!(stdout.contains("0 warnings, 0 errors"));
}

#[test]
fn file_with_children_is_an_error() {
    let loaded = load_project(Some(&fixture("file-with-children"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "file-with-children"
            && diagnostic.severity == Severity::Error));
}

#[test]
fn missing_name_is_a_warning() {
    let loaded = load_project(Some(&fixture("missing-name"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing-name"));
}

#[test]
fn json_format_prints_parseable_report() {
    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["check", "--format", "json"])
        .arg(fixture("missing-path"))
        .output()
        .expect("rojo-doctor should run");

    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("\"code\": \"missing-path\""));
    assert!(stdout.contains("\"warnings\": 1"));
    assert!(stdout.contains("\"errors\": 0"));
}

#[test]
fn missing_class_ignores_inferred_services_but_flags_plain_nodes() {
    let loaded = load_project(Some(&fixture("missing-class"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "missing-class" && diagnostic.subject == "ReplicatedStorage.Orphan"
    }));
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "missing-class")
            .count(),
        1
    );
}

#[test]
fn class_on_non_folder_is_an_error() {
    let loaded = load_project(Some(&fixture("class-on-file"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "class-on-non-folder" && diagnostic.severity == Severity::Error
    }));
}

#[test]
fn child_collision_is_an_error() {
    let loaded = load_project(Some(&fixture("child-collision"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "child-collision" && diagnostic.message.contains("child `Util`")
    }));
}

#[test]
fn ambiguous_init_is_a_warning() {
    let loaded = load_project(Some(&fixture("ambiguous-init"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "ambiguous-init" && diagnostic.severity == Severity::Warning
    }));
}

#[test]
fn orphan_meta_ignores_init_and_matched_siblings() {
    let loaded = load_project(Some(&fixture("orphan-meta"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    let orphan = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "orphan-meta")
        .collect::<Vec<_>>();
    assert_eq!(orphan.len(), 1);
    assert!(orphan[0].subject.ends_with("Ghost.meta.json"));
}

#[test]
fn check_all_runs_every_project_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["check", "--all"])
        .arg(fixture("multi-project"))
        .output()
        .expect("rojo-doctor should run");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("default.project.json"));
    assert!(stdout.contains("tests.project.json"));
}

#[test]
fn instance_name_from_entry_handles_rojo_suffixes() {
    assert_eq!(instance_name_from_entry("Foo.server.lua"), Some("Foo"));
    assert_eq!(instance_name_from_entry("init.lua"), None);
    assert_eq!(instance_name_from_entry("Foo.meta.json"), None);
    assert_eq!(instance_name_from_entry("Bar.luau"), Some("Bar"));
}
#[test]
fn pesde_packages_missing_path_suggests_pesde_install() {
    let loaded = load_project(Some(&fixture("pesde-packages"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    let missing = report
        .diagnostics
        .iter()
        .find(|d| d.code == "missing-path")
        .expect("should flag missing-path");

    assert!(missing
        .help
        .as_deref()
        .unwrap_or("")
        .contains("pesde install"));
}

#[test]
fn script_context_warns_on_client_script_in_serverscriptservice() {
    let loaded = load_project(Some(&fixture("script-context"))).expect("fixture should load");
    let report = rojo_doctor::checks::run_all(&loaded).expect("check should complete");

    let context_diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "script-context")
        .expect("should flag script-context");

    assert_eq!(context_diag.severity, Severity::Warning);
    assert!(context_diag.subject.ends_with("Bad.client.luau"));
    assert!(context_diag.message.contains("ServerScriptService"));
}

#[test]
fn fix_flag_removes_orphaned_metadata_files() {
    let temp_dir = std::env::temp_dir().join("rojo-doctor-test-fix");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("src")).expect("create temp dir");

    std::fs::write(
        temp_dir.join("default.project.json"),
        r#"{"name": "Temp", "tree": {"$className": "DataModel", "ReplicatedStorage": {"$path": "src"}}}"#,
    ).expect("write project");

    std::fs::write(temp_dir.join("src/Ghost.meta.json"), "{}").expect("write orphan");
    std::fs::write(temp_dir.join("src/Real.luau"), "return {}").expect("write real");
    std::fs::write(temp_dir.join("src/Real.meta.json"), "{}").expect("write real meta");

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["check", "--fix"])
        .arg(&temp_dir)
        .output()
        .expect("run rojo-doctor --fix");

    assert_eq!(output.status.code(), Some(0));
    assert!(!temp_dir.join("src/Ghost.meta.json").exists());
    assert!(temp_dir.join("src/Real.meta.json").exists());
    assert!(temp_dir.join("src/Real.luau").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
