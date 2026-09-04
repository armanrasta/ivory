//! Swarm service, handle, and events.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ivory_core::{Block, Transaction};
use ivory_primitives::H256;
use libp2p::futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm, SwarmBuilder, gossipsub, identify, kad};
use tokio::sync::mpsc;

use crate::behaviour::{IvoryBehaviour, IvoryBehaviourEvent, TOPIC_BLOCKS, TOPIC_SYNC, TOPIC_TXS};
use crate::codec::NetworkMessage;
use crate::error::NetworkError;

/// Listen address and optional bootstrap peers.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// TCP listen multiaddr (use `/ip4/127.0.0.1/tcp/0` in tests).
    pub listen: Multiaddr,
    /// Peers to dial at start.
    pub bootstrap: Vec<Multiaddr>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen: "/ip4/127.0.0.1/tcp/0".parse().expect("static multiaddr"),
            bootstrap: Vec::new(),
        }
    }
}

/// Inbound network notifications.
#[derive(Clone, Debug)]
pub enum NetworkEvent {
    /// A block arrived on gossip or sync.
    BlockReceived(Block),
    /// A transaction arrived on gossip.
    TxReceived(Transaction),
    /// Peer asked for a block hash (node should reply if it has it).
    BlockRequest(H256),
    /// New connection.
    PeerConnected(PeerId),
    /// Connection lost.
    PeerDisconnected(PeerId),
    /// Bound listen address (with resolved port).
    ListenAddr(Multiaddr),
}

enum Command {
    BroadcastBlock(Block),
    BroadcastTx(Transaction),
    RequestBlock(H256),
    Dial(Multiaddr),
}

/// Cloneable control plane for the swarm task.
#[derive(Clone, Debug)]
pub struct NetworkHandle {
    tx: mpsc::UnboundedSender<Command>,
    peer_id: PeerId,
    peers: Arc<AtomicUsize>,
}

impl NetworkHandle {
    /// Local libp2p peer id.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Live connection count (established minus closed).
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.load(Ordering::Relaxed)
    }

    /// Shared counter for RPC `ivory_nodeInfo`.
    #[must_use]
    pub fn peer_count_handle(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.peers)
    }

    /// Gossip a sealed block.
    ///
    /// # Errors
    ///
    /// [`NetworkError::Stopped`] if the swarm task has exited.
    pub fn broadcast_block(&self, block: Block) -> Result<(), NetworkError> {
        self.tx
            .send(Command::BroadcastBlock(block))
            .map_err(|_| NetworkError::Stopped)
    }

    /// Gossip a transaction.
    ///
    /// # Errors
    ///
    /// [`NetworkError::Stopped`] if the swarm task has exited.
    pub fn broadcast_transaction(&self, tx: Transaction) -> Result<(), NetworkError> {
        self.tx
            .send(Command::BroadcastTx(tx))
            .map_err(|_| NetworkError::Stopped)
    }

    /// Ask peers for a missing block (unknown parent walk).
    ///
    /// # Errors
    ///
    /// [`NetworkError::Stopped`] if the swarm task has exited.
    pub fn request_block(&self, hash: H256) -> Result<(), NetworkError> {
        self.tx
            .send(Command::RequestBlock(hash))
            .map_err(|_| NetworkError::Stopped)
    }

    /// Dial a peer multiaddr.
    ///
    /// # Errors
    ///
    /// [`NetworkError::Stopped`] if the swarm task has exited.
    pub fn dial(&self, addr: Multiaddr) -> Result<(), NetworkError> {
        self.tx
            .send(Command::Dial(addr))
            .map_err(|_| NetworkError::Stopped)
    }
}

/// Start the swarm on a background task.
///
/// # Errors
///
/// Transport, listen, or behaviour construction failures.
pub async fn start(
    config: NetworkConfig,
) -> Result<(NetworkHandle, mpsc::UnboundedReceiver<NetworkEvent>), NetworkError> {
    let mut swarm = build_swarm()?;
    swarm
        .listen_on(config.listen.clone())
        .map_err(|e| NetworkError::Swarm(e.to_string()))?;

    let peer_id = *swarm.local_peer_id();
    for addr in &config.bootstrap {
        swarm
            .dial(addr.clone())
            .map_err(|e| NetworkError::Swarm(e.to_string()))?;
        if let Some(pid) = peer_id_from_multiaddr(addr) {
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&pid, addr.clone());
        }
    }

    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let peers = Arc::new(AtomicUsize::new(0));

    tokio::spawn(run_loop(swarm, cmd_rx, event_tx, Arc::clone(&peers)));

    Ok((
        NetworkHandle {
            tx: cmd_tx,
            peer_id,
            peers,
        },
        event_rx,
    ))
}

