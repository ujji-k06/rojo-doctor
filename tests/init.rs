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
    let dir = std::env::temp_dir().join(format!("rojo_doctor_init_{prefix}_{now}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_dummy_definitions() -> PathBuf {
    let defs_path = create_temp_dir("dummy_defs").join("globalTypes.d.luau");
    fs::write(
        &defs_path,
        "-- dummy global definitions for test\ndeclare class Instance\nend\n",
    )
    .unwrap();
    defs_path
}

fn init_git_repo(path: &Path) {
    let output = Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .expect("git init should run");
    assert!(output.status.success(), "git init should succeed");

    let _ = Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(path)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(path)
        .output();
}

#[test]
fn fresh_dir_creates_all_four_artifacts_with_expected_content_and_executable_hook() {
    let dir = create_temp_dir("fresh_all");
    init_git_repo(&dir);
    let defs = create_dummy_definitions();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("rojo-doctor init should run");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("types/globalTypes.d.luau: created"));
    assert!(stdout.contains(".github/workflows/verify.yml: created"));
    assert!(stdout.contains(".githooks/pre-commit: created"));
    assert!(stdout.contains(".gitignore: created"));
    assert!(stdout.contains("git config core.hooksPath: configured (.githooks)"));

    // 1. types/globalTypes.d.luau
    let types_file = dir.join("types").join("globalTypes.d.luau");
    assert!(types_file.exists());
    assert_eq!(
        fs::read_to_string(&types_file).unwrap(),
        fs::read_to_string(&defs).unwrap()
    );

    // 2. .github/workflows/verify.yml
    let wf_file = dir.join(".github").join("workflows").join("verify.yml");
    assert!(wf_file.exists());
    let wf_content = fs::read_to_string(&wf_file).unwrap();
    assert!(wf_content.contains("name: verify"));
    assert!(wf_content.contains("branches: [main]"));
    assert!(wf_content.contains("pull_request:"));
    assert!(wf_content.contains("actions/checkout@v4"));
    assert!(wf_content.contains("fetch-depth: 0"));
    assert!(wf_content.contains("CompeyDev/setup-rokit@v0.2.1"));
    assert!(wf_content.contains("dtolnay/rust-toolchain@stable"));
    assert!(wf_content.contains("v0.4.1"));
    assert!(wf_content.contains(
        "cargo install --git https://github.com/ujji-k06/rojo-doctor --tag v0.4.1 --locked"
    ));
    assert!(wf_content.contains(
        "if [ -f rokit.toml ] && command -v rokit >/dev/null 2>&1; then rokit install; fi"
    ));
    assert!(wf_content.contains("rokit install"));

    // 3. .githooks/pre-commit
    let hook_file = dir.join(".githooks").join("pre-commit");
    assert!(hook_file.exists());
    let hook_content = fs::read_to_string(&hook_file).unwrap();
    assert!(hook_content.contains("git rev-parse --verify HEAD"));
    assert!(hook_content.contains("rojo-doctor verify $DIFF_FLAG --no-install"));
    assert!(hook_content.contains("--no-verify"));

    #[cfg(unix)]
    {
        let metadata = fs::metadata(&hook_file).unwrap();
        let mode = metadata.permissions().mode();
        assert_ne!(mode & 0o111, 0, "hook should be executable");
    }

    // 4. .gitignore
    let gitignore_file = dir.join(".gitignore");
    assert!(gitignore_file.exists());
    let gitignore_content = fs::read_to_string(&gitignore_file).unwrap();
    assert!(gitignore_content
        .lines()
        .any(|l| l.trim() == "sourcemap.json"));

    // Git config check
    let git_config = Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(&dir)
        .output()
        .expect("git config check");
    assert_eq!(
        String::from_utf8_lossy(&git_config.stdout).trim(),
        ".githooks"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rerun_is_noop() {
    let dir = create_temp_dir("rerun_noop");
    init_git_repo(&dir);
    let defs = create_dummy_definitions();

    // First run
    let out1 = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("first init");
    assert_eq!(out1.status.code(), Some(0));

    // Second run
    let out2 = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("second init");
    assert_eq!(out2.status.code(), Some(0));

    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(stdout2.contains("types/globalTypes.d.luau: already present"));
    assert!(stdout2.contains(".github/workflows/verify.yml: already present"));
    assert!(stdout2.contains(".githooks/pre-commit: already present"));
    assert!(stdout2.contains(".gitignore: already present"));
    assert!(stdout2.contains("git config core.hooksPath: already present (.githooks)"));
    assert!(!stdout2.contains("created"));
    assert!(!stdout2.contains("updated"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn force_overwrites() {
    let dir = create_temp_dir("force_overwrite");
    init_git_repo(&dir);
    let defs = create_dummy_definitions();

    // Initial run
    let out1 = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("initial init");
    assert_eq!(out1.status.code(), Some(0));

    // Mutate artifacts
    let wf_file = dir.join(".github").join("workflows").join("verify.yml");
    fs::write(&wf_file, "# modified ci workflow\n").unwrap();
    let hook_file = dir.join(".githooks").join("pre-commit");
    fs::write(&hook_file, "# modified hook\n").unwrap();
    let types_file = dir.join("types").join("globalTypes.d.luau");
    fs::write(&types_file, "-- modified types\n").unwrap();

    // Re-run with --force
    let out2 = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--force", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("force init");
    assert_eq!(out2.status.code(), Some(0));

    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(stdout2.contains("types/globalTypes.d.luau: updated"));
    assert!(stdout2.contains(".github/workflows/verify.yml: updated"));
    assert!(stdout2.contains(".githooks/pre-commit: updated"));

    // Check content was restored
    assert_eq!(
        fs::read_to_string(&types_file).unwrap(),
        fs::read_to_string(&defs).unwrap()
    );
    assert!(fs::read_to_string(&wf_file)
        .unwrap()
        .contains("name: verify"));
    assert!(fs::read_to_string(&hook_file)
        .unwrap()
        .contains("rojo-doctor verify --diff --no-install"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_ci_flag_omits_artifact() {
    let dir = create_temp_dir("no_ci");
    let defs = create_dummy_definitions();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--no-ci", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("init --no-ci");
    assert_eq!(output.status.code(), Some(0));

    assert!(!dir
        .join(".github")
        .join("workflows")
        .join("verify.yml")
        .exists());
    assert!(dir.join("types").join("globalTypes.d.luau").exists());
    assert!(dir.join(".githooks").join("pre-commit").exists());
    assert!(dir.join(".gitignore").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_hook_flag_omits_artifact_and_git_config() {
    let dir = create_temp_dir("no_hook");
    init_git_repo(&dir);
    let defs = create_dummy_definitions();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--no-hook", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("init --no-hook");
    assert_eq!(output.status.code(), Some(0));

    assert!(!dir.join(".githooks").join("pre-commit").exists());
    assert!(dir
        .join(".github")
        .join("workflows")
        .join("verify.yml")
        .exists());
    assert!(dir.join("types").join("globalTypes.d.luau").exists());
    assert!(dir.join(".gitignore").exists());

    let git_config = Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(&dir)
        .output()
        .expect("git config check");
    assert_ne!(git_config.status.code(), Some(0));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_types_flag_omits_artifact() {
    let dir = create_temp_dir("no_types");

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--no-types"])
        .arg(&dir)
        .output()
        .expect("init --no-types");
    assert_eq!(output.status.code(), Some(0));

    assert!(!dir.join("types").join("globalTypes.d.luau").exists());
    assert!(dir
        .join(".github")
        .join("workflows")
        .join("verify.yml")
        .exists());
    assert!(dir.join(".githooks").join("pre-commit").exists());
    assert!(dir.join(".gitignore").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn non_git_dir_does_not_run_git_config() {
    let dir = create_temp_dir("non_git");
    let defs = create_dummy_definitions();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("init non-git");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("core.hooksPath"));
    assert!(!dir.join(".git").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn definitions_from_copies_bytes_verbatim() {
    let dir = create_temp_dir("defs_verbatim");
    let custom_bytes = b"-- verbatim content\r\n\x00custom bytes 12345\n";
    let custom_file = create_temp_dir("custom_src").join("source.luau");
    fs::write(&custom_file, custom_bytes).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&custom_file)
        .arg(&dir)
        .output()
        .expect("init --definitions-from");
    assert_eq!(output.status.code(), Some(0));

    let copied_path = dir.join("types").join("globalTypes.d.luau");
    assert_eq!(fs::read(&copied_path).unwrap(), custom_bytes);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_file(&custom_file);
}

#[test]
fn project_with_default_project_json_gets_commands_without_no_project() {
    let dir = create_temp_dir("with_project");
    fs::write(
        dir.join("default.project.json"),
        r#"{"name": "Game", "tree": {}}"#,
    )
    .unwrap();
    let defs = create_dummy_definitions();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("init with project");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("warning: no *.project.json"));

    let wf_content =
        fs::read_to_string(dir.join(".github").join("workflows").join("verify.yml")).unwrap();
    assert!(wf_content.contains("rojo-doctor verify --no-install"));
    assert!(
        wf_content.contains("rojo-doctor verify --no-install --diff origin/${{ github.base_ref }}")
    );
    assert!(!wf_content.contains("--no-project"));

    let hook_content = fs::read_to_string(dir.join(".githooks").join("pre-commit")).unwrap();
    assert!(hook_content.contains("rojo-doctor verify $DIFF_FLAG --no-install"));
    assert!(!hook_content.contains("--no-project"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn project_without_project_file_gets_warning_and_commands_with_no_project() {
    let dir = create_temp_dir("without_project");
    let defs = create_dummy_definitions();

    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("init without project");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(
        "warning: no *.project.json or *.project.jsonc found; verify will need --no-project"
    ));

    let wf_content =
        fs::read_to_string(dir.join(".github").join("workflows").join("verify.yml")).unwrap();
    assert!(wf_content.contains("rojo-doctor verify --no-install --no-project"));
    assert!(wf_content.contains(
        "rojo-doctor verify --no-install --diff origin/${{ github.base_ref }} --no-project"
    ));
    assert!(wf_content.contains("--no-project"));

    let hook_content = fs::read_to_string(dir.join(".githooks").join("pre-commit")).unwrap();
    assert!(hook_content.contains("rojo-doctor verify $DIFF_FLAG --no-install --no-project"));
    assert!(hook_content.contains("--no-project"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn existing_definitions_survives_failed_force_refresh() {
    let dir = create_temp_dir("failed_force");
    let defs_original = dir.join("original.d.luau");
    fs::write(&defs_original, "-- original definitions content\n").unwrap();

    // Initial valid setup
    let out1 = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs_original)
        .arg(&dir)
        .output()
        .expect("initial init");
    assert_eq!(out1.status.code(), Some(0));

    let dest_defs = dir.join("types").join("globalTypes.d.luau");
    assert_eq!(
        fs::read_to_string(&dest_defs).unwrap(),
        "-- original definitions content\n"
    );

    // Create a directory to force failure via --definitions-from
    let bad_source_dir = dir.join("a_bad_directory_as_source");
    fs::create_dir_all(&bad_source_dir).unwrap();

    // Re-run with --force pointing to directory (deterministic failure without network)
    let out2 = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--force", "--definitions-from"])
        .arg(&bad_source_dir)
        .arg(&dir)
        .output()
        .expect("forced refresh with bad source");
    assert_eq!(out2.status.code(), Some(2));

    // The original definitions file must survive untouched
    assert!(dest_defs.exists());
    assert_eq!(
        fs::read_to_string(&dest_defs).unwrap(),
        "-- original definitions content\n"
    );

    // No leftover temporary files in types/
    let entries: Vec<_> = fs::read_dir(dir.join("types"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name(), "globalTypes.d.luau");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hook_runs_successfully_on_unborn_branch() {
    let dir = create_temp_dir("unborn_hook");
    init_git_repo(&dir);
    let defs = create_dummy_definitions();

    // Add a valid luau file and rojo project so verify can pass
    fs::write(dir.join("test.luau"), "return {}\n").unwrap();
    fs::write(
        dir.join("default.project.json"),
        r#"{"name": "Game", "tree": {"$className": "DataModel"}}"#,
    )
    .unwrap();

    let init_out = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&dir)
        .output()
        .expect("init run");
    assert_eq!(init_out.status.code(), Some(0));

    let hook_path = dir.join(".githooks").join("pre-commit");
    assert!(hook_path.exists());

    // Prepend the target/debug directory to PATH so the hook can execute rojo-doctor
    let bin_path = PathBuf::from(env!("CARGO_BIN_EXE_rojo-doctor"));
    let bin_dir = bin_path.parent().unwrap();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), current_path);

    let hook_out = Command::new(&hook_path)
        .current_dir(&dir)
        .env("PATH", new_path)
        .output()
        .expect("execute pre-commit hook");

    assert_eq!(
        hook_out.status.code(),
        Some(0),
        "hook should pass on unborn branch: stdout: {}, stderr: {}",
        String::from_utf8_lossy(&hook_out.stdout),
        String::from_utf8_lossy(&hook_out.stderr)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn git_repo_detected_in_subdirectory() {
    let repo_dir = create_temp_dir("git_repo_parent");
    init_git_repo(&repo_dir);

    let sub_dir = repo_dir.join("packages").join("lib");
    fs::create_dir_all(&sub_dir).unwrap();

    let defs = create_dummy_definitions();
    let output = Command::new(env!("CARGO_BIN_EXE_rojo-doctor"))
        .args(["init", "--definitions-from"])
        .arg(&defs)
        .arg(&sub_dir)
        .output()
        .expect("init in subdir");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("git config core.hooksPath: configured (.githooks)"),
        "stdout was: {stdout}"
    );

    let _ = fs::remove_dir_all(&repo_dir);
}
