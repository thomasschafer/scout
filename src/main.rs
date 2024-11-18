use clap::Parser;
use logging::setup_logging;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf, str::FromStr};
use tui::Tui;

use crate::{
    app::App,
    event::{Event, EventHandler},
};

mod app;
mod event;
mod fields;
mod logging;
mod tui;
mod ui;
mod utils;

#[derive(Parser, Debug)]
#[command(about = "Interactive find and replace TUI.")]
#[command(version)]
struct Args {
    /// Directory in which to search
    #[arg(index = 1)]
    directory: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging()?;

    let args = Args::parse();

    let directory = match args.directory {
        None => None,
        Some(d) => Some(PathBuf::from_str(d.as_str())?),
    };

    let events = EventHandler::new();
    let app_event_sender = events.app_event_sender.clone();
    let mut app = App::new(directory, app_event_sender);

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let mut tui = Tui::new(terminal, events);
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;
        let exit = match tui.events.next().await? {
            Event::Key(key_event) => app.handle_key_events(&key_event)?,
            Event::Mouse(_) => false,
            Event::Resize(_, _) => false,
            Event::App(app_event) => app.handle_event(app_event),
        };
        if exit {
            break;
        }
    }

    tui.exit()?;

    Ok(())
}
