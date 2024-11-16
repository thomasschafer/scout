use itertools::Itertools;
use ratatui::{
    layout::Constraint,
    layout::{Alignment, Direction, Flex, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};
use similar::{ChangeTag, TextDiff};
use std::{cmp::min, iter};

use crate::app::{
    App, CurrentScreen, FieldName, ReplaceResult, SearchField, SearchResult, NUM_SEARCH_FIELDS,
};

impl FieldName {
    pub(crate) fn title(&self) -> &str {
        match self {
            FieldName::Search => "Search text",
            FieldName::Replace => "Replace text",
            FieldName::FixedStrings => "Fixed strings",
            FieldName::FilenamePattern => "Filename pattern (regex)",
        }
    }
}

fn render_search_view(frame: &mut Frame, app: &App, rect: Rect) {
    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let areas: [Rect; NUM_SEARCH_FIELDS] =
        Layout::vertical(iter::repeat(Constraint::Length(4)).take(app.search_fields.fields.len()))
            .flex(Flex::Center)
            .areas(area);

    app.search_fields
        .fields
        .iter()
        .zip(areas)
        .enumerate()
        .for_each(|(idx, (SearchField { name, field }, field_area))| {
            field.borrow().render(
                frame,
                field_area,
                name.title().to_owned(),
                idx == app.search_fields.highlighted,
            )
        });

    let highlighted_area = areas[app.search_fields.highlighted];
    if let Some(cursor_idx) = app.search_fields.highlighted_field().borrow().cursor_idx() {
        frame.set_cursor(
            highlighted_area.x + cursor_idx as u16 + 1,
            highlighted_area.y + 1,
        )
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Diff {
    pub text: String,
    pub fg_colour: Color,
    pub bg_colour: Color,
}

fn diff_to_line<'a>(diff: Vec<Diff>) -> Line<'a> {
    let spans = diff
        .into_iter()
        .map(|d| Span::styled(d.text, Style::new().fg(d.fg_colour).bg(d.bg_colour)))
        .collect::<Vec<_>>();

    Line::from(spans)
}

pub fn line_diff<'a>(old_line: &'a str, new_line: &'a str) -> (Vec<Diff>, Vec<Diff>) {
    let diff = TextDiff::configure()
        .algorithm(similar::Algorithm::Myers)
        .timeout(std::time::Duration::from_millis(100))
        .diff_chars(old_line, new_line);

    let mut old_spans = vec![Diff {
        text: "- ".to_owned(),
        fg_colour: Color::Red,
        bg_colour: Color::Reset,
    }];
    let mut new_spans = vec![Diff {
        text: "+ ".to_owned(),
        fg_colour: Color::Green,
        bg_colour: Color::Reset,
    }];

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                old_spans.push(Diff {
                    text: change.value().to_owned(),
                    fg_colour: Color::Black,
                    bg_colour: Color::Red,
                });
            }
            ChangeTag::Insert => {
                new_spans.push(Diff {
                    text: change.value().to_owned(),
                    fg_colour: Color::Black,
                    bg_colour: Color::Green,
                });
            }
            ChangeTag::Equal => {
                old_spans.push(Diff {
                    text: change.value().to_owned(),
                    fg_colour: Color::Red,
                    bg_colour: Color::Reset,
                });
                new_spans.push(Diff {
                    text: change.value().to_owned(),
                    fg_colour: Color::Green,
                    bg_colour: Color::Reset,
                });
            }
        };
    }

    (old_spans, new_spans)
}

fn render_confirmation_view(frame: &mut Frame, app: &App, rect: Rect) {
    let [area] = Layout::horizontal([Constraint::Percentage(80)])
        .flex(Flex::Center)
        .areas(rect);
    let [num_results_area, list_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Fill(1)])
            .flex(Flex::Start)
            .areas(area);

    let complete_state = app.results.search_complete();

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
        let (old_line, new_line) = line_diff(result.line.as_str(), result.replacement.as_str());

        let file_path = format!(
            "[{}] {}:{}",
            if result.included { 'x' } else { ' ' },
            app.relative_path(result.path.clone()),
            result.line_number
        );
        let file_path_style = if complete_state.selected == idx {
            Style::new().bg(if result.included {
                Color::Blue
            } else {
                Color::Red
            })
        } else {
            Style::new()
        };

        [
            ListItem::new(Text::styled(file_path, file_path_style)),
            ListItem::new(diff_to_line(old_line)),
            ListItem::new(diff_to_line(new_line)),
            ListItem::new(""),
        ]
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

    let replace_results = app.results.replace_complete();
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
                    .skip(app.results.replace_complete().replacement_errors_pos)
                    .flatten(),
            ),
            list_area,
        );
    };
}

fn render_loading_view(text: String) -> impl Fn(&mut Frame, &App, Rect) {
    move |frame: &mut Frame, _app: &App, rect: Rect| {
        let [area] = Layout::vertical([Constraint::Length(4)])
            .flex(Flex::Center)
            .areas(rect);

        let text = Paragraph::new(Line::from(Span::raw(&text)))
            .block(Block::default())
            .alignment(Alignment::Center);

        frame.render_widget(text, area);
    }
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

type RenderFn = Box<dyn Fn(&mut Frame, &App, Rect)>;

pub fn render(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.size());

    let title_block = Block::default().style(Style::default());
    let title = Paragraph::new(Text::styled("Scooter", Style::default()))
        .block(title_block)
        .alignment(Alignment::Center);
    frame.render_widget(title, chunks[0]);

    let render_fn: RenderFn = match app.current_screen {
        CurrentScreen::Searching => Box::new(render_search_view),
        CurrentScreen::PerformingSearch => {
            Box::new(render_loading_view("Performing search...".to_owned()))
        }
        CurrentScreen::Confirmation => Box::new(render_confirmation_view),
        CurrentScreen::PerformingReplacement => {
            Box::new(render_loading_view("Performing replacement...".to_owned()))
        }
        CurrentScreen::Results => Box::new(render_results_view),
    };
    render_fn(frame, app, chunks[1]);

    let global_keys = ["<C-r> reset", "<esc> quit"];
    let current_keys = match app.current_screen {
        CurrentScreen::Searching => {
            vec!["<enter> search", "<tab> focus next", "<S-tab> focus prev"]
        }

        CurrentScreen::Confirmation => {
            vec![
                "<enter> search",
                "<space> toggle",
                "<j> down",
                "<k> up",
                "<C-o> back",
            ]
        }
        CurrentScreen::PerformingSearch
        | CurrentScreen::PerformingReplacement
        | CurrentScreen::Results => vec![],
    };
    let all_keys = current_keys.iter().chain(global_keys.iter()).join(" / ");
    let keys_hint = Span::styled(all_keys, Color::default());

    let footer = Paragraph::new(Line::from(keys_hint))
        .block(Block::default())
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}
