use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Paragraph},
    Frame,
};

#[derive(Default)]
pub(crate) struct TextField {
    text: String,
    cursor_idx: usize,
}

impl TextField {
    pub(crate) fn text(&self) -> String {
        self.text.to_owned()
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

    fn handle_keys(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match (code, modifiers) {
            (KeyCode::Char('w'), KeyModifiers::CONTROL)
            | (KeyCode::Backspace, KeyModifiers::ALT) => {
                self.delete_word_backward();
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL)
            | (KeyCode::Backspace, KeyModifiers::META) => {
                self.clear();
            }
            (KeyCode::Backspace, _) => {
                self.delete_char();
            }
            (KeyCode::Left | KeyCode::Char('b') | KeyCode::Char('B'), _)
                if modifiers.contains(KeyModifiers::ALT) =>
            {
                self.move_cursor_back_word();
            }
            (KeyCode::Home, _) => {
                self.move_cursor_start();
            }
            (KeyCode::Left, _) => {
                self.move_cursor_left();
            }
            (KeyCode::Right | KeyCode::Char('f') | KeyCode::Char('F'), _)
                if modifiers.contains(KeyModifiers::ALT) =>
            {
                self.move_cursor_forward_word();
            }
            (KeyCode::Right, KeyModifiers::META) => {
                self.move_cursor_end();
            }
            (KeyCode::End, _) => {
                self.move_cursor_end();
            }
            (KeyCode::Right, _) => {
                self.move_cursor_right();
            }
            (KeyCode::Char('d'), KeyModifiers::ALT) | (KeyCode::Delete, KeyModifiers::ALT) => {
                self.delete_word_forward();
            }
            (KeyCode::Delete, _) => {
                self.delete_char_forward();
            }
            (KeyCode::Char(value), _) => {
                self.enter_char(value);
            }
            (_, _) => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct CheckboxField {
    pub(crate) checked: bool,
}
impl CheckboxField {
    fn handle_keys(&mut self, code: KeyCode, _modifiers: KeyModifiers) {
        if code == KeyCode::Char(' ') {
            self.checked = !self.checked;
        }
    }
}

pub enum Field {
    Text(TextField),
    Checkbox(CheckboxField),
}

impl Field {
    pub(crate) fn text() -> Field {
        Field::Text(TextField::default())
    }

    pub(crate) fn checkbox() -> Field {
        Field::Checkbox(CheckboxField::default())
    }

    pub(crate) fn handle_keys(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match self {
            Field::Text(f) => f.handle_keys(code, modifiers),
            Field::Checkbox(f) => f.handle_keys(code, modifiers),
        }
    }

    pub(crate) fn cursor_idx(&self) -> Option<usize> {
        match self {
            Field::Text(f) => Some(f.cursor_idx()),
            Field::Checkbox(_) => None,
        }
    }

    pub(crate) fn render(&self, frame: &mut Frame, area: Rect, title: String, highlighted: bool) {
        let mut block = Block::bordered();
        if highlighted {
            block = block.border_style(Style::new().green());
        }

        match self {
            Field::Text(f) => {
                block = block.title(title);
                frame.render_widget(Paragraph::new(f.text()).block(block), area);
            }
            Field::Checkbox(f) => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(5), Constraint::Min(0)])
                    .split(area);
                frame.render_widget(
                    Paragraph::new(if f.checked { " X " } else { "" }).block(block),
                    chunks[0],
                );
                frame.render_widget(
                    Paragraph::new(Text::styled(
                        format!("\n {}", title),
                        if highlighted {
                            Color::Green
                        } else {
                            Color::Reset
                        },
                    )),
                    chunks[1],
                );
            }
        }
    }
}
