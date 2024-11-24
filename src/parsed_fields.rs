use log::{error, warn};
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    event::{BackgroundProcessingEvent, SearchResult},
    utils::relative_path_from,
};

#[derive(Clone, Debug)]
pub enum SearchType {
    Pattern(Regex),
    Fixed(String),
}

#[derive(Clone, Debug)]
pub struct ParsedFields {
    search_pattern: SearchType,
    replace_string: String,
    path_pattern: Option<Regex>,
    root_dir: PathBuf,

    background_processing_sender: UnboundedSender<BackgroundProcessingEvent>,
}

impl ParsedFields {
    pub fn new(
        search_pattern: SearchType,
        replace_string: String,
        path_pattern: Option<Regex>,
        root_dir: PathBuf,
        background_processing_sender: UnboundedSender<BackgroundProcessingEvent>,
    ) -> Self {
        Self {
            search_pattern,
            replace_string,
            path_pattern,
            root_dir,
            background_processing_sender,
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
                                let send_result = self
                                    .background_processing_sender
                                    .send(BackgroundProcessingEvent::AddSearchResult(result)); // TODO: we need to get rid of all of these when state is reset?
                                if send_result.is_err() {
                                    // likely state reset, thread about to be killed
                                    return;
                                }
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
