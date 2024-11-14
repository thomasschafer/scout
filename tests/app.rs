use scooter::{
    App, CurrentScreen, EventHandler, ReplaceResult, ReplaceState, Results, SearchFields,
    SearchResult, SearchState,
};
use std::fs;
use std::fs::create_dir_all;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_search_state() {
    let mut state = SearchState {
        results: vec![
            SearchResult {
                path: PathBuf::from("test1.txt"),
                line_number: 1,
                line: "test line 1".to_string(),
                replacement: "replacement 1".to_string(),
                included: true,
                replace_result: None,
            },
            SearchResult {
                path: PathBuf::from("test2.txt"),
                line_number: 2,
                line: "test line 2".to_string(),
                replacement: "replacement 2".to_string(),
                included: false,
                replace_result: None,
            },
        ],
        selected: 0,
    };

    state.move_selected_down();
    assert_eq!(state.selected, 1);
    state.move_selected_down();
    assert_eq!(state.selected, 0);
    state.move_selected_up();
    assert_eq!(state.selected, 1);
    state.move_selected_up();
    assert_eq!(state.selected, 0);

    state.toggle_selected_inclusion();
    assert!(!state.results[0].included);
    state.move_selected_down();
    state.toggle_selected_inclusion();
    assert!(state.results[1].included);
}

#[tokio::test]
async fn test_replace_state() {
    let mut state = ReplaceState {
        num_successes: 2,
        num_ignored: 1,
        errors: (1..3)
            .map(|n| SearchResult {
                path: PathBuf::from(format!("error-{}.txt", n)),
                line_number: 1,
                line: format!("line {}", n),
                replacement: format!("error replacement {}", n),
                included: true,
                replace_result: Some(ReplaceResult::Error(format!("Test error {}", n))),
            })
            .collect::<Vec<_>>(),
        replacement_errors_pos: 0,
    };

    state.scroll_replacement_errors_down();
    assert_eq!(state.replacement_errors_pos, 1);
    state.scroll_replacement_errors_down();
    assert_eq!(state.replacement_errors_pos, 0);
    state.scroll_replacement_errors_up();
    assert_eq!(state.replacement_errors_pos, 1);
    state.scroll_replacement_errors_up();
    assert_eq!(state.replacement_errors_pos, 0);
}

#[tokio::test]
async fn test_app_reset() {
    let events = EventHandler::new();
    let mut app = App::new(None, events.app_event_sender);
    app.current_screen = CurrentScreen::Results;
    app.results = Results::ReplaceComplete(ReplaceState {
        num_successes: 5,
        num_ignored: 2,
        errors: vec![],
        replacement_errors_pos: 0,
    });

    app.reset();

    assert!(matches!(app.current_screen, CurrentScreen::Searching));
    assert!(matches!(app.results, Results::Loading));
}

macro_rules! create_test_files {
    ($temp_dir:expr, $($name:expr => {$($line:expr),+ $(,)?}),+ $(,)?) => {
        {
            $(
                let contents = concat!($($line,"\n",)+);
                let path = [$temp_dir.path().to_str().unwrap(), $name].join("/");
                let path = Path::new(&path);
                create_dir_all(path.parent().unwrap()).unwrap();
                let mut file = File::create(path).unwrap();
                file.write_all(contents.as_bytes()).unwrap();
                file.sync_all().unwrap();
            )+
        }
    };
}

fn setup_env_simple_files() -> App {
    let temp_dir = TempDir::new().unwrap();

    create_test_files! {
        temp_dir,
        "file1.txt" => {
            "This is a test file",
            "It contains some test content",
            "For testing purposes",
        },
        "file2.txt" => {
            "Another test file",
            "With different content",
            "Also for testing",
        },
        "file3.txt" => {
            "something",
            "123 bar[a-b]+.*bar)(baz 456",
            "something",
        }
    };

    let events = EventHandler::new();
    App::new(Some(temp_dir.into_path()), events.app_event_sender)
}

