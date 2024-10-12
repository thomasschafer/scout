use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    rc::Rc,
};

use ignore::WalkBuilder;
use itertools::Itertools;
use regex::Regex;
use tokio::sync::mpsc;

use crate::fields::{CheckboxField, Field, TextField};

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

pub struct SearchFields {
    pub fields: Vec<SearchField>,
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

    pub fn with_values(
        search: impl Into<String>,
        replace: impl Into<String>,
        checked: bool,
    ) -> Self {
        Self {
            fields: vec![
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

impl App {
    pub fn new(directory: Option<PathBuf>, event_sender: mpsc::UnboundedSender<AppEvent>) -> App {
        let directory = match directory {
            Some(d) => d,
            None => std::env::current_dir().unwrap(),
        };

        App {
            current_screen: CurrentScreen::Searching,
            search_fields: SearchFields::with_values("", "", false),
            results: Results::Loading,
            directory, // TODO: add this as a field that can be edited

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
                self.update_search_results()
                    .expect("Failed to unwrap search results"); // TODO: handle?
                self.current_screen = CurrentScreen::Confirmation;
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

    pub fn update_search_results(&mut self) -> anyhow::Result<()> {
        let pattern = match self.search_fields.search_type() {
            Err(e) => {
                if e.downcast_ref::<regex::Error>().is_some() {
                    self.search_fields
                        .search()
                        .set_error("Couldn't parse regex".to_owned());
                    return Ok(());
                } else {
                    return Err(e);
                }
            }
            Ok(p) => p,
        };

        self.current_screen = CurrentScreen::Confirmation;

        let mut results = vec![];

        let walker = WalkBuilder::new(&self.directory).build();
        for entry in walker.flatten() {
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                let path = entry.path();
                let file = match File::open(path) {
                    Ok(file) => file,
                    Err(_err) => {
                        // TODO: log the error here
                        continue;
                    }
                };
                let reader = BufReader::new(file);

                for (line_number, line) in reader.lines().enumerate() {
                    match line {
                        Ok(line) => {
                            let maybe_replacement = match pattern {
                                SearchType::Fixed(ref s) => {
                                    if line.contains(s) {
                                        Some(line.replace(
                                            s,
                                            self.search_fields.replace().text().as_str(),
                                        ))
                                    } else {
                                        None
                                    }
                                }
                                SearchType::Pattern(ref p) => {
                                    if p.is_match(&line) {
                                        Some(
                                            p.replace_all(
                                                &line,
                                                self.search_fields.replace().text(),
                                            )
                                            .to_string(),
                                        )
                                    } else {
                                        None
                                    }
                                }
                            };

                            if let Some(replacement) = maybe_replacement {
                                results.push(SearchResult {
                                    path: entry.path().to_path_buf(),
                                    line_number: line_number + 1,
                                    line: line.clone(),
                                    replacement,
                                    included: true,
                                    replace_result: None,
                                });
                            }
                        }
                        Err(_err) => {
                            // TODO: log the error here
                            continue;
                        }
                    }
                }
            }
        }

        // thread::sleep(time::Duration::from_secs(2)); // TODO: use this to verify loading state

        self.results = Results::SearchComplete(SearchState {
            results,
            selected: 0,
        });

        Ok(())
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
}
