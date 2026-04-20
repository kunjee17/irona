use crate::errors::IronaError;
use crate::scanner::ArtifactEntry;
use std::path::PathBuf;
use std::time::Duration;

use super::entry::{EntryModel, EntryMsg};

pub struct EntryListModel {
    pub entries: Vec<EntryModel>,
    pub cursor: usize,
}

impl Default for EntryListModel {
    fn default() -> Self {
        Self::new()
    }
}

impl EntryListModel {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
        }
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

    pub fn deleted_size_bytes(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| matches!(e.delete_state, super::entry::DeleteState::Deleted { .. }))
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
            let msg = EntryMsg::DeleteResult { elapsed, outcome };
            row.apply(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{ArtifactEntry, Language};
    use std::path::PathBuf;

    fn make_entry(name: &str) -> ArtifactEntry {
        ArtifactEntry {
            path: PathBuf::from(name),
            language: Language::Rust,
            size_bytes: 100,
        }
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
        m.add(ArtifactEntry {
            path: PathBuf::from("a"),
            language: Language::Rust,
            size_bytes: 100,
        });
        m.add(ArtifactEntry {
            path: PathBuf::from("b"),
            language: Language::Rust,
            size_bytes: 200,
        });
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
