use std::io;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
            KeyModifiers,
        },
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};

mod app;
mod ui;
use crate::{
    app::{App, CurrentScreen},
    ui::ui,
};

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                // Skip events that are not KeyEventKind::Press
                continue;
            }

            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL
            {
                return Ok(());
            }

            match app.current_screen {
                CurrentScreen::Searching => match (key.code, key.modifiers) {
                    (KeyCode::Char('w'), KeyModifiers::CONTROL)
                    | (KeyCode::Backspace, KeyModifiers::ALT) => {
                        while let Some(' ') = app.search_text.pop() {}
                        loop {
                            match app.search_text.pop() {
                                Some(' ') | None => {
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    (KeyCode::Char('u'), KeyModifiers::CONTROL)
                    | (KeyCode::Backspace, KeyModifiers::META) => {
                        app.search_text.clear();
                    }
                    (KeyCode::Char(value), _) => {
                        app.search_text.push(value);
                    }
                    (KeyCode::Backspace, _) => {
                        app.search_text.pop();
                    }
                    _ => {}
                },
                CurrentScreen::Confirmation => match key.code {
                    _ => {}
                },
                CurrentScreen::Results => match key.code {
                    _ => {}
                },
                _ => {}
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
