use crate::{message::Message, user::User};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum ClientCommand {
    SetName(String),
    SendMessage(Message),
}

#[derive(Serialize, Deserialize)]
pub enum ServerCommand {
    NewMessage((User, Message)),
}
