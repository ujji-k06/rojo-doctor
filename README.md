# rojo-doctor

Catch broken [Rojo](https://rojo.space/) projects before Studio does.

```text
$ rojo-doctor check --all

Checking default.project.json...

✓ 4 mapped paths resolved

warning[missing-path]
  Packages
  referenced by ReplicatedStorage.Packages but does not exist, so Rojo cannot resolve this mapping

  help: run `wally install` to create this path

1 warning, 0 errors
```

One binary. rustc-style diagnostics. Exit codes you can put in CI.

## Install

```sh
cargo install rojo-doctor --git https://github.com/ujji-k06/rojo-doctor --locked
```

## Usage

```text
rojo-doctor check
rojo-doctor check path/to/default.project.json
rojo-doctor check --all
rojo-doctor check --format json
```

`--all` checks every `.project.json` / `.project.jsonc` in the directory, including `tests.project.json`.

| Exit | Meaning |
|------|---------|
| `0` | clean |
| `1` | one or more diagnostics |
| `2` | could not load the project |

## GitHub Actions

```yaml
- uses: ujji-k06/rojo-doctor@v0.3.0
```

Runs `rojo-doctor check --all` on the repo root. Run `wally install` first if the project maps `Packages` or `DevPackages`.

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

Optional `$path` mappings (`{ "optional": "..." }`) are allowed to be missing. If `Packages` or `DevPackages` is missing and `wally.toml` is present, the help text is `wally install`.

JSONC project files work. Reserved `$` fields such as `$properties` are ignored instead of being treated as instances.

## Test

```sh
cargo test
```

## Why this exists

Rojo fails at serve/build time with a stack of path errors. `rojo-doctor` is the same structural pass, readable in a terminal, and scriptable via `--format json`.
