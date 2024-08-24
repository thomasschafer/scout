use std::cmp::min;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, CurrentScreen, SearchResult};

fn render_search_view(frame: &mut Frame, app: &App, rect: Rect) {
    let block = Block::bordered()
        .border_style(Style::new().green())
        .title("Enter some text to search:");
    let search_input = Paragraph::new(app.search_text_field.text());
    let area = flex_area(
        rect,
        Constraint::Percentage(80),
        Constraint::Length(3),
        Flex::Center,
    );

    frame.render_widget(search_input.block(block), area);
    frame.set_cursor(
        area.x + app.search_text_field.cursor_idx() as u16 + 1,
        area.y + 1,
    );
}

fn render_confirmation_view(frame: &mut Frame, app: &App, rect: Rect) {
    let block = Block::bordered()
        .border_style(Style::new())
        .title("Text searched for:");
    let search_input = Paragraph::new(app.search_text_field.text()).block(block);

    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let [search_input_area, list_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)])
            .flex(Flex::Start)
            .areas(area);
    frame.render_widget(search_input, search_input_area);

    let complete_state = app.search_results.complete();
    let results_iter = complete_state.results.iter().enumerate();

    let list_area_height = list_area.height as usize;
    let midpoint = list_area_height / 2;
    let num_results = complete_state.results.len();

    let results_iter: Box<dyn Iterator<Item = (usize, &SearchResult)>> =
        if complete_state.selected > midpoint {
            Box::new(results_iter.skip(min(
                complete_state.selected - midpoint,
                num_results.saturating_sub(list_area_height),
            )))
        } else {
            Box::new(results_iter)
        };

    let search_results = results_iter.map(|(idx, result)| {
        let mut style = Style::default();
        if result.included {
            style = style.fg(Color::Green);
        }
        if complete_state.selected == idx {
            style = style.bg(Color::LightBlue);
        }
        ListItem::new(Line::from(Span::styled(
            format!("{}:{} - {}", result.path, result.line_number, result.line),
            style,
        )))
    });

    frame.render_widget(List::new(search_results), list_area);
}

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

    match app.current_screen {
        CurrentScreen::Searching => {
            render_search_view(frame, app, chunks[1]);
        }
        CurrentScreen::Confirmation => {
            render_confirmation_view(frame, app, chunks[1]);
        }
        CurrentScreen::Results => {}
    }

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

fn flex_area(
    area: Rect,
    horizontal: Constraint,
    vertical: Constraint,
    flex_vertical: Flex,
) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(flex_vertical).areas(area);
    area
}
