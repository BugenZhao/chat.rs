use crate::message::*;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum ClientCommand {
    SetName(String),
    SendMessage(Message),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ServerCommand {
    NewMessage(User, Message),
    ServerMessage(Message)
}

#[derive(Clone)]
pub enum ServerOperation {
    FromClient(ClientCommand),
    FromPeer(User, Message),
    FromServer(Message),
}
