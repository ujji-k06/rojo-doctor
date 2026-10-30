use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn create_temp_dir(prefix: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rojo_doctor_{prefix}_{now}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_shim(bin_dir: &Path, name: &str, exit_code: i32) {
    let path = bin_dir.join(name);
    let log_path = bin_dir.join("invocations.log");
    let script = format!(
        r#"#!/bin/sh
echo "{name} $@" >> "{log_path}"
if [ "{name}" = "rojo" ]; then
    prev=""
    for arg in "$@"; do
        if [ "$prev" = "-o" ]; then
            touch "$arg"
        fi
        prev="$arg"
    done
fi
exit {exit_code}
"#,
        name = name,
        log_path = log_path.display(),
        exit_code = exit_code
    );
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
}

fn setup_fixture() -> (PathBuf, PathBuf) {
    let project_dir = create_temp_dir("proj");
    let bin_dir = create_temp_dir("bin");

    // Create minimal default.project.json
    let project_json = r#"{
  "name": "TestProject",
  "tree": {
    "$className": "DataModel",
    "ReplicatedStorage": {
      "$className": "ReplicatedStorage",
      "Source": {
        "$path": "src"
      }
    }
  }
}"#;
    fs::write(project_dir.join("default.project.json"), project_json).unwrap();

    // Create src/a.luau
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("a.luau"), "return 42\n").unwrap();

    // Create minimal selene.toml
    fs::write(project_dir.join("selene.toml"), "std = \"roblox\"\n").unwrap();

    // Create default shims (all exit 0)
    for tool in ["stylua", "selene", "luau-lsp", "lune", "pesde", "rojo"] {
        create_shim(&bin_dir, tool, 0);
    }

    (project_dir, bin_dir)
}

