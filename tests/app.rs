use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use scooter::{
    App, EventHandler, ReplaceResult, ReplaceState, Screen, SearchFields, SearchResult, SearchState,
};
use std::cmp::max;
use std::fs::{self, create_dir_all, File};
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};
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
    let mut app = App::new(None, false, events.app_event_sender);
    app.current_screen = Screen::Results(ReplaceState {
        num_successes: 5,
        num_ignored: 2,
        errors: vec![],
        replacement_errors_pos: 0,
    });

    app.reset();

    assert!(matches!(app.current_screen, Screen::SearchFields));
}

#[tokio::test]
async fn test_back_from_results() {
    let events = EventHandler::new();
    let mut app = App::new(None, false, events.app_event_sender);
    app.current_screen = Screen::SearchComplete(SearchState {
        results: vec![],
        selected: 0,
    });
    app.search_fields = SearchFields::with_values("foo", "bar", true, "pattern");

    let res = app
        .handle_key_events(&KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
        .unwrap();
    assert!(!res.exit);
    assert_eq!(app.search_fields.search().text, "foo");
    assert_eq!(app.search_fields.replace().text, "bar");
    assert!(app.search_fields.fixed_strings().checked);
    assert_eq!(app.search_fields.path_pattern().text, "pattern");
    assert!(matches!(app.current_screen, Screen::SearchFields));
}

macro_rules! create_test_files {
    ($($name:expr => {$($line:expr),+ $(,)?}),+ $(,)?) => {
        {
            let temp_dir = TempDir::new().unwrap();
            $(
                let contents = concat!($($line,"\n",)+);
                let path = [temp_dir.path().to_str().unwrap(), $name].join("/");
                let path = Path::new(&path);
                create_dir_all(path.parent().unwrap()).unwrap();
                {
                    let mut file = File::create(path).unwrap();
                    file.write_all(contents.as_bytes()).unwrap();
                    file.sync_all().unwrap();
                }
            )+

            #[cfg(windows)]
            sleep(Duration::from_millis(100));

            temp_dir
        }
    };
}
fn collect_files(dir: &Path, base: &Path, files: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let rel_path = path
                .strip_prefix(base)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
                .replace("\\", "/");
            files.push(rel_path);
        } else if path.is_dir() {
            collect_files(&path, base, files);
        }
    }
}

macro_rules! assert_test_files {
    ($temp_dir:expr, $($name:expr => {$($line:expr),+ $(,)?}),+ $(,)?) => {
        {
            use std::fs;
            use std::path::Path;

            $(
                let expected_contents = concat!($($line,"\n",)+);
                let path = Path::new($temp_dir.path()).join($name);

                assert!(path.exists(), "File {} does not exist", $name);

                let actual_contents = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("Failed to read file {}: {}", $name, e));
                assert_eq!(
                    actual_contents,
                    expected_contents,
                    "Contents mismatch for file {}.\nExpected:\n{}\nActual:\n{}",
                    $name,
                    expected_contents,
                    actual_contents
                );
            )+

            let mut expected_files: Vec<String> = vec![$($name.to_string()),+];
            expected_files.sort();

            let mut actual_files = Vec::new();
            collect_files(
                $temp_dir.path(),
                $temp_dir.path(),
                &mut actual_files
            );
            actual_files.sort();

            assert_eq!(
                actual_files,
                expected_files,
                "Directory contains unexpected files.\nExpected files: {:?}\nActual files: {:?}",
                expected_files,
                actual_files
            );
        }
    };
}
pub fn wait_until<F>(condition: F, timeout: Duration) -> bool
where
    F: Fn() -> bool,
{
    let start = Instant::now();
    let sleep_duration = max(timeout / 50, Duration::from_millis(1));
    while !condition() && start.elapsed() <= timeout {
        sleep(sleep_duration);
    }
    condition()
}

async fn process_bp_events(app: &mut App) {
    let timeout = Duration::from_secs(5);
    let start = Instant::now();

    while let Some(event) = app.background_processing_recv().await {
        app.handle_background_processing_event(event);
        if start.elapsed() > timeout {
            panic!("Couldn't process background events in a reasonable time");
        }
    }
}

