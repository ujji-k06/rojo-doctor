# rojo-doctor init

`rojo-doctor init` provisions a Roblox or Luau project with engine type definitions, a pre-commit hook, and a GitHub Actions verification workflow in one command.

```text
rojo-doctor init [PATH] [OPTIONS]
```

Passing no `PATH` initializes the current working directory. The command is idempotent: re-running against an initialized project reports `already present` and preserves existing files.

## Artifacts

| Artifact | Purpose | Skip Flag |
|----------|---------|-----------|
| `types/globalTypes.d.luau` | Roblox global engine definitions for `luau-lsp analyze`, downloaded from `JohnnyMorganz/luau-lsp` tag `1.70.1`. Automatically picked up by `rojo-doctor verify`. | `--no-types` |
| `.github/workflows/verify.yml` | Self-contained CI workflow triggering on pushes to `main` and on pull requests. Sets up full fetch depth, configures `rokit` via `CompeyDev/setup-rokit@v0.2.1`, installs `rojo-doctor`, runs `rokit install` when `rokit.toml` exists, and gates the build. | `--no-ci` |
| `.githooks/pre-commit` | Executable hook running `rojo-doctor verify` from repository root. Uses `--diff` when `HEAD` exists (or full verification on unborn branches), exits non-zero on failure, and prints a `--no-verify` bypass hint. Requires `rojo-doctor` on `PATH`. | `--no-hook` |
| `.gitignore` | Appends `sourcemap.json` so generated Rojo sourcemaps remain untracked. | — |
| Git hooks path | Runs `git config core.hooksPath .githooks` when the target is a git repository. | `--no-hook` |

## Flags

- `--force`: Overwrite existing files (reports `updated`).
- `--no-ci`: Skip GitHub Actions CI workflow generation.
- `--no-hook`: Skip pre-commit hook generation and git hooks configuration.
- `--no-types`: Skip Luau engine type definitions.
- `--definitions-from <PATH>`: Copy engine definitions from a local file instead of downloading.

## Enforcement Story

1. **Local Pre-Commit Gate**: Developers catch structural errors, formatting issues, lint violations, and type errors before commits enter history. The hook requires `rojo-doctor` on `PATH`. It checks `git rev-parse --verify HEAD` so that initial commits on unborn branches run full verification safely, while subsequent commits use `--diff` to check changed `.luau`/`.lua` files quickly.
2. **CI Enforcement Gate**: Pull requests run `rojo-doctor verify --no-install --diff origin/${{ github.base_ref }}` against the target branch, while pushes to `main` run full verification (`rojo-doctor verify --no-install`).
3. **Toolchain Degradation**: When toolchain binaries (`stylua`, `selene`, `luau-lsp`, `lune`) are missing, the verification pipeline degrades to exit 1 rather than passing silently (`src/verify/pipeline.rs:1126-1142`). The generated workflow installs `rokit` via `CompeyDev/setup-rokit@v0.2.1` and runs `rokit install` if `rokit.toml` exists.
4. **Automatic Project Detection**: If no `*.project.json` or `*.project.jsonc` file is present, `init` prints a warning and automatically appends `--no-project` to the generated hook and CI commands, supporting standalone Luau libraries out-of-the-box.

## Updating Engine Definitions

The downloaded `types/globalTypes.d.luau` is pinned to `JohnnyMorganz/luau-lsp` release `1.70.1`.

- **Re-download or bump definitions**: Run `rojo-doctor init --force` to re-download the pinned definitions, or download a newer release from GitHub directly into `types/globalTypes.d.luau`.
- **Custom / unpinned definitions**: Supply `--definitions-from <PATH>` during `init` to install custom definitions, use `--definitions <PATH>` at verification time, or export `$ROJO_DOCTOR_DEFINITIONS`.
