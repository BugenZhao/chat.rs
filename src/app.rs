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

use crate::error::*;

type Tx<T> = mpsc::UnboundedSender<T>;
type Rx<T> = mpsc::UnboundedReceiver<T>;

// pub trait App {
//     fn start(input_tx: Tx<String>, msg_rx: Rx<String>) -> Result<()>;
// }

pub struct BasicApp {}

impl BasicApp {
    pub fn start(input_tx: Tx<String>, mut msg_rx: Rx<String>) -> Result<()> {
        tokio::spawn(async move {
            loop {
                let input = {
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf
                };
                input_tx.send(input).unwrap();
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

#[derive(Default)]
pub struct TuiApp {
    input: String,
    messages: Vec<String>,
}

#[derive(Debug)]
enum TuiAppEvent {
    Key(termion::event::Key),
    Message(String),
}

impl TuiApp {
    pub async fn app_loop(input_tx: Tx<String>, mut msg_rx: Rx<String>) -> Result<()> {
        let stdout = std::io::stdout().into_raw_mode()?;
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let mut app = Self::default();

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                while let Some(content) = msg_rx.next().await {
                    event_tx.send(TuiAppEvent::Message(content)).unwrap();
                }
            });
        }

        {
            let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let mut keys = std::io::stdin().keys();
                for _ in 1..5 {
                    event_tx.send(TuiAppEvent::Key(Key::Char('c'))).unwrap();
                    event_tx.send(TuiAppEvent::Key(Key::Char('\n'))).unwrap();
                }

                loop {
                    if let Some(Ok(key)) = keys.next() {
                        event_tx.send(TuiAppEvent::Key(key)).unwrap();
                        println!("{:?}", key);
                    }
                }
            });
        }

        loop {
            terminal
                .draw(|f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                        .split(f.size());

                    let input_widget = Paragraph::new(app.input.as_ref())
                        .wrap(Wrap { trim: true })
                        .block(Block::default().borders(Borders::ALL).title("Input"));
                    f.render_widget(input_widget, chunks[0]);

                    let messages: Vec<_> = app
                        .messages
                        .iter()
                        .map(|m| ListItem::new(Span::raw(m)))
                        .collect();
                    let message_widget = List::new(messages)
                        .block(Block::default().borders(Borders::ALL).title("Messages"));
                    f.render_widget(message_widget, chunks[1]);
                })
                .unwrap();

            match event_rx.recv().await {
                Some(TuiAppEvent::Key(key)) => match key {
                    Key::Char('\n') => {
                        let text = app.input.drain(..).collect();
                        input_tx.send(text).unwrap();
                    }
                    Key::Char(char) => {
                        // println!("{}", char);
                        app.input.push(char);
                    }
                    Key::Backspace => {
                        app.input.pop();
                    }
                    Key::Esc => {
                        println!("ESCAPE");
                        break;
                    }
                    _ => {}
                },
                Some(TuiAppEvent::Message(content)) => {
                    // println!("{}", content);
                    app.messages.push(content);
                }
                None => {}
            }
        }

        Ok(())
    }
}