fn build_swarm() -> Result<Swarm<IvoryBehaviour>, NetworkError> {
    let swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default().nodelay(true),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .map_err(|e| NetworkError::Swarm(e.to_string()))?
        .with_behaviour(|key| IvoryBehaviour::new(key).expect("gossipsub config"))
        .map_err(|e| NetworkError::Swarm(e.to_string()))?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();
    Ok(swarm)
}

fn peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|p| match p {
        Protocol::P2p(peer) => Some(peer),
        _ => None,
    })
}

fn publish(
    swarm: &mut Swarm<IvoryBehaviour>,
    topic: &str,
    msg: &NetworkMessage,
) -> Result<(), NetworkError> {
    let data = msg.encode()?;
    match swarm
        .behaviour_mut()
        .gossipsub
        .publish(gossipsub::IdentTopic::new(topic), data)
    {
        Ok(_) => Ok(()),
        Err(gossipsub::PublishError::InsufficientPeers) => Ok(()),
        Err(e) => Err(NetworkError::Publish(e.to_string())),
    }
}

async fn run_loop(
    mut swarm: Swarm<IvoryBehaviour>,
    mut cmd_rx: mpsc::UnboundedReceiver<Command>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    peers: Arc<AtomicUsize>,
) {
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else {
                    break;
                };
                if let Err(e) = handle_command(&mut swarm, cmd) {
                    tracing::debug!(error = %e, "network command failed");
                }
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(&mut swarm, event, &event_tx, &peers);
            }
        }
    }
}

fn handle_command(swarm: &mut Swarm<IvoryBehaviour>, cmd: Command) -> Result<(), NetworkError> {
    match cmd {
        Command::BroadcastBlock(block) => {
            publish(swarm, TOPIC_BLOCKS, &NetworkMessage::Block(block))
        }
        Command::BroadcastTx(tx) => publish(swarm, TOPIC_TXS, &NetworkMessage::Transaction(tx)),
        Command::RequestBlock(hash) => publish(swarm, TOPIC_SYNC, &NetworkMessage::GetBlock(hash)),
        Command::Dial(addr) => swarm
            .dial(addr)
            .map_err(|e| NetworkError::Swarm(e.to_string())),
    }
}

fn handle_swarm_event(
    swarm: &mut Swarm<IvoryBehaviour>,
    event: SwarmEvent<IvoryBehaviourEvent>,
    event_tx: &mpsc::UnboundedSender<NetworkEvent>,
    peers: &AtomicUsize,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            let _ = event_tx.send(NetworkEvent::ListenAddr(address));
        }
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
            peers.fetch_add(1, Ordering::Relaxed);
            let _ = event_tx.send(NetworkEvent::PeerConnected(peer_id));
        }
        SwarmEvent::ConnectionClosed { peer_id, .. } => {
            swarm
                .behaviour_mut()
                .gossipsub
                .remove_explicit_peer(&peer_id);
            let _ = peers.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_sub(1))
            });
            let _ = event_tx.send(NetworkEvent::PeerDisconnected(peer_id));
        }
        SwarmEvent::Behaviour(IvoryBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            message,
            ..
        })) => match NetworkMessage::decode(&message.data) {
            Ok(NetworkMessage::Block(block)) => {
                let _ = event_tx.send(NetworkEvent::BlockReceived(block));
            }
            Ok(NetworkMessage::Transaction(tx)) => {
                let _ = event_tx.send(NetworkEvent::TxReceived(tx));
            }
            Ok(NetworkMessage::GetBlock(hash)) => {
                let _ = event_tx.send(NetworkEvent::BlockRequest(hash));
            }
            Err(_) => {
                tracing::debug!("dropping malformed gossip payload");
            }
        },
        SwarmEvent::Behaviour(IvoryBehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
            peer,
            ..
        })) => {
            tracing::trace!(%peer, "kademlia routing updated");
        }
        SwarmEvent::Behaviour(IvoryBehaviourEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        })) => {
            for addr in info.listen_addrs {
                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
            }
        }
        _ => {}
    }
}
