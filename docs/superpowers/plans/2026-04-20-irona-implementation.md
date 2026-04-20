# irona Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build irona — a Ratatui TUI that scans a user-provided directory for Rust/Node/C# build artifacts, shows a live interactive checklist with sizes, and permanently deletes selected ones.

**Architecture:** Scanner thread uses walkdir to find artifact folders via marker files, then rayon calculates sizes in parallel, streaming results over a crossbeam channel into the Ratatui event loop. State and rendering are split across `app.rs` and `ui.rs`. Deletion is async via tokio. Note: spec lists `tui.rs` — this plan splits it into `app.rs` + `ui.rs` for testability.

**Tech Stack:** Rust, Ratatui 0.29, Crossterm 0.28, Walkdir, Crossbeam-channel, Rayon, Tokio, Clap, Anyhow

---

## Branching Strategy

One branch → one commit → `gh pr create`. Stack branches when follow-on work depends on an unmerged PR.

| Phase | Branch | Depends on |
|-------|--------|------------|
| 1 | `feat/scaffold` | main |
| 2 | `feat/scanner` | feat/scaffold |
| 3 | `feat/app-state` | feat/scanner |
| 4 | `feat/wire` | feat/app-state |
| 5 | `feat/github-actions` | feat/wire |
| 6 | `feat/landing-page` | main (independent) |

---

## File Map

| File | Responsibility |
|------|---------------|
| `src/main.rs` | CLI args (clap), terminal setup/teardown, spawn scanner thread, run event loop |
| `src/scanner.rs` | Walkdir traversal, marker-file detection, rayon size calc, crossbeam send |
| `src/app.rs` | `AppState`, `AppStatus`, all state mutations (navigate, select, delete trigger) |
| `src/ui.rs` | Ratatui render functions only — no state mutation |
| `src/deleter.rs` | Async `tokio::fs::remove_dir_all` per path, returns results |
| `.github/workflows/ci.yml` | fmt + clippy + test on push/PR |
| `.github/workflows/release.yml` | Cross-platform binaries on `v*` tag |
| `docs/index.html` | GitHub Pages landing page |

---

## Phase 1 — Project Scaffold (branch: `feat/scaffold`)

### Task 1: Cargo project + dependencies

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/scanner.rs`
- Create: `src/app.rs`
- Create: `src/ui.rs`
- Create: `src/deleter.rs`

- [ ] **Step 1: Init cargo project**

```bash
cargo init --name irona
```

- [ ] **Step 2: Replace Cargo.toml**

```toml
[package]
name = "irona"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "irona"
path = "src/main.rs"

