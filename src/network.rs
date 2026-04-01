use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::{HashMap, HashSet};

use crate::lib_p2p::*;
use crate::post::Post;
use crate::storage::Storage;
use crate::crypto::KeyPair;

pub type SharedStorage = Arc<Mutex<Storage>>;
pub type SharedPeers = Arc<Mutex<HashMap<SocketAddr, (String, u64)>>>;  // addr -> (pubkey, last_seen)
pub type SharedNetworkNodes = Arc<Mutex<Vec<NetworkNode>>>;  // Liste des nœuds du réseau
pub type SeenPosts = Arc<Mutex<HashSet<String>>>;  // IDs des posts déjà vus (pour éviter les boucles)

pub struct NetworkState {
    pub socket: Arc<UdpSocket>,
    pub storage: SharedStorage,
    pub peers: SharedPeers,
    pub network_nodes: SharedNetworkNodes,
    pub seen_posts: SeenPosts,
    pub keypair: KeyPair,
    pub public_addr: SocketAddr,
    pub hub_addr: SocketAddr,
    pub is_relay: bool,
}

impl NetworkState {
    pub fn new(
        socket: UdpSocket,
        storage: Storage,
        keypair: KeyPair,
        public_addr: SocketAddr,
        hub_addr: SocketAddr,
        is_relay: bool,
    ) -> Self {
        NetworkState {
            socket: Arc::new(socket),
            storage: Arc::new(Mutex::new(storage)),
            peers: Arc::new(Mutex::new(HashMap::new())),
            network_nodes: Arc::new(Mutex::new(Vec::new())),
            seen_posts: Arc::new(Mutex::new(HashSet::new())),
            keypair,
            public_addr,
            hub_addr,
            is_relay,
        }
    }

    pub async fn broadcast_post(&self, post: &Post) {
        // Marquer ce post comme vu pour ne pas le relayer en boucle
        {
            let mut seen = self.seen_posts.lock().await;
            seen.insert(post.id.clone());
        }

        let msg = Message::PublishPost { post: post.clone() };

        // Envoyer uniquement aux relays connus
        let nodes = self.network_nodes.lock().await;
        for node in nodes.iter() {
            if node.is_relay && node.addr != self.public_addr {
                let _ = self.socket.send_msg(&msg, node.addr).await;
            }
        }

        println!("[NET] Post envoyé aux relays");
    }

    pub async fn request_posts_from_peers(&self, since: u64, pubkeys: Vec<String>) {
        let msg = Message::RequestPosts {
            src_addr: self.public_addr,
            since,
            pubkeys,
        };

        // Envoyer uniquement aux relays connus
        let nodes = self.network_nodes.lock().await;
        for node in nodes.iter() {
            if node.is_relay && node.addr != self.public_addr {
                let _ = self.socket.send_msg(&msg, node.addr).await;
            }
        }
    }

    pub async fn announce_self(&self) {
        let msg = Message::NodeAnnounce {
            addr: self.public_addr,
            pubkey: self.keypair.public_hex(),
            is_relay: self.is_relay,
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
                // Vérifier si on a déjà vu ce post (éviter les boucles)
                // On vérifie AVANT la signature pour économiser du CPU
                let is_new = {
                    let mut seen = self.seen_posts.lock().await;
                    seen.insert(post.id.clone())
                };

                if !is_new {
                    // Déjà vu, ignorer
                    return;
                }

                // Vérifier la signature
                if !post.verify() {
                    eprintln!("[WARN] Post invalide reçu (ID: {}, signature incorrecte)", post.id);
                    return;
                }

                // Stocker le post (TOUS les nœuds stockent : clients ET relays)
                {
                    let storage = self.storage.lock().await;
                    if let Err(e) = storage.save_post(&post) {
                        eprintln!("[ERROR] Failed to save post: {}", e);
                    }
                }
                println!("[NET] Post stocké: {}", post);

                // SEULS les relays propagent aux autres nœuds
                if self.is_relay {
                    let relay_msg = Message::PublishPost { post };

                    // Propager aux autres relays (pour qu'ils relaient à leurs clients)
                    let nodes = self.network_nodes.lock().await;
                    for node in nodes.iter() {
                        if node.is_relay && node.addr != self.public_addr && node.addr != sender_addr {
                            let _ = self.socket.send_msg(&relay_msg, node.addr).await;
                        }
                    }
                    drop(nodes);

                    // Propager aussi aux clients connus (connectés à ce relay)
                    let peers = self.peers.lock().await;
                    for (addr, _) in peers.iter() {
                        if *addr != sender_addr {
                            let _ = self.socket.send_msg(&relay_msg, *addr).await;
                        }
                    }

                    println!("[RELAY] Post propagé aux autres relays et clients");
                }
            }

            Message::RequestPosts { src_addr, since, pubkeys } => {
                // SEULS les relays répondent aux requêtes
                if !self.is_relay {
                    return; // Les clients ne répondent pas
                }

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
                } else {
                    println!("[RELAY] Réponse envoyée à {}", src_addr);
                }
            }

            Message::PostsBatch { posts } => {
                // TOUS les nœuds stockent les posts reçus (après vérification)
                let storage = self.storage.lock().await;
                let mut seen = self.seen_posts.lock().await;
                for post in posts {
                    if post.verify() {
                        let _ = storage.save_post(&post);
                        // Marquer comme vu pour éviter la re-propagation
                        seen.insert(post.id.clone());
                    }
                }
            }

            Message::NodeAnnounce { addr, pubkey, is_relay: _, time } => {
                // TOUS les nœuds acceptent les annonces
                // Ajouter/mettre à jour le peer
                let mut peers = self.peers.lock().await;
                peers.insert(addr, (pubkey.clone(), time));
                drop(peers);

                let storage = self.storage.lock().await;
                let _ = storage.save_peer(&addr.to_string(), Some(&pubkey), time);
            }

            Message::AllNodesList { nodes } => {
                let mut network_nodes = self.network_nodes.lock().await;
                *network_nodes = nodes;
                println!("[NET] Liste des nœuds mise à jour ({} nœuds)", network_nodes.len());
            }

            // Messages destinés au HubRelay uniquement (ignorés par les clients/relays)
            Message::GetAllNodes { .. } => {
                // Ce message est destiné au HubRelay, pas aux clients/relays
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
