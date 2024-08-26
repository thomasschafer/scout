use std::{
    cell::RefCell,
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    rc::Rc,
};

use ignore::WalkBuilder;
use itertools::Itertools;
use regex::Regex;

use crate::log;

pub(crate) enum CurrentScreen {
    Searching,
    Confirmation,
    Results,
}

#[derive(Default, Clone)]
pub(crate) struct TextField {
    text: String,
    cursor_idx: usize,
}

impl TextField {
    pub(crate) fn text(&self) -> &str {
        self.text.as_str()
    }

    pub(crate) fn cursor_idx(&self) -> usize {
        self.cursor_idx
    }

    pub(crate) fn move_cursor_left(&mut self) {
        self.move_cursor_left_by(1)
    }

    pub(crate) fn move_cursor_start(&mut self) {
        self.cursor_idx = 0;
    }

    fn move_cursor_left_by(&mut self, n: usize) {
        let cursor_moved_left = self.cursor_idx.saturating_sub(n);
        self.cursor_idx = self.clamp_cursor(cursor_moved_left);
    }

    pub(crate) fn move_cursor_right(&mut self) {
        self.move_cursor_right_by(1)
    }

    fn move_cursor_right_by(&mut self, n: usize) {
        let cursor_moved_right = self.cursor_idx.saturating_add(n);
        self.cursor_idx = self.clamp_cursor(cursor_moved_right);
    }

    pub(crate) fn move_cursor_end(&mut self) {
        self.cursor_idx = self.text.chars().count();
    }

    pub(crate) fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.text.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&mut self) -> usize {
        self.text
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_idx)
            .unwrap_or(self.text.len())
    }

    pub(crate) fn delete_char(&mut self) {
        if self.cursor_idx == 0 {
            return;
        }

        let before_char = self.text.chars().take(self.cursor_idx - 1);
        let after_char = self.text.chars().skip(self.cursor_idx);

        self.text = before_char.chain(after_char).collect();
        self.move_cursor_left();
    }

    pub(crate) fn delete_char_forward(&mut self) {
        let before_char = self.text.chars().take(self.cursor_idx);
        let after_char = self.text.chars().skip(self.cursor_idx + 1);

        self.text = before_char.chain(after_char).collect();
    }

    fn previous_word_start(&self) -> usize {
        if self.cursor_idx == 0 {
            return 0;
        }

        let before_char = self.text.chars().take(self.cursor_idx).collect::<Vec<_>>();
        let mut idx = self.cursor_idx - 1;
        while idx > 0 && before_char[idx] == ' ' {
            idx -= 1;
        }
        while idx > 0 && before_char[idx] != ' ' {
            idx -= 1;
        }
        idx
    }

    pub(crate) fn move_cursor_back_word(&mut self) {
        self.cursor_idx = self.previous_word_start();
    }

    pub(crate) fn delete_word_backward(&mut self) {
        let new_cursor_pos = self.previous_word_start();
        let before_char = self.text.chars().take(new_cursor_pos);
        let after_char = self.text.chars().skip(self.cursor_idx);

        self.text = before_char.chain(after_char).collect();
        self.cursor_idx = new_cursor_pos;
    }

    fn next_word_start(&self) -> usize {
        let after_char = self.text.chars().skip(self.cursor_idx).collect::<Vec<_>>();
        let mut idx = 0;
        let num_chars = after_char.len();
        while idx < num_chars && after_char[idx] == ' ' {
            idx += 1;
        }
        while idx < num_chars && after_char[idx] != ' ' {
            idx += 1;
        }
        self.cursor_idx + idx
    }

    pub(crate) fn move_cursor_forward_word(&mut self) {
        self.cursor_idx = self.next_word_start();
    }

    pub(crate) fn delete_word_forward(&mut self) {
        let before_char = self.text.chars().take(self.cursor_idx);
        let after_char = self.text.chars().skip(self.next_word_start());

        self.text = before_char.chain(after_char).collect();
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.text.chars().count())
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor_idx = 0;
    }
}

#[derive(Clone)]
pub(crate) struct SearchResult {
    pub(crate) path: PathBuf,
    pub(crate) line_number: usize,
    pub(crate) line: String,
    pub(crate) replacement: String,
    pub(crate) included: bool,
}

pub(crate) struct CompleteState {
    pattern: Regex,
    pub(crate) results: Vec<SearchResult>,
    pub(crate) selected: usize, // TODO: allow for selection of ranges
}

impl CompleteState {
    pub(crate) fn move_selected_up(&mut self) {
        if self.selected == 0 {
            self.selected = self.results.len();
        }
        self.selected = self.selected.saturating_sub(1);
    }

