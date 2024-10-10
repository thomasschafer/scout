use crate::app::AppEvent;
use anyhow::anyhow;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures::StreamExt;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    App(AppEvent),
}

#[derive(Debug)]
pub struct EventHandler {
    pub sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
    pub app_event_sender: mpsc::UnboundedSender<AppEvent>,
    #[allow(dead_code)]
    handler: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (app_event_sender, mut app_event_receiver) = mpsc::unbounded_channel();
        let _sender = sender.clone();
        let handler = tokio::spawn(async move {
            let mut reader = event::EventStream::new();
            loop {
                tokio::select! {
                    Some(Ok(evt)) = reader.next() => {
                        match evt {
                            CrosstermEvent::Key(key) => {
                                if key.kind == crossterm::event::KeyEventKind::Press {
                                    _sender.send(Event::Key(key)).unwrap();
                                }
                            },
                            CrosstermEvent::Mouse(mouse) => {
                                _sender.send(Event::Mouse(mouse)).unwrap();
                            },
                            CrosstermEvent::Resize(x, y) => {
                                _sender.send(Event::Resize(x, y)).unwrap();
                            },
                            _ => {}
                        }
                    }
                    Some(app_evt) = app_event_receiver.recv() => {
                        _sender.send(Event::App(app_evt)).unwrap();
                    }
                    else => break,
                };
            }
        });
        Self {
            sender,
            receiver,
            app_event_sender,
            handler,
        }
    }

    pub async fn next(&mut self) -> anyhow::Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or(anyhow!("Event stream ended unexpectedly"))
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}