[dependencies]
ratatui = "0.29"
crossterm = "0.28"
walkdir = "2"
crossbeam-channel = "0.5"
rayon = "1"
tokio = { version = "1", features = ["rt-multi-thread", "fs", "macros"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write src/main.rs stub**

```rust
mod app;
mod deleter;
mod scanner;
mod ui;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "irona", about = "Reclaim disk space from build artifacts")]
struct Args {
    /// Root directory to scan (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();
    println!("Scanning: {}", args.path.display());
}
```

- [ ] **Step 4: Create empty stub files**

`src/scanner.rs`:
```rust
```

`src/app.rs`:
```rust
```

`src/ui.rs`:
```rust
```

`src/deleter.rs`:
```rust
```

- [ ] **Step 5: Verify it builds**

```bash
cargo build
```
Expected: compiles with no errors or warnings.

- [ ] **Step 6: Commit and PR**

```bash
git checkout -b feat/scaffold
git add Cargo.toml src/
git commit -m "feat: project scaffold with all dependencies"
gh pr create --title "feat: project scaffold" --body "Cargo.toml with all deps, stub modules, basic clap CLI."
```

---

## Phase 2 — Scanner Module (branch: `feat/scanner`)

```bash
git checkout feat/scaffold   # or main if merged
git checkout -b feat/scanner
```

### Task 2: Types and marker-file detection

**Files:**
- Modify: `src/scanner.rs`

- [ ] **Step 1: Define types in src/scanner.rs**

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    NodeJs,
    CSharp,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::NodeJs => write!(f, "Node.js"),
            Language::CSharp => write!(f, "C#"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactEntry {
    pub path: PathBuf,
    pub language: Language,
    pub size_bytes: u64,
}

#[derive(Debug)]
pub enum ScanMessage {
    Found(ArtifactEntry),
    Done,
}
```

- [ ] **Step 2: Write detect_artifacts function**

```rust
use std::fs;

/// Checks `dir` for marker files and returns artifact subdirectories found.
/// Only returns a folder if its parent contains the expected marker — avoids
/// false positives on unrelated folders named "bin" or "target".
pub fn detect_artifacts(dir: &std::path::Path) -> Vec<(PathBuf, Language)> {
    let mut found = Vec::new();

    let names: Vec<String> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => return found,
    };

    // Rust: Cargo.toml -> target/
    if names.iter().any(|n| n == "Cargo.toml") {
        let p = dir.join("target");
        if p.is_dir() {
            found.push((p, Language::Rust));
        }
    }

    // Node.js: package.json -> node_modules/
    if names.iter().any(|n| n == "package.json") {
        let p = dir.join("node_modules");
        if p.is_dir() {
            found.push((p, Language::NodeJs));
        }
    }

    // C#: *.csproj or *.sln -> bin/ and obj/
    if names.iter().any(|n| n.ends_with(".csproj") || n.ends_with(".sln")) {
        for folder in &["bin", "obj"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::CSharp));
            }
        }
    }

    found
}
```

- [ ] **Step 3: Write tests for detect_artifacts**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_rust_target() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();

        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Rust);
        assert!(results[0].0.ends_with("target"));
    }

    #[test]
    fn detects_node_modules() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();

        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::NodeJs);
        assert!(results[0].0.ends_with("node_modules"));
    }

    #[test]
    fn detects_csharp_bin_and_obj() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("App.csproj"), "<Project/>").unwrap();
        fs::create_dir(tmp.path().join("bin")).unwrap();
        fs::create_dir(tmp.path().join("obj")).unwrap();

        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::CSharp));
    }

    #[test]
    fn no_false_positive_target_without_cargo_toml() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();

        let results = detect_artifacts(tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn no_false_positive_bin_without_csproj() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("bin")).unwrap();

        let results = detect_artifacts(tmp.path());
        assert!(results.is_empty());
    }
}
```

- [ ] **Step 4: Run tests — expect all pass**

```bash
cargo test detect_artifacts
```
Expected: 5 tests pass.

### Task 3: Size calculation + scan thread

**Files:**
- Modify: `src/scanner.rs`

- [ ] **Step 1: Add dir_size function**

```rust
use walkdir::WalkDir;

/// Sums sizes of all files under `path` recursively.
pub fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
```

- [ ] **Step 2: Write test for dir_size**

```rust
    #[test]
    fn calculates_dir_size() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap();  // 5 bytes
        fs::write(tmp.path().join("b.txt"), "world!").unwrap(); // 6 bytes
        assert_eq!(dir_size(tmp.path()), 11);
    }
```

- [ ] **Step 3: Run test — expect pass**

```bash
cargo test dir_size
```
Expected: 1 test passes.

- [ ] **Step 4: Add scan function**

```rust
use crossbeam_channel::Sender;
use rayon::prelude::*;

pub fn scan(root: PathBuf, tx: Sender<ScanMessage>) {
    // Phase 1: walkdir to collect candidate artifact paths (fast — metadata only).
    // filter_entry skips descending INTO known artifact dirs, preventing
    // redundant deep traversal of e.g. target/ which can be millions of files.
    let mut candidates: Vec<(PathBuf, Language)> = Vec::new();

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if !e.file_type().is_dir() {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), "target" | "node_modules" | "bin" | "obj" | ".git")
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        candidates.extend(detect_artifacts(entry.path()));
    }

    // Phase 2: rayon calculates sizes in parallel, sends each result immediately.
    candidates.par_iter().for_each(|(path, language)| {
        let size_bytes = dir_size(path);
        tx.send(ScanMessage::Found(ArtifactEntry {
            path: path.clone(),
            language: language.clone(),
            size_bytes,
        }))
        .ok();
    });

    tx.send(ScanMessage::Done).ok();
}
```

