use crate::error::*;

use std::sync::{Arc, Mutex};
use std::{collections::HashMap, net::SocketAddr};
use tokio::net::TcpStream;

use crate::message::Message;
use crate::protocol::{ClientCommand, ServerCommand};
use crate::user::User;

pub struct Client {
    name: String,
    stream: Arc<TcpStream>,
}

impl Client {
    async fn send_command(&self, command: ClientCommand) -> Result<()> {
        self.stream.writable().await?;
        self.stream.try_write(&serde_json::to_vec(&command)?)?;

        Ok(())
    }

    pub async fn new(name: &str, server: &str, port: u16) -> Result<Self> {
        let stream = TcpStream::connect((server.to_owned(), port)).await?;
        let client = Self {
            name: name.to_owned(),
            stream: Arc::new(stream),
        };
        Ok(client)
    }

    pub async fn run(&self) -> Result<()> {
        self.send_command(ClientCommand::SetName(self.name.to_owned()))
            .await?;
        let stream = self.stream.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                stream.readable().await;
                stream.try_read(&mut buf);
                let command = serde_json::from_slice::<ServerCommand>(&buf).unwrap();

                match command {
                    ServerCommand::NewMessage((name, message)) => {
                        println!("[{}] {:?}", name, message);
                    }
                }
            }
        });

        loop {
            let msg_content = {
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf)?;
                buf
            };
            self.send_command(ClientCommand::SendMessage(Message::Text(msg_content)))
                .await?;
        }
    }
}
