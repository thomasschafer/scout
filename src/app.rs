use ignore::{WalkBuilder, WalkState};
use itertools::Itertools;
use log::{error, info};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use regex::Regex;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    mem,
    path::{Path, PathBuf},
    sync::{
        Arc, MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard,
        RwLockWriteGuard,
    },
    time::{Duration, Instant},
};
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};

use crate::{
    event::{AppEvent, ReplaceResult, SearchResult},
    fields::{CheckboxField, Field, TextField},
    parsed_fields::{ParsedFields, SearchType},
    utils::relative_path_from,
    EventHandlingResult,
};

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

#[derive(Debug)]
pub struct SearchInProgressState {
    pub search_state: SearchState,
    pub last_render: Instant,
    pub handle: JoinHandle<()>,
}

// #[derive(Debug)]
// pub enum Results {
//     NotStarted,
//     SearchInProgress(SearchInProgressState),
//     SearchComplete(SearchState),
//     ReplaceComplete(ReplaceState),
// }

#[derive(Debug)]
pub enum CurrentScreen {
    Search,
    ConfirmationSearchProgressing(SearchInProgressState),
    ConfirmationSearchComplete(SearchState),
    PerformingReplacement,
    Results(ReplaceState),
}

impl CurrentScreen {
    pub fn name(&self) -> String {
        match self {
            CurrentScreen::Search => "Search",
            CurrentScreen::ConfirmationSearchProgressing(_) => "ConfirmationSearchProgressing",
            CurrentScreen::ConfirmationSearchComplete(_) => "ConfirmationSearchComplete",
            CurrentScreen::PerformingReplacement => "PerformingReplacement",
            CurrentScreen::Results(_) => "Results",
        }
        .to_owned()
    }

    // pub fn search_results_mut(&mut self) -> &mut SearchState {
    //     match self {
    //         Results::SearchInProgress(SearchInProgressState { search_state, .. }) => search_state,
    //         Results::SearchComplete(search_state) => search_state,
    //         _ => panic!(
    //             "Expected SearchInProgress or SearchComplete, found {}",
    //             self.name()
    //         ),
    //     }
    // }
}

#[derive(PartialEq)]
pub enum FieldName {
    Search,
    Replace,
    FixedStrings,
    PathPattern,
}

pub struct SearchField {
    pub name: FieldName,
    pub field: Arc<RwLock<Field>>,
}

pub const NUM_SEARCH_FIELDS: usize = 4;

pub struct SearchFields {
    pub fields: [SearchField; NUM_SEARCH_FIELDS],
    pub highlighted: usize,
}

macro_rules! define_field_accessor {
    ($method_name:ident, $field_name:expr, $field_variant:ident, $return_type:ty) => {
        pub fn $method_name(&self) -> MappedRwLockReadGuard<'_, $return_type> {
            let field = self
                .fields
                .iter()
                .find(|SearchField { name, .. }| *name == $field_name)
                .expect("Couldn't find field");

            RwLockReadGuard::map(
                field.field.read().expect("Failed to acquire read lock"),
                |f| {
                    if let Field::$field_variant(ref inner) = f {
                        inner
                    } else {
                        panic!("Incorrect field type")
                    }
                },
            )
        }
    };
}

