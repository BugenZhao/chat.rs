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

type StyledString = (String, Style);

/// An app with a clear terminal UI
#[derive(Default)]
pub struct TuiApp {
    input: String,
    last_input: String,
    messages: Vec<StyledString>,
    username: User,
    users: Vec<StyledString>,
    server_name: String,
}

/// Events from both stdin and the client that the tui app must respond,
/// generated by different tasks
#[derive(Debug)]
enum AppEvent {
    Key(termion::event::Key),          // stdin: key pressed
    Message(StyledString),             // msg_rx: new message to show
    UserList(Vec<(User, SocketAddr)>), // msg_rx: updated user list
    ServerName(String),                // msg_rx: server name to show
}

impl super::App for TuiApp {
    fn start(input_tx: Tx<ClientInput>, mut msg_rx: Rx<ServerCommand>, name: &str) -> Result<()> {
        // init tui
        let stdout = std::io::stdout().into_raw_mode()?;
        let stdout = AlternateScreen::from(stdout);
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut app = Self {
            username: name.to_owned(),
            server_name: "Chat".to_string(),
            ..Self::default()
        };

        // TODO: why does TOKIO mpsc channel fail to work perperly?
        let (event_tx, event_rx) = std::sync::mpsc::channel();

        // receive msg(command)s forwarded from client,
        // then wrap them into `AppEvent` and send through `event_tx`
        let _msg_task = {
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
                                .send(AppEvent::Message((msg, Style::default())))
                                .unwrap();
                        }
                        ServerCommand::ServerMessage(message) => {
                            let msg = format!("=> {}", message);
                            event_tx
                                .send(AppEvent::Message((
                                    msg,
                                    Style::default().add_modifier(Modifier::BOLD),
                                )))
                                .unwrap();
                        }
                        ServerCommand::UserList(users) => {
                            event_tx.send(AppEvent::UserList(users)).unwrap();
                        }
                        ServerCommand::Error(message) => {
                            let msg = format!("=> Error: {}", message);
                            event_tx
                                .send(AppEvent::Message((
                                    msg,
                                    Style::default()
                                        .add_modifier(Modifier::BOLD)
                                        .fg(Color::LightRed),
                                )))
                                .unwrap();
                        }

                        ServerCommand::ServerName(name) => {
                            event_tx.send(AppEvent::ServerName(name)).unwrap();
                        }
                    }
                }
            })
        };

        // receive keyboard input from stdin,
        // then wrap them into `AppEvent` and send through `event_tx`
        let _key_task = {
            // let event_tx = event_tx.clone();
            tokio::spawn(async move {
                let mut keys = std::io::stdin().keys();
                while let Some(Ok(key)) = keys.next() {
                    let _ = event_tx.send(AppEvent::Key(key));
                }
            })
        };

        // tui task, `app` & `event_rx` moved
        let _tui_task = tokio::spawn(async move {
            let mut exited = false;
            loop {
                // draw tui
                terminal
                    .draw(|f| {
                        // chunk represents an area
                        /*
                        |        chunks[0]        |
                        | schunks[0] | schunks[1] | <- chunks[1]
                        |        chunks[2]        |
                        */
                        let chunks = Layout::default()
                            .margin(1)
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Max(2),
                                Constraint::Percentage(70),
                                Constraint::Percentage(20),
                            ])
                            .split(f.size());

                        let schunks = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                            .split(chunks[1]);

                        // --------
                        let help_widget = Paragraph::new(Text::from(Spans::from(vec![
                            Span::styled(
                                app.server_name.clone(),
                                Style::default()
                                    .add_modifier(Modifier::ITALIC)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(" -- Press "),
                            Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(" or send "),
                            Span::styled(":exit", Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(" to exit"),
                        ])));
                        f.render_widget(help_widget, chunks[0]);

                        // --------
                        let messages: Vec<_> = app
                            .messages
                            .iter()
                            .rev()
                            .take((chunks[1].height - 2) as usize) // take visiable ones
                            .rev()
                            .map(|(c, s)| ListItem::new(Span::styled(c, s.clone())))
                            .collect();
                        let message_widget = List::new(messages)
                            .block(Block::default().borders(Borders::ALL).title("Messages"));
                        f.render_widget(message_widget, schunks[0]);

                        // --------
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

                        // --------
                        let input_widget = Paragraph::new(app.input.as_ref())
                            .style(Style::default().fg(Color::Yellow))
                            .wrap(Wrap { trim: true })
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title(app.username.clone()),
                            );
                        f.render_widget(input_widget, chunks[2]);

                        // --------
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

                // receive event from other tasks
                match event_rx.recv() {
                    // keyboard
                    Ok(AppEvent::Key(key)) => match key {
                        // return key
                        Key::Char('\n') if app.input.is_empty() => {}
                        Key::Char('\n') => {
                            let text: String = app.input.drain(..).collect();
                            app.last_input = text.clone();

                            if text.starts_with(":") {
                                // client command
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
                                // normal message
                                input_tx.send(ClientInput::Text(text)).unwrap();
                            }
                        }
                        // push new char
                        Key::Char(char) if app.input.len() < 140 => {
                            app.input.push(char);
                        }
                        // pop the last char
                        Key::Backspace => {
                            app.input.pop();
                        }
                        // escape
                        Key::Esc => {
                            input_tx.send(ClientInput::Exit).unwrap();
                            exited = true;
                        }
                        // fetch last input
                        Key::Up if app.input.is_empty() => {
                            app.input = app.last_input.clone();
                        }
                        _ => {}
                    },
                    // show message
                    Ok(AppEvent::Message(content)) => {
                        app.messages.push(content);
                    }
                    // update user list
                    Ok(AppEvent::UserList(users)) => {
                        app.users = users
                            .into_iter()
                            .map(|(n, _a)| n)
                            .map(|s| (s, Style::default()))
                            .collect();
                    }
                    // update server name
                    Ok(AppEvent::ServerName(name)) => {
                        app.server_name = name;
                    }
                    Err(_) => {}
                }
            }
        });

        Ok(())
    }
}
