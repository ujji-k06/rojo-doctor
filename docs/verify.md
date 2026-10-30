# rojo-doctor verify

`rojo-doctor verify` runs package installation, structural checks, formatting, linting, typechecking, and tests for Roblox and Luau projects.

```text
rojo-doctor verify [PATHS]... [OPTIONS]
```

Passing no `PATHS` checks the entire project from the current working directory.

Stage 0 runs first. Stages 1-5 run concurrently on separate threads, and the summary prints in stage order once execution finishes.

> Limits: `verify` is Rust-only (not in the Lune/pesde package). It is not read-only: it can install packages and regenerate `sourcemap.json`. The generated `sourcemap.json` lands in the project root and belongs in the project's `.gitignore`. `--fix` modifies source files in place.

## Stages

| Stage | Name | Tool / Mechanism | Behavior |
|-------|------|------------------|----------|
| 0 | `install` | `pesde install` / `wally install` | Skipped if lockfile and packages satisfy manifest, or with `--no-install`. |
| 1 | `structure` | `rojo-doctor` (in-process) | Runs `checks::run_all` (`checks::fix_all` under `--fix`). |
| 2 | `style` | `stylua --check` | Formats files in-place under `--fix`. |
| 3 | `lint` | `selene` | Prints summary before diagnostic dump. |
| 4 | `types` | `luau-lsp analyze` | Regenerates stale `sourcemap.json`. Defaults to Solver V2. |
| 5 | `tests` | `lune` | Headless test suites (up to 8 suites concurrently). |

## Flags

- `--fix`: apply `stylua` formatting and safe structural fixes.
- `--no-tests`: skip the `tests` stage.
- `--solver=v1|v2`: Luau type solver version (default `v2`; also accepts `--solver v1`).
- `--test=<pattern>`: filter test suites by path substring relative to `tests/`.
- `--diff[=<REV>]`: verify only Luau files changed vs `<REV>` (defaults to `HEAD`).
- `--format text|json`: stdout format (`text` by default).
- `--definitions=<path>`: explicit engine definitions file for `luau-lsp`.
- `--no-install`: skip dependency installation (stage 0).
- `--no-project`: accept a plain Luau tree without requiring a `*.project.json`.
- `--verbose`: display full diagnostic output and notes.

## Test Discovery

1. If `tests/run.luau` exists, it runs as the single test suite.
2. Otherwise, `tests/` is walked recursively up to depth 3 for `*.test.luau`, `*.spec.luau`, `test_*.luau`, `test_*.lua`, and `*_test.luau`. Discovered suites run up to 8 at a time.
3. `--test=<pattern>` filters suites matching `<pattern>` relative to `tests/`.
4. If no test suites are found, stage 5 is marked as skipped, not passed.

## Configuration and Definitions Resolution

### Selene Config

1. Project `selene.toml` in the project root.
2. Nearest ancestor `selene.toml`.
3. Synthesized temporary config: `std = "roblox"` if a `*.project.json` is present, `std = "luau"` otherwise.

The stage records which configuration was used.

### Engine Definitions

1. `--definitions <PATH>` CLI override.
2. `$ROJO_DOCTOR_DEFINITIONS` environment variable.
3. `$DSH_HOME/types/globalTypes.d.luau`.
4. `./globalTypes.d.luau`.
5. `./types/globalTypes.d.luau`.

If an explicit path (`--definitions` or `$ROJO_DOCTOR_DEFINITIONS`) does not exist, verification aborts with exit code 2. If no definitions are found anywhere in the search order, verification proceeds and records a stage note.

## Exit Codes

| Code | Status | Meaning |
|------|--------|---------|
| `0` | Clean | All stages passed or skipped by design, or `--diff` found no changed Luau files. |
| `1` | Failed / Degraded | Any stage failed, a required external tool is missing, a directory lacks `*.project.json` without `--no-project`, or no Luau files were found. |
| `2` | Fatal | Unparseable `*.project.json`, invalid or mutually exclusive flags (e.g. `--diff` with explicit paths), git failure during `--diff`, or nonexistent explicit definitions path. |

## Diff Recipes

`--diff` resolves changed `.luau` and `.lua` files against the git repository root. If no changed files are found, it prints `nothing to verify` and exits 0. A git failure exits 2.

```sh
# Verify uncommitted and committed changes against HEAD
rojo-doctor verify --diff

# Verify changes against another revision or branch
rojo-doctor verify --diff=origin/main

# Explicit pipe via git diff
git diff --name-only HEAD | grep -E '\.(luau|lua)$' | xargs -r rojo-doctor verify
```

## JSON Output

With `--format json`, `verify` writes a single JSON object to stdout:

```json
{
  "project": "default.project.json",
  "ok": true,
  "verdict": "passed",
  "exit": 0,
  "stages": [
    {
      "name": "structure",
      "status": "passed",
      "duration_ms": 12,
      "notes": [],
      "diagnostics": []
    }
  ]
}
```

The `exit` field mirrors the process exit code (`0`, `1`, or `2`). The `verdict` field is `"passed"`, `"failed"`, or `"degraded"`.
