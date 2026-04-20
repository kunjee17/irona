# irona — Elm Architecture Refactor Design

**Date:** 2026-04-20  
**Status:** Approved  
**Branch:** fix/delete-and-format

---

## Motivation

The current `main.rs` event loop uses `match (&state.status, key.code)` which holds an immutable borrow on `state.status` for the whole match expression. Keys that mutate `state.status` (specifically `d`) silently do nothing because of the conflicting borrow. Rather than patch around it, we refactor to the Elm Architecture using tui-realm, which gives correct message routing, eliminates borrow tangles, and provides a clean foundation for future features.

---

## Architecture: Elm Pattern

Each UI region is a **Component** with its own **Model**, **Msg**, and **update** function. The parent routes messages down — it never handles child concerns directly. This is the tui-realm component model applied to irona.

```
Application (tui-realm runtime)
├── HeaderComponent          — clock, scan status, scan elapsed
├── EntryListComponent       — scrollable list, delegates per-row
│   └── EntryRowComponent×N  — one per ArtifactEntry
└── StatusBarComponent       — hints / confirm prompt / delete progress
```

---

## Nested Models

### AppModel (top-level)

```rust
struct AppModel {
    status: AppStatus,
    root: PathBuf,
    clock: DateTime<Local>,
    scan_start: Instant,
    scan_elapsed: Option<Duration>,
    delete_start: Option<Instant>,
    delete_elapsed: Option<Duration>,
    entry_list: EntryListModel,
}

enum AppStatus {
    Scanning,
    Ready,
    ConfirmDelete,
    Deleting,
    Done,
}
```

### EntryListModel

```rust
struct EntryListModel {
    entries: Vec<EntryModel>,
    cursor: usize,
}
```

### EntryModel (per-row)

```rust
struct EntryModel {
    entry: ArtifactEntry,
    selected: bool,
    delete_state: DeleteState,
}

enum DeleteState {
    Pending,
    Deleted { elapsed: Duration },
    Failed { error: IronaError, elapsed: Duration },
}
```

---

## Nested Messages

```rust
// Top-level — app-wide concerns only
enum AppMsg {
    Tick,                          // 1s subscription — drives clock & elapsed
    Quit,
    EntryList(EntryListMsg),       // routes to EntryListModel::update
    DeleteBar(DeleteBarMsg),       // routes to confirm/delete logic
}

// List component
enum EntryListMsg {
    ScanFound(ArtifactEntry),
    ScanDone,
    MoveUp,
    MoveDown,
    ToggleSelect,
    SelectAll,
    Entry(usize, EntryMsg),        // routes to EntryModel::update
}

// Per-row
enum EntryMsg {
    DeleteResult(Result<Duration, IronaError>),
}

// Confirm/delete bar
enum DeleteBarMsg {
    Request,                       // d key → ConfirmDelete
    ConfirmYes,
    ConfirmNo,
    AllDone { elapsed: Duration },
}
```

`AppModel::update` pattern-matches on `AppMsg` and routes — never handles list navigation directly. `EntryListModel::update` handles navigation and fans out `EntryMsg` to individual rows.

---

## Components & View

### HeaderComponent
- Renders: current time (`clock`), scan status, scan elapsed once done
- Subscribes to: `Tick`

### EntryListComponent
- Renders: scrollable list of `EntryRowComponent`
- Each row renders based on `DeleteState`:
  - `Pending` — `[ ]` or `[✓]` + path + size
  - `Deleted` — `[✓]` green + "deleted in 0.3s"
  - `Failed` — `[✗]` red + inline error message (replaces size column)
- Subscribes to: keyboard events (↑↓ Space a)
- `d` key emits `AppMsg::DeleteBar(DeleteBarMsg::Request)` — routed by the app, not handled inside the list

### StatusBarComponent
- Swaps content based on `AppStatus`:
  - `Scanning | Ready` — normal key hints
  - `ConfirmDelete` — yellow "Delete N folders (X GB)? y / Esc cancel"
  - `Deleting` — red "deleting… elapsed Xs"
  - `Done` — green "done — freed X GB in Xs"

---

## Error Types

```rust
// src/errors.rs
#[derive(Debug, thiserror::Error)]
enum IronaError {
    #[error("delete failed: {0}")]
    DeleteFailed(#[from] std::io::Error),

    #[error("scan error: {0}")]
    ScanError(String),
}
```

- `thiserror` for typed errors at the library boundary
- `anyhow` in `main.rs` only — top-level `?` chain
- No other module uses `anyhow` directly

---

## Timing

| What | How |
|------|-----|
| Current time | `chrono::Local::now()` updated on every `Tick` |
| Scan elapsed (live) | `scan_start.elapsed()` rendered on every `Tick` while `Scanning` |
| Scan elapsed (final) | `scan_elapsed: Some(Duration)` set on `ScanDone` |
| Delete elapsed per row | `Instant` captured at delete start, stored in `DeleteState::Deleted/Failed` |
| Total delete elapsed | `delete_start.elapsed()` stored on `DeleteAllDone` |

---

## New Dependencies

```toml
thiserror = "1"
chrono = { version = "0.4", features = ["clock"] }
tui-realm = "2"
```

`anyhow` already present.

---

## Module Structure

```
src/
├── main.rs              — tui-realm Application setup, top-level run loop
├── errors.rs            — IronaError (thiserror)
├── scanner.rs           — unchanged
├── deleter.rs           — returns (Duration, Result) per path
├── model/
│   ├── mod.rs           — AppModel, AppMsg, AppStatus
│   ├── entry_list.rs    — EntryListModel, EntryListMsg
│   └── entry.rs         — EntryModel, EntryMsg, DeleteState
└── components/
    ├── header.rs        — HeaderComponent
    ├── entry_list.rs    — EntryListComponent
    └── status_bar.rs    — StatusBarComponent
```

`app.rs` and `ui.rs` are replaced by `model/` and `components/` respectively.

---

## What Does NOT Change

- `scanner.rs` — walkdir traversal, language detection, size calc, crossbeam channel
- `deleter.rs` — only change is returning `Duration` alongside the result
- CLI (`clap`) — same `irona [path]` interface
- TUI layout — same three-row split (header / list / footer)
- Supported languages — Rust, Node.js, C# (v1 scope unchanged)
