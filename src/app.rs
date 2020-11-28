use std::net::SocketAddr;

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
                    ServerCommand::UserMessage(user, message) => {
                        let msg = format!("[{}] {}", user, message);
                        println!("{}", msg);
                    }
                    ServerCommand::ServerMessage(message) => {
                        let msg = format!("<SERVER> {}", message);
                        println!("{}", msg);
                    }
                    ServerCommand::UserList(users) => {
                        let msg = format!("<SERVER> Online users: {:?}", users);
                        println!("{}", msg);
                    }
                    ServerCommand::Error(message) => {
                        let msg = format!("<SERVER> unknown: {}", message);
                        println!("{}", msg);
                    }
                }
            }
        });

        Ok(())
    }
}

type TuiStyledString = (String, Style);

#[derive(Default)]
pub struct TuiApp {
    input: String,
    last_input: String,
    messages: Vec<TuiStyledString>,
    name: User,
    users: Vec<TuiStyledString>,
}

#[derive(Debug)]
enum TuiAppEvent {
    Key(termion::event::Key),
    Message(TuiStyledString),
    UserList(Vec<(User, SocketAddr)>),
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
                        ServerCommand::UserMessage(user, message) => {
                            let msg = format!(
                                "[{}, {}] {}",
                                user,
                                chrono::Local::now().format("%H:%M:%S"),
                                message
                            );
                            event_tx
                                .send(TuiAppEvent::Message((msg, Style::default())))
                                .unwrap();
                        }
                        ServerCommand::ServerMessage(message) => {
                            let msg = format!("=> {}", message);
                            event_tx
                                .send(TuiAppEvent::Message((
                                    msg,
                                    Style::default().add_modifier(Modifier::BOLD),
                                )))
                                .unwrap();
                        }
                        ServerCommand::UserList(users) => {
                            event_tx.send(TuiAppEvent::UserList(users)).unwrap();
                        }
                        ServerCommand::Error(message) => {
                            let msg = format!("=> Error: {}", message);
                            event_tx
                                .send(TuiAppEvent::Message((
                                    msg,
                                    Style::default()
                                        .add_modifier(Modifier::BOLD)
                                        .fg(Color::LightRed),
                                )))
                                .unwrap();
                        }
                    }
                }
            })
        };

        let _key_handler = {
            // let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let mut keys = std::io::stdin().keys();
                while let Some(Ok(key)) = keys.next() {
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
                                Constraint::Percentage(75),
                                Constraint::Percentage(25),
                            ])
                            .split(f.size());

                        let schunks = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                            .split(chunks[1]);

                        let help_widget = Paragraph::new(Text::from(Spans::from(vec![
                            Span::styled(
                                " Chat ",
                                Style::default()
                                    .add_modifier(Modifier::ITALIC)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw("-- Press "),
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
                            .map(|(c, s)| ListItem::new(Span::styled(c, s.clone())))
                            .collect();
                        let message_widget = List::new(messages)
                            .block(Block::default().borders(Borders::ALL).title("Messages"));
                        f.render_widget(message_widget, schunks[0]);

                        let users: Vec<_> = app
                            .users
                            .iter()
                            .map(|(c, s)| ListItem::new(Span::styled(c, s.clone())))
                            .collect();
                        let users_widget = List::new(users).block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(format!("{} Users", app.users.len())),
                        );
                        f.render_widget(users_widget, schunks[1]);

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
                    })
                    .unwrap();

                if exited {
                    break;
                }

                match event_rx.recv() {
                    Ok(TuiAppEvent::Key(key)) => match key {
                        Key::Char('\n') if !app.input.is_empty() => {
                            let text: String = app.input.drain(..).collect();
                            app.last_input = text.clone();
                            if text.starts_with(":") {
                                match text[1..].to_lowercase().as_str() {
                                    "exit" => {
                                        input_tx.send(ClientInput::Exit).unwrap();
                                        exited = true;
                                    }
                                    "clear" => app.messages.clear(),

                                    "fuck" => app.messages.push((
                                        "=> What's your problem?".to_string(),
                                        Style::default().fg(Color::Red),
                                    )),
                                    cmd @ _ => app.messages.push((
                                        format!("=> Invalid command `{}`", cmd),
                                        Style::default().fg(Color::Red),
                                    )),
                                }
                            } else {
                                input_tx.send(ClientInput::Text(text)).unwrap();
                            }
                        }
                        Key::Char(char) if app.input.len() < 140 && !char.is_whitespace() => {
                            app.input.push(char);
                        }
                        Key::Backspace => {
                            app.input.pop();
                        }
                        Key::Esc => {
                            input_tx.send(ClientInput::Exit).unwrap();
                            exited = true;
                        }
                        Key::Up if app.input.is_empty() => {
                            app.input = app.last_input.clone();
                        }
                        _ => {}
                    },
                    Ok(TuiAppEvent::Message(content)) => {
                        app.messages.push(content);
                    }
                    Ok(TuiAppEvent::UserList(users)) => {
                        app.users = users
                            .into_iter()
                            .map(|(n, _a)| n)
                            .map(|s| (s, Style::default()))
                            .collect();
                    }
                    _ => {}
                }
            }
            drop(terminal);
        });

        Ok(())
    }
}
