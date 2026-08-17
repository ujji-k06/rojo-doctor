# rojo-doctor

`rojo-doctor` is a Rust CLI for finding structural and configuration problems in [Rojo](https://rojo.space/) projects and explaining how to fix them.

The project is being built incrementally. The current v0.1 slice discovers Rojo project files, loads JSON/JSONC project trees, recursively resolves `$path` mappings, and reports required mapped paths that do not exist.

## Install from source

```sh
git clone https://github.com/ujji-k06/rojo-doctor.git
cd rojo-doctor
cargo install --path .
```

## Usage

From a directory containing a default Rojo project:

```text
rojo-doctor check
```

Or pass a project file/directory explicitly:

```text
rojo-doctor check path/to/default.project.json
rojo-doctor check path/to/project-directory
```

Example diagnostic:

```text
Checking default.project.json...

✓ 1 mapped path resolved

warning[missing-path]
  src/server/Inventory
  referenced by ServerScriptService.Inventory but does not exist, so Rojo cannot resolve this mapping

  help: create the mapped path or update the `$path` entry

1 warning, 0 errors
```

Missing optional `$path` mappings are allowed and do not produce a diagnostic.

## v0.1 goals

- discover or accept a Rojo project file;
- parse the project tree safely;
- recursively validate `$path` mappings;
- produce actionable diagnostics for missing mapped paths;
- exercise valid and invalid projects with fixtures;
- use predictable exit codes suitable for scripts and CI.

## Current exit codes

- `0`: checks completed with no diagnostics;
- `1`: checks completed and produced one or more diagnostics, including warnings;
- `2`: the project/check could not run because of invalid CLI usage, project loading, parsing, or filesystem inspection errors.

The warning-to-exit-code policy is intentionally simple for v0.1. A future configuration option can distinguish informational warnings from CI-failing diagnostics when more rules exist.

## Current limitations

- only missing required `$path` mappings are checked;
- the project model intentionally covers only the Rojo fields needed by current checks;
- no unreachable Luau detection or `require()` dependency analysis yet;
- no graph, asset-ID checks, configurable module-size warnings, or JSON diagnostics yet;
- no CI or release packaging yet;
- license and minimum supported Rust version are not chosen yet.

The loader intentionally ignores unrelated reserved Rojo node metadata instead of misclassifying fields such as `$properties` or `$attributes` as child instances. Filesystem paths are resolved relative to the directory containing the project file; absolute mappings remain absolute.
