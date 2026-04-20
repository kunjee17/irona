// App state is not yet wired into main — suppress dead_code for this WIP module.
#![allow(dead_code)]

use crate::scanner::ArtifactEntry;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    Scanning,
    Ready,
    ConfirmDelete,
    Deleting,
    Done,
}

#[derive(Debug)]
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
        if self.entries.is_empty() {
            return;
        }
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

    #[test]
    fn toggle_select_all_on_empty_list_is_noop() {
        let mut s = AppState::new(PathBuf::from("."));
        s.toggle_select_all(); // should not panic, selected stays empty
        assert!(s.selected.is_empty());
        s.toggle_select_all(); // still a noop
        assert!(s.selected.is_empty());
    }
}
