pub enum CurrentScreen {
    Searching,
    Confirmation,
    Results,
}

pub struct App {
    pub current_screen: CurrentScreen,
    search_text: String,
    cursor_idx: usize,
}

impl App {
    pub fn new() -> App {
        App {
            current_screen: CurrentScreen::Searching,
            search_text: String::new(),
            cursor_idx: 0,
        }
    }

    pub fn search_text(&self) -> &str {
        self.search_text.as_str()
    }

    pub fn cursor_idx(&self) -> usize {
        self.cursor_idx
    }

    pub fn move_cursor_left(&mut self) {
        self.move_cursor_left_by(1)
    }

    pub fn move_cursor_start(&mut self) {
        self.cursor_idx = 0;
    }

    fn move_cursor_left_by(&mut self, n: usize) {
        let cursor_moved_left = self.cursor_idx.saturating_sub(n);
        self.cursor_idx = self.clamp_cursor(cursor_moved_left);
    }

    pub fn move_cursor_right(&mut self) {
        self.move_cursor_right_by(1)
    }

    fn move_cursor_right_by(&mut self, n: usize) {
        let cursor_moved_right = self.cursor_idx.saturating_add(n);
        self.cursor_idx = self.clamp_cursor(cursor_moved_right);
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_idx = self.search_text.chars().count();
    }

    pub fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.search_text.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&mut self) -> usize {
        self.search_text
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_idx)
            .unwrap_or(self.search_text.len())
    }

    pub fn delete_char(&mut self) {
        if self.cursor_idx == 0 {
            return;
        }

        let before_char = self.search_text.chars().take(self.cursor_idx - 1);
        let after_char = self.search_text.chars().skip(self.cursor_idx);

        self.search_text = before_char.chain(after_char).collect();
        self.move_cursor_left();
    }

    pub fn delete_char_forward(&mut self) {
        let before_char = self.search_text.chars().take(self.cursor_idx);
        let after_char = self.search_text.chars().skip(self.cursor_idx + 1);

        self.search_text = before_char.chain(after_char).collect();
    }

    fn previous_word_start(&self) -> usize {
        if self.cursor_idx == 0 {
            return 0;
        }

        let before_char = self
            .search_text
            .chars()
            .take(self.cursor_idx)
            .collect::<Vec<_>>();
        let mut idx = self.cursor_idx - 1;
        while idx > 0 && before_char[idx] == ' ' {
            idx -= 1;
        }
        while idx > 0 && before_char[idx] != ' ' {
            idx -= 1;
        }
        idx
    }

    pub fn move_cursor_back_word(&mut self) {
        self.cursor_idx = self.previous_word_start();
    }

    pub fn delete_word_backward(&mut self) {
        let new_cursor_pos = self.previous_word_start();
        let before_char = self.search_text.chars().take(new_cursor_pos);
        let after_char = self.search_text.chars().skip(self.cursor_idx);

        self.search_text = before_char.chain(after_char).collect();
        self.cursor_idx = new_cursor_pos;
    }

    fn next_word_start(&self) -> usize {
        let after_char = self
            .search_text
            .chars()
            .skip(self.cursor_idx)
            .collect::<Vec<_>>();
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

    pub fn move_cursor_forward_word(&mut self) {
        self.cursor_idx = self.next_word_start();
    }

    pub fn delete_word_forward(&mut self) {
        let before_char = self.search_text.chars().take(self.cursor_idx);
        let after_char = self.search_text.chars().skip(self.next_word_start());

        self.search_text = before_char.chain(after_char).collect();
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.search_text.chars().count())
    }

    pub fn clear_search_text(&mut self) {
        self.search_text.clear();
        self.cursor_idx = 0;
    }
}
