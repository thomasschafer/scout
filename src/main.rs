use std::io;

use log::{Log, LogLevel};
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
mod log;
mod ui;
use crate::{
    app::{App, CurrentScreen},
    ui::ui,
};

fn handle_key_searching(app: &mut App, key: &KeyEvent) -> bool {
    match (key.code, key.modifiers) {
        (KeyCode::Char('w'), KeyModifiers::CONTROL) | (KeyCode::Backspace, KeyModifiers::ALT) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .delete_word_backward();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) | (KeyCode::Backspace, KeyModifiers::META) => {
            app.search_fields.highlighted_field().borrow_mut().clear();
        }
        (KeyCode::Backspace, _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .delete_char();
        }
        (KeyCode::Left, KeyModifiers::ALT) | (KeyCode::Char('b') | KeyCode::Char('B'), _)
            if key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_back_word();
        }
        (KeyCode::Home, _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_start();
        }
        (KeyCode::Left, _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_left();
        }
        (KeyCode::Right, KeyModifiers::ALT) | (KeyCode::Char('f') | KeyCode::Char('F'), _)
            if key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_forward_word();
        }
        (KeyCode::Right, KeyModifiers::META) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_end();
        }
        (KeyCode::End, _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_end();
        }
        (KeyCode::Right, _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .move_cursor_right();
        }
        (KeyCode::Char('d'), KeyModifiers::ALT) | (KeyCode::Delete, KeyModifiers::ALT) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .delete_word_forward();
        }
        (KeyCode::Delete, _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .delete_char_forward();
        }
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
        (KeyCode::Char(value), _) => {
            app.search_fields
                .highlighted_field()
                .borrow_mut()
                .enter_char(value);
        }
        _ => {}
    };
    false
}

fn handle_key_confirmation(app: &mut App, key: &KeyEvent) -> bool {
    match (key.code, key.modifiers) {
        (KeyCode::Char('j') | KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.search_results
                .search_complete_mut()
                .move_selected_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.search_results.search_complete_mut().move_selected_up();
        }
        (KeyCode::Char(' '), _) => {
            app.search_results
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
            app.search_results
                .replace_complete_mut()
                .scroll_replacement_errors_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.search_results
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
    let mut logger = Log::new(LogLevel::Info);

    loop {
        terminal.draw(|f| ui(f, app))?;
        logger.info("Redraw performed");

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }

            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL
            {
                return Ok(());
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
