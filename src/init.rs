use std::{
    fs,
    path::Path,
    process::{Command, ExitCode},
};

use crate::cli::InitArgs;

const GLOBAL_TYPES_URL: &str =
    "https://raw.githubusercontent.com/JohnnyMorganz/luau-lsp/1.70.1/scripts/globalTypes.d.luau";

pub fn run(args: InitArgs) -> ExitCode {
    let target = &args.path;
    let (target_dir, has_project) = if target.is_file() {
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        let is_proj = name.ends_with(".project.json") || name.ends_with(".project.jsonc");
        let dir = target
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        (dir, is_proj)
    } else {
        let has = has_project_file(target);
        (target.clone(), has)
    };

    if let Err(err) = fs::create_dir_all(&target_dir) {
        eprintln!(
            "error[init]: failed to create target directory `{}`: {err}",
            target_dir.display()
        );
        return ExitCode::from(2);
    }

    if !has_project {
        println!(
            "warning: no *.project.json or *.project.jsonc found; verify will need --no-project"
        );
    }

    // 1. Engine definitions
    if !args.no_types {
        let types_dir = target_dir.join("types");
        let dest_path = types_dir.join("globalTypes.d.luau");

        if dest_path.exists() && !args.force {
            println!("types/globalTypes.d.luau: already present");
        } else {
            let is_update = dest_path.exists();
            if let Err(err) = fs::create_dir_all(&types_dir) {
                eprintln!("error[init]: failed to create types directory: {err}");
                return ExitCode::from(2);
            }

            let pid = std::process::id();
            let temp_path = types_dir.join(format!("globalTypes.d.luau.tmp-{pid}"));

            let copy_or_download_res = if let Some(from_path) = &args.definitions_from {
                if !from_path.exists() {
                    eprintln!(
                        "error[init]: definitions source file not found: `{}`",
                        from_path.display()
                    );
                    Err(())
                } else if from_path.is_dir() {
                    eprintln!(
                        "error[init]: definitions source is a directory, expected a file: `{}`",
                        from_path.display()
                    );
                    Err(())
                } else if let Err(err) = fs::copy(from_path, &temp_path) {
                    eprintln!(
                        "error[init]: failed to copy definitions from `{}`: {err}",
                        from_path.display()
                    );
                    Err(())
                } else {
                    Ok(())
                }
            } else {
                let res = Command::new("curl")
                    .args(["-fsSL", GLOBAL_TYPES_URL, "-o"])
                    .arg(&temp_path)
                    .output();
                match res {
                    Ok(out) if out.status.success() => Ok(()),
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        eprintln!(
                            "error[init]: failed to download engine definitions from {GLOBAL_TYPES_URL}: {}",
                            stderr.trim()
                        );
                        Err(())
                    }
                    Err(err) => {
                        eprintln!(
                            "error[init]: failed to execute curl to download engine definitions: {err}"
                        );
                        Err(())
                    }
                }
            };

            if copy_or_download_res.is_err() {
                let _ = fs::remove_file(&temp_path);
                return ExitCode::from(2);
            }

            if let Err(err) = fs::rename(&temp_path, &dest_path) {
                let _ = fs::remove_file(&temp_path);
                eprintln!(
                    "error[init]: failed to place engine definitions into `{}`: {err}",
                    dest_path.display()
                );
                return ExitCode::from(2);
            }

            if is_update {
                println!("types/globalTypes.d.luau: updated");
            } else {
                println!("types/globalTypes.d.luau: created");
            }
        }
    }

    // 2. CI workflow
    if !args.no_ci {
        let workflows_dir = target_dir.join(".github").join("workflows");
        let dest_path = workflows_dir.join("verify.yml");

        if dest_path.exists() && !args.force {
            println!(".github/workflows/verify.yml: already present");
        } else {
            let is_update = dest_path.exists();
            if let Err(err) = fs::create_dir_all(&workflows_dir) {
                eprintln!("error[init]: failed to create workflow directory: {err}");
                return ExitCode::from(2);
            }

            let (push_cmd, pr_cmd) = if has_project {
                (
                    "rojo-doctor verify --no-install",
                    "rojo-doctor verify --no-install --diff origin/${{ github.base_ref }}",
                )
            } else {
                (
                    "rojo-doctor verify --no-install --no-project",
                    "rojo-doctor verify --no-install --diff origin/${{ github.base_ref }} --no-project",
                )
            };

            let version = env!("CARGO_PKG_VERSION");
            let workflow_yaml = format!(
                r#"name: verify

on:
  push:
    branches: [main]
  pull_request:

jobs:
  verify:
    name: Verify
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: CompeyDev/setup-rokit@v0.2.1
      - uses: dtolnay/rust-toolchain@stable
      - name: Install rojo-doctor
        run: cargo install --git https://github.com/ujji-k06/rojo-doctor --tag v{version} --locked
      - name: Install toolchain via rokit
        run: if [ -f rokit.toml ] && command -v rokit >/dev/null 2>&1; then rokit install; fi
      - name: Run verification (push)
        if: github.event_name == 'push'
        run: {push_cmd}
      - name: Run verification (pull_request)
        if: github.event_name == 'pull_request'
        run: {pr_cmd}
"#
            );

            if let Err(err) = fs::write(&dest_path, workflow_yaml) {
                eprintln!("error[init]: failed to write workflow file: {err}");
                return ExitCode::from(2);
            }

            if is_update {
                println!(".github/workflows/verify.yml: updated");
            } else {
                println!(".github/workflows/verify.yml: created");
            }
        }
    }

    // 3. Pre-commit hook
    if !args.no_hook {
        let hooks_dir = target_dir.join(".githooks");
        let dest_path = hooks_dir.join("pre-commit");

        if dest_path.exists() && !args.force {
            println!(".githooks/pre-commit: already present");
        } else {
            let is_update = dest_path.exists();
            if let Err(err) = fs::create_dir_all(&hooks_dir) {
                eprintln!("error[init]: failed to create hooks directory: {err}");
                return ExitCode::from(2);
            }

            let no_project_flag = if has_project { "" } else { " --no-project" };

            let hook_sh = format!(
                r#"#!/bin/sh
set -e

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$REPO_ROOT"

if git rev-parse --verify HEAD >/dev/null 2>&1; then
    DIFF_FLAG="--diff"
else
    DIFF_FLAG=""
fi

if ! rojo-doctor verify $DIFF_FLAG --no-install{no_project_flag}; then
    echo "" >&2
    echo "rojo-doctor verification failed." >&2
    echo "Hint: to bypass this check, commit with --no-verify (git commit --no-verify)." >&2
    exit 1
fi
"#
            );

            if let Err(err) = fs::write(&dest_path, hook_sh) {
                eprintln!("error[init]: failed to write pre-commit hook: {err}");
                return ExitCode::from(2);
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Err(err) = fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755))
                {
                    eprintln!("warning[init]: could not set executable permissions on hook: {err}");
                }
            }

            if is_update {
                println!(".githooks/pre-commit: updated");
            } else {
                println!(".githooks/pre-commit: created");
            }
        }
    }

    // 4. Git config core.hooksPath
    if !args.no_hook {
        let is_git_repo = Command::new("git")
            .arg("-C")
            .arg(&target_dir)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false);

        if is_git_repo {
            let current_hooks = Command::new("git")
                .arg("-C")
                .arg(&target_dir)
                .args(["config", "--get", "core.hooksPath"])
                .output();
            let already_set = current_hooks
                .ok()
                .filter(|out| out.status.success())
                .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
                == Some(".githooks".to_string());

            if already_set && !args.force {
                println!("git config core.hooksPath: already present (.githooks)");
            } else {
                let result = Command::new("git")
                    .arg("-C")
                    .arg(&target_dir)
                    .args(["config", "core.hooksPath", ".githooks"])
                    .output();
                match result {
                    Ok(out) if out.status.success() => {
                        println!("git config core.hooksPath: configured (.githooks)");
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        eprintln!(
                            "warning[git]: could not set core.hooksPath: {}",
                            stderr.trim()
                        );
                    }
                    Err(err) => {
                        eprintln!("warning[git]: failed to execute git config: {err}");
                    }
                }
            }
        }
    }

    // 5. .gitignore
    let gitignore_path = target_dir.join(".gitignore");
    if !gitignore_path.exists() {
        if let Err(err) = fs::write(&gitignore_path, "sourcemap.json\n") {
            eprintln!("error[init]: failed to write .gitignore: {err}");
            return ExitCode::from(2);
        }
        println!(".gitignore: created");
    } else {
        match fs::read_to_string(&gitignore_path) {
            Ok(content) => {
                if content.lines().any(|line| line.trim() == "sourcemap.json") {
                    println!(".gitignore: already present");
                } else {
                    let mut new_content = content;
                    if !new_content.is_empty() && !new_content.ends_with('\n') {
                        new_content.push('\n');
                    }
                    new_content.push_str("sourcemap.json\n");
                    if let Err(err) = fs::write(&gitignore_path, new_content) {
                        eprintln!("error[init]: failed to update .gitignore: {err}");
                        return ExitCode::from(2);
                    }
                    println!(".gitignore: updated");
                }
            }
            Err(err) => {
                eprintln!("error[init]: failed to read .gitignore: {err}");
                return ExitCode::from(2);
            }
        }
    }

    ExitCode::SUCCESS
}

fn has_project_file(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        if let Ok(file_type) = entry.file_type() {
            if file_type.is_file() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".project.json") || name_str.ends_with(".project.jsonc") {
                    return true;
                }
            }
        }
    }
    false
}
