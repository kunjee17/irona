# irona — Design Spec

**Date:** 2026-04-20  
**Status:** Approved

---

## Overview

`irona` is a Ratatui-based terminal UI tool for reclaiming disk space from build artifacts and dependency folders. The user points it at a root directory; it scans recursively, presents a live checklist of found artifact folders with sizes, and permanently deletes the selected ones.

Inspired by `cargo-wipe` and `kondo`, built as a learning project to explore Ratatui, Rayon, Crossbeam, and Tokio together.

---

## Usage

```bash
irona [path]   # defaults to current directory (.)
```

Example:
```bash
irona /home/kunjee/Workspace
irona .
```

---

## Supported Languages (v1)

| Language | Marker file       | Artifact folders removed |
|----------|-------------------|--------------------------|
| Rust     | `Cargo.toml`      | `target/`                |
| Node.js  | `package.json`    | `node_modules/`          |
| C#       | `*.csproj`, `*.sln` | `bin/`, `obj/`         |

Detection uses the marker-file approach: only remove an artifact folder if its parent contains the expected marker. This avoids false positives on folders that happen to be named `bin/` or `target/`.

---

## Architecture

Four modules with clear boundaries:

```
main.rs      — CLI arg parsing, spawn scanner thread, start TUI event loop
scanner.rs   — walkdir traversal, language detection, parallel size calc
tui.rs       — app state, Ratatui render, keyboard event handling
deleter.rs   — async concurrent fs::remove_dir_all per selected path
```

### Data Flow

```
main
 ├─ parses root path (CLI arg or ".")
 ├─ spawns scan thread
 │    ├─ walkdir traversal from root
 │    ├─ detects artifact folders via marker files
 │    ├─ rayon::par_iter calculates size per found folder
 │    └─ sends ArtifactEntry { path, language, size_bytes } over crossbeam channel
 └─ starts Ratatui event loop
      ├─ polls crossbeam channel → appends entries to state list (live update)
      ├─ handles keyboard input → navigate, toggle select, select all, delete, quit
      └─ on delete → tokio runtime → spawns concurrent remove_dir_all per selected path
```

### Key Types

```rust
struct ArtifactEntry {
    path: PathBuf,
    language: Language,
    size_bytes: u64,
}

enum Language { Rust, NodeJs, CSharp }

enum ScanMessage {
    Found(ArtifactEntry),
    Done,
}

struct AppState {
    entries: Vec<ArtifactEntry>,
    selected: HashSet<usize>,   // indices into entries
    cursor: usize,
    status: AppStatus,
}

enum AppStatus { Scanning, Ready, Deleting, Done }
```

---

## TUI Layout

```
┌─ irona ──────────────────────── scanning /home/kunjee/Workspace... ─┐
│                                                                       │
│  [✓]  node_modules   ~/Workspace/web-app               847 MB        │
│  [ ]  target         ~/Workspace/api                   1.2 GB        │
│  [✓]  bin            ~/Workspace/desktop/MyApp          23 MB        │
│  [ ]  obj            ~/Workspace/desktop/MyApp           4 MB        │
│  ...                                                                  │
│                                                                       │
│  Total selected: 870 MB                                               │
├───────────────────────────────────────────────────────────────────────┤
│  ↑↓ navigate   Space select   a select all   d delete   q quit        │
└───────────────────────────────────────────────────────────────────────┘
```

- Header shows app name + scan status (scanning path / done / deleting)
- List is scrollable; selected items show `[✓]`
- Footer shows total size of selected items
- Status bar at bottom shows keybindings

---

## Keyboard Controls

| Key       | Action                          |
|-----------|---------------------------------|
| `↑` / `↓` | Navigate list                   |
| `Space`   | Toggle selection on cursor item |
| `a`       | Select / deselect all           |
| `d`       | Delete all selected (with confirmation prompt) |
| `q`       | Quit                            |

Deletion shows a confirmation: `Delete 3 folders (870 MB)? [y/N]`

---

## Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
walkdir = "2"
crossbeam-channel = "0.5"
rayon = "1"
tokio = { version = "1", features = ["rt-multi-thread", "fs", "macros"] }
fs_extra = "1"
clap = { version = "4", features = ["derive"] }
```

---

## Project Structure

```
irona/
├── src/
│   ├── main.rs
│   ├── scanner.rs
│   ├── tui.rs
│   └── deleter.rs
├── docs/
│   ├── index.html          ← GitHub Pages landing page
│   └── superpowers/specs/
├── .github/
│   └── workflows/
│       ├── ci.yml          ← build + test on PRs
│       └── release.yml     ← cross-platform binaries on tag push
├── Cargo.toml
└── README.md
```

---

## GitHub Pages Landing Page

A single-page HTML site served from `docs/index.html` (GitHub Pages `docs/` mode). Designed with `frontend-design` skill during implementation.

Content:
- Hero: tool name, tagline, animated terminal demo (GIF or CSS animation)
- Feature list: languages supported, TUI screenshot
- Install instructions (cargo install, pre-built binaries)
- Link to GitHub releases

---

## GitHub Actions

### `ci.yml` — triggered on push/PR to main
- `cargo fmt --check`
- `cargo clippy`
- `cargo test`

### `release.yml` — triggered on `v*` tag push
Builds release binaries for:

| Target                        | OS      |
|-------------------------------|---------|
| `x86_64-unknown-linux-gnu`    | Linux   |
| `aarch64-apple-darwin`        | macOS M |
| `x86_64-apple-darwin`         | macOS Intel |
| `x86_64-pc-windows-msvc`      | Windows |

Uses `cross` for Linux cross-compilation, uploads artifacts to GitHub Release.

---

## Out of Scope (v1)

- Trash / recoverable deletion
- Python, Go, Swift, Flutter support (add later)
- Config file for custom artifact patterns
- Global scan (always requires explicit root path)