    pub(crate) fn move_selected_down(&mut self) {
        if self.selected >= self.results.len().saturating_sub(1) {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    pub(crate) fn toggle_selected_inclusion(&mut self) {
        if self.selected < self.results.len() {
            let selected_result = &mut self.results[self.selected];
            selected_result.included = !selected_result.included;
        } else {
            self.selected = self.results.len()
        }
    }
}

pub(crate) enum SearchResults {
    Loading,
    Complete(CompleteState),
}

macro_rules! complete_impl {
    ($self:ident, $ret:ty) => {
        match $self {
            SearchResults::Complete(state) => state,
            SearchResults::Loading => {
                panic!("Search results still loading, expected this to have completed")
            }
        }
    };
}

impl SearchResults {
    pub(crate) fn complete(&self) -> &CompleteState {
        complete_impl!(self, &CompleteState)
    }

    pub(crate) fn complete_mut(&mut self) -> &mut CompleteState {
        complete_impl!(self, &mut CompleteState)
    }
}

#[derive(PartialEq)]
pub(crate) enum FieldName {
    Search,
    Replace,
}

pub(crate) struct SearchFields {
    pub(crate) fields: Vec<(FieldName, Rc<RefCell<TextField>>)>,
    pub(crate) highlighted: usize,
}

impl SearchFields {
    pub(crate) fn find(&self, field_name: FieldName) -> Rc<RefCell<TextField>> {
        self.fields
            .iter()
            .find(|field| field.0 == field_name)
            .expect("Couldn't find search field")
            .1
            .clone()
    }

    pub(crate) fn search(&self) -> Rc<RefCell<TextField>> {
        self.find(FieldName::Search)
    }

    pub(crate) fn pattern(&self) -> anyhow::Result<Regex> {
        Ok(Regex::new(self.search().borrow_mut().text())?)
    }

    pub(crate) fn replace(&self) -> Rc<RefCell<TextField>> {
        self.find(FieldName::Replace)
    }

    pub(crate) fn focus_next(&mut self) {
        self.highlighted = (self.highlighted + 1) % self.fields.len();
    }

    pub(crate) fn focus_prev(&mut self) {
        self.highlighted =
            (self.highlighted + self.fields.len().saturating_sub(1)) % self.fields.len();
    }

    pub(crate) fn highlighted_field(&self) -> &Rc<RefCell<TextField>> {
        &self.fields[self.highlighted].1
    }
}

pub(crate) struct App {
    pub(crate) current_screen: CurrentScreen,
    pub(crate) search_fields: SearchFields,
    pub(crate) search_results: SearchResults,
}

impl App {
    pub(crate) fn new() -> App {
        App {
            current_screen: CurrentScreen::Searching,
            search_fields: SearchFields {
                fields: vec![
                    (FieldName::Search, Rc::new(TextField::default().into())),
                    (FieldName::Replace, Rc::new(TextField::default().into())),
                ],
                highlighted: 0,
            },
            search_results: SearchResults::Loading,
        }
    }

    pub(crate) fn update_search_results(&mut self) -> anyhow::Result<()> {
        let repo_path = ".";
        let pattern = Regex::new(self.search_fields.search().borrow_mut().text())?;

        let mut results = vec![];

        let walker = WalkBuilder::new(repo_path).ignore(true).build();

        for entry in walker.flatten() {
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                let path = entry.path();

                let file = match File::open(path) {
                    Ok(file) => file,
                    Err(err) => {
                        // TODO: log the error here
                        continue;
                    }
                };
                let reader = BufReader::new(file);

                for (line_number, line) in reader.lines().enumerate() {
                    match line {
                        Ok(line) => {
                            if pattern.is_match(&line) {
                                results.push(SearchResult {
                                    path: entry.path().to_path_buf(),
                                    line_number: line_number + 1,
                                    line: line.clone(),
                                    replacement: pattern
                                        .replace_all(
                                            &line,
                                            // TODO: use capture groups from search pattern in replacement
                                            self.search_fields.replace().borrow().text(),
                                        )
                                        .to_string(),
                                    included: true,
                                });
                            }
                        }
                        Err(err) => {
                            // TODO: log the error here
                            continue;
                        }
                    }
                }
            }
        }

        // thread::sleep(time::Duration::from_secs(2)); // TODO: use this to verify loading state

        self.search_results = SearchResults::Complete(CompleteState {
            results,
            selected: 0,
            pattern,
        });

        Ok(())
    }

    pub(crate) fn perform_replacement(&self) {
        for (path, results) in &self
            .search_results
            .complete()
            .results
            .iter()
            .filter(|res| res.included)
            .chunk_by(|res| res.path.clone())
        {
            // TODO: show successes and failures (file was updated, error when replacing etc.) in results screen
            self.replace_in_file(path, results.collect())
                .expect("Todo - show this error");
        }
    }

    fn replace_in_file(
        &self,
        file_path: PathBuf,
        results: Vec<&SearchResult>,
    ) -> anyhow::Result<()> {
        let line_map: HashMap<usize, (String, String)> = HashMap::from_iter(
            results
                .into_iter()
                .map(|res| (res.line_number, (res.line.clone(), res.replacement.clone()))),
        );

        let input = File::open(file_path.clone())?;
        let buffered = BufReader::new(input);

        let temp_file_path = file_path.with_extension("tmp");
        let output = File::create(temp_file_path.clone())?;
        let mut writer = BufWriter::new(output);

        for (index, line) in buffered.lines().enumerate() {
            let mut line = line?;
            if let Some((cur_line, replacement)) = line_map.get(&(index + 1)) {
                // TODO: check that the lines match
                line.clone_from(replacement);
            }
            writeln!(writer, "{}", line)?;
        }

        writer.flush()?;
        fs::rename(temp_file_path, file_path)?;
        Ok(())
    }
}
