use std::{
    path::{Path, PathBuf},
    process::Command,
};

use rojo_doctor::{
    checks::missing_path, diagnostic::Severity, inspection::collect_mapped_paths,
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
