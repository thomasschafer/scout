use crate::app::AppEvent;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    App(AppEvent),
    #[allow(dead_code)]
    Mouse(MouseEvent),
    #[allow(dead_code)]
    Resize(u16, u16),
}

#[derive(Debug)]
pub struct EventHandler {
    pub receiver: mpsc::UnboundedReceiver<Event>,
    pub app_event_sender: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (app_event_sender, mut app_event_receiver) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut reader = event::EventStream::new();
            loop {
                tokio::select! {
                    Some(Ok(evt)) = reader.next() => {
                        match evt {
                            CrosstermEvent::Key(key) => {
                                if key.kind == crossterm::event::KeyEventKind::Press {
                                    sender.send(Event::Key(key)).unwrap();
                                }
                            },
                            CrosstermEvent::Mouse(mouse) => {
                                sender.send(Event::Mouse(mouse)).unwrap();
                            },
                            CrosstermEvent::Resize(x, y) => {
                                sender.send(Event::Resize(x, y)).unwrap();
                            },
                            _ => {}
                        }
                    }
                    Some(app_evt) = app_event_receiver.recv() => {
                        sender.send(Event::App(app_evt)).unwrap();
                    }
                    else => break,
                };
            }
        });
        Self {
            receiver,
            app_event_sender,
        }
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}
