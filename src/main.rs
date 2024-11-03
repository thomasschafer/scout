use app::AppEvent;
use clap::Parser;
use logging::setup_logging;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    Terminal,
};
use std::{io, path::PathBuf, str::FromStr};
use tui::Tui;

mod app;
mod event;
mod fields;
mod logging;
mod tui;
mod ui;
mod utils;
use crate::{
    app::{App, CurrentScreen},
    event::{Event, EventHandler},
};

fn handle_key_searching(app: &mut App, key: &KeyEvent) -> bool {
    app.search_fields.clear_errors();
    match (key.code, key.modifiers) {
        (KeyCode::Enter, _) => {
            app.current_screen = CurrentScreen::PerformingSearch;
            app.event_sender.send(AppEvent::PerformSearch).unwrap();
        }
        (KeyCode::BackTab, _) | (KeyCode::Tab, KeyModifiers::ALT) => {
            app.search_fields.focus_prev();
        }
        (KeyCode::Tab, _) => {
            app.search_fields.focus_next();
        }
        (code, modifiers) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .handle_keys(code, modifiers);
        }
    };
    false
}

fn handle_key_confirmation(app: &mut App, key: &KeyEvent) -> bool {
    match (key.code, key.modifiers) {
        (KeyCode::Char('j') | KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.results.search_complete_mut().move_selected_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.results.search_complete_mut().move_selected_up();
        }
        (KeyCode::Char(' '), _) => {
            app.results
                .search_complete_mut()
                .toggle_selected_inclusion();
        }
        (KeyCode::Enter, _) => {
            app.current_screen = CurrentScreen::PerformingReplacement;
            app.event_sender.send(AppEvent::PerformReplacement).unwrap();
        }
        _ => {}
    };
    false
}

fn handle_key_results(app: &mut App, key: &KeyEvent) -> bool {
    let mut exit = false;
    match (key.code, key.modifiers) {
        (KeyCode::Char('j') | KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.results
                .replace_complete_mut()
                .scroll_replacement_errors_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.results
                .replace_complete_mut()
                .scroll_replacement_errors_up();
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {} // TODO
        (KeyCode::PageDown, _) => {}                      // TODO
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {} // TODO
        (KeyCode::PageUp, _) => {}                        // TODO
        (KeyCode::Enter | KeyCode::Char('q'), _) => {
            exit = true;
        }
        _ => {}
    };
    exit
}

pub fn handle_key_events(key: KeyEvent, app: &mut App) -> anyhow::Result<bool> {
    if key.kind == KeyEventKind::Release {
        return Ok(false);
    }

    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(true),
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
            app.reset();
            return Ok(false);
        }
        (_, _) => {}
    }

    let exit = match app.current_screen {
        CurrentScreen::Searching => handle_key_searching(app, &key),
        CurrentScreen::Confirmation => handle_key_confirmation(app, &key),
        CurrentScreen::PerformingSearch | CurrentScreen::PerformingReplacement => false,
        CurrentScreen::Results => handle_key_results(app, &key),
    };
    Ok(exit)
}

#[derive(Parser, Debug)]
#[command(about = "Interactive find and replace TUI.")]
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
            Event::Key(key_event) => handle_key_events(key_event, &mut app)?,
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
