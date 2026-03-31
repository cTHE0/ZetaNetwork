use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use std::sync::Arc;
use std::collections::HashMap;
use std::net::SocketAddr;

use crate::lib_p2p::*;

// Un noeud = [Addr, (pubkey, derniere connection en secs, is_relay)]
pub type NodesMap = Arc<Mutex<HashMap<SocketAddr, NodeInfo>>>;

#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub pubkey: String,
    pub last_seen: u64,
    pub is_relay: bool,
}

pub async fn main_hubRelay(peer_id: String, hubRelay_addr: SocketAddr) {
    // Le hub relay démarre l'écoute
    let socket = UdpSocket::bind("0.0.0.0:55555").await.expect("Failed to bind");
    let public_addr: SocketAddr = get_public_ip(&socket).await.expect("Public IP not obtained.");
    println!("\nThe hub relay listens on {} ({})...", public_addr, peer_id);
    if hubRelay_addr != public_addr {
        println!("[ERROR] The hub relay has an address different as expected");
        return;
    }

    // Crée la liste de tous les noeuds (relais et clients)
    let nodes_list: NodesMap = Arc::new(Mutex::new(HashMap::new()));

    // Suppression automatique des noeuds inactifs
    let nodes_cleanup = Arc::clone(&nodes_list);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            cleanup_inactive_nodes(&nodes_cleanup).await;
        }
    });

    // Affichage périodique des stats
    let nodes_stats = Arc::clone(&nodes_list);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            let nodes = nodes_stats.lock().await;
            let relays = nodes.values().filter(|n| n.is_relay).count();
            let clients = nodes.len() - relays;
            println!("[STATS] {} relays, {} clients actifs", relays, clients);
        }
    });

    let mut buf = vec![0; 4096];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((size, sender_addr)) => {
                if size == 0 || size >= 4096 { continue; }
                let msg: Message = match bincode::deserialize(&buf[..size]) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("[ERROR] Deserialization failed: {}", e);
                        continue;
                    }
                };
                println!("{}", msg);

                match &msg {
                    // Un relai se déclare
                    Message::BeNewRelay { src_addr, src_id, time, .. } => {
                        nodes_list.lock().await.insert(*src_addr, NodeInfo {
                            pubkey: src_id.clone(),
                            last_seen: *time,
                            is_relay: true,
                        });

                        let ack = Message::Ack {
                            src_addr: public_addr,
                            src_id: peer_id.clone(),
                            time: now_secs(),
                        };
                        let _ = socket.send_msg(&ack, sender_addr).await;
                        println!("{}", ack);
                    }

                    // Un noeud s'annonce avec sa pubkey
                    Message::NodeAnnounce { addr, pubkey, time } => {
                        let mut nodes = nodes_list.lock().await;
                        nodes.entry(*addr)
                            .and_modify(|n| {
                                n.pubkey = pubkey.clone();
                                n.last_seen = *time;
                            })
                            .or_insert(NodeInfo {
                                pubkey: pubkey.clone(),
                                last_seen: *time,
                                is_relay: false,
                            });
                    }

                    // Un peer cherche un relai : on lui en renvoie un
                    Message::NeedRelay { src_addr, src_id, .. } => {
                        let nodes = nodes_list.lock().await;
                        // Trouver un relay actif
                        let relay = nodes.iter()
                            .find(|(_, info)| info.is_relay)
                            .map(|(addr, info)| (*addr, info.pubkey.clone()));
                        drop(nodes);

                        if let Some((relay_addr, relay_pubkey)) = relay {
                            let msg = Message::PeerInfo {
                                peer_addr: relay_addr,
                                peer_id: relay_pubkey,
                            };
                            let _ = socket.send_msg(&msg, *src_addr).await;
                            println!("{}", msg);

                            // Avertissons le relais concerné
                            let msg = Message::RelayHasNewClient {
                                src_addr: public_addr,
                                src_id: peer_id.clone(),
                                peer_addr: *src_addr,
                                peer_id: src_id.clone(),
                                time: now_secs(),
                            };
                            let _ = socket.send_msg(&msg, relay_addr).await;
                            println!("{}", msg);
                        } else {
                            let msg = Message::NoRelayAvailable {
                                src_addr: public_addr,
                                src_id: peer_id.clone(),
                                dst_addr: *src_addr,
                                dst_id: src_id.clone(),
                                time: now_secs(),
                            };
                            let _ = socket.send_msg(&msg, *src_addr).await;
                            println!("{}", msg);
                        }
                    }

                    // Un peer demande la liste des peers
                    Message::GetPeers { src_addr, .. } => {
                        let nodes = nodes_list.lock().await;
                        let peers: Vec<(SocketAddr, String)> = nodes.iter()
                            .filter(|(addr, _)| **addr != *src_addr)
                            .take(20)  // Limiter à 20 peers
                            .map(|(addr, info)| (*addr, info.pubkey.clone()))
                            .collect();
                        drop(nodes);

                        let msg = Message::PeersList { peers };
                        let _ = socket.send_msg(&msg, *src_addr).await;
                        println!("{}", msg);
                    }

                    _ => {
                        // Met à jour le last_seen si on reçoit un message d'un noeud connu
                        let mut nodes = nodes_list.lock().await;
                        if let Some(info) = nodes.get_mut(&sender_addr) {
                            info.last_seen = now_secs();
                        }
                    }
                }
            }
            Err(e) => eprintln!("[ERROR] a message contain an error ({})", e),
        }
    }
}

async fn cleanup_inactive_nodes(nodes: &NodesMap) {
    let mut nodes_map = nodes.lock().await;
    let now = now_secs();
    nodes_map.retain(|addr, info| {
        let active = now - info.last_seen < 300;  // 5 minutes timeout
        if !active {
            println!("[INFO] Node {} disconnected (timeout)", addr);
        }
        active
    });
}
