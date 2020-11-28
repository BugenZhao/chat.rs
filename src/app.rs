use termion::event::Key;
use termion::{input::TermRead, raw::IntoRawMode, screen::AlternateScreen};
use tokio::{stream::StreamExt, sync::mpsc};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use unicode_width::UnicodeWidthStr;

use crate::{client::ClientInput, error::*, message::User, protocol::ServerCommand};

type Tx<T> = mpsc::UnboundedSender<T>;
type Rx<T> = mpsc::UnboundedReceiver<T>;

// pub trait App {
//     fn start(input_tx: Tx<String>, msg_rx: Rx<String>) -> Result<()>;
// }

pub struct BasicApp {}

impl BasicApp {
    pub fn start(input_tx: Tx<ClientInput>, mut msg_rx: Rx<ServerCommand>) -> Result<()> {
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
            while let Some(command) = msg_rx.recv().await {
                match command {
                    ServerCommand::NewMessage(user, message) => {
                        let msg = format!("[{}] {}", user, message);
                        println!("{}", msg);
                    }
                    ServerCommand::ServerMessage(message) => {
                        let msg = format!("<SERVER> {}", message);
                        println!("{}", msg);
                    }
                    // TODO:
                    ServerCommand::NewUserList(users) => {}
                }
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
    name: User,
    users: Vec<User>,
}

#[derive(Debug)]
enum TuiAppEvent {
    Key(termion::event::Key),
    Message(String),
    UserList(Vec<User>),
}

impl TuiApp {
    pub fn start(
        input_tx: Tx<ClientInput>,
        mut msg_rx: Rx<ServerCommand>,
        name: &str,
    ) -> Result<()> {
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
                while let Some(command) = msg_rx.next().await {
                    match command {
                        ServerCommand::NewMessage(user, message) => {
                            let msg = format!("[{}] {}", user, message);
                            event_tx.send(TuiAppEvent::Message(msg)).unwrap();
                        }
                        ServerCommand::ServerMessage(message) => {
                            let msg = format!("<SERVER> {}", message);
                            event_tx.send(TuiAppEvent::Message(msg)).unwrap();
                        }
                        // TODO:
                        ServerCommand::NewUserList(users) => {
                            event_tx.send(TuiAppEvent::UserList(users)).unwrap();
                        }
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
                            .margin(1)
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Min(2),
                                Constraint::Percentage(50),
                                Constraint::Percentage(20),
                                Constraint::Percentage(30),
                            ])
                            .split(f.size());

                        let help_widget = Paragraph::new(Text::from(Spans::from(vec![
                            Span::styled(
                                " Chat -- ",
                                Style::default()
                                    .add_modifier(Modifier::ITALIC)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw("Press "),
                            Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(" or send "),
                            Span::styled(":exit", Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(" to exit"),
                        ])));
                        f.render_widget(help_widget, chunks[0]);

                        let messages: Vec<_> = app
                            .messages
                            .iter()
                            .rev()
                            .take((chunks[1].height - 2) as usize)
                            .rev()
                            .map(|m| ListItem::new(Span::raw(m)))
                            .collect();
                        let message_widget = List::new(messages)
                            .block(Block::default().borders(Borders::ALL).title("Messages"));
                        f.render_widget(message_widget, chunks[1]);

                        let input_widget = Paragraph::new(app.input.as_ref())
                            .style(Style::default().fg(Color::Yellow))
                            .wrap(Wrap { trim: true })
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title(app.name.clone()),
                            );
                        f.render_widget(input_widget, chunks[2]);

                        let x =
                            chunks[2].x + (app.input.width() as u16 % (chunks[2].width - 2)) + 1;
                        let y =
                            chunks[2].y + (app.input.width() as u16 / (chunks[2].width - 2)) + 1;
                        f.set_cursor(x, y);

                        let users: Vec<_> = app
                            .users
                            .iter()
                            .map(|u| ListItem::new(Span::raw(u)))
                            .collect();
                        let users_widget = List::new(users);
                        f.render_widget(users_widget, chunks[3]);
                    })
                    .unwrap();

                if exited {
                    break;
                }

                match event_rx.recv() {
                    Ok(TuiAppEvent::Key(key)) => match key {
                        Key::Char('\n') => {
                            let text: String = app.input.drain(..).collect();
                            if text == ":exit" {
                                input_tx.send(ClientInput::Exit).unwrap();
                                exited = true;
                            } else if !text.is_empty() {
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
                    Ok(TuiAppEvent::UserList(users)) => {
                        app.users = users;
                    }
                    _ => {}
                }
            }
            drop(terminal);
        });

        Ok(())
    }
}