#[test]
fn verify_passes_when_all_stages_succeed() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    // Add globalTypes.d.luau and tests/run.luau
    fs::write(project_dir.join("globalTypes.d.luau"), "").unwrap();
    let tests_dir = project_dir.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(tests_dir.join("run.luau"), "print('suite')\n").unwrap();

    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Text format
    let output = Command::new(bin_path)
        .arg("verify")
        .arg("--no-install")
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VERDICT: PASSED"));

    // Check shim invocations
    let log_content = fs::read_to_string(bin_dir.join("invocations.log")).unwrap_or_default();
    assert!(
        log_content.contains("stylua --check"),
        "stylua not invoked with --check: {log_content}"
    );
    assert!(
        log_content.contains("src/a.luau"),
        "src/a.luau missing from shims: {log_content}"
    );
    assert!(
        log_content.contains("selene --config"),
        "selene not invoked with --config: {log_content}"
    );
    let expected_selene_cfg = project_dir.join("selene.toml");
    assert!(
        log_content.contains(&format!("--config {}", expected_selene_cfg.display())),
        "selene not invoked with expected project config: {log_content}"
    );
    assert!(
        log_content.contains("luau-lsp analyze"),
        "luau-lsp analyze not invoked: {log_content}"
    );
    assert!(
        log_content.contains("--definitions="),
        "--definitions missing from luau-lsp: {log_content}"
    );
    assert!(
        log_content.contains("--sourcemap="),
        "--sourcemap missing from luau-lsp: {log_content}"
    );
    assert!(
        log_content.contains("lune run tests/run.luau"),
        "lune run tests/run.luau not invoked: {log_content}"
    );

    // Clear log for json run
    let _ = fs::remove_file(bin_dir.join("invocations.log"));

    // JSON format
    let json_output = Command::new(bin_path)
        .args(["verify", "--no-install", "--format", "json"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify json");

    assert_eq!(json_output.status.code(), Some(0));
    let json_str = String::from_utf8_lossy(&json_output.stdout);
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("should parse JSON");

    assert_eq!(val["ok"], true);
    assert_eq!(val["verdict"], "passed");
    assert_eq!(val["exit"], 0);
    assert!(val["stages"].is_array());
    assert_eq!(val["stages"].as_array().unwrap().len(), 6);

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_fails_when_a_stage_fails() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    // Make selene fail
    create_shim(&bin_dir, "selene", 1);

    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Text format
    let output = Command::new(bin_path)
        .arg("verify")
        .arg("--no-install")
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VERDICT: FAILED"));

    // JSON format
    let json_output = Command::new(bin_path)
        .args(["verify", "--no-install", "--format", "json"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify json");

    assert_eq!(json_output.status.code(), Some(1));
    let json_str = String::from_utf8_lossy(&json_output.stdout);
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("should parse JSON");

    assert_eq!(val["ok"], false);
    assert_eq!(val["verdict"], "failed");
    assert_eq!(val["exit"], 1);

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_degrades_when_a_tool_is_missing() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    // Remove stylua shim so stylua is missing from PATH
    let _ = fs::remove_file(bin_dir.join("stylua"));

    let path_env = bin_dir.display().to_string();

    // Text format
    let output = Command::new(bin_path)
        .arg("verify")
        .arg("--no-install")
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VERDICT: DEGRADED"));
    assert!(stdout.contains("(tool not found: stylua)"));

    // JSON format
    let json_output = Command::new(bin_path)
        .args(["verify", "--no-install", "--format", "json"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify json");

    assert_eq!(json_output.status.code(), Some(1));
    let json_str = String::from_utf8_lossy(&json_output.stdout);
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("should parse JSON");

    assert_eq!(val["ok"], false);
    assert_eq!(val["verdict"], "degraded");
    assert_eq!(val["exit"], 1);

    let style_stage = val["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "style")
        .expect("style stage should exist");

    assert_eq!(style_stage["status"], "skipped");
    assert!(style_stage["notes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n.as_str().unwrap().contains("tool not found: stylua")));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_diff_with_no_changes_exits_zero() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Init git repo and commit everything so diff is empty
    let _ = Command::new("git")
        .args(["init"])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&project_dir)
        .output();

    // Add an untracked sourcemap.json (non-luau) to verify it is ignored by --diff
    fs::write(project_dir.join("sourcemap.json"), "{}").unwrap();

    let output = Command::new(bin_path)
        .args(["verify", "--diff"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify --diff");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to verify"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_runs_tests_run_luau_entrypoint() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    let tests_dir = project_dir.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(tests_dir.join("run.luau"), "print('running entrypoint')\n").unwrap();
    // Also add another test file to ensure run.luau takes precedence as the single suite
    fs::write(tests_dir.join("other.test.luau"), "return true\n").unwrap();

    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = Command::new(bin_path)
        .arg("verify")
        .arg("--no-install")
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("STATUS: {:?}", output.status);
    println!("STDOUT:\n{stdout}");
    println!("STDERR:\n{stderr}");

    assert_eq!(output.status.code(), Some(0));
    assert!(stdout.to_lowercase().contains("all passed (1 suites)"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_runs_nested_spec_suites() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    let spec_dir = project_dir.join("tests").join("spec");
    fs::create_dir_all(&spec_dir).unwrap();
    fs::write(spec_dir.join("x.spec.luau"), "return true\n").unwrap();

    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = Command::new(bin_path)
        .arg("verify")
        .arg("--no-install")
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout.to_lowercase().contains("all passed (1 suites)"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_without_luau_files_exits_one() {
    let project_dir = create_temp_dir("empty_proj");
    let bin_dir = create_temp_dir("bin");
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");

    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    // default.project.json only, no luau files
    let project_json = r#"{
      "name": "Empty",
      "tree": {
        "$className": "DataModel"
      }
    }"#;
    fs::write(project_dir.join("default.project.json"), project_json).unwrap();

    let output = Command::new(bin_path)
        .arg("verify")
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(1));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_diff_with_bad_rev_exits_two() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let _ = Command::new("git")
        .args(["init"])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&project_dir)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&project_dir)
        .output();

    let output = Command::new(bin_path)
        .args(["verify", "--diff=nosuchrev", "--no-install"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify --diff=nosuchrev");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[git]"),
        "expected error[git] in stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_diff_without_git_repo_exits_two() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = Command::new(bin_path)
        .args(["verify", "--diff", "--no-install"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .env("GIT_CEILING_DIRECTORIES", project_dir.parent().unwrap())
        .output()
        .expect("should run verify --diff outside git");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[git]"),
        "expected error[git] in stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_diff_with_git_missing_from_path_exits_two() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    // PATH contains bin_dir only (no git)
    let path_env = bin_dir.display().to_string();

    let output = Command::new(bin_path)
        .args(["verify", "--diff", "--no-install"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify --diff with no git");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[git]"),
        "expected error[git] in stderr: {stderr}"
    );

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_diff_in_subdirectory_with_non_ascii_file() {
    let repo_dir = create_temp_dir("repo");
    let bin_dir = create_temp_dir("bin");
    for tool in ["stylua", "selene", "luau-lsp", "lune", "pesde", "rojo"] {
        create_shim(&bin_dir, tool, 0);
    }
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let _ = Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output();

    let app_dir = repo_dir.join("app");
    fs::create_dir_all(&app_dir).unwrap();

    let project_json = r#"{
      "name": "App",
      "tree": {
        "$className": "DataModel",
        "ReplicatedStorage": {
          "$path": "src"
        }
      }
    }"#;
    fs::write(app_dir.join("default.project.json"), project_json).unwrap();

    let src_dir = app_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("a.luau"), "return 1\n").unwrap();
    // Non-ASCII filename
    fs::write(src_dir.join("ü.luau"), "return 'unicode'\n").unwrap();

    // Initial commit
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .output();

    // Now modify app/src/a.luau
    fs::write(src_dir.join("a.luau"), "return 2\n").unwrap();

    // Run from inside app_dir
    let output = Command::new(bin_path)
        .args(["verify", "--diff", "--no-install"])
        .current_dir(&app_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify in subdir");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stdout.contains("VERDICT: PASSED"),
        "expected PASSED but got: {stdout}"
    );
    assert!(
        !stdout.contains("nothing to verify"),
        "should not be empty diff"
    );

    let invocations = fs::read_to_string(bin_dir.join("invocations.log")).unwrap_or_default();
    assert!(
        invocations.contains("a.luau"),
        "a.luau was not verified in subdir: {invocations}"
    );

    let _ = fs::remove_dir_all(&repo_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_unparseable_project_exits_two() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    fs::write(project_dir.join("default.project.json"), "{ invalid json ").unwrap();

    let output = Command::new(bin_path)
        .args(["verify", "--no-install"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[project-load]"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_missing_project_without_flag_degrades_exit_one() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let _ = fs::remove_file(project_dir.join("default.project.json"));

    let output = Command::new(bin_path)
        .args(["verify", "--no-install"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VERDICT: DEGRADED"));
    assert!(stdout.contains("(no Rojo project; --no-project to accept)"));

    // And with --no-project, it succeeds
    let output2 = Command::new(bin_path)
        .args(["verify", "--no-install", "--no-project"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify with --no-project");

    assert_eq!(output2.status.code(), Some(0));
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("VERDICT: PASSED"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_diff_with_explicit_paths_exits_two() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = Command::new(bin_path)
        .args(["verify", "--diff=HEAD", "src/a.luau"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[invocation]"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_nonexistent_rojo_doctor_definitions_exits_two() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = Command::new(bin_path)
        .args(["verify", "--no-install"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .env(
            "ROJO_DOCTOR_DEFINITIONS",
            "/nonexistent/definitions/path.luau",
        )
        .output()
        .expect("should run verify");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[definitions]"));

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_relative_paths_in_subdirectory_resolves_against_cwd() {
    let repo_dir = create_temp_dir("two_level_repo");
    let bin_dir = create_temp_dir("bin");
    for tool in ["stylua", "selene", "luau-lsp", "lune", "pesde", "rojo"] {
        create_shim(&bin_dir, tool, 0);
    }
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let _ = Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output();

    let app_dir = repo_dir.join("app");
    let src_dir = app_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let project_json = r#"{
      "name": "App",
      "tree": {
        "$className": "DataModel",
        "ReplicatedStorage": {
          "$path": "src"
        }
      }
    }"#;
    fs::write(app_dir.join("default.project.json"), project_json).unwrap();
    fs::write(src_dir.join("a.luau"), "return 1\n").unwrap();

    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .output();

    // Run `rojo-doctor verify src --no-install --no-tests` from within `app_dir`
    let output = Command::new(bin_path)
        .args(["verify", "src", "--no-install", "--no-tests"])
        .current_dir(&app_dir)
        .env("PATH", &path_env)
        .output()
        .expect("should run verify src in subdir");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stdout.contains("VERDICT: PASSED"),
        "expected PASSED but got: {stdout}"
    );
    assert!(!stdout.contains("no Luau files found"));

    let invocations = fs::read_to_string(bin_dir.join("invocations.log")).unwrap_or_default();
    assert!(
        invocations.contains("a.luau"),
        "a.luau was not verified: {invocations}"
    );

    let _ = fs::remove_dir_all(&repo_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_synthesizes_selene_config_when_missing() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Ensure no selene.toml exists in project_dir
    let _ = fs::remove_file(project_dir.join("selene.toml"));

    // Write typed Luau in src/a.luau
    fs::write(
        project_dir.join("src").join("a.luau"),
        "local function foo(x: number): number\n    return x\nend\nreturn foo\n",
    )
    .unwrap();

    let output = Command::new(bin_path)
        .args(["verify", "--no-install", "--no-tests", "--format", "json"])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        // Ensure DSH_HOME points to an empty folder so no types/selene.toml exists
        .env("DSH_HOME", project_dir.display().to_string())
        .output()
        .expect("should run verify json");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(val["verdict"], "passed");

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_selene_ancestor_precedence_and_synthesized_modes() {
    let repo_dir = create_temp_dir("selene_precedence_repo");
    let bin_dir = create_temp_dir("bin");
    for tool in ["stylua", "selene", "luau-lsp", "lune", "pesde", "rojo"] {
        create_shim(&bin_dir, tool, 0);
    }
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Ancestor repo has selene.toml
    let root_selene = repo_dir.join("selene.toml");
    fs::write(&root_selene, "std = \"roblox\"\n").unwrap();

    let sub_app = repo_dir.join("app");
    let sub_src = sub_app.join("src");
    fs::create_dir_all(&sub_src).unwrap();
    fs::write(sub_src.join("a.luau"), "return 1\n").unwrap();

    let project_json = r#"{
      "name": "App",
      "tree": {
        "$className": "DataModel",
        "ReplicatedStorage": {
          "$path": "src"
        }
      }
    }"#;
    fs::write(sub_app.join("default.project.json"), project_json).unwrap();

    // 1. In sub_app, with ancestor selene.toml: must pick up ancestor selene.toml
    let output = Command::new(bin_path)
        .args(["verify", "--no-install", "--no-tests"])
        .current_dir(&sub_app)
        .env("PATH", &path_env)
        .output()
        .expect("run verify");
    assert_eq!(output.status.code(), Some(0));

    let invocations = fs::read_to_string(bin_dir.join("invocations.log")).unwrap_or_default();
    assert!(
        invocations.contains(&format!("--config {}", root_selene.display())),
        "expected ancestor selene.toml to be picked up: {invocations}"
    );

    // 2. Project local selene.toml beats ancestor
    let local_selene = sub_app.join("selene.toml");
    fs::write(&local_selene, "std = \"roblox\"\n").unwrap();
    let _ = fs::remove_file(bin_dir.join("invocations.log"));

    let output2 = Command::new(bin_path)
        .args(["verify", "--no-install", "--no-tests"])
        .current_dir(&sub_app)
        .env("PATH", &path_env)
        .output()
        .expect("run verify local");
    assert_eq!(output2.status.code(), Some(0));

    let invocations2 = fs::read_to_string(bin_dir.join("invocations.log")).unwrap_or_default();
    assert!(
        invocations2.contains(&format!("--config {}", local_selene.display())),
        "expected local selene.toml to beat ancestor: {invocations2}"
    );

    // 3. Plain Luau repo without project synthesizes std = "luau"
    let _ = fs::remove_file(&local_selene);
    let _ = fs::remove_file(&root_selene);
    let _ = fs::remove_file(sub_app.join("default.project.json"));

    let output3 = Command::new(bin_path)
        .args([
            "verify",
            "--no-project",
            "--no-install",
            "--no-tests",
            "--format",
            "json",
        ])
        .current_dir(&sub_app)
        .env("PATH", &path_env)
        .env("DSH_HOME", repo_dir.display().to_string())
        .output()
        .expect("run verify plain luau");
    assert_eq!(output3.status.code(), Some(0));
    let stdout3 = String::from_utf8_lossy(&output3.stdout);
    assert!(
        stdout3.contains("synthesized default luau config: std = \\\"luau\\\""),
        "expected synthesized std = luau for plain luau project: {stdout3}"
    );

    let _ = fs::remove_dir_all(&repo_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}

#[test]
fn verify_relative_definitions_canonicalizes_for_luau_lsp() {
    let (project_dir, bin_dir) = setup_fixture();
    let bin_path = env!("CARGO_BIN_EXE_rojo-doctor");
    let path_env = format!("{}:/usr/bin:/bin", bin_dir.display());

    let defs_path = project_dir.join("customDefs.d.luau");
    fs::write(&defs_path, "-- custom types\n").unwrap();

    let output = Command::new(bin_path)
        .args([
            "verify",
            "--no-install",
            "--no-tests",
            "--definitions",
            "customDefs.d.luau",
        ])
        .current_dir(&project_dir)
        .env("PATH", &path_env)
        .output()
        .expect("run verify relative definitions");

    assert_eq!(output.status.code(), Some(0));
    let invocations = fs::read_to_string(bin_dir.join("invocations.log")).unwrap_or_default();
    let canonical_defs = fs::canonicalize(&defs_path).unwrap();
    assert!(
        invocations.contains(&format!("--definitions={}", canonical_defs.display())),
        "luau-lsp did not receive canonical definitions path: {invocations}"
    );

    let _ = fs::remove_dir_all(&project_dir);
    let _ = fs::remove_dir_all(&bin_dir);
}
