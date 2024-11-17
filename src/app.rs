use ignore::WalkBuilder;
use itertools::Itertools;
use log::{info, warn};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use regex::Regex;
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    rc::Rc,
};
use tokio::sync::mpsc;

use crate::{
    fields::{CheckboxField, Field, TextField},
    utils::replace_start,
};

#[derive(Debug, Eq, PartialEq)]
pub enum CurrentScreen {
    Searching,
    PerformingSearch,
    Confirmation,
    PerformingReplacement,
    Results,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplaceResult {
    Success,
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
    pub replacement: String,
    pub included: bool,
    pub replace_result: Option<ReplaceResult>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SearchState {
    pub results: Vec<SearchResult>,
    pub selected: usize, // TODO: allow for selection of ranges
}

impl SearchState {
    pub fn move_selected_up(&mut self) {
        if self.selected == 0 {
            self.selected = self.results.len();
        }
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_selected_down(&mut self) {
        if self.selected >= self.results.len().saturating_sub(1) {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    pub fn toggle_selected_inclusion(&mut self) {
        if self.selected < self.results.len() {
            let selected_result = &mut self.results[self.selected];
            selected_result.included = !selected_result.included;
        } else {
            self.selected = self.results.len().saturating_sub(1);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReplaceState {
    pub num_successes: usize,
    pub num_ignored: usize,
    pub errors: Vec<SearchResult>,
    pub replacement_errors_pos: usize,
}

impl ReplaceState {
    pub fn scroll_replacement_errors_up(&mut self) {
        if self.replacement_errors_pos == 0 {
            self.replacement_errors_pos = self.errors.len();
        }
        self.replacement_errors_pos = self.replacement_errors_pos.saturating_sub(1);
    }

    pub fn scroll_replacement_errors_down(&mut self) {
        if self.replacement_errors_pos >= self.errors.len().saturating_sub(1) {
            self.replacement_errors_pos = 0;
        } else {
            self.replacement_errors_pos += 1;
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Results {
    Loading,
    SearchComplete(SearchState),
    ReplaceComplete(ReplaceState),
}

impl Results {
    fn name(&self) -> String {
        match self {
            Self::Loading => "Loading",
            Self::SearchComplete(_) => "SearchComplete",
            Self::ReplaceComplete(_) => "ReplaceComplete",
        }
        .to_owned()
    }
}

macro_rules! complete_state_impl {
    ($self:ident, $variant:ident) => {
        match $self {
            Results::$variant(state) => state,
            _ => {
                panic!("Expected {}, found {}", stringify!($variant), $self.name())
            }
        }
    };
}

impl Results {
    pub fn search_complete(&self) -> &SearchState {
        complete_state_impl!(self, SearchComplete)
    }

    pub fn search_complete_mut(&mut self) -> &mut SearchState {
        complete_state_impl!(self, SearchComplete)
    }

    pub fn replace_complete(&self) -> &ReplaceState {
        complete_state_impl!(self, ReplaceComplete)
    }

    pub fn replace_complete_mut(&mut self) -> &mut ReplaceState {
        complete_state_impl!(self, ReplaceComplete)
    }
}

#[derive(PartialEq)]
pub enum FieldName {
    Search,
    Replace,
    FixedStrings,
    FilenamePattern,
}

pub struct SearchField {
    pub name: FieldName,
    pub field: Rc<RefCell<Field>>,
}

impl SearchField {
    #[allow(dead_code)] // TODO: use
    fn set_error(&mut self, error: String) {
        self.field.borrow_mut().set_error(error);
    }
}

pub const NUM_SEARCH_FIELDS: usize = 4; // needed because Ratatui .areas method returns an array

pub struct SearchFields {
    pub fields: [SearchField; NUM_SEARCH_FIELDS],
    pub highlighted: usize,
}

// TODO: add non-mutable versions
macro_rules! define_field_accessor {
    ($method_name:ident, $field_name:expr, $field_variant:ident, $return_type:ty) => {
        pub fn $method_name(&self) -> RefMut<'_, $return_type> {
            self.fields
                .iter()
                .find(|SearchField { name, .. }| *name == $field_name)
                .and_then(|SearchField { field, .. }| {
                    RefMut::filter_map(field.borrow_mut(), |f| {
                        if let Field::$field_variant(inner) = f {
                            Some(inner)
                        } else {
                            None
                        }
                    })
                    .ok()
                })
                .expect("Couldn't find field")
        }
    };
}

impl SearchFields {
    define_field_accessor!(search, FieldName::Search, Text, TextField);
    define_field_accessor!(replace, FieldName::Replace, Text, TextField);
    define_field_accessor!(
        fixed_strings,
        FieldName::FixedStrings,
        Checkbox,
        CheckboxField
    );
    define_field_accessor!(
        filename_pattern,
        FieldName::FilenamePattern,
        Text,
        TextField
    );

    pub fn focus_next(&mut self) {
        self.highlighted = (self.highlighted + 1) % self.fields.len();
    }

    pub fn focus_prev(&mut self) {
        self.highlighted =
            (self.highlighted + self.fields.len().saturating_sub(1)) % self.fields.len();
    }

    pub fn highlighted_field(&self) -> &Rc<RefCell<Field>> {
        &self.fields[self.highlighted].field
    }

    pub fn search_type(&self) -> anyhow::Result<SearchType> {
        let search = self.search();
        let search_text = search.text();
        let result = if self.fixed_strings().checked {
            SearchType::Fixed(search_text)
        } else {
            SearchType::Pattern(Regex::new(&search_text)?)
        };
        Ok(result)
    }

    pub fn clear_errors(&mut self) {
        self.fields.iter().for_each(|field| {
            field.field.borrow_mut().clear_error();
        });
    }

    pub fn with_values(
        search: impl Into<String>,
        replace: impl Into<String>,
        checked: bool,
        filname_pattern: impl Into<String>,
    ) -> Self {
        Self {
            fields: [
                SearchField {
                    name: FieldName::Search,
                    field: Rc::new(RefCell::new(Field::text(search.into()))),
                },
                SearchField {
                    name: FieldName::Replace,
                    field: Rc::new(RefCell::new(Field::text(replace.into()))),
                },
                SearchField {
                    name: FieldName::FixedStrings,
                    field: Rc::new(RefCell::new(Field::checkbox(checked))),
                },
                SearchField {
                    name: FieldName::FilenamePattern,
                    field: Rc::new(RefCell::new(Field::text(filname_pattern.into()))),
                },
            ],
            highlighted: 0,
        }
    }
}

pub enum SearchType {
    Pattern(Regex),
    Fixed(String),
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Rerender,
    PerformSearch,
    PerformReplacement,
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub search_fields: SearchFields,
    pub results: Results,
    pub directory: PathBuf,

    pub running: bool,
    pub event_sender: mpsc::UnboundedSender<AppEvent>,
}

const BINARY_EXTENSIONS: &[&str] = &["png", "gif", "jpg", "jpeg", "ico", "svg", "pdf"];

impl App {
    pub fn new(directory: Option<PathBuf>, event_sender: mpsc::UnboundedSender<AppEvent>) -> App {
        let directory = match directory {
            Some(d) => d,
            None => std::env::current_dir().unwrap(),
        };

        App {
            current_screen: CurrentScreen::Searching,
            search_fields: SearchFields::with_values("", "", false, ""),
            results: Results::Loading,
            directory, // TODO: add this as a field that can be edited, e.g. allow glob patterns

            running: true,
            event_sender,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(Some(self.directory.clone()), self.event_sender.clone());
    }

    pub fn handle_event(&mut self, event: AppEvent) -> bool {
        match event {
            AppEvent::Rerender => {}
            AppEvent::PerformSearch => {
                let continue_to_confirmation = self
                    .update_search_results()
                    .expect("Failed to unwrap search results");
                self.current_screen = if continue_to_confirmation {
                    CurrentScreen::Confirmation
                } else {
                    CurrentScreen::Searching
                };
                self.event_sender.send(AppEvent::Rerender).unwrap();
            }
            AppEvent::PerformReplacement => {
                self.perform_replacement();
                self.current_screen = CurrentScreen::Results;
                self.event_sender.send(AppEvent::Rerender).unwrap();
            }
        };
        false
    }

    fn handle_key_searching(&mut self, key: &KeyEvent) -> bool {
        self.search_fields.clear_errors();
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => {
                self.current_screen = CurrentScreen::PerformingSearch;
                self.event_sender.send(AppEvent::PerformSearch).unwrap();
            }
            (KeyCode::BackTab, _) | (KeyCode::Tab, KeyModifiers::ALT) => {
                self.search_fields.focus_prev();
            }
            (KeyCode::Tab, _) => {
                self.search_fields.focus_next();
            }
            (code, modifiers) => {
                self.search_fields
                    .highlighted_field()
                    .borrow_mut()
                    .handle_keys(code, modifiers);
            }
        };
        false
    }

    fn handle_key_confirmation(&mut self, key: &KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('j') | KeyCode::Down, _)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.results.search_complete_mut().move_selected_down();
            }
            (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.results.search_complete_mut().move_selected_up();
            }
            (KeyCode::Char(' '), _) => {
                self.results
                    .search_complete_mut()
                    .toggle_selected_inclusion();
            }
            (KeyCode::Enter, _) => {
                self.current_screen = CurrentScreen::PerformingReplacement;
                self.event_sender
                    .send(AppEvent::PerformReplacement)
                    .unwrap();
            }
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.current_screen = CurrentScreen::Searching;
                self.event_sender.send(AppEvent::Rerender).unwrap();
            }
            _ => {}
        };
        false
    }

    fn handle_key_results(&mut self, key: &KeyEvent) -> bool {
        let mut exit = false;
        match (key.code, key.modifiers) {
            (KeyCode::Char('j') | KeyCode::Down, _)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.results
                    .replace_complete_mut()
                    .scroll_replacement_errors_down();
            }
            (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.results
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

    pub fn handle_key_events(&mut self, key: &KeyEvent) -> anyhow::Result<bool> {
        if key.kind == KeyEventKind::Release {
            return Ok(false);
        }

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(true),
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                self.reset();
                return Ok(false);
            }
            (_, _) => {}
        }

        let exit = match self.current_screen {
            CurrentScreen::Searching => self.handle_key_searching(key),
            CurrentScreen::Confirmation => self.handle_key_confirmation(key),
            CurrentScreen::PerformingSearch | CurrentScreen::PerformingReplacement => false,
            CurrentScreen::Results => self.handle_key_results(key),
        };
        Ok(exit)
    }

    pub fn update_search_results(&mut self) -> anyhow::Result<bool> {
        let pattern = match self.search_fields.search_type() {
            Err(e) => {
                if e.downcast_ref::<regex::Error>().is_some() {
                    info!("Error when parsing search regex {}", e);
                    self.search_fields
                        .search()
                        .set_error("Couldn't parse regex".to_owned());
                    return Ok(false);
                } else {
                    return Err(e);
                }
            }
            Ok(p) => p,
        };

        self.current_screen = CurrentScreen::Confirmation;

        let mut results = vec![];

        let s = self.search_fields.filename_pattern().text();
        let patt = if s.is_empty() {
            None
        } else {
            match Regex::new(s.as_str()) {
                Err(e) => {
                    info!("Error when parsing filname pattern regex {}", e);
                    self.search_fields
                        .filename_pattern()
                        .set_error("Couldn't parse regex".to_owned());
                    return Ok(false);
                }
                Ok(r) => Some(r),
            }
        };

        let paths: Vec<_> = WalkBuilder::new(&self.directory)
            .build()
            .flatten()
            .filter(|entry| entry.file_type().map_or(false, |ft| ft.is_file()))
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| {
                patt.as_ref().map_or(true, |p| {
                    p.is_match(self.relative_path(path.clone()).as_str())
                })
            })
            .collect();

        for path in paths {
            if self.ignore_file(&path) {
                continue;
            }

            match File::open(path.clone()) {
                Ok(file) => {
                    let reader = BufReader::new(file);

                    for (line_number, line) in reader.lines().enumerate() {
                        match line {
                            Ok(line) => {
                                if let Some(res) = self.replacement_if_match(
                                    &pattern,
                                    line,
                                    path.clone(),
                                    line_number,
                                ) {
                                    results.push(res);
                                };
                            }
                            Err(err) => {
                                warn!("Error retrieving line {} of {:?}: {err}", line_number, path);
                            }
                        }
                    }
                }
                Err(err) => {
                    warn!("Error opening file {:?}: {err}", path);
                }
            }
        }

        self.results = Results::SearchComplete(SearchState {
            results,
            selected: 0,
        });

        Ok(true)
    }

    fn replacement_if_match(
        &mut self,
        pattern: &SearchType,
        line: String,
        path: PathBuf,
        line_number: usize,
    ) -> Option<SearchResult> {
        let maybe_replacement = match *pattern {
            SearchType::Fixed(ref s) => {
                if line.contains(s) {
                    Some(line.replace(s, self.search_fields.replace().text().as_str()))
                } else {
                    None
                }
            }
            SearchType::Pattern(ref p) => {
                if p.is_match(&line) {
                    Some(
                        p.replace_all(&line, self.search_fields.replace().text())
                            .to_string(),
                    )
                } else {
                    None
                }
            }
        };

        maybe_replacement.map(|replacement| SearchResult {
            path,
            line_number: line_number + 1,
            line: line.clone(),
            replacement,
            included: true,
            replace_result: None,
        })
    }

    pub fn perform_replacement(&mut self) {
        for (path, results) in &self
            .results
            .search_complete_mut()
            .results
            .iter_mut()
            .filter(|res| res.included)
            .chunk_by(|res| res.path.clone())
        {
            let mut results = results.collect::<Vec<_>>();
            if let Err(file_err) = Self::replace_in_file(path, &mut results) {
                results.iter_mut().for_each(|res| {
                    res.replace_result = Some(ReplaceResult::Error(file_err.to_string()))
                });
            }
        }

        // TODO (test): add tests for this
        let mut num_successes = 0;
        let mut num_ignored = 0;
        let mut errors = vec![];

        self.results
            .search_complete()
            .results
            .iter()
            .for_each(|res| match (res.included, &res.replace_result) {
                (false, _) => {
                    num_ignored += 1;
                }
                (_, Some(ReplaceResult::Success)) => {
                    num_successes += 1;
                }
                (_, None) => {
                    let mut res = res.clone();
                    res.replace_result = Some(ReplaceResult::Error(
                        "Failed to find search result in file".to_owned(),
                    ));
                    errors.push(res);
                }
                (_, Some(ReplaceResult::Error(_))) => {
                    errors.push(res.clone());
                }
            });

        self.results = Results::ReplaceComplete(ReplaceState {
            num_successes,
            num_ignored,
            errors,
            replacement_errors_pos: 0,
        });
    }

    fn replace_in_file(
        file_path: PathBuf,
        results: &mut [&mut SearchResult],
    ) -> anyhow::Result<()> {
        let mut line_map: HashMap<_, _> =
            HashMap::from_iter(results.iter_mut().map(|res| (res.line_number, res)));

        let input = File::open(file_path.clone())?;
        let buffered = BufReader::new(input);

        let temp_file_path = file_path.with_extension("tmp");
        let output = File::create(temp_file_path.clone())?;
        let mut writer = BufWriter::new(output);

        for (index, line) in buffered.lines().enumerate() {
            let mut line = line?;
            if let Some(res) = line_map.get_mut(&(index + 1)) {
                if line == res.line {
                    line.clone_from(&res.replacement);
                    res.replace_result = Some(ReplaceResult::Success);
                } else {
                    res.replace_result = Some(ReplaceResult::Error(
                        "File changed since last search".to_owned(),
                    ));
                }
            }
            writeln!(writer, "{}", line)?;
        }

        writer.flush()?;
        fs::rename(temp_file_path, file_path)?;
        Ok(())
    }

    pub fn relative_path(self: &App, path: PathBuf) -> String {
        let current_dir = self.directory.to_str().unwrap();
        let path = path
            .into_os_string()
            .into_string()
            .expect("Failed to display path");
        replace_start(path, current_dir, ".")
    }

    fn ignore_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                if BINARY_EXTENSIONS.contains(&ext_str.to_lowercase().as_str()) {
                    return true;
                }
            }
        }
        false
    }
}
