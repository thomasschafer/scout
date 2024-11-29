use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use scooter::{
    parsed_fields::SearchType, CheckboxField, Field, FieldName, SearchField, SearchFields,
    TextField,
};
use std::sync::{Arc, RwLock};

#[test]
fn test_text_field_operations() {
    let mut field = TextField::default();

    // Test input
    for c in "Hello".chars() {
        field.enter_char(c);
    }
    assert_eq!(field.text(), "Hello");
    assert_eq!(field.cursor_idx(), 5);

    // Test cursor movement
    field.move_cursor_left();
    assert_eq!(field.cursor_idx(), 4);
    field.move_cursor_right();
    assert_eq!(field.cursor_idx(), 5);
    field.move_cursor_start();
    assert_eq!(field.cursor_idx(), 0);
    field.move_cursor_end();
    assert_eq!(field.cursor_idx(), 5);

    // Test word movement
    field.clear();
    for c in "Hello world".chars() {
        field.enter_char(c);
    }
    field.move_cursor_start();
    field.move_cursor_forward_word();
    assert_eq!(field.cursor_idx(), 6);
    field.move_cursor_forward_word();
    assert_eq!(field.cursor_idx(), 11);
    field.move_cursor_forward_word();
    assert_eq!(field.cursor_idx(), 11);
    field.move_cursor_back_word();
    assert_eq!(field.cursor_idx(), 6);

    // Test deletion
    field.move_cursor_start();
    field.delete_char_forward();
    assert_eq!(field.text(), "ello world");
    field.move_cursor_end();
    field.delete_char();
    assert_eq!(field.text(), "ello worl");
    field.move_cursor_start();
    field.delete_word_forward();
    assert_eq!(field.text(), "worl");
    field.move_cursor_end();
    field.delete_word_backward();
    assert_eq!(field.text(), "");
}

#[test]
fn test_checkbox_field() {
    let mut field = CheckboxField::new(false);
    assert!(!field.checked);

    field.handle_keys(KeyCode::Char(' '), KeyModifiers::empty());
    assert!(field.checked);

    field.handle_keys(KeyCode::Char(' '), KeyModifiers::empty());
    assert!(!field.checked);

    // Test that other keys don't affect the checkbox
    field.handle_keys(KeyCode::Enter, KeyModifiers::empty());
    assert!(!field.checked);
}

#[test]
fn test_search_fields() {
    let mut search_fields = SearchFields {
        fields: [
            SearchField {
                name: FieldName::Search,
                field: Arc::new(RwLock::new(Field::text(""))),
            },
            SearchField {
                name: FieldName::Replace,
                field: Arc::new(RwLock::new(Field::text(""))),
            },
            SearchField {
                name: FieldName::FixedStrings,
                field: Arc::new(RwLock::new(Field::checkbox(false))),
            },
            SearchField {
                name: FieldName::PathPattern,
                field: Arc::new(RwLock::new(Field::text(""))),
            },
        ],
        highlighted: 0,
    };

    // Test focus navigation
    assert_eq!(search_fields.highlighted, 0);
    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 1);
    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 2);
    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 3);
    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 0);
    search_fields.focus_prev();
    assert_eq!(search_fields.highlighted, 3);
    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 0);

    for c in "test search".chars() {
        search_fields
            .highlighted_field()
            .write()
            .unwrap()
            .handle_keys(KeyCode::Char(c), KeyModifiers::NONE);
    }
    assert_eq!(search_fields.search().text, "test search");

    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 1);
    for c in "test replace".chars() {
        search_fields
            .highlighted_field()
            .write()
            .unwrap()
            .handle_keys(KeyCode::Char(c), KeyModifiers::NONE);
    }
    assert_eq!(search_fields.replace().text, "test replace");

    search_fields.focus_next();
    assert_eq!(search_fields.highlighted, 2);
    search_fields
        .highlighted_field()
        .write()
        .unwrap()
        .handle_keys(KeyCode::Char(' '), KeyModifiers::NONE);
    assert!(search_fields.fixed_strings().checked);

    let search_type = search_fields.search_type().unwrap();
    match search_type {
        SearchType::Fixed(s) => assert_eq!(s, "test search"),
        SearchType::Pattern(_) => panic!("Expected Fixed, got Pattern"),
    }

    search_fields
        .highlighted_field()
        .write()
        .unwrap()
        .handle_keys(KeyCode::Char(' '), KeyModifiers::NONE);
    let search_type = search_fields.search_type().unwrap();
    match search_type {
        SearchType::Pattern(_) => {}
        SearchType::Fixed(_) => panic!("Expected Pattern, got Fixed"),
    }
}
