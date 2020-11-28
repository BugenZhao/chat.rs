use crate::error::*;

use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_util::codec::{Framed, LinesCodec};

use crate::app::{App, BasicApp, TuiApp};
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

/// Types of input from the app
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
        let server = (self.server.to_owned(), self.port);
        println!("Connecting to {:?}...", server);
        let stream = TcpStream::connect(server).await?;
        let (mut tcp_tx, mut tcp_rx) = Framed::new(stream, LinesCodec::new()) // split tcp stream data into framed line's
            .split::<String>(); // split the framed stream into two halves

        // the following channels are used to communicate between the client and the app
        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<ServerCommand>();
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<ClientInput>();

        // launch the app task
        if self.tui {
            TuiApp::start(input_tx, msg_rx, &self.name)?;
        } else {
            BasicApp::start(input_tx, msg_rx, &self.name)?;
        }

        // recv task: read from `tcp_rx`, send to `msg_tx`
        let _recv_task = tokio::spawn(async move {
            while let Some(result) = tcp_rx.next().await {
                match result {
                    Ok(raw_str) => {
                        if let Ok(command) = serde_json::from_str::<ServerCommand>(&raw_str) {
                            // deserialized into ServerCommand
                            let _ = msg_tx.send(command);
                        } else {
                            let _ = msg_tx.send(ServerCommand::Error(raw_str));
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        // send task: read from `input_rx`, send to `tcp_tx`
        let _send_task = {
            macro_rules! send {
                ($msg:expr) => {
                    let _ = tcp_tx.send(serde_json::to_string(&$msg).unwrap()).await;
                };
            }

            // set name first to register
            send!(ClientCommand::SetName(self.name.clone()));

            while let Some(input) = input_rx.next().await {
                match input {
                    ClientInput::Text(text) => {
                        // read messages from input_rx(app) and send them
                        send!(ClientCommand::SendMessage(Message::Text(text)));
                    }
                    ClientInput::Exit => {
                        break;
                    }
                }
            }
        };

        Ok(())
    }
}
