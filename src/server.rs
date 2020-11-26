use crate::error::*;

use std::sync::{Arc, Mutex};
use std::{collections::HashMap, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

use crate::message::Message;
use crate::protocol::{ClientCommand, ServerCommand};
use crate::user::User;

struct ServerState {
    user_count: u32,
    messages: Vec<(User, Message)>,
    all_streams: HashMap<SocketAddr, Arc<TcpStream>>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            user_count: 0,
            messages: Vec::new(),
            all_streams: HashMap::new(),
        }
    }
}

type SharedState = Arc<Mutex<ServerState>>;

pub struct Server {
    listener: TcpListener,
    state: SharedState,
}

impl Server {
    pub async fn new(port: u16) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(("127.0.0.1", port)).await?,
            state: Arc::new(Mutex::new(ServerState::new())),
        })
    }

    pub async fn run(&self) -> Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            let arc_state = self.state.clone();
            tokio::spawn(async move {
                let _ = Self::pre_handle(arc_state, stream, addr).await;
            });
        }
    }

    async fn pre_handle(arc_state: SharedState, stream: TcpStream, addr: SocketAddr) -> Result<()> {
        let arc_stream = Arc::new(stream);

        arc_state
            .lock()
            .unwrap()
            .all_streams
            .insert(addr, arc_stream.clone());

        let _ = Self::handle(arc_stream, addr, &arc_state).await;

        arc_state.lock().unwrap().all_streams.remove(&addr);

        Ok(())
    }

    async fn handle(stream: Arc<TcpStream>, addr: SocketAddr, state: &SharedState) -> Result<()> {
        let mut opt_name = Option::<String>::None;
        let mut buf = [0u8; 4096];
        loop {
            stream.readable().await?;
            stream.try_read(&mut buf)?;
            println!("received!");

            let command = serde_json::from_slice::<ClientCommand>(&buf)?;
            match command {
                ClientCommand::SetName(new_name) => {
                    opt_name = Some(new_name);
                }
                ClientCommand::SendMessage(message) => {
                    let name = opt_name
                        .as_ref()
                        .ok_or(Error::ChatError("no name".into()))?
                        .to_owned();

                    state
                        .lock()
                        .unwrap()
                        .messages
                        .push((name.clone(), message.clone()));

                    println!("[{}] {:?}", name, message);

                    let broadcast_msg =
                        serde_json::to_vec(&ServerCommand::NewMessage((name, message)))?;
                    for (&other_addr, other_stream) in state.lock().unwrap().all_streams.iter() {
                        if addr != other_addr {
                            let _ = other_stream.try_write(&broadcast_msg); // TODO: too inelegant!
                        }
                    }
                }
            }
        }
    }
}
