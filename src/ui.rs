use ratatui::{
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::app::{App, CurrentScreen};

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.size());

    let title_block = Block::default().style(Style::default());
    let title = Paragraph::new(Text::styled("Scout", Style::default()))
        .block(title_block)
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    let text = Span::styled(app.search_text(), Style::default().fg(Color::Green));
    let area = center(
        chunks[1],
        Constraint::Length(text.width() as u16),
        Constraint::Length(1),
    );
    frame.render_widget(text, area);
    frame.set_cursor(area.x + app.cursor_idx() as u16, area.y);

    let current_keys_hint = {
        match app.current_screen {
            CurrentScreen::Searching => Span::styled("(esc) to quit", Style::default()),
            CurrentScreen::Confirmation => Span::styled("(esc) to quit", Style::default()),
            CurrentScreen::Results => Span::styled("(esc) to quit", Style::default()),
        }
    };

    let footer = Paragraph::new(Line::from(current_keys_hint))
        .block(Block::default())
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}
