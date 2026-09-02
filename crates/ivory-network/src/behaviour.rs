//! Combined libp2p behaviour.

use std::time::Duration;

use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, gossipsub, identify, kad, ping};

use crate::error::NetworkError;

/// Gossipsub topic for sealed blocks.
pub const TOPIC_BLOCKS: &str = "ivory/blocks/1";
/// Gossipsub topic for transactions.
pub const TOPIC_TXS: &str = "ivory/txs/1";
/// Gossipsub topic for missing-block requests and responses.
pub const TOPIC_SYNC: &str = "ivory/sync/1";

/// Identify protocol string.
pub const PROTOCOL_VERSION: &str = "ivory/1.0.0";

/// Ivory swarm behaviour: gossip + identify + ping + kademlia.
#[derive(NetworkBehaviour)]
pub struct IvoryBehaviour {
    /// Block / tx / sync gossip.
    pub gossipsub: gossipsub::Behaviour,
    /// Protocol/peer info.
    pub identify: identify::Behaviour,
    /// Liveness.
    pub ping: ping::Behaviour,
    /// Peer discovery.
    pub kademlia: kad::Behaviour<MemoryStore>,
}

impl IvoryBehaviour {
    /// Build subscribed gossip + discovery behaviours.
    ///
    /// # Errors
    ///
    /// [`NetworkError::Swarm`] if gossipsub config is invalid.
    pub fn new(keypair: &Keypair) -> Result<Self, NetworkError> {
        let peer_id = PeerId::from(keypair.public());
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_millis(200))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .mesh_n(1)
            .mesh_n_low(1)
            .mesh_n_high(3)
            .mesh_outbound_min(0)
            .build()
            .map_err(|e| NetworkError::Swarm(e.to_string()))?;

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        )
        .map_err(|e| NetworkError::Swarm(e.to_string()))?;

        gossipsub
            .subscribe(&gossipsub::IdentTopic::new(TOPIC_BLOCKS))
            .map_err(|e| NetworkError::Swarm(e.to_string()))?;
        gossipsub
            .subscribe(&gossipsub::IdentTopic::new(TOPIC_TXS))
            .map_err(|e| NetworkError::Swarm(e.to_string()))?;
        gossipsub
            .subscribe(&gossipsub::IdentTopic::new(TOPIC_SYNC))
            .map_err(|e| NetworkError::Swarm(e.to_string()))?;

        let identify = identify::Behaviour::new(identify::Config::new(
            PROTOCOL_VERSION.to_string(),
            keypair.public(),
        ));
        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(10)));
        let kademlia = kad::Behaviour::new(peer_id, MemoryStore::new(peer_id));

        Ok(Self {
            gossipsub,
            identify,
            ping,
            kademlia,
        })
    }
}
