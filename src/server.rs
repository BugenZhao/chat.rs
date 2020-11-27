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

use log::*;

use crate::message::*;
use crate::protocol::*;

type SharedState = Arc<Mutex<ServerState>>;
type Transport = Framed<TcpStream, LinesCodec>;

type Tx = mpsc::UnboundedSender<ServerOperation>;
type Rx = mpsc::UnboundedReceiver<ServerOperation>;

/// Peer representing a registered user
/// - transport: a framed tcp stream, used for communicating between server and client
/// - rx: the recv half of the inter-peer channels, used for **receiving** broadcast messages from other peers
struct Peer {
    transport: Transport,
    rx: Rx,
}

impl Peer {
    /// Will allocate a channel, insert the send half into shared state, and return a Peer which owns the recv half and the transport
    async fn register(state: SharedState, addr: SocketAddr, transport: Transport) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        state.lock().await.peers.insert(addr, tx);

        Ok(Self { transport, rx })
    }
}

impl Stream for Peer {
    type Item = Result<ServerOperation>;

    /// Poll ServerOperation's from both transport and rx, so that we can use `next()` to receive all kinds of ops
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

/// The state of a server, will be shared among all peers through `Arc<Mutex<_>>`
struct ServerState {
    history: Vec<(User, Message)>,
    peers: HashMap<SocketAddr, Tx>, // send halves of all peers
}

impl ServerState {
    fn new() -> Self {
        Self {
            history: Vec::new(),
            peers: HashMap::new(),
        }
    }

    /// Broadcast an operation to all peers through their send halves in the state
    async fn broadcast(&mut self, op: ServerOperation) {
        for (&_peer_addr, peer_tx) in self.peers.iter_mut() {
            let _ = peer_tx.send(op.clone());
        }
    }
}

/// The chat server
pub struct Server {
    listener: TcpListener,
    state: SharedState,
}

impl Server {
    pub async fn new(port: u16) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(("0.0.0.0", port)).await?,
            state: Arc::new(Mutex::new(ServerState::new())),
        })
    }

    /// An infinite loop that accepts connections and then spawn tasks to process
    pub async fn run(&self) -> Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            let arc_state = self.state.clone();
            tokio::spawn(async move {
                let transport = Framed::new(stream, LinesCodec::new());
                let _ = Self::handle(transport, addr, arc_state).await;
            });
        }
    }

    /// Connection handler
    async fn handle(transport: Transport, addr: SocketAddr, state: SharedState) -> Result<()> {
        let mut peer = Peer::register(state.clone(), addr, transport).await?; // register the new peer in shared state
        let mut name = "".to_string();

        macro_rules! server_log{
            ($($x:expr),+) => {
                info!("[{}({})] {}", addr, name, format!($($x),+));
            }
        }
        macro_rules! send {
            ($msg:expr) => {
                peer.transport
                    .send(serde_json::to_string(&$msg).unwrap())
                    .await?;
            };
        }

        server_log!("joined");

        // poll all kinds of server ops
        while let Some(result) = peer.next().await {
            match result {
                Ok(op) => match op {
                    ServerOperation::FromClient(command) => match command {
                        // a request from the client
                        ClientCommand::SetName(new_name) => {
                            if new_name.is_empty() {
                                continue;
                            }
                            server_log!("change name to: {}", new_name);
                            if name.is_empty() {
                                state
                                    .lock()
                                    .await
                                    .broadcast(ServerOperation::FromServer(Message::Text(format!(
                                        "Welcome, {}!",
                                        new_name
                                    ))))
                                    .await;
                            }
                            name = new_name;
                        }
                        _ if name.is_empty() => {
                            // commands without name are ignored
                            continue;
                        }
                        ClientCommand::SendMessage(message) => {
                            let mut state = state.lock().await;
                            state.history.push((name.clone(), message.clone()));
                            // send FromPeer ops to broadcast the latest incoming message to all peers
                            state
                                .broadcast(ServerOperation::FromPeer(name.clone(), message.clone()))
                                .await;
                            server_log!("{:?}", message);
                        }
                    },
                    ServerOperation::FromPeer(user, message) => {
                        // a broadcast from other peers
                        send!(&ServerCommand::NewMessage(user, message));
                    }
                    ServerOperation::FromServer(message) => {
                        // a message from server itself, forward to the client
                        send!(&ServerCommand::ServerMessage(message));
                    }
                },
                Err(e) => {
                    server_log!("error: {}", e);
                    send!(&ServerCommand::ServerMessage(Message::Text(
                        "What's that?".to_owned(),
                    )));
                }
            }
        }

        // release resources
        {
            let mut state = state.lock().await;
            state.peers.remove(&addr);
            let leave_msg = Message::Text(format!("{} left", name));
            state
                .broadcast(ServerOperation::FromServer(leave_msg))
                .await;
            server_log!("left");
        }

        Ok(())
    }
}
