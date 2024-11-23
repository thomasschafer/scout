use log::{error, warn};
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    event::{AppEvent, SearchResult},
    utils::relative_path_from,
};

#[derive(Clone, Debug)]
pub enum SearchType {
    Pattern(Regex),
    Fixed(String),
}

#[derive(Clone, Debug)] // TODO: make this not clone
pub struct ParsedFields {
    search_pattern: SearchType,
    replace_string: String, // self.search_fields.replace().text().as_str())
    path_pattern: Option<Regex>,
    root_dir: PathBuf,

    app_event_sender: UnboundedSender<AppEvent>,
}

impl ParsedFields {
    pub fn new(
        search_pattern: SearchType,
        replace_string: String,
        path_pattern: Option<Regex>,
        root_dir: PathBuf,
        app_event_sender: UnboundedSender<AppEvent>,
    ) -> Self {
        Self {
            search_pattern,
            replace_string,
            path_pattern,
            root_dir,
            app_event_sender,
        }
    }

    pub fn handle_path(&self, path: &Path) {
        if let Some(ref p) = self.path_pattern {
            let matches_pattern = p.is_match(relative_path_from(&self.root_dir, path).as_str());
            if !matches_pattern {
                return;
            }
        }

        match File::open(path) {
            Ok(file) => {
                let reader = BufReader::new(file);

                for (line_number, line) in reader.lines().enumerate() {
                    match line {
                        Ok(line) => {
                            if let Some(result) =
                                self.replacement_if_match(path.to_path_buf(), line, line_number)
                            {
                                error!("Pushing result"); // TODO: remove this and other unneeded logs
                                self.app_event_sender
                                    .send(AppEvent::AddSearchResult(result))
                                    .unwrap();
                            }
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

    fn replacement_if_match(
        &self,
        path: PathBuf,
        line: String,
        line_number: usize,
    ) -> Option<SearchResult> {
        let maybe_replacement = match self.search_pattern {
            SearchType::Fixed(ref s) => {
                if line.contains(s) {
                    Some(line.replace(s, &self.replace_string))
                } else {
                    None
                }
            }
            SearchType::Pattern(ref p) => {
                if p.is_match(&line) {
                    Some(p.replace_all(&line, &self.replace_string).to_string())
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
}
