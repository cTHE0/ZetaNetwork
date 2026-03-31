use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::HashMap;

use crate::lib_p2p::*;
use crate::post::Post;
use crate::storage::Storage;
use crate::crypto::KeyPair;

pub type SharedStorage = Arc<Mutex<Storage>>;
pub type SharedPeers = Arc<Mutex<HashMap<SocketAddr, (String, u64)>>>;  // addr -> (pubkey, last_seen)
pub type SharedNetworkNodes = Arc<Mutex<Vec<NetworkNode>>>;  // Liste des nœuds du réseau

pub struct NetworkState {
    pub socket: Arc<UdpSocket>,
    pub storage: SharedStorage,
    pub peers: SharedPeers,
    pub network_nodes: SharedNetworkNodes,
    pub keypair: KeyPair,
    pub public_addr: SocketAddr,
    pub hub_addr: SocketAddr,
}

impl NetworkState {
    pub fn new(
        socket: UdpSocket,
        storage: Storage,
        keypair: KeyPair,
        public_addr: SocketAddr,
        hub_addr: SocketAddr,
    ) -> Self {
        NetworkState {
            socket: Arc::new(socket),
            storage: Arc::new(Mutex::new(storage)),
            peers: Arc::new(Mutex::new(HashMap::new())),
            network_nodes: Arc::new(Mutex::new(Vec::new())),
            keypair,
            public_addr,
            hub_addr,
        }
    }

    pub async fn broadcast_post(&self, post: &Post) {
        let msg = Message::PublishPost { post: post.clone() };
        let peers = self.peers.lock().await;
        for (addr, _) in peers.iter() {
            if let Err(e) = self.socket.send_msg(&msg, *addr).await {
                eprintln!("[WARN] Failed to send post to {}: {}", addr, e);
            }
        }
        println!("[NET] Post diffusé à {} peers", peers.len());
    }

    pub async fn request_posts_from_peers(&self, since: u64, pubkeys: Vec<String>) {
        let msg = Message::RequestPosts {
            src_addr: self.public_addr,
            since,
            pubkeys,
        };
        let peers = self.peers.lock().await;
        for (addr, _) in peers.iter() {
            if let Err(e) = self.socket.send_msg(&msg, *addr).await {
                eprintln!("[WARN] Failed to request posts from {}: {}", addr, e);
            }
        }
    }

    pub async fn announce_self(&self) {
        let msg = Message::NodeAnnounce {
            addr: self.public_addr,
            pubkey: self.keypair.public_hex(),
            time: now_secs(),
        };
        let peers = self.peers.lock().await;
        for (addr, _) in peers.iter() {
            let _ = self.socket.send_msg(&msg, *addr).await;
        }
    }

    pub async fn request_network_nodes(&self) {
        let msg = Message::GetAllNodes {
            src_addr: self.public_addr,
            time: now_secs(),
        };
        if let Err(e) = self.socket.send_msg(&msg, self.hub_addr).await {
            eprintln!("[WARN] Failed to request network nodes: {}", e);
        }
    }

    pub async fn get_network_nodes(&self) -> Vec<NetworkNode> {
        self.network_nodes.lock().await.clone()
    }

    pub async fn add_peer(&self, addr: SocketAddr, pubkey: String) {
        let mut peers = self.peers.lock().await;
        peers.insert(addr, (pubkey.clone(), now_secs()));

        // Persister dans storage
        let storage = self.storage.lock().await;
        let _ = storage.save_peer(&addr.to_string(), Some(&pubkey), now_secs());
    }

    pub async fn handle_message(&self, msg: Message, sender_addr: SocketAddr) {
        println!("{}", msg);

        match msg {
            Message::PublishPost { post } => {
                // Vérifier la signature
                if !post.verify() {
                    eprintln!("[WARN] Post invalide reçu (signature incorrecte)");
                    return;
                }
                // Stocker le post (acceptation automatique pour l'instant)
                let storage = self.storage.lock().await;
                if let Err(e) = storage.save_post(&post) {
                    eprintln!("[ERROR] Failed to save post: {}", e);
                }
                println!("[NET] Post stocké: {}", post);
            }

            Message::RequestPosts { src_addr, since, pubkeys } => {
                // Récupérer les posts demandés
                let storage = self.storage.lock().await;
                let posts = if pubkeys.is_empty() {
                    storage.get_posts_since(since).unwrap_or_default()
                } else {
                    storage.get_posts_by_authors(&pubkeys, 100).unwrap_or_default()
                        .into_iter()
                        .filter(|p| p.timestamp > since)
                        .collect()
                };
                drop(storage);

                // Envoyer la réponse
                let response = Message::PostsBatch { posts };
                if let Err(e) = self.socket.send_msg(&response, src_addr).await {
                    eprintln!("[WARN] Failed to send posts batch: {}", e);
                }
            }

            Message::PostsBatch { posts } => {
                // Stocker les posts reçus (après vérification)
                let storage = self.storage.lock().await;
                for post in posts {
                    if post.verify() {
                        let _ = storage.save_post(&post);
                    }
                }
            }

            Message::NodeAnnounce { addr, pubkey, time } => {
                // Ajouter/mettre à jour le peer
                let mut peers = self.peers.lock().await;
                peers.insert(addr, (pubkey.clone(), time));
                drop(peers);

                let storage = self.storage.lock().await;
                let _ = storage.save_peer(&addr.to_string(), Some(&pubkey), time);
            }

            Message::GetPeers { src_addr, .. } => {
                let peers = self.peers.lock().await;
                let peers_list: Vec<(SocketAddr, String)> = peers
                    .iter()
                    .map(|(a, (pk, _))| (*a, pk.clone()))
                    .collect();
                drop(peers);

                let response = Message::PeersList { peers: peers_list };
                let _ = self.socket.send_msg(&response, src_addr).await;
            }

            Message::PeersList { peers } => {
                for (addr, pubkey) in peers {
                    if addr != self.public_addr {
                        self.add_peer(addr, pubkey).await;
                    }
                }
            }

            Message::AllNodesList { nodes } => {
                let mut network_nodes = self.network_nodes.lock().await;
                *network_nodes = nodes;
                println!("[NET] Liste des nœuds mise à jour ({} nœuds)", network_nodes.len());
            }

            Message::Register { src_addr, src_id, time, .. } => {
                // Un peer s'enregistre
                let mut peers = self.peers.lock().await;
                peers.insert(src_addr, (src_id.clone(), time));
            }

            _ => {
                // Messages non gérés ici (traités ailleurs)
            }
        }
    }

    pub async fn cleanup_old_peers(&self) {
        let mut peers = self.peers.lock().await;
        let now = now_secs();
        peers.retain(|addr, (_, last_seen)| {
            let active = now - *last_seen < 300;  // 5 minutes timeout
            if !active {
                println!("[INFO] Peer {} déconnecté (timeout)", addr);
            }
            active
        });
    }
}