- [ ] **Step 5: Run all scanner tests**

```bash
cargo test
```
Expected: 6 tests pass, no compiler warnings.

- [ ] **Step 6: Commit and PR**

```bash
git add src/scanner.rs
git commit -m "feat: scanner module with detection, size calc, and scan thread"
gh pr create --title "feat: scanner module" --body "Marker-file detection for Rust/Node/C#, rayon parallel size calc, crossbeam streaming. filter_entry skips artifact dirs during traversal. 6 tests passing."
```

---

## Phase 3 — App State + TUI Render (branch: `feat/app-state`)

```bash
git checkout feat/scanner   # or main if merged
git checkout -b feat/app-state
```

### Task 4: App state

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Write AppState in src/app.rs**

```rust
use std::collections::HashSet;
use std::path::PathBuf;
use crate::scanner::ArtifactEntry;

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Scanning,
    Ready,
    ConfirmDelete,
    Deleting,
    Done,
}

pub struct AppState {
    pub entries: Vec<ArtifactEntry>,
    pub selected: HashSet<usize>,
    pub cursor: usize,
    pub status: AppStatus,
    pub root: PathBuf,
}

impl AppState {
    pub fn new(root: PathBuf) -> Self {
        Self {
            entries: Vec::new(),
            selected: HashSet::new(),
            cursor: 0,
            status: AppStatus::Scanning,
            root,
        }
    }

    pub fn add_entry(&mut self, entry: ArtifactEntry) {
        self.entries.push(entry);
    }

    pub fn mark_scan_done(&mut self) {
        self.status = AppStatus::Ready;
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() && self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
        }
    }

    pub fn toggle_selected(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        if self.selected.contains(&self.cursor) {
            self.selected.remove(&self.cursor);
        } else {
            self.selected.insert(self.cursor);
        }
    }

    pub fn toggle_select_all(&mut self) {
        if self.selected.len() == self.entries.len() {
            self.selected.clear();
        } else {
            self.selected = (0..self.entries.len()).collect();
        }
    }

    pub fn selected_size_bytes(&self) -> u64 {
        self.selected
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .map(|e| e.size_bytes)
            .sum()
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        let mut indices: Vec<usize> = self.selected.iter().cloned().collect();
        indices.sort_unstable();
        indices
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .map(|e| e.path.clone())
            .collect()
    }
}
```

