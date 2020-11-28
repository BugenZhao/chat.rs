use std::net::SocketAddr;

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
    UserMessage(User, Message),
    ServerMessage(Message),
    UserList(Vec<(User, SocketAddr)>),
    Error(String),
}

/// Peer (inside the server) needs to receive messages from...
/// - client (client command)
/// - server (notification)
/// - other peers (message broadcast)
///
/// Use this enum to identify among them.
#[derive(Clone)]
pub enum Operation {
    FromClient(ClientCommand),
    FromPeer(User, Message),
    FromServer(ServerCommand),
}