macro_rules! define_field_accessor_mut {
    ($method_name:ident, $field_name:expr, $field_variant:ident, $return_type:ty) => {
        pub fn $method_name(&self) -> MappedRwLockWriteGuard<'_, $return_type> {
            let field = self
                .fields
                .iter()
                .find(|SearchField { name, .. }| *name == $field_name)
                .expect("Couldn't find field");

            RwLockWriteGuard::map(
                field.field.write().expect("Failed to acquire write lock"),
                |f| {
                    if let Field::$field_variant(ref mut inner) = f {
                        inner
                    } else {
                        panic!("Incorrect field type")
                    }
                },
            )
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
    define_field_accessor!(path_pattern, FieldName::PathPattern, Text, TextField);

    define_field_accessor_mut!(search_mut, FieldName::Search, Text, TextField);
    define_field_accessor_mut!(path_pattern_mut, FieldName::PathPattern, Text, TextField);

    pub fn with_values(
        search: impl Into<String>,
        replace: impl Into<String>,
        fixed_strings: bool,
        filename_pattern: impl Into<String>,
    ) -> Self {
        Self {
            fields: [
                SearchField {
                    name: FieldName::Search,
                    field: Arc::new(RwLock::new(Field::text(search.into()))),
                },
                SearchField {
                    name: FieldName::Replace,
                    field: Arc::new(RwLock::new(Field::text(replace.into()))),
                },
                SearchField {
                    name: FieldName::FixedStrings,
                    field: Arc::new(RwLock::new(Field::checkbox(fixed_strings))),
                },
                SearchField {
                    name: FieldName::PathPattern,
                    field: Arc::new(RwLock::new(Field::text(filename_pattern.into()))),
                },
            ],
            highlighted: 0,
        }
    }

    pub fn highlighted_field(&self) -> &Arc<RwLock<Field>> {
        &self.fields[self.highlighted].field
    }

    pub fn focus_next(&mut self) {
        self.highlighted = (self.highlighted + 1) % self.fields.len();
    }

    pub fn focus_prev(&mut self) {
        self.highlighted =
            (self.highlighted + self.fields.len().saturating_sub(1)) % self.fields.len();
    }

    pub fn clear_errors(&mut self) {
        self.fields
            .iter_mut()
            .try_for_each(|field| {
                field
                    .field
                    .write()
                    .map(|mut f| f.clear_error())
                    .map_err(|e| format!("Failed to clear error: {}", e))
            })
            .expect("Failed to clear field errors");
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
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub search_fields: SearchFields,
    pub directory: PathBuf,
    pub include_hidden: bool,

    pub running: bool,
    pub app_event_sender: UnboundedSender<AppEvent>,
}

const BINARY_EXTENSIONS: &[&str] = &["png", "gif", "jpg", "jpeg", "ico", "svg", "pdf"];

impl App {
    pub fn new(
        directory: Option<PathBuf>,
        include_hidden: bool,
        app_event_sender: UnboundedSender<AppEvent>,
    ) -> Self {
        let directory = match directory {
            Some(d) => d,
            None => std::env::current_dir().unwrap(),
        };

        Self {
            current_screen: CurrentScreen::Search,
            search_fields: SearchFields::with_values("", "", false, ""),
            directory, // TODO: add this as a field that can be edited, e.g. allow glob patterns
            include_hidden,

            running: true,
            app_event_sender,
        }
    }

    pub fn cancel_search(&mut self) {
        if let CurrentScreen::ConfirmationSearchProgressing(SearchInProgressState {
            handle, ..
        }) = &self.current_screen
        {
            handle.abort();
        }
        self.current_screen = CurrentScreen::Search;
    }

    pub fn reset(&mut self) {
        self.cancel_search();
        *self = Self::new(
            Some(self.directory.clone()),
            self.include_hidden,
            self.app_event_sender.clone(),
        );
    }

    pub async fn handle_app_event(&mut self, event: AppEvent) -> EventHandlingResult {
        match event {
            AppEvent::Rerender => {
                error!("In AppEvent::Rerender");
                EventHandlingResult {
                    exit: false,
                    rerender: true,
                }
            }
            AppEvent::PerformSearch => {
                match self.validate_fields().unwrap() {
                    None => {
                        self.current_screen = CurrentScreen::Search;
                    }
                    Some(parsed_fields) => {
                        let handle = self.update_search_results(parsed_fields); // TODO: we need to be able to kill the thread this kicks off on reset or back
                        self.current_screen =
                            CurrentScreen::ConfirmationSearchProgressing(SearchInProgressState {
                                search_state: SearchState {
                                    results: vec![],
                                    selected: 0,
                                },
                                last_render: Instant::now(),
                                handle,
                            });
                    }
                };
                EventHandlingResult {
                    exit: false,
                    rerender: true,
                }
            }
            AppEvent::AddSearchResult(result) => {
                let mut rerender = false;
                if let CurrentScreen::ConfirmationSearchProgressing(search_in_progress_state) =
                    &mut self.current_screen
                {
                    search_in_progress_state.search_state.results.push(result);

                    if search_in_progress_state.last_render.elapsed() >= Duration::from_millis(100)
                    {
                        rerender = true;
                        search_in_progress_state.last_render = Instant::now();
                    }
                }
                EventHandlingResult {
                    exit: false,
                    rerender,
                }
            }
            AppEvent::SearchCompleted => {
                if let CurrentScreen::ConfirmationSearchProgressing(SearchInProgressState {
                    search_state,
                    ..
                }) = mem::replace(&mut self.current_screen, CurrentScreen::Search)
                {
                    self.current_screen = CurrentScreen::ConfirmationSearchComplete(search_state);
                    self.app_event_sender.send(AppEvent::Rerender).unwrap();
                }
                EventHandlingResult {
                    exit: false,
                    rerender: true,
                }
            }
            AppEvent::PerformReplacement => {
                self.perform_replacement();
                self.current_screen = CurrentScreen::Results;
                EventHandlingResult {
                    exit: false,
                    rerender: true,
                }
            }
        }
    }

    fn handle_key_searching(&mut self, key: &KeyEvent) -> bool {
        self.search_fields.clear_errors();
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => {
                self.app_event_sender.send(AppEvent::PerformSearch).unwrap();
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
                    .write()
                    .unwrap()
                    .handle_keys(code, modifiers);
            }
        };
        false
    }

    fn handle_key_confirmation(&mut self, key: &KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('j') | KeyCode::Down, _)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.results.search_results_mut().move_selected_down();
            }
            (KeyCode::Char('k') | KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                // TODO: need to fix issue where screen gets out of sync with state
                self.results.search_results_mut().move_selected_up();
            }
            (KeyCode::Char(' '), _) => {
                self.results
                    .search_results_mut()
                    .toggle_selected_inclusion();
            }
            (KeyCode::Enter, _) => {
                self.current_screen = CurrentScreen::PerformingReplacement;
                self.app_event_sender
                    .send(AppEvent::PerformReplacement)
                    .unwrap();
            }
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.cancel_search();
                self.current_screen = CurrentScreen::Search;
                self.app_event_sender.send(AppEvent::Rerender).unwrap();
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

    pub fn handle_key_events(&mut self, key: &KeyEvent) -> anyhow::Result<EventHandlingResult> {
        if key.kind == KeyEventKind::Release {
            return Ok(EventHandlingResult {
                exit: false,
                rerender: true,
            });
        }

        // TODO: why doesn't this work while search (or replacement?) are being completed? Also ignore other keys, i.e. don't allow them to queue up
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                return Ok(EventHandlingResult {
                    exit: true,
                    rerender: true,
                })
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                self.reset();
                return Ok(EventHandlingResult {
                    exit: false,
                    rerender: true,
                });
            }
            (_, _) => {}
        }

        let exit = match self.current_screen {
            CurrentScreen::Search => self.handle_key_searching(key),
            CurrentScreen::Confirmation => self.handle_key_confirmation(key),
            CurrentScreen::PerformingReplacement => false,
            CurrentScreen::Results => self.handle_key_results(key),
        };
        Ok(EventHandlingResult {
            exit,
            rerender: true,
        })
    }

    fn validate_fields(&self) -> anyhow::Result<Option<ParsedFields>> {
        let search_pattern = match self.search_fields.search_type() {
            Err(e) => {
                if e.downcast_ref::<regex::Error>().is_some() {
                    info!("Error when parsing search regex {}", e);
                    self.search_fields
                        .search_mut()
                        .set_error("Couldn't parse regex".to_owned());
                    return Ok(None);
                } else {
                    return Err(e);
                }
            }
            Ok(p) => p,
        };

        let path_pattern_text = self.search_fields.path_pattern().text();
        let path_pattern = if path_pattern_text.is_empty() {
            None
        } else {
            match Regex::new(path_pattern_text.as_str()) {
                Err(e) => {
                    info!("Error when parsing filname pattern regex {}", e);
                    self.search_fields
                        .path_pattern_mut()
                        .set_error("Couldn't parse regex".to_owned());
                    return Ok(None);
                }
                Ok(r) => Some(r),
            }
        };

        Ok(Some(ParsedFields::new(
            search_pattern,
            self.search_fields.replace().text(),
            path_pattern,
            self.directory.clone(),
            self.app_event_sender.clone(),
        )))
    }

    pub fn update_search_results(&mut self, parsed_fields: ParsedFields) -> JoinHandle<()> {
        let walker = WalkBuilder::new(&self.directory)
            .hidden(!self.include_hidden)
            .filter_entry(|entry| entry.file_name() != ".git")
            .build_parallel();

        let app_event_sender = self.app_event_sender.clone();

        tokio::spawn(async move {
            walker.run(|| {
                let parsed_fields = parsed_fields.clone(); // TODO: do we need to clone it?

                Box::new(move |entry| {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => return WalkState::Continue,
                    };

                    if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                        return WalkState::Continue;
                    };

                    if Self::ignore_file(entry.path()) {
                        return WalkState::Continue;
                    }

                    parsed_fields.handle_path(entry.path());

                    WalkState::Continue
                })
            });

            // if err this is likely because state was reset, so we can ignore
            let _ = app_event_sender.send(AppEvent::SearchCompleted);
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

    pub fn relative_path(&self, path: &Path) -> String {
        relative_path_from(&self.directory, path)
    }

    fn ignore_file(path: &Path) -> bool {
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
