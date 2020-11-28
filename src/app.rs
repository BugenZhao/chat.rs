use termion::event::Key;
use termion::{input::TermRead, raw::IntoRawMode, screen::AlternateScreen};
use tokio::{stream::StreamExt, sync::mpsc};
use tokio_util::codec::{BytesCodec, FramedRead, LinesCodec};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use unicode_width::UnicodeWidthStr;

use crate::{client::ClientInput, error::*};

type Tx<T> = mpsc::UnboundedSender<T>;
type Rx<T> = mpsc::UnboundedReceiver<T>;

// pub trait App {
//     fn start(input_tx: Tx<String>, msg_rx: Rx<String>) -> Result<()>;
// }

pub struct BasicApp {}

impl BasicApp {
    pub fn start(input_tx: Tx<ClientInput>, mut msg_rx: Rx<String>) -> Result<()> {
        tokio::spawn(async move {
            loop {
                let input = {
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf
                };
                input_tx.send(ClientInput::Text(input)).unwrap();
            }
        });

        tokio::spawn(async move {
            while let Some(content) = msg_rx.recv().await {
                println!("{}", content);
            }
        });

        Ok(())
    }
}

struct AsyncKeys {
    keys: termion::input::Keys<std::io::Stdin>,
}

impl AsyncKeys {
    fn new() -> Self {
        Self {
            keys: std::io::stdin().keys(),
        }
    }

    async fn next(
        &mut self,
    ) -> std::option::Option<std::result::Result<termion::event::Key, std::io::Error>> {
        return self.keys.next();
    }
}

#[derive(Default)]
pub struct TuiApp {
    input: String,
    messages: Vec<String>,
    name: String,
}

#[derive(Debug)]
enum TuiAppEvent {
    Key(termion::event::Key),
    Message(String),
}

impl TuiApp {
    pub fn start(input_tx: Tx<ClientInput>, mut msg_rx: Rx<String>, name: &str) -> Result<()> {
        let stdout = std::io::stdout().into_raw_mode()?;
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let mut app = Self {
            name: name.to_owned(),
            ..Self::default()
        };

        let (event_tx, event_rx) = std::sync::mpsc::channel();

        let _msg_handler = {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                while let Some(content) = msg_rx.next().await {
                    if event_tx.send(TuiAppEvent::Message(content)).is_err() {
                        break;
                    }
                }
            })
        };

        let _key_handler = {
            // let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let mut async_keys = AsyncKeys::new();
                while let Some(Ok(key)) = async_keys.next().await {
                    let _ = event_tx.send(TuiAppEvent::Key(key));
                }
            })
        };

        tokio::spawn(async move {
            let mut exited = false;
            loop {
                terminal
                    .draw(|f| {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                            .split(f.size());

                        let input_widget = Paragraph::new(app.input.as_ref())
                            .wrap(Wrap { trim: true })
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title(app.name.clone()),
                            );
                        f.render_widget(input_widget, chunks[0]);

                        let messages: Vec<_> = app
                            .messages
                            .iter()
                            .map(|m| ListItem::new(Span::raw(m)))
                            .collect();
                        let message_widget = List::new(messages)
                            .block(Block::default().borders(Borders::ALL).title("Messages"));
                        f.render_widget(message_widget, chunks[1]);

                        let x =
                            chunks[0].x + (app.input.width() as u16 % (chunks[0].width - 2)) + 1;
                        let y =
                            chunks[0].y + (app.input.width() as u16 / (chunks[0].width - 2)) + 1;
                        f.set_cursor(x, y);
                    })
                    .unwrap();

                if exited {
                    break;
                }

                match event_rx.recv() {
                    Ok(TuiAppEvent::Key(key)) => match key {
                        Key::Char('\n') => {
                            let text = app.input.drain(..).collect();
                            if text == ":exit" {
                                input_tx.send(ClientInput::Exit).unwrap();
                                exited = true;
                            } else {
                                input_tx.send(ClientInput::Text(text)).unwrap();
                            }
                        }
                        Key::Char(char) => {
                            if app.input.len() < 140 {
                                app.input.push(char);
                            }
                        }
                        Key::Backspace => {
                            app.input.pop();
                        }
                        Key::Esc => {
                            input_tx.send(ClientInput::Exit).unwrap();
                            exited = true;
                        }
                        _ => {}
                    },
                    Ok(TuiAppEvent::Message(content)) => {
                        app.messages.push(content);
                    }
                    _ => {}
                }
            }
            drop(terminal);
        });

        Ok(())
    }
}