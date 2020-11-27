use crate::error::*;

use futures::SinkExt;
use std::{collections::HashMap, net::SocketAddr};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::net::{TcpListener, TcpStream};
use tokio::stream::{Stream, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::{Framed, LinesCodec};

use crate::message::*;
use crate::protocol::*;

type SharedState = Arc<Mutex<ServerState>>;
type Transport = Framed<TcpStream, LinesCodec>;

type Tx = mpsc::UnboundedSender<ServerOperation>;
type Rx = mpsc::UnboundedReceiver<ServerOperation>;

struct Peer {
    transport: Transport,
    rx: Rx,
}

impl Peer {
    async fn register(state: SharedState, addr: SocketAddr, transport: Transport) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        state.lock().await.peers.insert(addr, tx);

        Ok(Self { transport, rx })
    }
}

impl Stream for Peer {
    type Item = Result<ServerOperation>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // First poll the `UnboundedReceiver`.
        if let Poll::Ready(Some(op)) = Pin::new(&mut self.rx).poll_next(cx) {
            return Poll::Ready(Some(Ok(op)));
        }

        // Secondly poll the `Framed` stream.
        let result: Option<_> = futures::ready!(Pin::new(&mut self.transport).poll_next(cx));
        Poll::Ready(match result {
            Some(Ok(de_str)) => {
                let command = serde_json::from_str::<ClientCommand>(&de_str)?;
                Some(Ok(ServerOperation::FromClient(command)))
            }
            Some(Err(e)) => Some(Err(e.into())),
            _ => None,
        })
    }
}

struct ServerState {
    user_count: u32,
    messages: Vec<(User, Message)>,
    peers: HashMap<SocketAddr, Tx>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            user_count: 0,
            messages: Vec::new(),
            peers: HashMap::new(),
        }
    }

    async fn broadcast(&mut self, op: ServerOperation) {
        // broadcast to all peers
        for (&_peer_addr, peer_tx) in self.peers.iter_mut() {
            let _ = peer_tx.send(op.clone());
        }
    }
}

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
                let _ = Self::pre_handle(stream, addr, arc_state).await;
            });
        }
    }

    async fn pre_handle(stream: TcpStream, addr: SocketAddr, arc_state: SharedState) -> Result<()> {
        let transport = Framed::new(stream, LinesCodec::new());
        arc_state.lock().await.user_count += 1;
        let _ = Self::handle(transport, addr, &arc_state).await;
        arc_state.lock().await.user_count -= 1;

        Ok(())
    }

    async fn handle(transport: Transport, addr: SocketAddr, state: &SharedState) -> Result<()> {
        let mut peer = Peer::register(state.clone(), addr, transport).await?;

        let mut name = "<Anonymous>".into();
        while let Some(result) = peer.next().await {
            match result {
                Ok(op) => {
                    match op {
                        ServerOperation::FromClient(command) => match command {
                            ClientCommand::SetName(new_name) => {
                                name = new_name;
                            }
                            ClientCommand::SendMessage(message) => {
                                let mut state = state.lock().await;
                                state.messages.push((name.clone(), message.clone()));
                                state
                                    .broadcast(ServerOperation::FromPeer(
                                        name.clone(),
                                        message.clone(),
                                    ))
                                    .await;
                                println!("[{}({})] {:?}", addr, name, message);
                            }
                        },
                        ServerOperation::FromPeer(user, message) => {
                            // TODO: ServerCommand
                            peer.transport
                                .send(
                                    serde_json::to_string(&ServerCommand::NewMessage(
                                        user, message,
                                    ))
                                    .unwrap(),
                                )
                                .await?;
                        }
                        ServerOperation::FromServer(message) => {
                            // TODO: ServerCommand
                            peer.transport
                                .send(
                                    serde_json::to_string(&ServerCommand::ServerMessage(message))
                                        .unwrap(),
                                )
                                .await?;
                        }
                    }
                }
                Err(e) => {
                    println!("error: {}", e);
                    peer.transport
                        .send(
                            serde_json::to_string(&ServerCommand::ServerMessage(Message::Text(
                                "What's that?".to_owned(),
                            )))
                            .unwrap(),
                        )
                        .await?;
                }
            }
        }

        {
            let mut state = state.lock().await;
            state.peers.remove(&addr);
            let leave_msg = Message::Text(format!("{} left", name));
            state
                .broadcast(ServerOperation::FromServer(leave_msg))
                .await;
        }

        Ok(())
    }
}
