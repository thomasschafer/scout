#[cfg(test)]
mod tests {
    use ratatui::style::Color;
    use scooter::{line_diff, Diff};

    #[test]
    fn test_identical_lines() {
        let (old_actual, new_actual) = line_diff("hello", "hello");

        let old_expected = vec![
            Diff {
                text: "- ".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "hello".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
        ];

        let new_expected = vec![
            Diff {
                text: "+ ".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "hello".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
        ];

        assert_eq!(old_expected, old_actual);
        assert_eq!(new_expected, new_actual);
    }

    #[test]
    fn test_single_char_difference() {
        let (old_actual, new_actual) = line_diff("hello", "hallo");

        let old_expected = vec![
            Diff {
                text: "- ".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "h".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "e".to_owned(),
                fg_colour: Color::Black,
                bg_colour: Color::Red,
            },
            Diff {
                text: "llo".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
        ];

        let new_expected = vec![
            Diff {
                text: "+ ".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "h".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "a".to_owned(),
                fg_colour: Color::Black,
                bg_colour: Color::Green,
            },
            Diff {
                text: "llo".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
        ];

        assert_eq!(old_expected, old_actual);
        assert_eq!(new_expected, new_actual);
    }

    #[test]
    fn test_completely_different_strings() {
        let (old_actual, new_actual) = line_diff("foo", "bar");

        let old_expected = vec![
            Diff {
                text: "- ".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "foo".to_owned(),
                fg_colour: Color::Black,
                bg_colour: Color::Red,
            },
        ];

        let new_expected = vec![
            Diff {
                text: "+ ".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "bar".to_owned(),
                fg_colour: Color::Black,
                bg_colour: Color::Green,
            },
        ];

        assert_eq!(old_expected, old_actual);
        assert_eq!(new_expected, new_actual);
    }

    #[test]
    fn test_empty_strings() {
        let (old_actual, new_actual) = line_diff("", "");

        let old_expected = vec![Diff {
            text: "- ".to_owned(),
            fg_colour: Color::Red,
            bg_colour: Color::Reset,
        }];

        let new_expected = vec![Diff {
            text: "+ ".to_owned(),
            fg_colour: Color::Green,
            bg_colour: Color::Reset,
        }];

        assert_eq!(old_expected, old_actual);
        assert_eq!(new_expected, new_actual);
    }

    #[test]
    fn test_addition_at_end() {
        let (old_actual, new_actual) = line_diff("hello", "hello!");

        let old_expected = vec![
            Diff {
                text: "- ".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "hello".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
        ];

        let new_expected = vec![
            Diff {
                text: "+ ".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "hello".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "!".to_owned(),
                fg_colour: Color::Black,
                bg_colour: Color::Green,
            },
        ];

        assert_eq!(old_expected, old_actual);
        assert_eq!(new_expected, new_actual);
    }

    #[test]
    fn test_addition_at_start() {
        let (old_actual, new_actual) = line_diff("hello", "!hello");

        let old_expected = vec![
            Diff {
                text: "- ".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "hello".to_owned(),
                fg_colour: Color::Red,
                bg_colour: Color::Reset,
            },
        ];

        let new_expected = vec![
            Diff {
                text: "+ ".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
            Diff {
                text: "!".to_owned(),
                fg_colour: Color::Black,
                bg_colour: Color::Green,
            },
            Diff {
                text: "hello".to_owned(),
                fg_colour: Color::Green,
                bg_colour: Color::Reset,
            },
        ];

        assert_eq!(old_expected, old_actual);
        assert_eq!(new_expected, new_actual);
    }
}
