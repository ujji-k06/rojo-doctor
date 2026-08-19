# rojo-doctor

Catch broken [Rojo](https://rojo.space/) projects before Studio does.

```text
$ rojo-doctor check

Checking default.project.json...

✓ 3 mapped paths resolved

warning[missing-path]
  src/server/Inventory
  referenced by ServerScriptService.Inventory but does not exist, so Rojo cannot resolve this mapping

  help: create the mapped path or update the `$path` entry

error[file-with-children]
  src/health.lua
  ReplicatedStorage.Health maps to a file but also lists child instances, which Rojo cannot merge

  help: remove the child instances or point `$path` at a directory

1 warning, 1 error
```

One binary. rustc-style diagnostics. Exit codes you can put in CI.

## Install

```sh
git clone https://github.com/ujji-k06/rojo-doctor.git
cd rojo-doctor
cargo install --path .
```

## Usage

```text
rojo-doctor check
rojo-doctor check path/to/default.project.json
rojo-doctor check --format json
```

| Exit | Meaning |
|------|---------|
| `0` | clean |
| `1` | one or more diagnostics |
| `2` | could not load the project |

## What it checks

| Code | Severity | When |
|------|----------|------|
| `missing-path` | warning | required `$path` does not exist on disk |
| `file-with-children` | error | `$path` is a file, but the node also lists children |
| `missing-name` | warning | project has no top-level `name` |

Optional `$path` mappings (`{ "optional": "..." }`) are allowed to be missing.

JSONC project files work. Reserved `$` fields such as `$properties` are ignored instead of being treated as instances.

## Why this exists

Rojo fails at serve/build time with a stack of path errors. `rojo-doctor` is the same structural pass, readable in a terminal, and scriptable via `--format json`.
