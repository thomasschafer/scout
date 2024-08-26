use std::{cmp::min, iter};

use ratatui::{
    crossterm::event::KeyModifiers,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, CurrentScreen, SearchResult};

fn render_search_view(frame: &mut Frame, app: &App, rect: Rect) {
    // TODO: tidy this up this repetition
    let search_block = Block::bordered().title("Search text:");
    let search = app.search_fields.search();
    let search = search.borrow();
    let search_input = Paragraph::new(search.text());

    let replace_block = Block::bordered().title("Replacement text:");
    let replace = app.search_fields.replace();
    let replace = replace.borrow();
    let replace_input = Paragraph::new(replace.text());

    let mut fields = vec![(search_block, search_input), (replace_block, replace_input)];

    fields[app.search_fields.highlighted].0 = fields[app.search_fields.highlighted]
        .0
        .clone()
        .border_style(Style::new().green());

    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let areas: [Rect; 2] = Layout::vertical(iter::repeat(Constraint::Length(3)).take(fields.len()))
        .flex(Flex::Center)
        .areas(area);

    fields
        .iter()
        .zip(areas.iter())
        .for_each(|((block, input), area)| {
            frame.render_widget(input.clone().block(block.clone()), *area);
        });

    let highlighted_area = areas[app.search_fields.highlighted];
    let cursor_idx = app.search_fields.highlighted_field().borrow().cursor_idx();

    frame.set_cursor(
        highlighted_area.x + cursor_idx as u16 + 1,
        highlighted_area.y + 1,
    );
}

fn render_confirmation_view(frame: &mut Frame, app: &App, rect: Rect) {
    let block = Block::bordered()
        .border_style(Style::new())
        .title("Text searched for:");
    let search = app.search_fields.search();
    let search = search.borrow();
    let search_input = Paragraph::new(search.text()).block(block);

    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let [search_input_area, list_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Fill(1)])
            .flex(Flex::Start)
            .areas(area);
    frame.render_widget(search_input, search_input_area);

    let complete_state = app.search_results.complete();

    let list_area_height = list_area.height as usize;
    let item_height = 4; // TODO: find a better way of doing this
    let midpoint = list_area_height / (2 * item_height);
    let num_results = complete_state.results.len();

    let results_iter = complete_state.results.iter().enumerate().skip(min(
        complete_state.selected.saturating_sub(midpoint),
        num_results.saturating_sub(list_area_height / item_height),
    ));

    let search_results = results_iter.flat_map(|(idx, result)| {
        [
            (
                format!(
                    "[{}] {}:{}",
                    if result.included { '*' } else { ' ' },
                    result.path,
                    result.line_number
                ),
                Style::default().bg(if complete_state.selected == idx {
                    Color::LightBlue
                } else {
                    Color::Reset
                }),
            ),
            (result.line.to_owned(), Style::default().fg(Color::Red)),
            (
                result.line_replaced.to_owned(),
                Style::default().fg(Color::Green),
            ),
            ("".to_owned(), Style::default()),
        ]
        .map(|(s, style)| ListItem::new(Text::styled(s, style)))
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
