use metrics::{Counter, counter};
use sf_peer_id::PeerID;
use std::collections::HashMap;
use tracing::info;

use crate::peer::Peer;

#[derive(Debug)]
pub struct PeerManager {
    /// Stores the peers managed by this manager.
    peers: HashMap<PeerID, Peer>,
    /// Tracks the IDs of all the peers discovered
    known_peer_ids: Vec<PeerID>,
    /// Tracks the number of peers added
    peer_count: Counter,
}

impl PeerManager {
    pub fn new() -> Self {
        let peer_count = counter!("peer_manager.peer_count");
        Self {
            peers: HashMap::new(),
            known_peer_ids: Vec::new(),
            peer_count,
        }
    }

    pub fn add_peer(&mut self, peer: Peer) {
        let id = peer.id();
        info!("Added peer: {}", id);
        self.peers.insert(*id, peer.clone());
        self.add_known_peer_id(*id);
        self.peer_count.increment(1);
    }

    pub fn get_peer(&self, id: &PeerID) -> Option<&Peer> {
        self.peers.get(id)
    }

    //pub fn remove_peer(&mut self, id: &PeerID) {
    //    info!("removing peer: {id}");
    //    self.peers.remove(id);
    //}

    pub fn add_known_peer_id(&mut self, id: PeerID) {
        if self.add_known_peer_id_internal(id) {
            info!("Added known peer ID: {}", id);
        }
    }

    #[inline]
    fn add_known_peer_id_internal(&mut self, peer_id: PeerID) -> bool {
        if self.known_peer_ids.contains(&peer_id) {
            return false;
        }
        self.known_peer_ids.push(peer_id);
        true
    }

    #[inline]
    pub fn get_known_peer_ids(&self) -> Vec<PeerID> {
        self.known_peer_ids.clone()
    }
}
