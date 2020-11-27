use crate::error::*;

use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use std::{collections::HashMap, net::SocketAddr};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use crate::message::*;
use crate::protocol::*;

type Transport = Framed<TcpStream, LinesCodec>;
type Tx = SplitSink<Transport, String>;
type Rx = SplitStream<Transport>;

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

    pub async fn run(&self) -> Result<()> {
        let stream = TcpStream::connect((self.server.to_owned(), self.port)).await?;
        let (mut tx, mut rx) = Framed::new(stream, LinesCodec::new()).split::<String>();

        tokio::spawn(async move {
            while let Some(result) = rx.next().await {
                match result {
                    Ok(raw_str) => {
                        if let Ok(command) = serde_json::from_str::<ServerCommand>(&raw_str) {
                            match command {
                                ServerCommand::NewMessage(user, message) => {
                                    println!("[{}] {:?}", user, message);
                                }
                                ServerCommand::ServerMessage(message) => {
                                    println!("<SERVER> {:?}", message);
                                }
                            }
                        } else {
                            println!("error: unknown server command: {}", raw_str);
                        }
                    }
                    Err(_) => {}
                }
            }
        });

        macro_rules! send {
            ($msg:expr) => {
                tx.send(serde_json::to_string(&$msg).unwrap()).await?;
            };
        }
        
        send!(ClientCommand::SetName(self.name.clone()));
        loop {
            let input = {
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf)?;
                buf
            };
            send!(ClientCommand::SendMessage(Message::Text(input)));
        }
    }
}
