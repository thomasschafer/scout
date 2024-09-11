use std::io;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
            KeyModifiers,
        },
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};

mod app;
mod fields;
mod log;
mod ui;
use crate::{
    app::{App, CurrentScreen},
    ui::ui,
};

fn handle_key_searching(app: &mut App, key: &KeyEvent) -> bool {
    match (key.code, key.modifiers) {
        (KeyCode::Enter, _) => {
            app.current_screen = CurrentScreen::Confirmation;
            // TODO: handle the error here, e.g. from regex parse errors
            app.update_search_results()
                .expect("Failed to unwrap search results"); // TODO: make this async - currently hangs until completed
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
            app.current_screen = CurrentScreen::Results;
            app.perform_replacement(); // TODO: make this async
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
        (KeyCode::Enter, _) => {
            exit = true;
        }
        _ => {}
    };
    exit
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }

            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(()),
                (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                    app.reset();
                    continue;
                }
                (_, _) => {}
            }

            let exit = match app.current_screen {
                CurrentScreen::Searching => handle_key_searching(app, &key),
                CurrentScreen::Confirmation => handle_key_confirmation(app, &key),
                CurrentScreen::Results => handle_key_results(app, &key),
            };
            if exit {
                return Ok(());
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}
