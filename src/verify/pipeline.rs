use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;

use crate::{
    checks,
    cli::{OutputFormat, SolverVersion, VerifyArgs},
    diagnostic::Diagnostic,
    project,
    verify::tools::{
        needs_pesde_install, put_summary_first, resolve_definitions, resolve_selene_config,
        run_tool, tool_exists, SeleneConfig,
    },
};

const BANNED_PATH_SUBSTRINGS: &[&str] = &[
    "/.local/",
    "/.config/",
    "/.cache/",
    "/node_modules/",
    "/.git/",
    "/.cargo/",
    "/undo/",
    "/.pesde/",
    "/roblox_packages/",
    "/roblox_server_packages/",
    "/pesde_packages/",
    "/Packages/",
    "/build/",
    "/out/",
];

pub fn is_safe_luau_file(file_path: &Path) -> bool {
    let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if ext != "luau" && ext != "lua" {
        return false;
    }
    let file_name = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if file_name.ends_with(".d.luau") || file_name.ends_with(".d.lua") || file_name.contains('%') {
        return false;
    }

    let normalized = file_path.to_string_lossy().replace('\\', "/");
    for &banned in BANNED_PATH_SUBSTRINGS {
        let without_slash = &banned[1..];
        if normalized.contains(banned) || normalized.starts_with(without_slash) {
            return false;
        }
    }

    let Ok(meta) = fs::metadata(file_path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }

    let Ok(mut f) = fs::File::open(file_path) else {
        return false;
    };
    let mut buf = [0u8; 512];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    if buf[..n].contains(&0) {
        return false;
    }

    true
}

pub fn find_project_root(start_path: &Path) -> Option<PathBuf> {
    let mut cur = match fs::canonicalize(start_path) {
        Ok(p) => p,
        Err(_) => start_path.to_path_buf(),
    };
    if cur.is_file() {
        if let Some(parent) = cur.parent() {
            cur = parent.to_path_buf();
        }
    }

    let root_limit = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));

    loop {
        if let Ok(entries) = fs::read_dir(&cur) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if s.ends_with(".project.json") || s.ends_with(".project.jsonc") {
                    return Some(cur);
                }
            }
        }

        if cur == root_limit {
            break;
        }

        let Some(parent) = cur.parent() else {
            break;
        };
        if parent == cur {
            break;
        }
        cur = parent.to_path_buf();
    }

    None
}

fn scan_dir_for_safe_luau_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if name_str.starts_with('.')
            || name_str == "node_modules"
            || name_str == "roblox_packages"
            || name_str == "pesde_packages"
            || name_str == "Packages"
            || name_str == "build"
            || name_str == "out"
        {
            continue;
        }
        if let Ok(ft) = entry.file_type() {
            if ft.is_dir() {
                scan_dir_for_safe_luau_files(&path, out);
            } else if ft.is_file() && is_safe_luau_file(&path) {
                out.push(path);
            }
        }
    }
}

pub enum DiscoveryOutcome {
    Files(Vec<PathBuf>),
    DiffEmpty,
    NoneFound,
    InvocationError(String),
}

