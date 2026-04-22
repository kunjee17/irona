# irona

A terminal UI tool for reclaiming disk space from build artifacts. Supports Rust (`target/`), Node.js (`node_modules/`), and C# (`bin/`, `obj/`) projects.

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
- **d** — delete selected entries
- **q / Esc** — quit

irona scans your home directory for build artifact folders and shows their size. Select what you want to clean up and press `d` to delete.

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