- [ ] **Step 2: Write tests for AppState**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ArtifactEntry, Language};

    fn make_entry(name: &str, size: u64) -> ArtifactEntry {
        ArtifactEntry {
            path: PathBuf::from(name),
            language: Language::Rust,
            size_bytes: size,
        }
    }

    fn state_with_three() -> AppState {
        let mut s = AppState::new(PathBuf::from("."));
        s.add_entry(make_entry("a/target", 100));
        s.add_entry(make_entry("b/target", 200));
        s.add_entry(make_entry("c/target", 300));
        s
    }

    #[test]
    fn move_down_advances_cursor() {
        let mut s = state_with_three();
        s.move_down();
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn move_down_clamps_at_last() {
        let mut s = state_with_three();
        s.cursor = 2;
        s.move_down();
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let mut s = state_with_three();
        s.move_up();
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn toggle_selected_adds_and_removes() {
        let mut s = state_with_three();
        s.toggle_selected();
        assert!(s.selected.contains(&0));
        s.toggle_selected();
        assert!(!s.selected.contains(&0));
    }

    #[test]
    fn toggle_select_all_selects_all() {
        let mut s = state_with_three();
        s.toggle_select_all();
        assert_eq!(s.selected.len(), 3);
    }

    #[test]
    fn toggle_select_all_clears_when_all_selected() {
        let mut s = state_with_three();
        s.toggle_select_all();
        s.toggle_select_all();
        assert!(s.selected.is_empty());
    }

    #[test]
    fn selected_size_sums_only_selected() {
        let mut s = state_with_three();
        s.selected.insert(0); // 100
        s.selected.insert(2); // 300
        assert_eq!(s.selected_size_bytes(), 400);
    }

    #[test]
    fn selected_paths_returns_sorted() {
        let mut s = state_with_three();
        s.selected.insert(2);
        s.selected.insert(0);
        let paths = s.selected_paths();
        assert_eq!(paths[0], PathBuf::from("a/target"));
        assert_eq!(paths[1], PathBuf::from("c/target"));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test app
```
Expected: 8 tests pass.

### Task 5: TUI render

**Files:**
- Modify: `src/ui.rs`

- [ ] **Step 1: Write render and format_bytes in src/ui.rs**

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use crate::app::{AppState, AppStatus};

pub fn render(f: &mut Frame, state: &AppState, list_state: &mut ListState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header_text = match &state.status {
        AppStatus::Scanning => format!("scanning {}...", state.root.display()),
        AppStatus::Ready => format!("done — {} items found", state.entries.len()),
        AppStatus::ConfirmDelete => format!(
            "Delete {} folder(s) ({})? [y/N]",
            state.selected.len(),
            format_bytes(state.selected_size_bytes())
        ),
        AppStatus::Deleting => "deleting...".to_string(),
        AppStatus::Done => "done".to_string(),
    };

    f.render_widget(
        Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).title(" irona ")),
        chunks[0],
    );

    // List
    let items: Vec<ListItem> = state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let check = if state.selected.contains(&i) { "[✓]" } else { "[ ]" };
            let name = entry.path.file_name().unwrap_or_default().to_string_lossy();
            let parent = entry.path.parent().unwrap_or(&entry.path).to_string_lossy();
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {} ", check)),
                Span::styled(format!("{:<15}", name), Style::default().fg(Color::Cyan)),
                Span::raw(format!("  {:<45}", parent)),
                Span::styled(
                    format!("{:>10}", format_bytes(entry.size_bytes)),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        })
        .collect();

    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        chunks[1],
        list_state,
    );

    // Footer
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("Selected: {}   ", format_bytes(state.selected_size_bytes())),
                Style::default().fg(Color::Green),
            ),
            Span::raw("↑↓ navigate  Space select  a all  d delete  q quit"),
        ]))
        .block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_gb() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn format_bytes_mb() {
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
    }

    #[test]
    fn format_bytes_kb() {
        assert_eq!(format_bytes(1_024), "1.0 KB");
    }

    #[test]
    fn format_bytes_bytes() {
        assert_eq!(format_bytes(512), "512 B");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test ui
```
Expected: 4 tests pass.

- [ ] **Step 3: Commit and PR**

```bash
git add src/app.rs src/ui.rs
git commit -m "feat: app state and ratatui render"
gh pr create --title "feat: app state + TUI render" --body "AppState with navigation/selection/size logic (8 tests). Ratatui header/list/footer render with format_bytes (4 tests). All 12 tests passing."
```

---

## Phase 4 — Deleter + Wire Everything (branch: `feat/wire`)

```bash
git checkout feat/app-state   # or main if merged
git checkout -b feat/wire
```

### Task 6: Deleter module

**Files:**
- Modify: `src/deleter.rs`

- [ ] **Step 1: Write src/deleter.rs**

```rust
use std::path::PathBuf;

#[derive(Debug)]
pub struct DeleteResult {
    pub path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

pub async fn delete_all(paths: Vec<PathBuf>) -> Vec<DeleteResult> {
    let handles: Vec<_> = paths
        .into_iter()
        .map(|path| {
            tokio::spawn(async move {
                match tokio::fs::remove_dir_all(&path).await {
                    Ok(_) => DeleteResult { path, success: true, error: None },
                    Err(e) => DeleteResult { path, success: false, error: Some(e.to_string()) },
                }
            })
        })
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    results
}
```

- [ ] **Step 2: Write tests for delete_all**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn deletes_existing_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("artifacts");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "data").unwrap();

        let results = delete_all(vec![dir.clone()]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn reports_error_for_missing_directory() {
        let results = delete_all(vec![PathBuf::from("/nonexistent/xyz/abc")]).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.is_some());
    }

    #[tokio::test]
    async fn deletes_multiple_concurrently() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();

        let results = delete_all(vec![a.clone(), b.clone()]).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
        assert!(!a.exists());
        assert!(!b.exists());
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test deleter
```
Expected: 3 tests pass.

### Task 7: Wire everything in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace src/main.rs with full event loop**

```rust
mod app;
mod deleter;
mod scanner;
mod ui;

use app::{AppState, AppStatus};
use clap::Parser;
use crossbeam_channel::unbounded;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use scanner::ScanMessage;
use std::{io, path::PathBuf, thread, time::Duration};

#[derive(Parser)]
#[command(name = "irona", about = "Reclaim disk space from build artifacts")]
struct Args {
    /// Root directory to scan (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let root = args.path.canonicalize().unwrap_or(args.path);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, root);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    root: PathBuf,
) -> anyhow::Result<()> {
    let (tx, rx) = unbounded::<ScanMessage>();
    let root_clone = root.clone();

    thread::spawn(move || scanner::scan(root_clone, tx));

    let mut state = AppState::new(root);
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    let rt = tokio::runtime::Runtime::new()?;

    loop {
        // Drain all pending channel messages this tick
        loop {
            match rx.try_recv() {
                Ok(ScanMessage::Found(entry)) => state.add_entry(entry),
                Ok(ScanMessage::Done) => {
                    state.mark_scan_done();
                    break;
                }
                Err(_) => break,
            }
        }

        if !state.entries.is_empty() {
            list_state.select(Some(state.cursor));
        }

        terminal.draw(|f| ui::render(f, &state, &mut list_state))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match (&state.status, key.code) {
                    (_, KeyCode::Char('q')) => break,

                    (AppStatus::Scanning | AppStatus::Ready, KeyCode::Up) => state.move_up(),
                    (AppStatus::Scanning | AppStatus::Ready, KeyCode::Down) => state.move_down(),
                    (AppStatus::Scanning | AppStatus::Ready, KeyCode::Char(' ')) => {
                        state.toggle_selected()
                    }
                    (AppStatus::Scanning | AppStatus::Ready, KeyCode::Char('a')) => {
                        state.toggle_select_all()
                    }

                    (AppStatus::Ready, KeyCode::Char('d')) if !state.selected.is_empty() => {
                        state.status = AppStatus::ConfirmDelete;
                    }

                    (AppStatus::ConfirmDelete, KeyCode::Char('y')) => {
                        state.status = AppStatus::Deleting;
                        terminal.draw(|f| ui::render(f, &state, &mut list_state))?;
                        let paths = state.selected_paths();
                        rt.block_on(deleter::delete_all(paths));
                        state.selected.clear();
                        state.entries.retain(|e| e.path.exists());
                        state.cursor = 0;
                        state.status = AppStatus::Ready;
                    }

                    (AppStatus::ConfirmDelete, KeyCode::Char('n') | KeyCode::Esc) => {
                        state.status = AppStatus::Ready;
                    }

                    _ => {}
                }
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Build**

```bash
cargo build
```
Expected: compiles with no errors.

- [ ] **Step 3: Smoke test — run against current workspace**

```bash
cargo run -- /home/kunjee/Workspace
```
Expected: TUI opens, scans workspace, shows artifact folders, `q` quits cleanly. Test navigation with `↑↓`, selection with `Space` and `a`.

- [ ] **Step 4: Run all tests**

```bash
cargo test
```
Expected: all tests pass (scanner: 6, app: 8, ui: 4, deleter: 3 = 21 total).

- [ ] **Step 5: Commit and PR**

```bash
git add src/main.rs src/deleter.rs
git commit -m "feat: deleter module and full event loop wiring"
gh pr create --title "feat: wire deleter + main event loop" --body "Tokio async deleter (3 tests), full Ratatui event loop — scanner streams into TUI live, keyboard nav/select/confirm-delete all working. 21 tests total passing."
```

---

## Phase 5 — GitHub Actions (branch: `feat/github-actions`)

Branch from main (independent of core code):
```bash
git checkout main
git checkout -b feat/github-actions
```

### Task 8: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create directory and ci.yml**

```bash
mkdir -p .github/workflows
```

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache
        uses: Swatinem/rust-cache@v2

      - name: Format check
        run: cargo fmt --all -- --check

      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: Test
        run: cargo test --all
```

### Task 9: Release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create release.yml**

`.github/workflows/release.yml`:
```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            binary: irona
            archive: irona-linux-x86_64.tar.gz
          - target: aarch64-apple-darwin
            os: macos-latest
            binary: irona
            archive: irona-macos-arm64.tar.gz
          - target: x86_64-apple-darwin
            os: macos-latest
            binary: irona
            archive: irona-macos-x86_64.tar.gz
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            binary: irona.exe
            archive: irona-windows-x86_64.zip

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache
        uses: Swatinem/rust-cache@v2

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Package (Unix)
        if: matrix.os != 'windows-latest'
        run: |
          cp target/${{ matrix.target }}/release/${{ matrix.binary }} .
          tar czf ${{ matrix.archive }} ${{ matrix.binary }}

      - name: Package (Windows)
        if: matrix.os == 'windows-latest'
        run: |
          cp target/${{ matrix.target }}/release/${{ matrix.binary }} .
          Compress-Archive -Path ${{ matrix.binary }} -DestinationPath ${{ matrix.archive }}

      - name: Upload to release
        uses: softprops/action-gh-release@v2
        with:
          files: ${{ matrix.archive }}

  publish:
    name: Publish to crates.io
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Publish
        run: cargo publish --token ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

- [ ] **Step 2: Commit and PR**

```bash
git add .github/
git commit -m "feat: GitHub Actions CI and release workflows"
gh pr create --title "feat: GitHub Actions CI + release" --body "CI: fmt/clippy/test on all PRs. Release: cross-platform binaries (Linux/macOS arm64+x86/Windows) + crates.io publish on v* tags."
```

---

## Phase 6 — Landing Page (branch: `feat/landing-page`)

Branch from main (independent):
```bash
git checkout main
git checkout -b feat/landing-page
```

### Task 10: GitHub Pages landing page

**Files:**
- Create: `docs/index.html`

- [ ] **Step 1: Invoke frontend-design skill**

Run `/frontend-design` with this brief:

> Build a single-page HTML landing page for `irona` — a Rust TUI CLI tool that reclaims disk space from build artifacts (Rust `target/`, Node `node_modules/`, C# `bin/`+`obj/`).
>
> Save to `docs/index.html`. No build step — pure HTML/CSS/JS only (GitHub Pages serves it as-is).
>
> Sections:
> 1. **Hero** — name "irona", tagline "Reclaim your disk. Clean your workspace.", install command (`cargo install irona`), link to GitHub releases for pre-built binaries
> 2. **How it works** — 3 steps: `irona /your/workspace` → interactive checklist with sizes → press `d` to delete
> 3. **Supported** — Rust, Node.js, C# with language icons
> 4. **Footer** — GitHub link
>
> Aesthetic: dark terminal theme, monospace font, feels like a dev tool page.

- [ ] **Step 2: Enable GitHub Pages in repo settings**

GitHub repo → Settings → Pages → Source: Deploy from branch → Branch: `main` → Folder: `/docs` → Save.

- [ ] **Step 3: Commit and PR**

```bash
git add docs/index.html
git commit -m "feat: GitHub Pages landing page"
gh pr create --title "feat: GitHub Pages landing page" --body "Single-page HTML landing in docs/index.html — dark terminal theme, hero/how-it-works/supported sections."
```
