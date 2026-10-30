use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub fn tool_exists(tool: &str) -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for mut p in std::env::split_paths(&paths) {
            p.push(tool);
            if p.is_file() {
                return true;
            }
            #[cfg(windows)]
            {
                p.set_extension("exe");
                if p.is_file() {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

pub fn run_tool(
    cmd: &str,
    args: &[&str],
    cwd: &Path,
    envs: &[(&str, &str)],
) -> std::io::Result<ToolResult> {
    let start = std::time::Instant::now();
    let mut command = std::process::Command::new(cmd);
    command.args(args).current_dir(cwd);
    for (k, v) in envs {
        command.env(k, v);
    }
    let output = command.output()?;
    let duration_ms = start.elapsed().as_millis();
    Ok(ToolResult {
        success: output.status.success(),
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms,
    })
}

pub fn resolve_definitions(
    cli_definitions: Option<&Path>,
    cwd: &Path,
) -> Result<Option<PathBuf>, String> {
    // 1. --definitions
    if let Some(p) = cli_definitions {
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        };
        let canonical = match fs::canonicalize(&abs) {
            Ok(c) => c,
            Err(_) => {
                if !abs.exists() {
                    return Err(format!(
                        "engine definitions file not found: `{}`",
                        p.display()
                    ));
                }
                abs
            }
        };
        return Ok(Some(canonical));
    }

    // 2. $ROJO_DOCTOR_DEFINITIONS
    if let Ok(val) = std::env::var("ROJO_DOCTOR_DEFINITIONS") {
        if !val.trim().is_empty() {
            let p = PathBuf::from(&val);
            let abs = if p.is_absolute() { p } else { cwd.join(p) };
            let canonical = match fs::canonicalize(&abs) {
                Ok(c) => c,
                Err(_) => {
                    if !abs.exists() {
                        return Err(format!(
                            "ROJO_DOCTOR_DEFINITIONS path does not exist: `{val}`"
                        ));
                    }
                    abs
                }
            };
            return Ok(Some(canonical));
        }
    }

    // 3. $DSH_HOME/types/globalTypes.d.luau
    let dsh_home = std::env::var_os("DSH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".dsh")
        });
    let dsh_types = dsh_home.join("types").join("globalTypes.d.luau");
    if dsh_types.exists() {
        let canonical = fs::canonicalize(&dsh_types).unwrap_or(dsh_types);
        return Ok(Some(canonical));
    }

    // 4. ./globalTypes.d.luau
    let local_root = cwd.join("globalTypes.d.luau");
    if local_root.exists() {
        let canonical = fs::canonicalize(&local_root).unwrap_or(local_root);
        return Ok(Some(canonical));
    }

    // 5. ./types/globalTypes.d.luau
    let local_types = cwd.join("types").join("globalTypes.d.luau");
    if local_types.exists() {
        let canonical = fs::canonicalize(&local_types).unwrap_or(local_types);
        return Ok(Some(canonical));
    }

    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeleneConfig {
    Existing(PathBuf),
    Synthesized(PathBuf),
}

pub fn resolve_selene_config(cwd: &Path, is_rojo: bool) -> SeleneConfig {
    let mut cur = match fs::canonicalize(cwd) {
        Ok(c) => c,
        Err(_) => cwd.to_path_buf(),
    };
    if cur.is_file() {
        if let Some(parent) = cur.parent() {
            cur = parent.to_path_buf();
        }
    }

    loop {
        let local_config = cur.join("selene.toml");
        if local_config.exists() {
            return SeleneConfig::Existing(local_config);
        }

        let Some(parent) = cur.parent() else {
            break;
        };
        if parent == cur {
            break;
        }
        cur = parent.to_path_buf();
    }

    let dsh_home = std::env::var_os("DSH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".dsh")
        });
    let dsh_config = dsh_home.join("types").join("selene.toml");
    if dsh_config.exists() {
        return SeleneConfig::Existing(dsh_config);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_cfg = std::env::temp_dir().join(format!(
        "rojo_doctor_selene_{}_{}.toml",
        std::process::id(),
        now
    ));
    let std_val = if is_rojo { "roblox" } else { "luau" };
    let _ = fs::write(&temp_cfg, format!("std = \"{std_val}\"\n"));
    SeleneConfig::Synthesized(temp_cfg)
}

/// Selene prints its counts ("Results:" then "N errors / N warnings / N parse errors")
/// after the whole diagnostic dump.
/// Print the summary first and the dump after it so buffers/viewers see the count immediately.
pub fn put_summary_first(raw: &str) -> String {
    let text = raw.trim();
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let mut end = lines.len() as isize - 1;
    while end >= 0 && lines[end as usize].trim().is_empty() {
        end -= 1;
    }

    let mut start = end;
    while start >= 0 {
        let trimmed = lines[start as usize].trim();
        let is_count_line = {
            let mut parts = trimmed.split_whitespace();
            match (parts.next(), parts.next(), parts.next()) {
                (
                    Some(num),
                    Some("error" | "errors" | "warning" | "warnings" | "parse errors"),
                    None,
                ) => num.chars().all(|c| c.is_ascii_digit()),
                (Some(num), Some("parse"), Some("error" | "errors")) => {
                    num.chars().all(|c| c.is_ascii_digit())
                }
                _ => false,
            }
        };

        if is_count_line {
            start -= 1;
        } else {
            break;
        }
    }

    if start == end {
        return text.to_string();
    }

    if start >= 0 && lines[start as usize].trim() == "Results:" {
        start -= 1;
    }

    let summary = lines[(start + 1) as usize..=end as usize].join("\n");
    let dump = if start >= 0 {
        lines[0..=start as usize].join("\n").trim().to_string()
    } else {
        String::new()
    };

    if dump.is_empty() {
        summary
    } else {
        format!("{summary}\n{dump}")
    }
}

pub fn declared_pesde_dependencies(manifest_path: &Path) -> Option<Vec<String>> {
    let content = fs::read_to_string(manifest_path).ok()?;
    let mut names = Vec::new();
    let mut in_deps = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]"
            || trimmed == "[dev-dependencies]"
            || (trimmed.starts_with("[target.")
                && (trimmed.ends_with(".dependencies]") || trimmed.ends_with(".dev-dependencies]")))
        {
            in_deps = true;
            continue;
        } else if trimmed.starts_with('[') {
            in_deps = false;
            continue;
        }

        if in_deps {
            if let Some(eq_idx) = trimmed.find('=') {
                let key = trimmed[..eq_idx]
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'');
                if !key.is_empty() {
                    names.push(key.to_string());
                }
            }
        }
    }
    Some(names)
}

