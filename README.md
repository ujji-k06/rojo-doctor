# rojo-doctor

`rojo-doctor` is a Rust CLI for finding structural and configuration problems in [Rojo](https://rojo.space/) projects and explaining how to fix them.

The project is being built incrementally. The current implementation is the first v0.1 slice: it can locate and load a Rojo project, including `default.project.json` and `default.project.jsonc`, but it does **not** run structural checks yet.

## Install from source

```sh
git clone <your-repository-url>
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

Current output:

```text
Checking default.project.json...

✓ project loaded
```

## v0.1 goals

- discover or accept a Rojo project file;
- parse the project tree safely;
- recursively validate `$path` mappings;
- produce actionable diagnostics for missing mapped paths;
- exercise valid and invalid projects with fixtures;
- use predictable exit codes suitable for scripts and CI.

## Current exit codes

- `0`: the command completed successfully;
- `2`: the project could not be discovered, read, or parsed (Clap also uses `2` for invalid CLI usage).

A future check-result exit code will be introduced when the first diagnostic rule lands.

## Current limitations

- no `$path` existence validation yet;
- no diagnostic/rule abstraction yet;
- this is not a complete validator for every Rojo project field;
- no graph, Luau `require()` analysis, assets checks, JSON diagnostics, CI, or release packaging yet;
- license and minimum supported Rust version are not chosen yet.

The loader intentionally understands only the project fields needed for analysis and ignores unrelated reserved Rojo node metadata. That keeps the model small without misclassifying `$properties` or `$attributes` as child instances.
