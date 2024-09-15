use scout::{App, SearchFields};
use scout::{CurrentScreen, ReplaceResult, ReplaceState, Results, SearchResult, SearchState};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_search_state() {
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

#[test]
fn test_replace_state() {
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

#[test]
fn test_app_reset() {
    let mut app = App::new();
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

fn setup_test_environment() -> (TempDir, App) {
    let temp_dir = TempDir::new().unwrap();

    // TODO: make a macro for this
    for (name, contents) in [
        (
            "file1.txt",
            concat!(
                "This is a test file\n",
                "It contains some test content\n",
                "For testing purposes\n"
            ),
        ),
        (
            "file2.txt",
            concat!(
                "Another test file\n",
                "With different content\n",
                "Also for testing\n"
            ),
        ),
        (
            "file3.txt",
            concat!(
                "something\n",
                "123 foo[a-b]+.*bar)(baz 456\n",
                "something\n"
            ),
        ),
    ] {
        let path = temp_dir.path().join(name);
        let mut file = File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.sync_all().unwrap();
    }

    let app = App::new();

    (temp_dir, app)
}

#[test]
fn test_update_search_results_fixed_string() {
    let (temp_dir, mut app) = setup_test_environment();

    app.search_fields = SearchFields::with_values(".*", "example", true);

    std::env::set_current_dir(temp_dir.path()).unwrap();

    app.update_search_results().unwrap();

    if let scout::Results::SearchComplete(search_state) = &app.results {
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

#[test]
fn test_update_search_results_regex() {
    // TODO: fix flakiness and remove logging below
    let (temp_dir, mut app) = setup_test_environment();

    // Print the contents of all files for debugging
    for entry in fs::read_dir(temp_dir.path()).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            println!("File: {:?}", path);
            println!("Contents:\n{}", fs::read_to_string(&path).unwrap());
        }
    }

    app.search_fields = SearchFields::with_values(r"\b\w+ing\b", "VERB", false);

    println!("Current directory: {:?}", std::env::current_dir().unwrap());
    std::env::set_current_dir(temp_dir.path()).unwrap();
    println!(
        "New current directory: {:?}",
        std::env::current_dir().unwrap()
    );

    let update_result = app.update_search_results();

    if let Err(ref e) = update_result {
        println!("Error updating search results: {:?}", e);
    }

    update_result.unwrap();

    if let scout::Results::SearchComplete(search_state) = &app.results {
        println!("Number of results: {}", search_state.results.len());
        for result in &search_state.results {
            println!("File: {:?}, Line: {}", result.path, result.line);
        }

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

        println!("File match counts: {:?}", file_match_counts);

        assert_eq!(*file_match_counts.get("file1.txt").unwrap_or(&0), 1);
        assert_eq!(*file_match_counts.get("file2.txt").unwrap_or(&0), 1);
        assert_eq!(*file_match_counts.get("file3.txt").unwrap_or(&0), 2);
    } else {
        panic!("Expected SearchComplete results");
    }
}
#[test]
fn test_update_search_results_no_matches() {
    let (temp_dir, mut app) = setup_test_environment();

    app.search_fields = SearchFields::with_values("nonexistent", "replacement", false);

    std::env::set_current_dir(temp_dir.path()).unwrap();

    app.update_search_results().unwrap();

    if let scout::Results::SearchComplete(search_state) = &app.results {
        assert_eq!(search_state.results.len(), 0);
    } else {
        panic!("Expected SearchComplete results");
    }
}

#[test]
fn test_update_search_results_invalid_regex() {
    let (temp_dir, mut app) = setup_test_environment();

    app.search_fields = SearchFields::with_values(r"[invalid regex", "replacement", false);

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = app.update_search_results();
    assert!(result.is_err());
}

// TODO: add tests for replacing in files