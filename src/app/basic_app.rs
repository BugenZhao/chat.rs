use tokio::sync::mpsc;

use crate::{client::ClientInput, error::*, protocol::ServerCommand};

type Tx<T> = mpsc::UnboundedSender<T>;
type Rx<T> = mpsc::UnboundedReceiver<T>;

/// A basic app which does not split input and output
pub struct BasicApp {}

impl super::App for BasicApp {
    fn start(input_tx: Tx<ClientInput>, mut msg_rx: Rx<ServerCommand>, name: &str) -> Result<()> {
        println!("Joined as `{}`.", name);

        let _in_task = tokio::spawn(async move {
            loop {
                let input = {
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf
                };
                // send msg to client
                input_tx.send(ClientInput::Text(input)).unwrap();
            }
        });

        let _out_task = tokio::spawn(async move {
            // recv command from client
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
                        let msg = format!("<SERVER> Unknown: {}", message);
                        println!("{}", msg);
                    }
                    ServerCommand::ServerName(name) => {
                        println!("<SERVER> Server's name is `{}`", name);
                    }
                }
            }
        });

        Ok(())
    }
}
