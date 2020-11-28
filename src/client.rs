use crate::error::*;

use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_util::codec::{Framed, LinesCodec};

use crate::app::{BasicApp, TuiApp};
use crate::message::*;
use crate::protocol::*;

type Transport = Framed<TcpStream, LinesCodec>;
type Tx = SplitSink<Transport, String>;
type Rx = SplitStream<Transport>;

/// The chat client
pub struct Client {
    name: String,
    server: String,
    port: u16,
    tui: bool,
}

#[derive(Debug)]
pub enum ClientInput {
    Text(String),
    Exit,
}

impl Client {
    pub fn new(name: &str, server: &str, port: u16, tui: bool) -> Self {
        let client = Self {
            name: name.to_owned(),
            server: server.to_owned(),
            port,
            tui,
        };
        client
    }

    /// Connect to server and then send/receive messages
    pub async fn run(&self) -> Result<()> {
        let stream = TcpStream::connect((self.server.to_owned(), self.port)).await?;
        let (mut tcp_tx, mut tcp_rx) = Framed::new(stream, LinesCodec::new()) // split tcp stream data into framed line's
            .split::<String>(); // split the framed stream into two halves

        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<ServerCommand>();
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<ClientInput>();

        // recv task, rx moved
        tokio::spawn(async move {
            while let Some(result) = tcp_rx.next().await {
                match result {
                    Ok(raw_str) => {
                        if let Ok(command) = serde_json::from_str::<ServerCommand>(&raw_str) {
                            // deserialized into ServerCommand
                            let _ = msg_tx.send(command);
                        } else {
                            println!("error: unknown server command: {}", raw_str);
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        if self.tui {
            TuiApp::start(input_tx, msg_rx, &self.name)?;
        } else {
            BasicApp::start(input_tx, msg_rx)?;
        }

        // send task
        macro_rules! send {
            ($msg:expr) => {
                tcp_tx.send(serde_json::to_string(&$msg).unwrap()).await?;
            };
        }

        send!(ClientCommand::SetName(self.name.clone()));
        while let Some(input) = input_rx.next().await {
            match input {
                ClientInput::Text(text) => {
                    // read messages from input_rx and send them
                    send!(ClientCommand::SendMessage(Message::Text(text)));
                }
                ClientInput::Exit => {
                    break;
                }
            }
        }

        Ok(())
    }
}
