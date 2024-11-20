#![feature(mapped_lock_guards)]

use clap::Parser;
use log::LevelFilter;
use logging::{setup_logging, DEFAULT_LOG_LEVEL};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, str::FromStr};
use tui::Tui;
use utils::validate_directory;

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

    /// Include hidden files and directories, such as those whose name starts with a dot (.)
    #[arg(short = '.', long, default_value = "false")]
    hidden: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(
        long,
        value_parser = parse_log_level,
        default_value = DEFAULT_LOG_LEVEL
    )]
    log_level: LevelFilter,
}

fn parse_log_level(s: &str) -> Result<LevelFilter, String> {
    LevelFilter::from_str(s).map_err(|_| format!("Invalid log level: {}", s))
}

// In main(), update the logging setup:
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    setup_logging(args.log_level)?;

    let args = Args::parse();

    let directory = match args.directory {
        None => None,
        Some(d) => Some(validate_directory(&d)?),
    };

    let events = EventHandler::new();
    let app_event_sender = events.app_event_sender.clone();
    let mut app = App::new(directory, args.hidden, app_event_sender);

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
            Event::App(app_event) => app.handle_event(app_event).await,
        };
        if exit {
            break;
        }
    }

    tui.exit()?;

    Ok(())
}