pub fn discover_target_files(
    input_paths: &[PathBuf],
    project_root: Option<&Path>,
    cwd: &Path,
    diff_rev: Option<&str>,
) -> DiscoveryOutcome {
    let effective_root = project_root.unwrap_or(cwd);

    if let Some(rev) = diff_rev {
        if !tool_exists("git") {
            return DiscoveryOutcome::InvocationError(
                "git is not installed or not on PATH".to_string(),
            );
        }

        // Find git repo root using git rev-parse --show-toplevel
        let toplevel_res = match run_tool(
            "git",
            &["rev-parse", "--show-toplevel"],
            effective_root,
            &[],
        ) {
            Ok(r) if r.success => r.stdout.trim().to_string(),
            Ok(r) => {
                let err = if !r.stderr.is_empty() {
                    r.stderr.trim().to_string()
                } else {
                    "not a git repository".to_string()
                };
                return DiscoveryOutcome::InvocationError(err);
            }
            Err(e) => {
                return DiscoveryOutcome::InvocationError(e.to_string());
            }
        };

        let repo_root = PathBuf::from(toplevel_res);
        let mut files = Vec::new();

        // git diff --name-only -z --diff-filter=ACMR <rev>
        match run_tool(
            "git",
            &["diff", "--name-only", "-z", "--diff-filter=ACMR", rev],
            &repo_root,
            &[],
        ) {
            Ok(r) if r.success => {
                for item in r.stdout.split('\0') {
                    if !item.is_empty() {
                        let path = repo_root.join(item);
                        // Only include if file exists, is a safe Luau file, and is under effective_root
                        if path.starts_with(effective_root) && is_safe_luau_file(&path) {
                            files.push(path);
                        }
                    }
                }
            }
            Ok(r) => {
                let err = if !r.stderr.is_empty() {
                    r.stderr.trim().to_string()
                } else {
                    format!("git diff failed for revision `{rev}`")
                };
                return DiscoveryOutcome::InvocationError(err);
            }
            Err(e) => {
                return DiscoveryOutcome::InvocationError(e.to_string());
            }
        }

        // git ls-files --others --exclude-standard -z
        match run_tool(
            "git",
            &["ls-files", "--others", "--exclude-standard", "-z"],
            &repo_root,
            &[],
        ) {
            Ok(r) if r.success => {
                for item in r.stdout.split('\0') {
                    if !item.is_empty() {
                        let path = repo_root.join(item);
                        if path.starts_with(effective_root) && is_safe_luau_file(&path) {
                            files.push(path);
                        }
                    }
                }
            }
            Ok(r) => {
                let err = if !r.stderr.is_empty() {
                    r.stderr.trim().to_string()
                } else {
                    "git ls-files failed".to_string()
                };
                return DiscoveryOutcome::InvocationError(err);
            }
            Err(e) => {
                return DiscoveryOutcome::InvocationError(e.to_string());
            }
        }

        files.sort();
        files.dedup();

        if files.is_empty() {
            return DiscoveryOutcome::DiffEmpty;
        }
        return DiscoveryOutcome::Files(files);
    }

    if !input_paths.is_empty() {
        let mut files = Vec::new();
        for p in input_paths {
            let abs = if p.is_absolute() {
                p.clone()
            } else {
                cwd.join(p)
            };
            if !abs.exists() {
                continue;
            }
            if abs.is_dir() {
                scan_dir_for_safe_luau_files(&abs, &mut files);
            } else if is_safe_luau_file(&abs) {
                files.push(abs);
            }
        }
        files.sort();
        files.dedup();
        if files.is_empty() {
            return DiscoveryOutcome::NoneFound;
        }
        return DiscoveryOutcome::Files(files);
    }

    // Default: check project_root's src, tests, lib
    let mut files = Vec::new();
    let root = effective_root;
    for cand in ["src", "tests", "lib"] {
        let cand_path = root.join(cand);
        if cand_path.is_dir() {
            scan_dir_for_safe_luau_files(&cand_path, &mut files);
        }
    }

    if files.is_empty() && tool_exists("git") {
        // Fallback to git status --porcelain -z -uall
        if let Ok(toplevel) = run_tool("git", &["rev-parse", "--show-toplevel"], root, &[]) {
            if toplevel.success {
                let repo_root = PathBuf::from(toplevel.stdout.trim());
                if let Ok(git_res) = run_tool(
                    "git",
                    &["status", "--porcelain", "-z", "-uall"],
                    &repo_root,
                    &[],
                ) {
                    if git_res.success {
                        let mut it = git_res.stdout.split('\0');
                        while let Some(item) = it.next() {
                            if item.len() > 3 {
                                let status_prefix = &item[..2];
                                let rel = if status_prefix.starts_with('R') {
                                    // In git status -z, renames are emitted as "R  <orig>\0<dest>\0"
                                    it.next().unwrap_or(&item[3..])
                                } else {
                                    &item[3..]
                                };
                                let path = repo_root.join(rel);
                                if path.starts_with(root) && is_safe_luau_file(&path) {
                                    files.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if files.is_empty() {
        // Fallback to scanning root itself
        scan_dir_for_safe_luau_files(root, &mut files);
    }

    files.sort();
    files.dedup();

    if files.is_empty() {
        DiscoveryOutcome::NoneFound
    } else {
        DiscoveryOutcome::Files(files)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VerifyVerdict {
    Passed,
    Failed,
    Degraded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StageStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageReport {
    pub name: String,
    pub status: StageStatus,
    pub duration_ms: u128,
    pub notes: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    pub project: String,
    pub ok: bool,
    pub verdict: VerifyVerdict,
    pub exit: i32,
    pub stages: Vec<StageReport>,
}

struct TestFailure {
    file: String,
    output: String,
}

pub fn execute_verify(args: VerifyArgs) -> (VerifyReport, ExitCode) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Validation: --diff and explicit PATHS are mutually exclusive
    if args.diff.is_some() && !args.paths.is_empty() {
        eprintln!("error[invocation]: `--diff` and explicit path arguments are mutually exclusive");
        let report = VerifyReport {
            project: cwd.display().to_string(),
            ok: false,
            verdict: VerifyVerdict::Failed,
            exit: 2,
            stages: Vec::new(),
        };
        return (report, ExitCode::from(2));
    }

    let first_target = args
        .paths
        .first()
        .map(|p| cwd.join(p))
        .unwrap_or_else(|| cwd.clone());
    let project_root = find_project_root(&first_target).or_else(|| find_project_root(&cwd));
    let effective_cwd = project_root.clone().unwrap_or_else(|| cwd.clone());

    // Definitions resolution early validation (L5: ROJO_DOCTOR_DEFINITIONS / --definitions nonexistent path exits 2)
    let resolved_defs_res = resolve_definitions(args.definitions.as_deref(), &effective_cwd);
    let resolved_defs = match resolved_defs_res {
        Ok(opt) => opt,
        Err(err) => {
            eprintln!("error[definitions]: {err}");
            let report = VerifyReport {
                project: effective_cwd.display().to_string(),
                ok: false,
                verdict: VerifyVerdict::Failed,
                exit: 2,
                stages: Vec::new(),
            };
            return (report, ExitCode::from(2));
        }
    };

    // Check project file existence and parsing (same order as check / discover_project_file)
    let (has_project, project_parse_error, discovered_project_path) =
        match project::discover_project_file(&effective_cwd) {
            Ok(path) => match project::load_project(Some(&path)) {
                Ok(_) => (true, None, Some(path)),
                Err(e) => (true, Some(e.to_string()), Some(path)),
            },
            Err(project::ProjectError::NoProjectFound { .. }) => (false, None, None),
            Err(e) => (true, Some(e.to_string()), None),
        };

    // If project file exists but is unparseable -> exit 2 with parse error
    if let Some(err) = project_parse_error {
        eprintln!("error[project-load]: {err}");
        let report = VerifyReport {
            project: effective_cwd.display().to_string(),
            ok: false,
            verdict: VerifyVerdict::Failed,
            exit: 2,
            stages: Vec::new(),
        };
        return (report, ExitCode::from(2));
    }

    let project_name = if let Some(ref p) = discovered_project_path {
        p.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "default.project.json".to_string())
    } else {
        effective_cwd.display().to_string()
    };

    // File discovery
    let diff_rev = args.diff.as_deref();
    let discovery = discover_target_files(&args.paths, project_root.as_deref(), &cwd, diff_rev);

    let files = match discovery {
        DiscoveryOutcome::InvocationError(err) => {
            eprintln!("error[git]: {err}");
            let report = VerifyReport {
                project: project_name,
                ok: false,
                verdict: VerifyVerdict::Failed,
                exit: 2,
                stages: Vec::new(),
            };
            return (report, ExitCode::from(2));
        }
        DiscoveryOutcome::DiffEmpty => {
            let report = VerifyReport {
                project: project_name,
                ok: true,
                verdict: VerifyVerdict::Passed,
                exit: 0,
                stages: Vec::new(),
            };
            return (report, ExitCode::SUCCESS);
        }
        DiscoveryOutcome::NoneFound => {
            let report = VerifyReport {
                project: project_name,
                ok: false,
                verdict: VerifyVerdict::Failed,
                exit: 1,
                stages: Vec::new(),
            };
            return (report, ExitCode::from(1));
        }
        DiscoveryOutcome::Files(f) => f,
    };

    // Stage 0: Install
    let mut install_report = StageReport {
        name: "install".to_string(),
        status: StageStatus::Skipped,
        duration_ms: 0,
        notes: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut setup_failed = false;

    if args.no_install {
        install_report.notes.push("(--no-install)".to_string());
    } else if needs_pesde_install(&effective_cwd) {
        if !tool_exists("pesde") {
            install_report.status = StageStatus::Failed;
            install_report
                .notes
                .push("(tool not found: pesde)".to_string());
            setup_failed = true;
        } else {
            let res = run_tool("pesde", &["install"], &effective_cwd, &[]);
            match res {
                Ok(r) if r.success => {
                    install_report.status = StageStatus::Passed;
                    install_report.duration_ms = r.duration_ms;
                }
                Ok(r) => {
                    install_report.status = StageStatus::Failed;
                    install_report.duration_ms = r.duration_ms;
                    let out = if !r.stderr.is_empty() {
                        r.stderr
                    } else {
                        r.stdout
                    };
                    install_report.notes.push(out);
                    setup_failed = true;
                }
                Err(e) => {
                    install_report.status = StageStatus::Failed;
                    install_report.notes.push(e.to_string());
                    setup_failed = true;
                }
            }
        }
    } else {
        let wally_toml = effective_cwd.join("wally.toml");
        let wally_pkgs = effective_cwd.join("Packages");
        if wally_toml.exists() && !wally_pkgs.exists() {
            if !tool_exists("wally") {
                install_report.status = StageStatus::Failed;
                install_report
                    .notes
                    .push("(tool not found: wally)".to_string());
                setup_failed = true;
            } else {
                let res = run_tool("wally", &["install"], &effective_cwd, &[]);
                match res {
                    Ok(r) if r.success => {
                        install_report.status = StageStatus::Passed;
                        install_report.duration_ms = r.duration_ms;
                    }
                    Ok(r) => {
                        install_report.status = StageStatus::Failed;
                        install_report.duration_ms = r.duration_ms;
                        let out = if !r.stderr.is_empty() {
                            r.stderr
                        } else {
                            r.stdout
                        };
                        install_report.notes.push(out);
                        setup_failed = true;
                    }
                    Err(e) => {
                        install_report.status = StageStatus::Failed;
                        install_report.notes.push(e.to_string());
                        setup_failed = true;
                    }
                }
            }
        } else {
            install_report.notes.push("(up to date)".to_string());
        }
    }

    // Load project if exists
    let loaded_project = if has_project {
        project::load_project(Some(&effective_cwd)).ok()
    } else {
        None
    };

    // If --fix: run stylua format and checks fix before concurrent checks
    let mut stylua_fix_duration = 0;
    let mut stylua_fix_failed = false;
    let mut stylua_fix_error = String::new();

    if args.fix {
        if let Some(loaded) = &loaded_project {
            let _ = checks::fix_all(loaded);
        }

        if tool_exists("stylua") {
            let str_files: Vec<&str> = files.iter().filter_map(|p| p.to_str()).collect();
            if let Ok(res) = run_tool("stylua", &str_files, &effective_cwd, &[]) {
                stylua_fix_duration = res.duration_ms;
                if !res.success {
                    stylua_fix_failed = true;
                    stylua_fix_error = if !res.stderr.is_empty() {
                        res.stderr
                    } else {
                        res.stdout
                    };
                }
            }
        }
    }

    // Sourcemap check & generation for types stage
    let default_project = effective_cwd.join("default.project.json");
    let sourcemap_path = effective_cwd.join("sourcemap.json");
    let mut sourcemap_failed = false;
    let mut sourcemap_error = String::new();

    if default_project.exists() {
        let p_mtime = fs::metadata(&default_project)
            .and_then(|m| m.modified())
            .ok();
        let s_mtime = fs::metadata(&sourcemap_path)
            .and_then(|m| m.modified())
            .ok();
        let needs_sm = match (p_mtime, s_mtime) {
            (_, None) => true,
            (Some(pm), Some(sm)) => pm > sm,
            _ => false,
        };
        if needs_sm {
            if !tool_exists("rojo") {
                sourcemap_failed = true;
                sourcemap_error =
                    "tool not found: rojo (needed to regenerate sourcemap.json)".to_string();
            } else {
                let sm_out = sourcemap_path.to_string_lossy().to_string();
                let res = run_tool(
                    "rojo",
                    &["sourcemap", "default.project.json", "-o", &sm_out],
                    &effective_cwd,
                    &[],
                );
                match res {
                    Ok(r) if r.success => {}
                    Ok(r) => {
                        sourcemap_failed = true;
                        sourcemap_error = if !r.stderr.is_empty() {
                            r.stderr
                        } else {
                            r.stdout
                        };
                    }
                    Err(e) => {
                        sourcemap_failed = true;
                        sourcemap_error = e.to_string();
                    }
                }
            }
        }
    }

    // Concurrent execution of Stages 1..5 using std::thread::scope
    let str_files: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    let effective_cwd_ref = &effective_cwd;
    let loaded_project_ref = loaded_project.as_ref();
    let str_files_ref = &str_files;

    let (structure_report, style_report, lint_report, types_report, tests_report) =
        std::thread::scope(|s| {
            // Stage 1: Structure
            let structure_handle = s.spawn(|| {
                let mut rep = StageReport {
                    name: "structure".to_string(),
                    status: StageStatus::Skipped,
                    duration_ms: 0,
                    notes: Vec::new(),
                    diagnostics: Vec::new(),
                };

                if !has_project {
                    if args.no_project {
                        rep.notes.push("(--no-project)".to_string());
                    } else {
                        rep.notes
                            .push("(no Rojo project; --no-project to accept)".to_string());
                    }
                    return rep;
                }

                let Some(loaded) = loaded_project_ref else {
                    rep.status = StageStatus::Failed;
                    rep.notes.push("could not parse project file".to_string());
                    return rep;
                };

                let start = std::time::Instant::now();
                match checks::run_all(loaded) {
                    Ok(check_report) => {
                        rep.duration_ms = start.elapsed().as_millis();
                        rep.diagnostics = check_report.diagnostics;
                        if rep.diagnostics.is_empty() {
                            rep.status = StageStatus::Passed;
                        } else {
                            rep.status = StageStatus::Failed;
                        }
                    }
                    Err(err) => {
                        rep.duration_ms = start.elapsed().as_millis();
                        rep.status = StageStatus::Failed;
                        rep.notes.push(err.to_string());
                    }
                }
                rep
            });

            // Stage 2: Style
            let style_handle = s.spawn(|| {
                let mut rep = StageReport {
                    name: "style".to_string(),
                    status: StageStatus::Skipped,
                    duration_ms: 0,
                    notes: Vec::new(),
                    diagnostics: Vec::new(),
                };

                if !tool_exists("stylua") {
                    rep.notes.push("(tool not found: stylua)".to_string());
                    return rep;
                }

                if args.fix {
                    rep.duration_ms = stylua_fix_duration;
                    if stylua_fix_failed {
                        rep.status = StageStatus::Failed;
                        rep.notes.push(stylua_fix_error);
                    } else {
                        rep.status = StageStatus::Passed;
                    }
                    return rep;
                }

                let mut cmd_args = vec!["--check"];
                let file_slices: Vec<&str> = str_files_ref.iter().map(|s| s.as_str()).collect();
                cmd_args.extend(file_slices);

                match run_tool("stylua", &cmd_args, effective_cwd_ref, &[]) {
                    Ok(r) => {
                        rep.duration_ms = r.duration_ms;
                        if r.success {
                            rep.status = StageStatus::Passed;
                        } else {
                            rep.status = StageStatus::Failed;
                            let out = if !r.stderr.is_empty() {
                                r.stderr
                            } else {
                                r.stdout
                            };
                            rep.notes.push(out);
                        }
                    }
                    Err(e) => {
                        rep.status = StageStatus::Failed;
                        rep.notes.push(e.to_string());
                    }
                }
                rep
            });

            // Stage 3: Lint
            let lint_handle = s.spawn(|| {
                let mut rep = StageReport {
                    name: "lint".to_string(),
                    status: StageStatus::Skipped,
                    duration_ms: 0,
                    notes: Vec::new(),
                    diagnostics: Vec::new(),
                };

                if !tool_exists("selene") {
                    rep.notes.push("(tool not found: selene)".to_string());
                    return rep;
                }

                let selene_cfg = resolve_selene_config(effective_cwd_ref, has_project);
                let (cfg_path, is_synthesized) = match &selene_cfg {
                    SeleneConfig::Existing(p) => (p.clone(), false),
                    SeleneConfig::Synthesized(p) => (p.clone(), true),
                };
                if is_synthesized {
                    let std_name = if has_project { "roblox" } else { "luau" };
                    rep.notes.push(format!(
                        "(using synthesized default {std_name} config: std = \"{std_name}\")"
                    ));
                }

                let cfg_str = cfg_path.to_string_lossy().to_string();
                let mut cmd_args = vec!["--config", cfg_str.as_str()];
                for f in str_files_ref {
                    cmd_args.push(f.as_str());
                }

                let tmp_dir = std::env::temp_dir();
                let tmp_str = tmp_dir.to_string_lossy();
                let xdg_cache =
                    std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| tmp_str.to_string());

                let run_res = run_tool(
                    "selene",
                    &cmd_args,
                    effective_cwd_ref,
                    &[("XDG_CACHE_HOME", &xdg_cache)],
                );

                if let SeleneConfig::Synthesized(ref p) = selene_cfg {
                    let _ = fs::remove_file(p);
                }

                match run_res {
                    Ok(r) => {
                        rep.duration_ms = r.duration_ms;
                        if r.success {
                            rep.status = StageStatus::Passed;
                        } else {
                            rep.status = StageStatus::Failed;
                            let combined = if !r.stderr.is_empty() && !r.stdout.is_empty() {
                                format!("{}\n{}", r.stderr.trim(), r.stdout.trim())
                            } else if !r.stderr.is_empty() {
                                r.stderr
                            } else {
                                r.stdout
                            };
                            rep.notes.push(put_summary_first(&combined));
                        }
                    }
                    Err(e) => {
                        rep.status = StageStatus::Failed;
                        rep.notes.push(e.to_string());
                    }
                }
                rep
            });

            // Stage 4: Types
            let types_handle = s.spawn(|| {
                let mut rep = StageReport {
                    name: "types".to_string(),
                    status: StageStatus::Skipped,
                    duration_ms: 0,
                    notes: Vec::new(),
                    diagnostics: Vec::new(),
                };

                if sourcemap_failed {
                    rep.status = StageStatus::Failed;
                    rep.notes
                        .push(format!("sourcemap generation failed: {sourcemap_error}"));
                    return rep;
                }

                if !tool_exists("luau-lsp") {
                    rep.notes.push("(tool not found: luau-lsp)".to_string());
                    return rep;
                }

                let mut cmd_args: Vec<String> = vec!["analyze".to_string()];
                if args.solver == SolverVersion::V2 {
                    cmd_args.push("--flag:LuauSolverV2=true".to_string());
                }

                let without_engine_defs = resolved_defs.is_none();
                if let Some(ref d) = resolved_defs {
                    cmd_args.push(format!("--definitions={}", d.display()));
                }

                let sm_path = effective_cwd_ref.join("sourcemap.json");
                if sm_path.exists() {
                    cmd_args.push(format!("--sourcemap={}", sm_path.display()));
                }

                cmd_args.push("--ignore=roblox_packages/**".to_string());
                cmd_args.push("--ignore=pesde_packages/**".to_string());
                cmd_args.push("--ignore=**/roblox_packages/**".to_string());
                cmd_args.push("--ignore=**/pesde_packages/**".to_string());
                cmd_args.push("--ignore=Packages/**".to_string());
                cmd_args.push("--ignore=**/Packages/**".to_string());

                for f in str_files_ref {
                    cmd_args.push(f.clone());
                }

                let arg_refs: Vec<&str> = cmd_args.iter().map(|s| s.as_str()).collect();
                match run_tool("luau-lsp", &arg_refs, effective_cwd_ref, &[]) {
                    Ok(r) => {
                        rep.duration_ms = r.duration_ms;
                        let combined = format!("{}\n{}", r.stderr, r.stdout);
                        let filtered_lines: Vec<&str> = combined
                            .lines()
                            .map(str::trim)
                            .filter(|l| {
                                !l.is_empty()
                                    && !l.starts_with("[INFO]")
                                    && !l.starts_with("[WARN]")
                                    && !l.starts_with("WARNING: --platform")
                            })
                            .collect();
                        let filtered_output = filtered_lines.join("\n");

                        if without_engine_defs {
                            rep.notes
                                .push("types ran without engine definitions".to_string());
                        }

                        if r.success && filtered_output.is_empty() {
                            rep.status = StageStatus::Passed;
                        } else if !r.success || filtered_output.contains("TypeError:") {
                            rep.status = StageStatus::Failed;
                            if !filtered_output.is_empty() {
                                rep.notes.push(filtered_output);
                            }
                        } else {
                            // Pass with warnings
                            rep.status = StageStatus::Passed;
                            rep.notes.push("passed with warnings".to_string());
                            if !filtered_output.is_empty() {
                                rep.notes.push(filtered_output);
                            }
                        }
                    }
                    Err(e) => {
                        rep.status = StageStatus::Failed;
                        rep.notes.push(e.to_string());
                    }
                }
                rep
            });

            // Stage 5: Tests
            let tests_handle = s.spawn(|| {
                let mut rep = StageReport {
                    name: "tests".to_string(),
                    status: StageStatus::Skipped,
                    duration_ms: 0,
                    notes: Vec::new(),
                    diagnostics: Vec::new(),
                };

                // Check M3: when explicit PATHS lie outside the project root, add note that <root>/tests was not covered
                let any_path_outside_root = !args.paths.is_empty()
                    && args.paths.iter().any(|p| {
                        let abs = if p.is_absolute() {
                            p.clone()
                        } else {
                            cwd.join(p)
                        };
                        !abs.starts_with(effective_cwd_ref)
                    });
                if any_path_outside_root {
                    rep.notes.push(format!(
                        "({}/tests was not covered)",
                        effective_cwd_ref.display()
                    ));
                }

                if args.no_tests {
                    rep.notes.push("(--no-tests)".to_string());
                    return rep;
                }

                let tests_dir = effective_cwd_ref.join("tests");
                if !tests_dir.is_dir() {
                    rep.notes.push("(no tests/ directory)".to_string());
                    return rep;
                }

                let run_luau = tests_dir.join("run.luau");
                let mut test_files = Vec::new();

                if run_luau.is_file() {
                    // a. Entrypoint: tests/run.luau exists -> single run is the suite
                    test_files.push("run.luau".to_string());
                } else {
                    // b. Walk tests/ recursively (bounded depth 3)
                    fn walk_tests(dir: &Path, base: &Path, depth: usize, out: &mut Vec<String>) {
                        if depth > 3 {
                            return;
                        }
                        if let Ok(entries) = fs::read_dir(dir) {
                            for entry in entries.flatten() {
                                let path = entry.path();
                                if path.is_file() {
                                    let fname = entry.file_name().to_string_lossy().to_string();
                                    if fname.ends_with(".test.luau")
                                        || fname.ends_with(".spec.luau")
                                        || fname.ends_with("_test.luau")
                                        || (fname.starts_with("test_")
                                            && (fname.ends_with(".luau")
                                                || fname.ends_with(".lua")))
                                    {
                                        if let Ok(rel) = path.strip_prefix(base) {
                                            out.push(rel.to_string_lossy().to_string());
                                        }
                                    }
                                } else if path.is_dir() {
                                    walk_tests(&path, base, depth + 1, out);
                                }
                            }
                        }
                    }

                    walk_tests(&tests_dir, &tests_dir, 1, &mut test_files);
                }

                if let Some(ref pat) = args.test {
                    test_files.retain(|name| name.contains(pat));
                }

                if test_files.is_empty() {
                    let reason = if let Some(ref pat) = args.test {
                        format!("(no tests matched \"{pat}\")")
                    } else {
                        "(no test suites found)".to_string()
                    };
                    rep.notes.push(reason);
                    return rep;
                }

                if !tool_exists("lune") {
                    rep.notes.push("(tool not found: lune)".to_string());
                    return rep;
                }

                test_files.sort();
                let total_suites = test_files.len();
                let mut failures: Vec<TestFailure> = Vec::new();
                let start = std::time::Instant::now();

                // Run in batches of up to 8 concurrent tests
                for chunk in test_files.chunks(8) {
                    let results: Vec<(
                        String,
                        Result<crate::verify::tools::ToolResult, std::io::Error>,
                    )> = std::thread::scope(|sub| {
                        let handles: Vec<_> = chunk
                            .iter()
                            .map(|tf| {
                                let tf = tf.clone();
                                sub.spawn(move || {
                                    let p = format!("tests/{tf}");
                                    let res =
                                        run_tool("lune", &["run", &p], effective_cwd_ref, &[]);
                                    (tf, res)
                                })
                            })
                            .collect();
                        handles.into_iter().map(|h| h.join().unwrap()).collect()
                    });

                    for (tf, res) in results {
                        match res {
                            Ok(r) if r.success => {}
                            Ok(r) => {
                                let out = if !r.stderr.is_empty() {
                                    r.stderr
                                } else {
                                    r.stdout
                                };
                                failures.push(TestFailure {
                                    file: tf,
                                    output: out,
                                });
                            }
                            Err(e) => {
                                failures.push(TestFailure {
                                    file: tf,
                                    output: e.to_string(),
                                });
                            }
                        }
                    }
                }

                rep.duration_ms = start.elapsed().as_millis();

                if failures.is_empty() {
                    rep.status = StageStatus::Passed;
                    rep.notes
                        .push(format!("all passed ({total_suites} suites)"));
                } else {
                    rep.status = StageStatus::Failed;
                    rep.notes.push(format!("{} failed", failures.len()));
                    for f in failures {
                        rep.notes
                            .push(format!("✖ {} failed:\n{}", f.file, f.output));
                    }
                }

                rep
            });

            (
                structure_handle.join().unwrap(),
                style_handle.join().unwrap(),
                lint_handle.join().unwrap(),
                types_handle.join().unwrap(),
                tests_handle.join().unwrap(),
            )
        });

    // Check degraded tool status or missing project without --no-project
    let check_degraded = |stage: &StageReport| -> bool {
        stage.status == StageStatus::Skipped
            && stage
                .notes
                .iter()
                .any(|n| n.starts_with("(tool not found:"))
    };

    let structure_missing_project = !has_project && !args.no_project;

    let is_degraded = structure_missing_project
        || check_degraded(&style_report)
        || check_degraded(&lint_report)
        || check_degraded(&types_report)
        || check_degraded(&tests_report);

    let any_failed = setup_failed
        || install_report.status == StageStatus::Failed
        || structure_report.status == StageStatus::Failed
        || style_report.status == StageStatus::Failed
        || lint_report.status == StageStatus::Failed
        || types_report.status == StageStatus::Failed
        || tests_report.status == StageStatus::Failed;

    let verdict = if any_failed {
        VerifyVerdict::Failed
    } else if is_degraded {
        VerifyVerdict::Degraded
    } else {
        VerifyVerdict::Passed
    };

    let exit_code = if verdict == VerifyVerdict::Passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    };

    let report = VerifyReport {
        project: project_name,
        ok: verdict == VerifyVerdict::Passed,
        verdict,
        exit: if verdict == VerifyVerdict::Passed {
            0
        } else {
            1
        },
        stages: vec![
            install_report,
            structure_report,
            style_report,
            lint_report,
            types_report,
            tests_report,
        ],
    };

    (report, exit_code)
}

pub fn print_verify_report(report: &VerifyReport, format: OutputFormat, verbose: bool) {
    match format {
        OutputFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(report) {
                println!("{json}");
            }
        }
        OutputFormat::Text => {
            if report.stages.is_empty() {
                if report.ok {
                    println!("nothing to verify");
                } else if report.exit == 1 {
                    eprintln!("no Luau files found");
                }
                return;
            }

            let stage_labels = [
                ("Dependency installation...", 0),
                ("Rojo project structure... ", 1),
                ("StyLua code formatting... ", 2),
                ("Selene static analysis...  ", 3),
                ("Luau-LSP type checking...  ", 4),
                ("Lune headless tests...     ", 5),
            ];

            for (label, idx) in stage_labels {
                if idx >= report.stages.len() {
                    continue;
                }
                let stage = &report.stages[idx];

                match stage.status {
                    StageStatus::Passed => {
                        let has_warn = stage.notes.iter().any(|n| n == "passed with warnings");
                        let without_defs = stage
                            .notes
                            .iter()
                            .any(|n| n == "types ran without engine definitions");
                        let tests_passed_note =
                            stage.notes.iter().find(|n| n.starts_with("all passed ("));

                        if let Some(note) = tests_passed_note {
                            // "all passed (N suites)" -> "✓ All passed (N suites) [123ms]"
                            let suites = note.strip_prefix("all passed ").unwrap_or("(suites)");
                            println!(
                                "  [{idx}/5] {label} ✓ All passed {suites} [{}ms]",
                                stage.duration_ms
                            );
                        } else if has_warn {
                            println!(
                                "  [{idx}/5] {label} ✓ Passed (with warnings) [{}ms]",
                                stage.duration_ms
                            );
                        } else if without_defs {
                            println!(
                                "  [{idx}/5] {label} ✓ Passed (without engine definitions) [{}ms]",
                                stage.duration_ms
                            );
                        } else {
                            println!("  [{idx}/5] {label} ✓ Passed [{}ms]", stage.duration_ms);
                        }
                    }
                    StageStatus::Failed => {
                        println!("  [{idx}/5] {label} ✖ FAILED [{}ms]", stage.duration_ms);
                    }
                    StageStatus::Skipped => {
                        let reason = stage.notes.first().map(|s| s.as_str()).unwrap_or("");
                        println!("  [{idx}/5] {label} ○ Skipped {reason}");
                    }
                }
            }

            // Print stage details / errors if any failed or verbose
            // 1. Structure diagnostics
            if let Some(structure_stage) = report.stages.get(1) {
                if !structure_stage.diagnostics.is_empty() {
                    for diag in &structure_stage.diagnostics {
                        println!();
                        println!("{diag}");
                    }
                }
            }

            // 2. Style errors
            if let Some(style_stage) = report.stages.get(2) {
                if style_stage.status == StageStatus::Failed {
                    for note in &style_stage.notes {
                        if !note.is_empty() {
                            println!();
                            println!("{note}");
                        }
                    }
                }
            }

            // 3. Selene errors
            if let Some(lint_stage) = report.stages.get(3) {
                if lint_stage.status == StageStatus::Failed {
                    for note in &lint_stage.notes {
                        if !note.is_empty() {
                            println!();
                            println!("{note}");
                        }
                    }
                }
            }

            // 4. Luau-LSP errors or verbose warnings
            if let Some(types_stage) = report.stages.get(4) {
                if types_stage.status == StageStatus::Failed {
                    for note in &types_stage.notes {
                        if note != "types ran without engine definitions" && !note.is_empty() {
                            println!();
                            println!("{note}");
                        }
                    }
                } else if verbose {
                    for note in &types_stage.notes {
                        if note != "passed with warnings"
                            && note != "types ran without engine definitions"
                            && !note.is_empty()
                        {
                            println!();
                            println!("{note}");
                        }
                    }
                }
            }

            // 5. Tests errors
            if let Some(tests_stage) = report.stages.get(5) {
                if tests_stage.status == StageStatus::Failed {
                    for note in &tests_stage.notes {
                        if note.starts_with("✖ ") {
                            println!();
                            println!("{note}");
                        }
                    }
                }
            }

            // Final verdict line
            println!();
            match report.verdict {
                VerifyVerdict::Passed => println!("VERDICT: PASSED"),
                VerifyVerdict::Failed => println!("VERDICT: FAILED"),
                VerifyVerdict::Degraded => println!("VERDICT: DEGRADED"),
            }
        }
    }
}
