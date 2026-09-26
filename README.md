# rojo-doctor

Catch broken [Rojo](https://rojo.space/) projects before Studio does.

```text
$ rojo-doctor check --all

Checking default.project.json...

✓ 4 mapped paths resolved

warning[missing-path]
  pesde_packages
  referenced by ReplicatedStorage.Packages but does not exist, so Rojo cannot resolve this mapping

  help: run `pesde install` to create this path

1 warning, 0 errors
```

Available both as a standalone **Rust CLI binary** and a native **pesde / Lune package**.

## Install

### via pesde / Lune (Recommended for Roblox Devs)

```sh
pesde add -D ujji/rojo_doctor
```

Or run directly with Lune:
```sh
lune run bin/rojo-doctor.luau check
```

### via Cargo

```sh
cargo install rojo-doctor --git https://github.com/ujji-k06/rojo-doctor --locked
```

## Usage

```text
rojo-doctor check
rojo-doctor check path/to/default.project.json
rojo-doctor check --all
rojo-doctor check --format json
rojo-doctor check --fix
```

- `--all` checks every `.project.json` / `.project.jsonc` in the directory, including `tests.project.json`.
- `--format json` outputs structured JSON for CI and IDE tooling.
- `--fix` automatically resolves safe issues (e.g. cleans up orphaned `*.meta.json` files).

| Exit | Meaning |
|------|---------|
| `0` | clean |
| `1` | one or more diagnostics |
| `2` | could not load the project |

## GitHub Actions

```yaml
- uses: ujji-k06/rojo-doctor@v0.4.0
```

Runs `rojo-doctor check --all` on the repo root. Run `pesde install` or `wally install` first if the project maps packages.

## What it checks

| Code | Severity | When |
|------|----------|------|
| `missing-path` | warning | required `$path` does not exist on disk |
| `file-with-children` | error | `$path` is a file, but the node also lists children |
| `missing-name` | warning | project has no top-level `name` |
| `missing-class` | error | instance has neither `$className` nor `$path` (services are inferred) |
| `class-on-non-folder` | error | `$className` and `$path` are both set, but `$path` is not a Folder |
| `child-collision` | error | project child name already exists inside the mapped directory |
| `ambiguous-init` | warning | directory has more than one `init` script |
| `orphan-meta` | warning | `*.meta.json` has no matching sibling instance |
| `script-context` | warning | client script placed in server container or server script in client container |

- **Package Manager Hints**: If `pesde_packages` or `Packages` is missing and `pesde.toml` is present, the help text suggests `pesde install`. If `wally.toml` is present, it suggests `wally install`.
- **Optional Paths**: Optional `$path` mappings (`{ "optional": "..." }`) are allowed to be missing.
- **JSONC Support**: Comments and trailing commas in `.project.jsonc` are supported. Reserved `$` fields such as `$properties` are ignored instead of being treated as instances.

## Testing

```sh
# Rust test suite
cargo test

# Lune / pesde test suite
pesde run test
# or: lune run tests/lune_test.luau
```

## Why this exists

Rojo fails at serve/build time with a stack of path errors. `rojo-doctor` is the same structural pass, readable in a terminal, and scriptable via `--format json`.