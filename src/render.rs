use crate::components::three_row_layout;
use crate::model::{AppModel, AppStatus, DeleteState};
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
                format!(
                    "[{}]  scanning {}...  {}s",
                    clock,
                    model.root.display(),
                    elapsed
                ),
                Style::default(),
            )
        }
        AppStatus::Ready => {
            let elapsed = model
                .scan_elapsed
                .map(|d| format!(" — scanned in {:.1}s", d.as_secs_f64()))
                .unwrap_or_default();
            (
                format!(
                    "[{}]  {} items found{}",
                    clock,
                    model.entries.entries.len(),
                    elapsed
                ),
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
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
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
                .map(|d| {
                    format!(
                        " — freed {} in {:.1}s",
                        format_bytes(model.entries.deleted_size_bytes()),
                        d.as_secs_f64()
                    )
                })
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
                DeleteState::Failed { message, elapsed } => (
                    "[✗]",
                    format!(
                        "{} ({:.1}s)",
                        message.chars().take(12).collect::<String>(),
                        elapsed.as_secs_f64()
                    ),
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
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
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
                format!(
                    "  Selected: {}   ",
                    format_bytes(model.entries.selected_size_bytes())
                ),
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
