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
}

impl Client {
    pub fn new(name: &str, server: &str, port: u16) -> Self {
        let client = Self {
            name: name.to_owned(),
            server: server.to_owned(),
            port,
        };
        client
    }

    /// Connect to server and then send/receive messages
    pub async fn run(&self) -> Result<()> {
        let stream = TcpStream::connect((self.server.to_owned(), self.port)).await?;
        let (mut tx, mut rx) = Framed::new(stream, LinesCodec::new()) // split tcp stream data into framed line's
            .split::<String>(); // split the framed stream into two halves

        let (msg_tx, msg_rx) = mpsc::unbounded_channel::<String>();
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<String>();

        // recv task, rx moved
        tokio::spawn(async move {
            while let Some(result) = rx.next().await {
                match result {
                    Ok(raw_str) => {
                        if let Ok(command) = serde_json::from_str::<ServerCommand>(&raw_str) {
                            // deserialized into ServerCommand
                            match command {
                                ServerCommand::NewMessage(user, message) => {
                                    msg_tx.send(format!("[{}] {}", user, message)).unwrap();
                                    // println!("[{}] {}", user, message);
                                }
                                ServerCommand::ServerMessage(message) => {
                                    msg_tx.send(format!("<SERVER> {}", message)).unwrap();
                                }
                            }
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

        // tokio::spawn(async move {
        //     let _ = TuiApp::app_loop(input_tx, msg_rx).await;
        // });
        BasicApp::start(input_tx, msg_rx)?;

        // send task
        macro_rules! send {
            ($msg:expr) => {
                tx.send(serde_json::to_string(&$msg).unwrap()).await?;
            };
        }

        send!(ClientCommand::SetName(self.name.clone()));
        while let Some(text) = input_rx.next().await {
            // read messages from input_rx and send them
            send!(ClientCommand::SendMessage(Message::Text(text)));
        }

        Ok(())
    }
}