pub fn locked_pesde_dependencies(lock_path: &Path) -> Option<HashSet<String>> {
    let content = fs::read_to_string(lock_path).ok()?;
    let mut names = HashSet::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") {
            if let Some(eq_idx) = trimmed.find('=') {
                let left = trimmed[..eq_idx].trim();
                if left == "name" {
                    let val = trimmed[eq_idx + 1..]
                        .trim()
                        .trim_matches(|c| c == '"' || c == '\'');
                    if !val.is_empty() {
                        names.insert(val.to_string());
                    }
                }
            }
        }
    }
    Some(names)
}

pub fn needs_pesde_install(dir: &Path) -> bool {
    let manifest = dir.join("pesde.toml");
    if !manifest.exists() {
        return false;
    }
    let declared = match declared_pesde_dependencies(&manifest) {
        Some(deps) => deps,
        None => return true,
    };
    if declared.is_empty() {
        return false;
    }
    let lock_path = dir.join("pesde.lock");
    if !lock_path.exists() {
        return true;
    }
    let has_packages_dir = dir.join("roblox_packages").exists()
        || dir.join("roblox_server_packages").exists()
        || dir.join("luau_packages").exists()
        || dir.join("lune_packages").exists()
        || dir.join("pesde_packages").exists();
    if !has_packages_dir {
        return true;
    }

    if let Some(locked) = locked_pesde_dependencies(&lock_path) {
        if declared.iter().all(|name| locked.contains(name)) {
            if let (Ok(m_meta), Ok(l_meta)) = (manifest.metadata(), lock_path.metadata()) {
                if let (Ok(m_time), Ok(l_time)) = (m_meta.modified(), l_meta.modified()) {
                    return m_time > l_time;
                }
            }
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_summary_first_reorders_selene_output() {
        let raw = "\
src/main.luau:12:5: warning[unused_variable]: unused variable `x`
src/main.luau:15:9: error[syntax_error]: unexpected token

Results:
1 error
1 warning";

        let formatted = put_summary_first(raw);
        assert_eq!(
            formatted,
            "\
Results:
1 error
1 warning
src/main.luau:12:5: warning[unused_variable]: unused variable `x`
src/main.luau:15:9: error[syntax_error]: unexpected token"
        );
    }

    #[test]
    fn put_summary_first_preserves_plain_text() {
        let raw = "all good\nno errors here";
        assert_eq!(put_summary_first(raw), raw);
    }
}
