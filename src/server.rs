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

type Tx = mpsc::UnboundedSender<Operation>;
type Rx = mpsc::UnboundedReceiver<Operation>;

/// Representing a registered user
/// - transport: a framed tcp stream, used for communicating between server and client
/// - rx: the recv half of the inter-peer channels, used for **receiving** broadcast messages from other peers
struct RecvPeer {
    transport: Transport,
    rx: Rx,
}

struct SendPeer {
    tx: Tx,
    username: User,
    addr: SocketAddr,
}

impl RecvPeer {
    /// Will allocate a channel, insert the send half into shared state, and return a Peer which owns the recv half and the transport
    async fn register(state: SharedState, addr: SocketAddr, transport: Transport) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        state.lock().await.peers.insert(
            addr,
            SendPeer {
                tx,
                username: User::new(),
                addr,
            },
        );

        Ok(Self { transport, rx })
    }
}

impl Stream for RecvPeer {
    type Item = Result<Operation>;

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
                Some(Ok(Operation::FromClient(command)))
            }
            Some(Err(e)) => Some(Err(e.into())),
            _ => None,
        })
    }
}

/// The state of a server, will be shared among all peers through `Arc<Mutex<_>>`
#[derive(Default)]
struct ServerState {
    name: String,
    history: Vec<(User, Message)>,
    peers: HashMap<SocketAddr, SendPeer>, // send halves of all peers
}

impl ServerState {
    /// Broadcast an operation to all peers through their send halves in the state
    fn broadcast(&mut self, op: Operation, excludes: Vec<SocketAddr>) {
        for (_peer_addr, peer) in self.peers.iter_mut() {
            if !excludes.contains(_peer_addr) {
                let _ = peer.tx.send(op.clone());
            }
        }
    }

    fn broadcast_user_list(&mut self) {
        let users = self
            .peers
            .values()
            .map(|p| (p.username.clone(), p.addr))
            .filter(|(n, _a)| !n.is_empty())
            .collect();
        self.broadcast(
            Operation::FromServer(ServerCommand::UserList(users)),
            vec![],
        );
    }
}

/// The chat server
pub struct Server {
    listener: TcpListener,
    state: SharedState,
}

impl Server {
    pub async fn new(port: u16, name: String) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(("0.0.0.0", port)).await?,
            state: Arc::new(Mutex::new(ServerState {
                name,
                ..ServerState::default()
            })),
        })
    }

    /// An infinite loop that accepts connections and then spawn tasks to process
    pub async fn run(&self) -> Result<()> {
        log::info!("listen on {:?}", self.listener.local_addr()?);

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
        let mut peer = RecvPeer::register(state.clone(), addr, transport).await?; // register the new peer in shared state
        let mut name = "".to_string();

        macro_rules! log{
            ($level:ident, $($x:expr),+) => {
                log::$level!("[{}({})] {}", addr, name, format!($($x),+));
            }
        }
        macro_rules! send {
            ($msg:expr) => {
                peer.transport
                    .send(serde_json::to_string(&$msg).unwrap())
                    .await?;
            };
        }

        log!(info, "joined");

        // poll all kinds of server ops
        while let Some(result) = peer.next().await {
            match result {
                Ok(op) => {
                    match op {
                        Operation::FromClient(command) => match command {
                            // a request from the client
                            ClientCommand::SetName(new_name) => {
                                if new_name.is_empty() {
                                    continue;
                                }
                                log!(info, "change name to: {}", new_name);

                                {
                                    let mut state = state.lock().await;
                                    if let Some(send_peer) = state.peers.get_mut(&addr) {
                                        send_peer.username = new_name.clone();
                                    }
                                    if name.is_empty() {
                                        state.broadcast(
                                            Operation::FromServer(ServerCommand::ServerMessage(
                                                Message::Text(format!("Welcome, {}!", new_name)),
                                            )),
                                            vec![],
                                        );
                                        send!(&ServerCommand::ServerName(state.name.clone()));
                                    }
                                    state.broadcast_user_list();
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
                                state.broadcast(
                                    Operation::FromPeer(name.clone(), message.clone()),
                                    vec![],
                                );
                                log!(info, "{:?}", message);
                            }
                        },
                        Operation::FromPeer(user, message) => {
                            // a broadcast from other peers
                            send!(&ServerCommand::UserMessage(user, message));
                        }
                        Operation::FromServer(message) => {
                            // a message from server itself, straightly forward to the client
                            send!(message);
                        }
                    }
                }
                Err(e) => {
                    log!(warn, "error: {}", e);
                    send!(&ServerCommand::Error("What's that?".to_owned(),));
                }
            }
        }

        // release resources
        {
            let mut state = state.lock().await;
            state.peers.remove(&addr);
            let leave_msg = Message::Text(format!("{} left.", name));
            let op = Operation::FromServer(ServerCommand::ServerMessage(leave_msg));
            state.broadcast(op, vec![]);
            state.broadcast_user_list();
            log!(info, "left");
        }

        Ok(())
    }
}
