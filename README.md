# irona

A terminal UI tool for reclaiming disk space from build artifacts. Scans your project directories and lets you select and delete artifact folders for a wide range of languages and package managers. Also does a `.gitignore`-aware pass to catch any project-specific artifact directories not covered by the built-in language rules.

## Install

### Pre-built binaries

Download the latest release for your platform from the [Releases page](https://github.com/kunjee17/irona/releases):

| Platform | Archive |
|----------|---------|
| Linux x64 | `irona-linux-x86_64.tar.gz` |
| Linux x64 (static/musl) | `irona-linux-x86_64-musl.tar.gz` |
| Linux ARM64 | `irona-linux-aarch64.tar.gz` |
| macOS Apple Silicon | `irona-macos-arm64.tar.gz` |
| macOS Intel | `irona-macos-x86_64.tar.gz` |
| Windows x64 | `irona-windows-x86_64.zip` |

Extract and place the binary somewhere on your `PATH`.

### From crates.io

```bash
cargo install irona-cli
```

### From source

```bash
git clone https://github.com/kunjee17/irona
cd irona
cargo install --path .
```

## Usage

- **↑ / ↓** — navigate entries
- **Space** — select / deselect entry
- **a** — select / deselect all
- **d** — delete selected entries
- **q / Esc** — quit

irona scans your home directory for build artifact folders and shows their size. Select what you want to clean up and press `d` to delete.

## Supported Languages

| Language / Ecosystem | Marker file(s) | Artifact folder(s) |
|---|---|---|
| Rust | `Cargo.toml` | `target/` |
| Node.js | `package.json` | `node_modules/` |
| C# | `*.csproj`, `*.sln` | `bin/`, `obj/` |
| .NET NuGet (packages.config) | `packages.config` | `packages/` |
| .NET Paket | `paket.dependencies` | `packages/`, `.paket/` |
| Python | `requirements.txt`, `pyproject.toml`, `setup.py` | `.venv/`, `venv/` |
| Java (Maven) | `pom.xml` | `target/` |
| Java / Kotlin / Android (Gradle) | `build.gradle`, `build.gradle.kts`, `settings.gradle*` | `build/`, `.gradle/` |
| Go | `go.mod` | `vendor/` |
| PHP (Composer) | `composer.json` | `vendor/` |
| Ruby (Bundler) | `Gemfile` | `vendor/`, `.bundle/` |
| Swift (SPM) | `Package.swift` | `.build/` |
| Haskell (Stack) | `stack.yaml` | `.stack-work/` |
| Elm | `elm.json` | `elm-stuff/` |
| Dart / Flutter | `pubspec.yaml` | `.dart_tool/`, `build/` |

## Gitignore-aware scan

In addition to the language rules above, irona walks every `.gitignore` file it encounters and surfaces any matching directories that exist on disk. This covers build outputs not hardcoded in irona — things like `dist/`, `out/`, `.cache/`, `coverage/`, `.next/`, or any project-specific pattern your `.gitignore` already documents.

Each entry in the list is labelled with its source (`Rust`, `Node.js`, `gitignore`, etc.) so you can see at a glance where it was found.

Directories that should never be deleted are excluded regardless of what `.gitignore` says: `.git`, `.vscode`, `.idea`, `.github`.

If a directory is found by both a language rule and a `.gitignore` pattern, it appears once and is attributed to the language rule.

## Releasing

Requires [`cargo-release`](https://github.com/crate-ci/cargo-release) and [`git-cliff`](https://github.com/orhun/git-cliff) installed locally:

```bash
cargo install cargo-release git-cliff
```

Cut a release from `main`:

```bash
cargo release patch   # 0.1.0 → 0.1.1  (bug fixes)
cargo release minor   # 0.1.0 → 0.2.0  (new features)
cargo release major   # 0.1.0 → 1.0.0  (breaking changes)
```

This will: bump the version in `Cargo.toml`, regenerate `CHANGELOG.md`, commit, tag `vX.Y.Z`, and push. GitHub Actions then builds all platform binaries, creates the GitHub Release, and publishes to crates.io.

### One-time setup for crates.io publishing

1. Create an API token at [crates.io/settings/tokens](https://crates.io/settings/tokens)
2. Add it as a repository secret named `CARGO_REGISTRY_TOKEN` in GitHub → Settings → Secrets and variables → Actions
