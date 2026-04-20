mod components;
mod deleter;
mod errors;
mod model;
mod render;
mod scanner;

use anyhow::Result;
use clap::Parser;
use components::{
    entry_list::EntryListComponent, header::HeaderComponent, status_bar::StatusBarComponent,
    ComponentId,
};
use crossbeam_channel::unbounded;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use model::{update, AppMsg, AppStatus, DeleteMsg, ListMsg};
use ratatui::{backend::CrosstermBackend, widgets::ListState, Terminal};
use render::render;
use scanner::ScanMessage;
use std::{io, path::PathBuf, sync::mpsc, thread, time::Duration};
use tuirealm::{
    event::NoUserEvent, Application, EventListenerCfg, PollStrategy, Sub, SubClause, SubEventClause,
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
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(e.into());
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, root);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, root: PathBuf) -> Result<()> {
    let (scan_tx, scan_rx) = unbounded::<ScanMessage>();
    let root_clone = root.clone();
    thread::spawn(move || scanner::scan(root_clone, scan_tx));

    let (del_tx, del_rx) = mpsc::channel::<deleter::DeleteResult>();

    let mut model = model::AppModel::new(root);
    let mut list_state = ListState::default();

    let mut app: Application<ComponentId, AppMsg, NoUserEvent> = Application::init(
        EventListenerCfg::default()
            .crossterm_input_listener(Duration::from_millis(20), 10)
            .tick_interval(Duration::from_secs(1)),
    );

    app.mount(
        ComponentId::Header,
        Box::new(HeaderComponent),
        vec![Sub::new(SubEventClause::Tick, SubClause::Always)],
    )?;
    app.mount(ComponentId::EntryList, Box::new(EntryListComponent), vec![])?;
    app.mount(ComponentId::StatusBar, Box::new(StatusBarComponent), vec![])?;
    app.active(&ComponentId::EntryList)?;

    let rt = tokio::runtime::Runtime::new()?;
    let mut delete_pending = 0usize;
    let mut scan_done = false;

    loop {
        if !scan_done {
            loop {
                match scan_rx.try_recv() {
                    Ok(ScanMessage::Found(entry)) => {
                        update(&mut model, AppMsg::List(ListMsg::ScanFound(entry)));
                    }
                    Ok(ScanMessage::Done) => {
                        update(&mut model, AppMsg::List(ListMsg::ScanDone));
                        scan_done = true;
                        break;
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        update(&mut model, AppMsg::List(ListMsg::ScanDone));
                        scan_done = true;
                        break;
                    }
                }
            }
        }

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

        loop {
            match del_rx.try_recv() {
                Ok(result) => {
                    update(
                        &mut model,
                        AppMsg::Delete(DeleteMsg::Result {
                            index: result.index,
                            elapsed: result.elapsed,
                            outcome: result.outcome,
                        }),
                    );
                    delete_pending = delete_pending.saturating_sub(1);
                    if delete_pending == 0 {
                        update(&mut model, AppMsg::Delete(DeleteMsg::AllDone));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if delete_pending > 0 {
                        delete_pending = 0;
                        update(&mut model, AppMsg::Delete(DeleteMsg::AllDone));
                    }
                    break;
                }
            }
        }

        match model.status {
            AppStatus::ConfirmDelete => {
                let _ = app.active(&ComponentId::StatusBar);
            }
            _ => {
                let _ = app.active(&ComponentId::EntryList);
            }
        }

        terminal.draw(|f| render(f, &model, &mut list_state))?;

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
