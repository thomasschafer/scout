pub enum CurrentScreen {
    Searching,
    Confirmation,
    Results,
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub search_text: String,
}

impl App {
    pub fn new() -> App {
        App {
            current_screen: CurrentScreen::Searching,
            search_text: String::new(),
        }
    }
}
