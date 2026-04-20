# irona Elm Architecture Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor irona to the Elm Architecture using tuirealm, fixing the status-mutation borrow bug, adding typed errors (thiserror), and timing display (chrono).

**Architecture:** `AppModel` is the single source of truth. Three tuirealm `Component`s handle keyboard events and emit `AppMsg` variants. A pure `update(model, msg)` function mutates `AppModel`. Rendering reads `AppModel` directly via our own `render()` function (not tuirealm's view system). Scanner and delete results arrive via crossbeam/tokio channels polled in the main loop and converted to `AppMsg`.

**Tech Stack:** Rust, ratatui 0.29, tuirealm 3, crossterm 0.28, thiserror 1, chrono 0.4, tokio, crossbeam-channel, anyhow, tempfile (dev)

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add tuirealm, thiserror, chrono |
| Create | `src/errors.rs` | `IronaError` (thiserror) |
| Modify | `src/deleter.rs` | Return `Duration` per delete result |
| Create | `src/model/mod.rs` | `AppModel`, `AppMsg`, `AppStatus`, `update()` |
| Create | `src/model/entry.rs` | `EntryModel`, `DeleteState`, nested `EntryMsg` |
| Create | `src/model/entry_list.rs` | `EntryListModel`, navigation/selection logic |
| Create | `src/components/mod.rs` | `ComponentId` enum, layout helper |
| Create | `src/components/header.rs` | `HeaderComponent` — event handler |
| Create | `src/components/entry_list.rs` | `EntryListComponent` — keyboard events |
| Create | `src/components/status_bar.rs` | `StatusBarComponent` — keyboard events |
| Create | `src/render.rs` | `render(f, model)` — all ratatui rendering |
| Rewrite | `src/main.rs` | tuirealm `Application` setup + event loop |
| Delete | `src/app.rs` | Replaced by `model/` |
| Delete | `src/ui.rs` | Replaced by `render.rs` |
| Unchanged | `src/scanner.rs` | No changes needed |

---

## Task 1: Add dependencies and create `errors.rs`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/errors.rs`

- [ ] **Step 1: Update Cargo.toml**

Replace the `[dependencies]` block with:

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
walkdir = "2"
crossbeam-channel = "0.5"
rayon = "1"
tokio = { version = "1", features = ["rt-multi-thread", "fs", "macros"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
thiserror = "1"
chrono = { version = "0.4", features = ["clock"] }
tuirealm = { version = "3", features = ["crossterm"] }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create `src/errors.rs`**

```rust
#[derive(Debug, thiserror::Error)]
pub enum IronaError {
    #[error("delete failed: {0}")]
    DeleteFailed(#[from] std::io::Error),

    #[error("scan error: {0}")]
    ScanError(String),
}

impl Clone for IronaError {
    fn clone(&self) -> Self {
        match self {
            Self::DeleteFailed(e) => Self::ScanError(e.to_string()),
            Self::ScanError(s) => Self::ScanError(s.clone()),
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no output (no errors).

---

## Task 2: Update `deleter.rs` to return `Duration` per result

**Files:**
- Modify: `src/deleter.rs`

- [ ] **Step 1: Rewrite `deleter.rs`**

```rust
use crate::errors::IronaError;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct DeleteResult {
    pub path: PathBuf,
    pub index: usize,
    pub elapsed: Duration,
    pub outcome: Result<(), IronaError>,
}

pub async fn delete_all(paths: Vec<(usize, PathBuf)>) -> Vec<DeleteResult> {
    let handles: Vec<_> = paths
        .into_iter()
        .map(|(index, path)| {
            tokio::spawn(async move {
                let start = Instant::now();
                let outcome = tokio::fs::remove_dir_all(&path)
                    .await
                    .map_err(IronaError::DeleteFailed);
                DeleteResult {
                    path,
                    index,
                    elapsed: start.elapsed(),
                    outcome,
                }
            })
        })
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(r) = handle.await {
            results.push(r);
        }
    }
    results
}
```

- [ ] **Step 2: Update tests in `deleter.rs`**

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

        let results = delete_all(vec![(0, dir.clone())]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_ok());
        assert!(!dir.exists());
        assert!(results[0].elapsed.as_nanos() > 0);
    }

    #[tokio::test]
    async fn reports_error_for_missing_directory() {
        let results = delete_all(vec![(0, PathBuf::from("/nonexistent/xyz/abc"))]).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_err());
    }

    #[tokio::test]
    async fn deletes_multiple_concurrently() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();

        let results = delete_all(vec![(0, a.clone()), (1, b.clone())]).await;
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.outcome.is_ok()));
        assert!(!a.exists());
        assert!(!b.exists());
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo nextest run deleter 2>&1 | tail -5
```

Expected: 3 tests pass.

---

## Task 3: Create `src/model/entry.rs`

**Files:**
- Create: `src/model/entry.rs`

- [ ] **Step 1: Create `src/model/` directory and `entry.rs`**

```bash
mkdir -p src/model
```

```rust
// src/model/entry.rs
use crate::errors::IronaError;
use crate::scanner::ArtifactEntry;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct EntryModel {
    pub entry: ArtifactEntry,
    pub selected: bool,
    pub delete_state: DeleteState,
}

#[derive(Debug, Clone)]
pub enum DeleteState {
    Pending,
    Deleted { elapsed: Duration },
    Failed { message: String, elapsed: Duration },
}

#[derive(Debug)]
pub enum EntryMsg {
    DeleteResult(Result<Duration, IronaError>),
}

impl EntryModel {
    pub fn new(entry: ArtifactEntry) -> Self {
        Self {
            entry,
            selected: false,
            delete_state: DeleteState::Pending,
        }
    }

    pub fn apply(&mut self, msg: EntryMsg) {
        match msg {
            EntryMsg::DeleteResult(Ok(elapsed)) => {
                self.delete_state = DeleteState::Deleted { elapsed };
            }
            EntryMsg::DeleteResult(Err(e)) => {
                self.delete_state = DeleteState::Failed {
                    message: e.to_string(),
                    elapsed: Duration::ZERO,
                };
            }
        }
    }
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build 2>&1 | grep "^error"
```

Expected: no errors (model/mod.rs doesn't exist yet — that's fine, we'll wire it in Task 5).

---

## Task 4: Create `src/model/entry_list.rs`

**Files:**
- Create: `src/model/entry_list.rs`

- [ ] **Step 1: Write the failing tests first**

```rust
// at the bottom of src/model/entry_list.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ArtifactEntry, Language};
    use std::path::PathBuf;

    fn make_entry(name: &str) -> ArtifactEntry {
        ArtifactEntry { path: PathBuf::from(name), language: Language::Rust, size_bytes: 100 }
    }

    fn list_with_three() -> EntryListModel {
        let mut m = EntryListModel::new();
        m.add(make_entry("a/target"));
        m.add(make_entry("b/target"));
        m.add(make_entry("c/target"));
        m
    }

    #[test]
    fn move_down_advances_cursor() {
        let mut m = list_with_three();
        m.move_down();
        assert_eq!(m.cursor, 1);
    }

    #[test]
    fn move_down_clamps_at_last() {
        let mut m = list_with_three();
        m.cursor = 2;
        m.move_down();
        assert_eq!(m.cursor, 2);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let mut m = list_with_three();
        m.move_up();
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn toggle_select_adds_and_removes() {
        let mut m = list_with_three();
        m.toggle_select();
        assert!(m.entries[0].selected);
        m.toggle_select();
        assert!(!m.entries[0].selected);
    }

    #[test]
    fn toggle_select_all_selects_all() {
        let mut m = list_with_three();
        m.toggle_select_all();
        assert!(m.entries.iter().all(|e| e.selected));
    }

    #[test]
    fn toggle_select_all_clears_when_all_selected() {
        let mut m = list_with_three();
        m.toggle_select_all();
        m.toggle_select_all();
        assert!(m.entries.iter().all(|e| !e.selected));
    }

    #[test]
    fn selected_size_sums_selected_only() {
        let mut m = EntryListModel::new();
        m.add(ArtifactEntry { path: PathBuf::from("a"), language: Language::Rust, size_bytes: 100 });
        m.add(ArtifactEntry { path: PathBuf::from("b"), language: Language::Rust, size_bytes: 200 });
        m.entries[0].selected = true;
        assert_eq!(m.selected_size_bytes(), 100);
    }

    #[test]
    fn selected_paths_returns_index_and_path() {
        let mut m = list_with_three();
        m.entries[0].selected = true;
        m.entries[2].selected = true;
        let paths = m.selected_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].0, 0);
        assert_eq!(paths[1].0, 2);
    }

    #[test]
    fn selected_count_returns_zero_when_empty() {
        let m = EntryListModel::new();
        assert_eq!(m.selected_count(), 0);
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo nextest run entry_list 2>&1 | tail -5
```

Expected: compile error — `EntryListModel` not defined yet.

- [ ] **Step 3: Implement `entry_list.rs`**

```rust
// src/model/entry_list.rs
use crate::errors::IronaError;
use crate::scanner::ArtifactEntry;
use std::path::PathBuf;
use std::time::Duration;

use super::entry::{EntryModel, EntryMsg};

pub struct EntryListModel {
    pub entries: Vec<EntryModel>,
    pub cursor: usize,
}

impl EntryListModel {
    pub fn new() -> Self {
        Self { entries: Vec::new(), cursor: 0 }
    }

    pub fn add(&mut self, entry: ArtifactEntry) {
        self.entries.push(EntryModel::new(entry));
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

    pub fn toggle_select(&mut self) {
        if let Some(e) = self.entries.get_mut(self.cursor) {
            e.selected = !e.selected;
        }
    }

    pub fn toggle_select_all(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let all_selected = self.entries.iter().all(|e| e.selected);
        for e in &mut self.entries {
            e.selected = !all_selected;
        }
    }

    pub fn selected_count(&self) -> usize {
        self.entries.iter().filter(|e| e.selected).count()
    }

    pub fn selected_size_bytes(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.selected)
            .map(|e| e.entry.size_bytes)
            .sum()
    }

    pub fn selected_paths(&self) -> Vec<(usize, PathBuf)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.selected)
            .map(|(i, e)| (i, e.entry.path.clone()))
            .collect()
    }

    pub fn apply_delete_result(
        &mut self,
        index: usize,
        elapsed: Duration,
        outcome: Result<(), IronaError>,
    ) {
        if let Some(row) = self.entries.get_mut(index) {
            let msg = EntryMsg::DeleteResult(outcome.map(|_| elapsed).map_err(|e| e));
            row.apply(msg);
        }
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo nextest run entry_list 2>&1 | tail -5
```

Expected: all 9 tests pass.

---

## Task 5: Create `src/model/mod.rs`

**Files:**
- Create: `src/model/mod.rs`

- [ ] **Step 1: Write `src/model/mod.rs`**

```rust
// src/model/mod.rs
pub mod entry;
pub mod entry_list;

pub use entry::{DeleteState, EntryModel, EntryMsg};
pub use entry_list::EntryListModel;

use crate::errors::IronaError;
use crate::scanner::ArtifactEntry;
use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Scanning,
    Ready,
    ConfirmDelete,
    Deleting,
    Done,
}

pub struct AppModel {
    pub status: AppStatus,
    pub root: PathBuf,
    pub clock: DateTime<Local>,
    pub scan_start: Instant,
    pub scan_elapsed: Option<Duration>,
    pub delete_start: Option<Instant>,
    pub delete_elapsed: Option<Duration>,
    pub entries: EntryListModel,
}

impl AppModel {
    pub fn new(root: PathBuf) -> Self {
        Self {
            status: AppStatus::Scanning,
            root,
            clock: Local::now(),
            scan_start: Instant::now(),
            scan_elapsed: None,
            delete_start: None,
            delete_elapsed: None,
            entries: EntryListModel::new(),
        }
    }
}

// ── Nested messages ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AppMsg {
    Tick,
    Quit,
    List(ListMsg),
    Delete(DeleteMsg),
}

#[derive(Debug)]
pub enum ListMsg {
    MoveUp,
    MoveDown,
    ToggleSelect,
    SelectAll,
    ScanFound(ArtifactEntry),
    ScanDone,
}

#[derive(Debug)]
pub enum DeleteMsg {
    Request,
    ConfirmYes,
    ConfirmNo,
    Result {
        index: usize,
        elapsed: Duration,
        outcome: Result<(), IronaError>,
    },
    AllDone,
}

// ── Pure update function ──────────────────────────────────────────────────────

/// Returns false when the app should quit.
pub fn update(model: &mut AppModel, msg: AppMsg) -> bool {
    match msg {
        AppMsg::Quit => return false,
        AppMsg::Tick => {
            model.clock = Local::now();
        }
        AppMsg::List(m) => update_list(model, m),
        AppMsg::Delete(m) => update_delete(model, m),
    }
    true
}

fn update_list(model: &mut AppModel, msg: ListMsg) {
    match msg {
        ListMsg::MoveUp => model.entries.move_up(),
        ListMsg::MoveDown => model.entries.move_down(),
        ListMsg::ToggleSelect => model.entries.toggle_select(),
        ListMsg::SelectAll => model.entries.toggle_select_all(),
        ListMsg::ScanFound(entry) => model.entries.add(entry),
        ListMsg::ScanDone => {
            model.scan_elapsed = Some(model.scan_start.elapsed());
            if model.status == AppStatus::Scanning {
                model.status = AppStatus::Ready;
            }
        }
    }
}

fn update_delete(model: &mut AppModel, msg: DeleteMsg) {
    match msg {
        DeleteMsg::Request => {
            if model.entries.selected_count() > 0
                && matches!(model.status, AppStatus::Ready | AppStatus::Scanning)
            {
                model.status = AppStatus::ConfirmDelete;
            }
        }
        DeleteMsg::ConfirmYes => {
            if model.status == AppStatus::ConfirmDelete {
                model.status = AppStatus::Deleting;
                model.delete_start = Some(Instant::now());
            }
        }
        DeleteMsg::ConfirmNo => {
            if model.status == AppStatus::ConfirmDelete {
                model.status = AppStatus::Ready;
            }
        }
        DeleteMsg::Result { index, elapsed, outcome } => {
            model.entries.apply_delete_result(index, elapsed, outcome);
        }
        DeleteMsg::AllDone => {
            if let Some(start) = model.delete_start {
                model.delete_elapsed = Some(start.elapsed());
            }
            model.status = AppStatus::Ready;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ArtifactEntry, Language};
    use std::path::PathBuf;

    fn entry(name: &str) -> ArtifactEntry {
        ArtifactEntry { path: PathBuf::from(name), language: Language::Rust, size_bytes: 100 }
    }

    fn ready_model_with_selection() -> AppModel {
        let mut m = AppModel::new(PathBuf::from("."));
        m.status = AppStatus::Ready;
        m.entries.add(entry("a/target"));
        m.entries.add(entry("b/target"));
        m.entries.toggle_select_all();
        m
    }

    #[test]
    fn quit_returns_false() {
        let mut m = AppModel::new(PathBuf::from("."));
        assert!(!update(&mut m, AppMsg::Quit));
    }

    #[test]
    fn tick_updates_clock() {
        let mut m = AppModel::new(PathBuf::from("."));
        let before = m.clock;
        std::thread::sleep(std::time::Duration::from_millis(5));
        update(&mut m, AppMsg::Tick);
        assert!(m.clock >= before);
    }

    #[test]
    fn delete_request_transitions_to_confirm() {
        let mut m = ready_model_with_selection();
        update(&mut m, AppMsg::Delete(DeleteMsg::Request));
        assert_eq!(m.status, AppStatus::ConfirmDelete);
    }

    #[test]
    fn delete_request_noop_when_nothing_selected() {
        let mut m = AppModel::new(PathBuf::from("."));
        m.status = AppStatus::Ready;
        m.entries.add(entry("a/target"));
        update(&mut m, AppMsg::Delete(DeleteMsg::Request));
        assert_eq!(m.status, AppStatus::Ready);
    }

    #[test]
    fn confirm_yes_transitions_to_deleting() {
        let mut m = ready_model_with_selection();
        m.status = AppStatus::ConfirmDelete;
        update(&mut m, AppMsg::Delete(DeleteMsg::ConfirmYes));
        assert_eq!(m.status, AppStatus::Deleting);
        assert!(m.delete_start.is_some());
    }

    #[test]
    fn confirm_no_returns_to_ready() {
        let mut m = ready_model_with_selection();
        m.status = AppStatus::ConfirmDelete;
        update(&mut m, AppMsg::Delete(DeleteMsg::ConfirmNo));
        assert_eq!(m.status, AppStatus::Ready);
    }

    #[test]
    fn scan_done_sets_elapsed_and_ready() {
        let mut m = AppModel::new(PathBuf::from("."));
        update(&mut m, AppMsg::List(ListMsg::ScanDone));
        assert_eq!(m.status, AppStatus::Ready);
        assert!(m.scan_elapsed.is_some());
    }
}
```

- [ ] **Step 2: Run model tests**

```bash
cargo nextest run model 2>&1 | tail -8
```

Expected: all 8 model tests pass.

---

## Task 6: Create `src/components/mod.rs` and `src/render.rs`

**Files:**
- Create: `src/components/mod.rs`
- Create: `src/render.rs`

These are shared building blocks needed by all components.

- [ ] **Step 1: Create `src/components/mod.rs`**

```rust
// src/components/mod.rs
pub mod entry_list;
pub mod header;
pub mod status_bar;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

/// Split the frame into [header(3), list(min), footer(3)].
pub fn three_row_layout(area: Rect) -> [Rect; 3] {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
        .split(area);
    [chunks[0], chunks[1], chunks[2]]
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ComponentId {
    Header,
    EntryList,
    StatusBar,
}
```

- [ ] **Step 2: Create `src/render.rs`**

```rust
// src/render.rs — reads AppModel and renders everything
use crate::model::{AppModel, AppStatus, DeleteState};
use crate::components::three_row_layout;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, model: &AppModel, list_state: &mut ListState) {
    let [header_area, list_area, footer_area] = three_row_layout(f.area());
    render_header(f, model, header_area);
    render_list(f, model, list_state, list_area);
    render_footer(f, model, footer_area);
}

fn render_header(f: &mut Frame, model: &AppModel, area: ratatui::layout::Rect) {
    let clock = model.clock.format("%H:%M:%S").to_string();

    let (text, style) = match &model.status {
        AppStatus::Scanning => {
            let elapsed = model.scan_start.elapsed().as_secs();
            (
                format!("[{}]  scanning {}...  {}s", clock, model.root.display(), elapsed),
                Style::default(),
            )
        }
        AppStatus::Ready => {
            let elapsed = model
                .scan_elapsed
                .map(|d| format!(" — scanned in {:.1}s", d.as_secs_f64()))
                .unwrap_or_default();
            (
                format!("[{}]  {} items found{}", clock, model.entries.entries.len(), elapsed),
                Style::default(),
            )
        }
        AppStatus::ConfirmDelete => (
            format!(
                "[{}]  Delete {} folder(s) ({})? [y/N]",
                clock,
                model.entries.selected_count(),
                format_bytes(model.entries.selected_size_bytes())
            ),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        AppStatus::Deleting => {
            let elapsed = model
                .delete_start
                .map(|s| format!(" {}s", s.elapsed().as_secs()))
                .unwrap_or_default();
            (
                format!("[{}]  deleting...{}", clock, elapsed),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        }
        AppStatus::Done => {
            let elapsed = model
                .delete_elapsed
                .map(|d| format!(" — freed {} in {:.1}s", format_bytes(model.entries.selected_size_bytes()), d.as_secs_f64()))
                .unwrap_or_default();
            (
                format!("[{}]  done{}", clock, elapsed),
                Style::default().fg(Color::Green),
            )
        }
    };

    f.render_widget(
        Paragraph::new(Span::styled(text, style))
            .block(Block::default().borders(Borders::ALL).title(" irona ")),
        area,
    );
}

fn render_list(
    f: &mut Frame,
    model: &AppModel,
    list_state: &mut ListState,
    area: ratatui::layout::Rect,
) {
    list_state.select(if model.entries.entries.is_empty() {
        None
    } else {
        Some(model.entries.cursor)
    });

    let items: Vec<ListItem> = model
        .entries
        .entries
        .iter()
        .map(|row| {
            let name = row
                .entry
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let parent = row
                .entry
                .path
                .parent()
                .unwrap_or(&row.entry.path)
                .to_string_lossy();

            let (check, right_col, right_style) = match &row.delete_state {
                DeleteState::Pending => (
                    if row.selected { "[✓]" } else { "[ ]" },
                    format!("{:>10}", format_bytes(row.entry.size_bytes)),
                    Style::default().fg(Color::Yellow),
                ),
                DeleteState::Deleted { elapsed } => (
                    "[✓]",
                    format!("deleted {:.1}s", elapsed.as_secs_f64()),
                    Style::default().fg(Color::Green),
                ),
                DeleteState::Failed { message, .. } => (
                    "[✗]",
                    message.chars().take(20).collect::<String>(),
                    Style::default().fg(Color::Red),
                ),
            };

            ListItem::new(Line::from(vec![
                Span::raw(format!(" {} ", check)),
                Span::styled(format!("{:<15}", name), Style::default().fg(Color::Cyan)),
                Span::raw(format!("  {:<45}", parent)),
                Span::styled(format!("{:>12}", right_col), right_style),
            ]))
        })
        .collect();

    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        area,
        list_state,
    );
}

fn render_footer(f: &mut Frame, model: &AppModel, area: ratatui::layout::Rect) {
    let hint = match &model.status {
        AppStatus::ConfirmDelete => Span::styled(
            "  y  confirm    n / Esc  cancel",
            Style::default().fg(Color::Yellow),
        ),
        AppStatus::Deleting => Span::styled(
            "  deleting — please wait...",
            Style::default().fg(Color::Red),
        ),
        _ => Span::raw("  ↑↓ navigate  Space select  a all  d delete  q quit"),
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("  Selected: {}   ", format_bytes(model.entries.selected_size_bytes())),
                Style::default().fg(Color::Green),
            ),
            hint,
        ]))
        .block(Block::default().borders(Borders::ALL)),
        area,
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
    fn format_bytes_gb() { assert_eq!(format_bytes(1_073_741_824), "1.0 GB"); }
    #[test]
    fn format_bytes_mb() { assert_eq!(format_bytes(1_048_576), "1.0 MB"); }
    #[test]
    fn format_bytes_kb() { assert_eq!(format_bytes(1_024), "1.0 KB"); }
    #[test]
    fn format_bytes_bytes() { assert_eq!(format_bytes(512), "512 B"); }
}
```

- [ ] **Step 3: Build (will fail until main.rs is updated — that's fine)**

```bash
cargo build 2>&1 | grep "^error" | head -10
```

Expected: errors about unused modules — acceptable at this stage.

---

## Task 7: Create the three tuirealm components

**Files:**
- Create: `src/components/header.rs`
- Create: `src/components/entry_list.rs`
- Create: `src/components/status_bar.rs`

Components are thin: they handle events in `on()` and emit `AppMsg`. `view()` is a no-op because rendering is done by `render.rs`.

- [ ] **Step 1: Create `src/components/header.rs`**

```rust
// src/components/header.rs
// HeaderComponent subscribes to Tick and emits AppMsg::Tick.
use crate::model::AppMsg;
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Event, NoUserEvent},
    props::{Attribute, AttrValue},
    Component, Frame, MockComponent, State,
};
use ratatui::layout::Rect;

#[derive(Default)]
pub struct HeaderComponent;

impl MockComponent for HeaderComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}
    fn query(&self, _attr: Attribute) -> Option<AttrValue> { None }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
    fn state(&self) -> State { State::None }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult { CmdResult::None }
}

impl Component<AppMsg, NoUserEvent> for HeaderComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        match ev {
            Event::Tick => Some(AppMsg::Tick),
            _ => None,
        }
    }
}
```

- [ ] **Step 2: Create `src/components/entry_list.rs`**

```rust
// src/components/entry_list.rs
// EntryListComponent handles keyboard navigation and selection.
use crate::model::{AppMsg, DeleteMsg, ListMsg};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Event, Key, KeyEvent, NoUserEvent},
    props::{Attribute, AttrValue},
    Component, Frame, MockComponent, State,
};
use ratatui::layout::Rect;

#[derive(Default)]
pub struct EntryListComponent;

impl MockComponent for EntryListComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}
    fn query(&self, _attr: Attribute) -> Option<AttrValue> { None }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
    fn state(&self) -> State { State::None }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult { CmdResult::None }
}

impl Component<AppMsg, NoUserEvent> for EntryListComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        match ev {
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                Some(AppMsg::List(ListMsg::MoveUp))
            }
            Event::Keyboard(KeyEvent { code: Key::Down, .. }) => {
                Some(AppMsg::List(ListMsg::MoveDown))
            }
            Event::Keyboard(KeyEvent { code: Key::Char(' '), .. }) => {
                Some(AppMsg::List(ListMsg::ToggleSelect))
            }
            Event::Keyboard(KeyEvent { code: Key::Char('a'), .. }) => {
                Some(AppMsg::List(ListMsg::SelectAll))
            }
            Event::Keyboard(KeyEvent { code: Key::Char('d'), .. }) => {
                Some(AppMsg::Delete(DeleteMsg::Request))
            }
            Event::Keyboard(KeyEvent { code: Key::Char('q'), .. }) => Some(AppMsg::Quit),
            _ => None,
        }
    }
}
```

- [ ] **Step 3: Create `src/components/status_bar.rs`**

```rust
// src/components/status_bar.rs
// StatusBarComponent handles y/n/Esc during ConfirmDelete.
use crate::model::{AppMsg, DeleteMsg};
use tuirealm::{
    command::{Cmd, CmdResult},
    event::{Event, Key, KeyEvent, NoUserEvent},
    props::{Attribute, AttrValue},
    Component, Frame, MockComponent, State,
};
use ratatui::layout::Rect;

#[derive(Default)]
pub struct StatusBarComponent;

impl MockComponent for StatusBarComponent {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}
    fn query(&self, _attr: Attribute) -> Option<AttrValue> { None }
    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}
    fn state(&self) -> State { State::None }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult { CmdResult::None }
}

impl Component<AppMsg, NoUserEvent> for StatusBarComponent {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<AppMsg> {
        match ev {
            Event::Keyboard(KeyEvent { code: Key::Char('y'), .. }) => {
                Some(AppMsg::Delete(DeleteMsg::ConfirmYes))
            }
            Event::Keyboard(KeyEvent { code: Key::Char('n'), .. })
            | Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => {
                Some(AppMsg::Delete(DeleteMsg::ConfirmNo))
            }
            Event::Keyboard(KeyEvent { code: Key::Char('q'), .. }) => Some(AppMsg::Quit),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Verify components compile**

```bash
cargo build 2>&1 | grep "^error" | head -10
```

Expected: errors about `main.rs` only — components should be clean.

---

## Task 8: Rewrite `src/main.rs`

**Files:**
- Modify: `src/main.rs`

The main loop:
1. Mount tuirealm components with subscriptions
2. Drain scan channel → `update(model, AppMsg::List(...))`
3. Active component switches between `EntryList` (normal) and `StatusBar` (confirm)
4. `app.tick()` → messages → `update()`
5. Spawn delete tasks when `Deleting` state starts; poll delete channel
6. `terminal.draw()` using our own `render()`

- [ ] **Step 1: Rewrite `main.rs`**

```rust
mod components;
mod deleter;
mod errors;
mod model;
mod render;
mod scanner;

use anyhow::Result;
use clap::Parser;
use components::{entry_list::EntryListComponent, header::HeaderComponent, status_bar::StatusBarComponent, ComponentId};
use crossbeam_channel::unbounded;
use model::{update, AppMsg, AppStatus, DeleteMsg, ListMsg};
use render::render;
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use scanner::ScanMessage;
use std::{io, path::PathBuf, sync::mpsc, thread, time::Duration};
use tuirealm::{
    Application, EventListenerCfg, PollStrategy, Sub, SubClause, SubEventClause,
    event::{Key, KeyEvent, NoUserEvent},
};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[derive(Parser)]
#[command(name = "irona", about = "Reclaim disk space from build artifacts")]
struct Args {
    #[arg(default_value = ".")]
    path: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = args.path.canonicalize().unwrap_or(args.path);

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

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
) -> Result<()> {
    // ── Scanner ───────────────────────────────────────────────��────────────
    let (scan_tx, scan_rx) = unbounded::<ScanMessage>();
    let root_clone = root.clone();
    thread::spawn(move || scanner::scan(root_clone, scan_tx));

    // ── Delete results channel ─────────────────────────────────────────────
    let (del_tx, del_rx) = mpsc::channel::<deleter::DeleteResult>();

    // ── App model ──────────────────────────────────────────────────────────
    let mut model = model::AppModel::new(root);
    let mut list_state = ListState::default();

    // ── tuirealm Application ───────────────────────────────────────────────
    let mut app: Application<ComponentId, AppMsg, NoUserEvent> = Application::init(
        EventListenerCfg::default()
            .crossterm_input_listener(Duration::from_millis(20), 10)
            .tick_interval(Duration::from_secs(1), 10),
    );

    app.mount(
        ComponentId::Header,
        Box::new(HeaderComponent::default()),
        vec![Sub::new(SubEventClause::Tick, SubClause::Always)],
    )?;
    app.mount(
        ComponentId::EntryList,
        Box::new(EntryListComponent::default()),
        vec![],
    )?;
    app.mount(
        ComponentId::StatusBar,
        Box::new(StatusBarComponent::default()),
        vec![],
    )?;

    // EntryList is active by default (handles navigation keys)
    app.active(&ComponentId::EntryList)?;

    let rt = tokio::runtime::Runtime::new()?;
    let mut delete_pending = 0usize;

    loop {
        // ── 1. Drain scan channel ─────────────────────────────────────────
        loop {
            match scan_rx.try_recv() {
                Ok(ScanMessage::Found(entry)) => {
                    update(&mut model, AppMsg::List(ListMsg::ScanFound(entry)));
                }
                Ok(ScanMessage::Done) => {
                    update(&mut model, AppMsg::List(ListMsg::ScanDone));
                    break;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    update(&mut model, AppMsg::List(ListMsg::ScanDone));
                    break;
                }
            }
        }

        // ── 2. Spawn deletes when transitioning to Deleting ───────────────
        if model.status == AppStatus::Deleting && delete_pending == 0 {
            let paths = model.entries.selected_paths();
            if paths.is_empty() {
                update(&mut model, AppMsg::Delete(DeleteMsg::AllDone));
            } else {
                delete_pending = paths.len();
                let tx = del_tx.clone();
                rt.spawn(async move {
                    let results = deleter::delete_all(paths).await;
                    for r in results {
                        let _ = tx.send(r);
                    }
                });
            }
        }

        // ── 3. Drain delete results ───────────────────────────────────────
        while let Ok(result) = del_rx.try_recv() {
            let outcome = result.outcome.map_err(|e| e);
            update(
                &mut model,
                AppMsg::Delete(DeleteMsg::Result {
                    index: result.index,
                    elapsed: result.elapsed,
                    outcome,
                }),
            );
            delete_pending = delete_pending.saturating_sub(1);
            if delete_pending == 0 {
                update(&mut model, AppMsg::Delete(DeleteMsg::AllDone));
            }
        }

        // ── 4. Switch active component based on status ────────────────────
        match model.status {
            AppStatus::ConfirmDelete => {
                let _ = app.active(&ComponentId::StatusBar);
            }
            _ => {
                let _ = app.active(&ComponentId::EntryList);
            }
        }

        // ── 5. Render ─────────────────────────────────────────────────────
        terminal.draw(|f| render(f, &model, &mut list_state))?;

        // ── 6. Tick tuirealm — get keyboard + tick messages ───────────────
        match app.tick(PollStrategy::Once) {
            Ok(messages) => {
                for msg in messages {
                    if !update(&mut model, msg) {
                        return Ok(());
                    }
                }
            }
            Err(_) => return Ok(()),
        }
    }
}
```

- [ ] **Step 2: Build**

```bash
cargo build 2>&1 | grep "^error"
```

Expected: no errors.

---

## Task 9: Remove `app.rs` and `ui.rs`, verify all tests pass

**Files:**
- Delete: `src/app.rs`
- Delete: `src/ui.rs`

- [ ] **Step 1: Delete old files**

```bash
rm src/app.rs src/ui.rs
```

- [ ] **Step 2: Run full test suite**

```bash
cargo nextest run 2>&1 | tail -10
```

Expected: all tests pass (entry_list, model, deleter, scanner, render format_bytes tests).

- [ ] **Step 3: Format check**

```bash
cargo fmt --check
```

If diffs: run `cargo fmt` then re-check.

- [ ] **Step 4: Smoke test — run the binary**

```bash
cargo run -- . 2>&1 | head -5
```

Expected: TUI launches, shows scanning, live clock in header ticks every second. Press `a` to select all, `d` to get yellow confirm prompt, `n` to cancel, `q` to quit.

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|-----------------|------|
| tuirealm component per UI region | Task 7 |
| Nested models (AppModel → EntryListModel → EntryModel) | Tasks 3–5 |
| Nested messages (AppMsg → ListMsg/DeleteMsg → EntryMsg) | Task 5 |
| update() is pure, owns model | Task 5 |
| thiserror IronaError | Task 1 |
| anyhow in main only | Task 8 |
| chrono current time in header | Tasks 5, 6 |
| Scan elapsed (live + final) | Tasks 5, 6 |
| Delete elapsed per row | Tasks 2, 3, 5 |
| Total delete elapsed | Tasks 5, 8 |
| Inline ✗ error display | Task 6 (render.rs) |
| d key fix (no borrow issue) | Task 7 — Component::on() owns no state ref |
| ConfirmDelete state + y/n keys | Tasks 5, 7 |
| All existing languages supported | scanner.rs unchanged |

**All spec requirements covered. No TBDs or placeholders.**
