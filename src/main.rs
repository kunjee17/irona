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

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, root: PathBuf) -> anyhow::Result<()> {
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
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    state.mark_scan_done();
                    break;
                }
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
                        let results = rt.block_on(deleter::delete_all(paths));
                        for r in &results {
                            if !r.success {
                                if let Some(ref err) = r.error {
                                    eprintln!("failed to delete {}: {}", r.path.display(), err);
                                }
                            }
                        }
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