macro_rules! wait_for_screen {
    ($app:expr, $variant:path) => {
        wait_until(
            || matches!($app.current_screen, $variant(_)),
            Duration::from_secs(1),
        )
    };
}

fn setup_app(temp_dir: &TempDir, search_fields: SearchFields, include_hidden: bool) -> App {
    let events = EventHandler::new();
    let mut app = App::new(
        Some(temp_dir.path().to_path_buf()),
        include_hidden,
        events.app_event_sender,
    );
    app.search_fields = search_fields;
    app
}

async fn search_and_replace_test(
    temp_dir: &TempDir,
    search_fields: SearchFields,
    include_hidden: bool,
    expected_matches: Vec<(&Path, usize)>,
) {
    let num_expected_matches = expected_matches
        .iter()
        .map(|(_, count)| count)
        .sum::<usize>();

    let mut app = setup_app(temp_dir, search_fields, include_hidden);
    let res = app.perform_search_if_valid();
    assert!(!res.exit);

    process_bp_events(&mut app).await;
    assert!(wait_for_screen!(&app, Screen::SearchComplete));

    // TODO: this mem::replace needs to be kept in sync with the same action that happens in app.rs - can we fix this?
    let mut search_state = if let Screen::SearchComplete(search_state) =
        mem::replace(&mut app.current_screen, Screen::PerformingReplacement)
    {
        for (file_path, num_matches) in &expected_matches {
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
                *num_matches
            );
        }

        assert_eq!(search_state.results.len(), num_expected_matches);

        search_state
    } else {
        panic!(
            "Expected SearchComplete results, found {:?}",
            app.current_screen
        );
    };

    app.perform_replacement(&mut search_state);

    if let Screen::Results(search_state) = &app.current_screen {
        assert_eq!(search_state.num_successes, num_expected_matches);
        assert_eq!(search_state.num_ignored, 0);
        assert_eq!(search_state.errors.len(), 0);
    } else {
        panic!(
            "Expected screen to be Screen::Results, instead found {:?}",
            app.current_screen
        );
    }
}

#[tokio::test]
async fn test_perform_search_fixed_string() {
    let temp_dir = create_test_files! {
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

    search_and_replace_test(
        &temp_dir,
        SearchFields::with_values(".*", "example", true, ""),
        false,
        vec![
            (Path::new("file1.txt"), 0),
            (Path::new("file2.txt"), 0),
            (Path::new("file3.txt"), 1),
        ],
    )
    .await;

    assert_test_files! {
        &temp_dir,
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
            "123 bar[a-b]+examplebar)(baz 456",
            "something",
        }
    };
}

