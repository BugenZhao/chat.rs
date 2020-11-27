use crate::error::*;

use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LinesCodec};

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

        // recv task, rx moved
        tokio::spawn(async move {
            while let Some(result) = rx.next().await {
                match result {
                    Ok(raw_str) => {
                        if let Ok(command) = serde_json::from_str::<ServerCommand>(&raw_str) {
                            // deserialized into ServerCommand
                            match command {
                                ServerCommand::NewMessage(user, message) => {
                                    println!("[{}] {}", user, message);
                                }
                                ServerCommand::ServerMessage(message) => {
                                    println!("<SERVER> {}", message);
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

        // send task
        {
            macro_rules! send {
                ($msg:expr) => {
                    tx.send(serde_json::to_string(&$msg).unwrap()).await?;
                };
            }

            send!(ClientCommand::SetName(self.name.clone()));
            loop {
                // read messages from stdin and send them
                let input = {
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf)?;
                    buf
                };
                send!(ClientCommand::SendMessage(Message::Text(input)));
            }
        }
    }
}
