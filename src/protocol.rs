use crate::message::*;

use serde::{Deserialize, Serialize};

/// Command from client to server
#[derive(Serialize, Deserialize, Clone)]
pub enum ClientCommand {
    SetName(String),
    SendMessage(Message),
}

/// Command from server to client
#[derive(Serialize, Deserialize, Clone)]
pub enum ServerCommand {
    NewMessage(User, Message),
    ServerMessage(Message)
}

/// Peer (inside the server) needs to receive messages from...
/// - client (client command)
/// - server (notification)
/// - other peers (message broadcast)
///
/// Use this enum to identify among them.
#[derive(Clone)]
pub enum ServerOperation {
    FromClient(ClientCommand),
    FromPeer(User, Message),
    FromServer(Message),
}