#[tokio::test]
async fn test_update_search_results_regex() {
    let temp_dir = &create_test_files! {
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

    search_and_replace_test(
        temp_dir,
        SearchFields::with_values(r"\b\w+ing\b", "VERB", false, ""),
        false,
        vec![
            (Path::new("file1.txt"), 1),
            (Path::new("file2.txt"), 1),
            (Path::new("file3.txt"), 2),
        ],
    )
    .await;

    assert_test_files! {
        temp_dir,
        "file1.txt" => {
            "This is a test file",
            "It contains some test content",
            "For VERB purposes",
        },
        "file2.txt" => {
            "Another test file",
            "With different content",
            "Also for VERB",
        },
        "file3.txt" => {
            "VERB",
            "123 bar[a-b]+.*bar)(baz 456",
            "VERB",
        }
    };
}

#[tokio::test]
async fn test_update_search_results_no_matches() {
    let temp_dir = &create_test_files! {
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

    search_and_replace_test(
        temp_dir,
        SearchFields::with_values(r"nonexistent-string", "replacement", true, ""),
        false,
        vec![
            (Path::new("file1.txt"), 0),
            (Path::new("file2.txt"), 0),
            (Path::new("file3.txt"), 0),
        ],
    )
    .await;

    assert_test_files! {
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
}

#[tokio::test]
async fn test_update_search_results_invalid_regex() {
    let temp_dir = &create_test_files! {
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

    let search_fields = SearchFields::with_values(r"[invalid regex", "replacement", false, "");
    let mut app = setup_app(temp_dir, search_fields, false);

    let res = app.perform_search_if_valid();
    assert!(!res.exit);
    assert!(matches!(app.current_screen, Screen::SearchFields));
    process_bp_events(&mut app).await;
    assert!(!wait_for_screen!(&app, Screen::SearchComplete)); // We shouldn't get to the SearchComplete page, so assert that we never get there
    assert!(matches!(app.current_screen, Screen::SearchFields));
}

#[tokio::test]
async fn test_update_search_results_filtered_dir() {
    let temp_dir = &create_test_files! {
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

    search_and_replace_test(
        temp_dir,
        SearchFields::with_values(r"testing", "f", false, "dir2"),
        false,
        vec![
            (&Path::new("dir1").join("file1.txt"), 0),
            (&Path::new("dir2").join("file2.txt"), 1),
            (&Path::new("dir2").join("file3.txt"), 1),
        ],
    )
    .await;

    assert_test_files! {
        temp_dir,
        "dir1/file1.txt" => {
            "This is a test file",
            "It contains some test content",
            "For testing purposes",
        },
        "dir2/file2.txt" => {
            "Another test file",
            "With different content",
            "Also for f",
        },
        "dir2/file3.txt" => {
            "something",
            "123 bar[a-b]+.*bar)(baz 456",
            "something f",
        }
    };
}

#[tokio::test]
async fn test_ignores_gif_file() {
    let temp_dir = &create_test_files! {
        "dir1/file1.txt" => {
            "This is a text file",
        },
        "dir2/file2.gif" => {
            "This is a gif file",
        },
        "file3.txt" => {
            "This is a text file",
        }
    };

    search_and_replace_test(
        temp_dir,
        SearchFields::with_values(r"is", "", false, ""),
        false,
        vec![
            (&Path::new("dir1").join("file1.txt"), 1),
            (&Path::new("dir2").join("file2.gif"), 0),
            (Path::new("file3.txt"), 1),
        ],
    )
    .await;

    assert_test_files! {
        temp_dir,
        "dir1/file1.txt" => {
            "Th  a text file",
        },
        "dir2/file2.gif" => {
            "This is a gif file",
        },
        "file3.txt" => {
            "Th  a text file",
        }
    };
}

#[tokio::test]
async fn test_ignores_hidden_files_by_default() {
    let temp_dir = &create_test_files! {
        "dir1/file1.txt" => {
            "This is a text file",
        },
        ".dir2/file2.rs" => {
            "This is a file in a hidden directory",
        },
        ".file3.txt" => {
            "This is a hidden text file",
        }
    };

    search_and_replace_test(
        temp_dir,
        SearchFields::with_values(r"\bis\b", "REPLACED", false, ""),
        false,
        vec![
            (&Path::new("dir1").join("file1.txt"), 1),
            (&Path::new(".dir2").join("file2.rs"), 0),
            (Path::new(".file3.txt"), 0),
        ],
    )
    .await;

    assert_test_files! {
        temp_dir,
        "dir1/file1.txt" => {
            "This REPLACED a text file",
        },
        ".dir2/file2.rs" => {
            "This is a file in a hidden directory",
        },
        ".file3.txt" => {
            "This is a hidden text file",
        }
    };
}

#[tokio::test]
async fn test_includes_hidden_files_with_flag() {
    let temp_dir = &create_test_files! {
        "dir1/file1.txt" => {
            "This is a text file",
        },
        ".dir2/file2.rs" => {
            "This is a file in a hidden directory",
        },
        ".file3.txt" => {
            "This is a hidden text file",
        }
    };

    search_and_replace_test(
        temp_dir,
        SearchFields::with_values(r"\bis\b", "REPLACED", false, ""),
        true,
        vec![
            (&Path::new("dir1").join("file1.txt"), 1),
            (&Path::new(".dir2").join("file2.rs"), 1),
            (Path::new(".file3.txt"), 1),
        ],
    )
    .await;

    assert_test_files! {
        temp_dir,
        "dir1/file1.txt" => {
            "This REPLACED a text file",
        },
        ".dir2/file2.rs" => {
            "This REPLACED a file in a hidden directory",
        },
        ".file3.txt" => {
            "This REPLACED a hidden text file",
        }
    };
}

// TODO:
// - Add:
//   - more tests for replacing in files
//   - tests for passing in directory via CLI arg
// - Tidy up tests - lots of duplication
