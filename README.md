# irona

A terminal UI tool for reclaiming disk space from build artifacts. Scans your project directories and lets you select and delete artifact folders for a wide range of languages and package managers. Also does a `.gitignore`-aware pass to catch any project-specific artifact directories not covered by the built-in language rules.

![irona demo](demo.gif)

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

irona has three modes: an interactive TUI, a headless `--clean` sweep, and an MCP server.

```sh
irona [PATH]                    # interactive TUI (PATH defaults to the current directory)
irona --clean [PATH]            # headless sweep, one summary line, no TUI
irona --mcp                     # Model Context Protocol server over stdio
irona --version                 # print version
irona --help                    # print all flags
```

| Flag | Mode | Description |
|---|---|---|
| `--clean` | headless | Scan and delete every artifact found, then print a summary |
| `--dry-run` | headless | With `--clean`: report what would be freed, delete nothing |
| `--include-gitignored` | headless | With `--clean`: also delete directories matched only by `.gitignore` |
| `--mcp` | server | Serve `scan_artifacts` and `clean_artifacts` over stdio |

### Interactive TUI

- **↑ / ↓** — navigate entries
- **Space** — select / deselect entry
- **a** — select / deselect all
- **d** — delete selected entries
- **q / Esc** — quit

irona scans `PATH` for build artifact folders and shows their size. Select what you want to clean up and press `d` to delete.

The TUI needs a real terminal. If stdin or stdout is not a TTY — in a pipe, a CI job, or an editor task runner — irona exits with code `1` and points you at `--clean` and `--mcp` instead of failing with a raw OS error.

## Headless mode

`--clean` skips the TUI entirely: it scans, deletes everything it finds, and prints one summary line. Built for AI tool session-end hooks and any other non-interactive context.

```sh
irona --clean ~/Workspace                   # scan, delete, report
irona --clean --dry-run ~/Workspace         # report only, delete nothing
irona --clean --include-gitignored ~/W      # also sweep gitignore-only matches
```

```
irona: freed 2.3 GB from 4 directories in 1.2s
irona: would free 2.3 GB from 4 directories (dry run)
irona: nothing to clean in /home/you/Workspace
```

By default headless mode deletes only directories backed by a marker file — `target/` next to a `Cargo.toml`, `node_modules/` next to a `package.json`, and the rest of the table below. Directories found only through `.gitignore` are left alone, because unattended they could include a gitignored `data/`, `logs/`, or `secrets/`. Pass `--include-gitignored` to sweep those too. The TUI is unaffected and still shows both kinds.

Exit code is `0` on success, including when nothing was found, so a clean workspace never fails your hook. It is `1` only if a deletion failed; the failing paths are reported on stderr.

### Claude Code hook

Sweep build artifacts every time a session ends, via `settings.json`:

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "irona --clean ~/Workspace" }
        ]
      }
    ]
  }
}
```

Run it once with `--dry-run` first to see what it would take.

## MCP server

irona can also run as a stdio [Model Context Protocol](https://modelcontextprotocol.io) server for AI tools:

Install irona and make sure Cargo's bin directory is on your `PATH`:

```bash
cargo install irona-cli
export PATH="$HOME/.cargo/bin:$PATH"
```

Using `irona` from `PATH` keeps MCP client configs portable and avoids hardcoding a local binary path such as `/home/alice/.cargo/bin/irona`.

```json
{
  "mcpServers": {
    "irona": {
      "command": "irona",
      "args": ["--mcp"]
    }
  }
}
```

In Claude Code the equivalent one-liner is:

```bash
claude mcp add irona -- irona --mcp
```

The MCP server exposes two tools:

| Tool | Input | Returns |
|---|---|---|
| `scan_artifacts` | `path` (optional, defaults to the working directory) | `root`, `count`, `total_size_bytes`, and an `artifacts` array of `{path, language, size_bytes}` |
| `clean_artifacts` | `paths` (array, at least one) | `requested_count`, `deleted_count`, `total_freed_bytes`, and a `results` array of `{path, deleted, size_bytes, elapsed_ms, error?}` |

Both tools return structured content alongside a text summary, and both carry MCP annotations — `scan_artifacts` is marked `readOnlyHint`, `clean_artifacts` `destructiveHint` — so clients that treat destructive tools differently can tell them apart.

`scan_artifacts` reports everything the TUI would show, including `.gitignore`-only matches, each labelled by source in the `language` field. This is the opposite default from `irona --clean`, which skips gitignore-only matches unless you pass `--include-gitignored`; the MCP flow shows them and leaves the decision to you.

`clean_artifacts` refuses any path irona would not itself classify as an artifact. A directory only qualifies if a marker file sits beside it (`target/` next to a `Cargo.toml`) or a `.gitignore` rule matches it. Handing it an arbitrary path fails the whole call and deletes nothing, so a wrong path from the model cannot take out your work.

The intended flow is two calls: `scan_artifacts` to see what exists and how much it costs, then `clean_artifacts` with the subset you approve.

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