#[tokio::test]
async fn test_update_search_results_fixed_string() {
    let mut app = setup_env_simple_files();

    app.search_fields = SearchFields::with_values(".*", "example", true, "");

    app.update_search_results().unwrap();

    if let scooter::Results::SearchComplete(search_state) = &app.results {
        assert_eq!(search_state.results.len(), 1);

        for (file_name, num_matches) in [("file1.txt", 0), ("file1.txt", 0), ("file3.txt", 1)] {
            assert_eq!(
                search_state
                    .results
                    .iter()
                    .filter(|r| r.path.file_name().unwrap() == file_name)
                    .count(),
                num_matches
            );
        }

        for result in &search_state.results {
            assert!(result.line.contains(".*"));
            assert_eq!(result.replacement, result.line.replace(".*", "example"));
        }
    } else {
        panic!("Expected SearchComplete results");
    }
}

#[tokio::test]
async fn test_update_search_results_regex() {
    let mut app = setup_env_simple_files();

    app.search_fields = SearchFields::with_values(r"\b\w+ing\b", "VERB", false, "");

    app.update_search_results().unwrap();

    if let scooter::Results::SearchComplete(search_state) = &app.results {
        assert_eq!(search_state.results.len(), 4,);

        let mut file_match_counts = std::collections::HashMap::new();

        for result in &search_state.results {
            *file_match_counts
                .entry(
                    result
                        .path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                )
                .or_insert(0) += 1;

            assert!(result.line.contains("testing") || result.line.contains("something"),);
            assert_eq!(
                result.replacement,
                result
                    .line
                    .replace("testing", "VERB")
                    .replace("something", "VERB"),
            );
        }

        assert_eq!(*file_match_counts.get("file1.txt").unwrap_or(&0), 1);
        assert_eq!(*file_match_counts.get("file2.txt").unwrap_or(&0), 1);
        assert_eq!(*file_match_counts.get("file3.txt").unwrap_or(&0), 2);
    } else {
        panic!("Expected SearchComplete results");
    }
}
#[tokio::test]
async fn test_update_search_results_no_matches() {
    let mut app = setup_env_simple_files();

    app.search_fields = SearchFields::with_values("nonexistent", "replacement", false, "");

    app.update_search_results().unwrap();

    if let scooter::Results::SearchComplete(search_state) = &app.results {
        assert_eq!(search_state.results.len(), 0);
    } else {
        panic!("Expected SearchComplete results");
    }
}

#[tokio::test]
async fn test_update_search_results_invalid_regex() {
    let mut app = setup_env_simple_files();

    app.search_fields = SearchFields::with_values(r"[invalid regex", "replacement", false, "");

    let result = app.update_search_results();
    assert!(result.is_ok());
}

fn setup_env_files_in_dirs() -> App {
    let temp_dir = TempDir::new().unwrap();

    create_test_files! {
        temp_dir,
        "dir1/file1.txt" => {
            "This is a test file",
            "It contains some test content",
            "For testing purposes",
        },
        "dir2/file2.txt" => {
            "Another test file",
            "With different content",
            "Also for testing",
        },
        "dir2/file3.txt" => {
            "something",
            "123 bar[a-b]+.*bar)(baz 456",
            "something testing",
        }
    };

    for dir in fs::read_dir(temp_dir.path()).unwrap() {
        for path in fs::read_dir(dir.unwrap().path()).unwrap() {
            println!("Name: {}", path.unwrap().path().display())
        }
    }

    let events = EventHandler::new();
    App::new(Some(temp_dir.into_path()), events.app_event_sender)
}

#[tokio::test]
async fn test_update_search_results_filtered_dir() {
    let mut app = setup_env_files_in_dirs();

    app.search_fields = SearchFields::with_values(r"testing", "f", false, "dir2");

    let result = app.update_search_results();
    assert!(result.is_ok());

    if let scooter::Results::SearchComplete(search_state) = &app.results {
        assert_eq!(search_state.results.len(), 2);

        for (file_path, num_matches) in [
            (Path::new("dir1").join("file1.txt"), 0),
            (Path::new("dir2").join("file2.txt"), 1),
            (Path::new("dir2").join("file3.txt"), 1),
        ] {
            println!("Results: {:?}", search_state.results);
            assert_eq!(
                search_state
                    .results
                    .iter()
                    .filter(|result| {
                        let result_path = result.path.to_str().unwrap();
                        let file_path = file_path.to_str().unwrap();
                        result_path.contains(file_path)
                    })
                    .count(),
                num_matches
            );
        }

        for result in &search_state.results {
            assert_eq!(result.replacement, result.line.replace("testing", "f"));
        }
    } else {
        panic!("Expected SearchComplete results");
    }
}

// TODO: add tests for:
// - replacing in files
// - more tests for passing in directory via CLI arg
