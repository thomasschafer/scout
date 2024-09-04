// todo
use std::{cmp::min, iter};

use ratatui::{
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, CurrentScreen, ReplaceResult, SearchResult};

fn render_search_view(frame: &mut Frame, app: &App, rect: Rect) {
    // TODO: tidy up this repetition
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
    let search_block = Block::bordered()
        .border_style(Style::new())
        .title("Search text:");
    let search = app.search_fields.search();
    let search = search.borrow();
    let search_input = Paragraph::new(search.text()).block(search_block);

    let replace_block = Block::bordered()
        .border_style(Style::new())
        .title("Replacement text:");
    let replace = app.search_fields.replace();
    let replace = replace.borrow();
    let replace_input = Paragraph::new(replace.text()).block(replace_block);

    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let [search_input_area, replace_input_area, num_results_area, list_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Fill(1),
    ])
    .flex(Flex::Start)
    .areas(area);
    frame.render_widget(search_input, search_input_area);
    frame.render_widget(replace_input, replace_input_area);

    let complete_state = app.search_results.search_complete();

    let list_area_height = list_area.height as usize;
    let item_height = 4; // TODO: find a better way of doing this
    let midpoint = list_area_height / (2 * item_height);
    let num_results = complete_state.results.len();

    frame.render_widget(
        Span::raw(format!("Results: {}", num_results)),
        num_results_area,
    );

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
                    result
                        .path
                        .clone()
                        .into_os_string()
                        .into_string()
                        .expect("Failed to display path"),
                    result.line_number
                ),
                Style::default().bg(if complete_state.selected == idx {
                    if result.included {
                        Color::Blue
                    } else {
                        Color::Red
                    }
                } else {
                    Color::Reset
                }),
            ),
            (result.line.to_owned(), Style::default().fg(Color::Red)),
            (
                result.replacement.to_owned(),
                Style::default().fg(Color::Green),
            ),
            ("".to_owned(), Style::default()),
        ]
        .map(|(s, style)| ListItem::new(Text::styled(s, style)))
    });

    frame.render_widget(List::new(search_results), list_area);
}

fn render_results_view(frame: &mut Frame, app: &App, rect: Rect) {
    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let [success_area, ignored_area, errors_area, list_title_area, list_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .flex(Flex::Start)
    .areas(area);

    let replace_results = app.search_results.replace_complete();
    let errors = replace_results
        .errors
        .iter()
        .map(|res| {
            error_result(
                res,
                match &res.replace_result {
                    Some(ReplaceResult::Error(error)) => error,
                    None => panic!("Found error result with no error message"),
                    Some(ReplaceResult::Success) => {
                        panic!("Found successful result in errors: {:?}", res)
                    }
                },
            )
        })
        .collect::<Vec<_>>();

    [
        (
            replace_results.num_successes,
            "Successful replacements:",
            success_area,
        ),
        (replace_results.num_ignored, "Ignored:", ignored_area),
        (errors.len(), "Errors:", errors_area),
    ]
    .iter()
    .for_each(|(num, title, area)| {
        frame.render_widget(
            Paragraph::new(num.to_string())
                .block(Block::bordered().border_style(Style::new()).title(*title)),
            *area,
        );
    });

    if !errors.is_empty() {
        frame.render_widget(Text::raw("Errors:"), list_title_area);
        frame.render_widget(
            List::new(
                errors
                    .into_iter()
                    .skip(app.search_results.replace_complete().replacement_errors_pos)
                    .flatten(),
            ),
            list_area,
        );
    };
}

fn error_result(result: &SearchResult, error: &str) -> [ratatui::widgets::ListItem<'static>; 3] {
    [
        ("".to_owned(), Style::default()),
        (
            format!(
                "{}:{}",
                result
                    .path
                    .clone()
                    .into_os_string()
                    .into_string()
                    .expect("Failed to display path"),
                result.line_number
            ),
            Style::default(),
        ),
        (error.to_owned(), Style::default().fg(Color::Red)),
    ]
    .map(|(s, style)| ListItem::new(Text::styled(s, style)))
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
        CurrentScreen::Results => {
            render_results_view(frame, app, chunks[1]);
        }
    }

    let current_keys_hint = {
        match app.current_screen {
            // TODO: update these
            CurrentScreen::Searching => Span::styled(
                "<enter> search / <tab> focus next / <S-tab> focus prev / <C-r> reset / <esc> quit",
                Style::default(),
            ),
            CurrentScreen::Confirmation => Span::styled(
                "<space> toggle / <j> down / <k> up / <C-r> reset / <esc> quit",
                Style::default(),
            ),
            CurrentScreen::Results => Span::styled("<C-r> reset / <esc> quit", Style::default()),
        }
    };

    let footer = Paragraph::new(Line::from(current_keys_hint))
        .block(Block::default())
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}
