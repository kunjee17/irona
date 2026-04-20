pub mod entry;
pub mod entry_list;

pub use entry::DeleteState;
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
            entries: EntryListModel::default(),
        }
    }
}

// ── Nested messages ───────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum AppMsg {
    Tick,
    Quit,
    List(ListMsg),
    Delete(DeleteMsg),
}

#[derive(Debug, PartialEq)]
pub enum ListMsg {
    MoveUp,
    MoveDown,
    ToggleSelect,
    SelectAll,
    ScanFound(ArtifactEntry),
    ScanDone,
}

#[derive(Debug, PartialEq)]
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
        DeleteMsg::Result {
            index,
            elapsed,
            outcome,
        } => {
            if model.status == AppStatus::Deleting {
                model.entries.apply_delete_result(index, elapsed, outcome);
            }
        }
        DeleteMsg::AllDone => {
            if model.status == AppStatus::Deleting {
                if let Some(start) = model.delete_start {
                    model.delete_elapsed = Some(start.elapsed());
                }
                model.status = AppStatus::Done;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ArtifactEntry, Language};
    use std::path::PathBuf;

    fn entry(name: &str) -> ArtifactEntry {
        ArtifactEntry {
            path: PathBuf::from(name),
            language: Language::Rust,
            size_bytes: 100,
        }
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

    #[test]
    fn delete_result_applies_to_entry() {
        let mut m = ready_model_with_selection();
        m.status = AppStatus::Deleting;
        update(
            &mut m,
            AppMsg::Delete(DeleteMsg::Result {
                index: 0,
                elapsed: Duration::from_millis(100),
                outcome: Ok(()),
            }),
        );
        assert!(matches!(
            m.entries.entries[0].delete_state,
            DeleteState::Deleted { .. }
        ));
    }

    #[test]
    fn all_done_sets_elapsed_and_transitions_to_done() {
        let mut m = ready_model_with_selection();
        m.status = AppStatus::Deleting;
        m.delete_start = Some(Instant::now());
        update(&mut m, AppMsg::Delete(DeleteMsg::AllDone));
        assert_eq!(m.status, AppStatus::Done);
        assert!(m.delete_elapsed.is_some());
    }

    #[test]
    fn all_done_noop_when_not_deleting() {
        let mut m = AppModel::new(PathBuf::from("."));
        m.status = AppStatus::Ready;
        update(&mut m, AppMsg::Delete(DeleteMsg::AllDone));
        assert_eq!(m.status, AppStatus::Ready);
        assert!(m.delete_elapsed.is_none());
    }
}
