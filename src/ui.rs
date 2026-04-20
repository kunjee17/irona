use crate::app::{AppState, AppStatus};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

fn header_style(status: &AppStatus) -> Style {
    match status {
        AppStatus::ConfirmDelete => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        AppStatus::Deleting => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

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
        Paragraph::new(Span::styled(header_text, header_style(&state.status)))
            .block(Block::default().borders(Borders::ALL).title(" irona ")),
        chunks[0],
    );

    // List
    let items: Vec<ListItem> = state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let check = if state.selected.contains(&i) {
                "[✓]"
            } else {
                "[ ]"
            };
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
    let hint = match &state.status {
        AppStatus::ConfirmDelete => Span::styled(
            "  y  confirm delete    n / Esc  cancel",
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
                format!("Selected: {}   ", format_bytes(state.selected_size_bytes())),
                Style::default().fg(Color::Green),
            ),
            hint,
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
