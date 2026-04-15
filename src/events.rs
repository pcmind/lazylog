use crossterm::event::EventStream;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct Events {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl Events {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick = time::interval(tick_rate);

            loop {
                let tx = tx_clone.clone();
                tokio::select! {
                    _ = tick.tick() => {
                        if tx.send(Event::Tick).is_err() {
                            break;
                        }
                    }
                    Some(Ok(evt)) = reader.next() => {
                        let event = match evt {
                            CrosstermEvent::Key(key) => Event::Key(key),
                            CrosstermEvent::Mouse(mouse) => Event::Mouse(mouse),
                            CrosstermEvent::Resize(x, y) => Event::Resize(x, y),
                            CrosstermEvent::FocusGained | CrosstermEvent::FocusLost | CrosstermEvent::Paste(_) => continue,
                        };
                        if tx.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
