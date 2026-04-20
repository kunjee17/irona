pub mod entry_list;
pub mod header;
pub mod status_bar;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn three_row_layout(area: Rect) -> [Rect; 3] {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);
    [chunks[0], chunks[1], chunks[2]]
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ComponentId {
    Header,
    EntryList,
    StatusBar,
}
