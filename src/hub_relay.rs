use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use std::sync::Arc;
use std::collections::HashMap;
use std::net::SocketAddr;

use crate::lib_p2p::*;

/// HubRelay : Serveur d'annuaire centralisé pour le réseau social décentralisé
///
/// Rôle : Stocker et distribuer la liste de tous les nœuds du réseau
/// Contenu : IP, clé publique, type de nœud (relay/client), dernière activité
///
/// Messages gérés :
/// - NodeAnnounce : Un nœud s'enregistre ou met à jour ses informations
/// - GetAllNodes : Un nœud demande la liste complète des nœuds actifs

type NodesMap = Arc<Mutex<HashMap<SocketAddr, NodeInfo>>>;

#[derive(Clone, Debug)]
struct NodeInfo {
    pubkey: String,     // Clé publique du nœud (identifiant unique)
    last_seen: u64,     // Timestamp Unix de la dernière activité
    is_relay: bool,     // Type : true = relay, false = simple client
}

impl NodeInfo {
    fn to_network_node(&self, addr: SocketAddr) -> NetworkNode {
        NetworkNode {
            addr,
            pubkey: self.pubkey.clone(),
            is_relay: self.is_relay,
            last_seen: self.last_seen,
        }
    }
}

pub async fn main_hub_relay(peer_id: String, hub_relay_addr: SocketAddr) {
    println!("\n=== HubRelay : Serveur d'annuaire du réseau social ===");

    // Bind sur le port configuré
    let socket = UdpSocket::bind("0.0.0.0:55555").await
        .expect("Failed to bind socket");

    let public_addr = get_public_ip(&socket).await
        .expect("Failed to get public IP");

    println!("Écoute sur {} (ID: {})", public_addr, peer_id);

    if hub_relay_addr != public_addr {
        eprintln!("[ERREUR] L'adresse publique ne correspond pas à l'adresse configurée");
        return;
    }

    // Base de données des nœuds actifs
    let nodes_list: NodesMap = Arc::new(Mutex::new(HashMap::new()));

    // Tâche de nettoyage automatique des nœuds inactifs (toutes les 30 secondes)
    let nodes_cleanup = Arc::clone(&nodes_list);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            cleanup_inactive_nodes(&nodes_cleanup).await;
        }
    });

    // Affichage périodique des statistiques (toutes les 60 secondes)
    let nodes_stats = Arc::clone(&nodes_list);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            print_stats(&nodes_stats).await;
        }
    });

    println!("Prêt à recevoir des nœuds...\n");

    // Boucle principale de réception
    let mut buf = vec![0; 4096];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((size, sender_addr)) => {
                if size == 0 || size >= 4096 {
                    continue;
                }

                let msg: Message = match bincode::deserialize(&buf[..size]) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("[ERREUR] Désérialisation : {}", e);
                        continue;
                    }
                };

                println!("← {}", msg);
                handle_message(&socket, &nodes_list, msg, sender_addr).await;
            }
            Err(e) => {
                eprintln!("[ERREUR] Réception : {}", e);
            }
        }
    }
}

/// Gère les messages reçus par le HubRelay
async fn handle_message(
    socket: &UdpSocket,
    nodes_list: &NodesMap,
    msg: Message,
    _sender_addr: SocketAddr,
) {
    match msg {
        // Un nœud s'enregistre ou met à jour ses informations
        Message::NodeAnnounce { addr, pubkey, is_relay, time } => {
            let mut nodes = nodes_list.lock().await;
            nodes.entry(addr)
                .and_modify(|node| {
                    node.pubkey = pubkey.clone();
                    node.is_relay = is_relay;
                    node.last_seen = time;
                })
                .or_insert(NodeInfo {
                    pubkey,
                    last_seen: time,
                    is_relay,
                });
            println!("  → Nœud enregistré : {}", addr);
        }

        // Un nœud demande la liste complète des nœuds actifs
        Message::GetAllNodes { src_addr, .. } => {
            let nodes = nodes_list.lock().await;
            let all_nodes: Vec<NetworkNode> = nodes
                .iter()
                .map(|(addr, info)| info.to_network_node(*addr))
                .collect();
            drop(nodes);

            let response = Message::AllNodesList { nodes: all_nodes };
            if let Err(e) = socket.send_msg(&response, src_addr).await {
                eprintln!("[ERREUR] Envoi : {}", e);
            } else {
                println!("  → Liste envoyée à {} ({} nœuds)", src_addr, response.nodes_count());
            }
        }

        // Messages non gérés par le HubRelay
        _ => {
            println!("  ⚠ Message ignoré (non géré par le HubRelay)");
        }
    }
}

/// Supprime les nœuds inactifs depuis plus de 5 minutes
async fn cleanup_inactive_nodes(nodes: &NodesMap) {
    let mut nodes_map = nodes.lock().await;
    let now = now_secs();
    let timeout = 300; // 5 minutes

    let before_count = nodes_map.len();
    nodes_map.retain(|addr, info| {
        let is_active = now - info.last_seen < timeout;
        if !is_active {
            println!("[INFO] Nœud déconnecté (timeout) : {}", addr);
        }
        is_active
    });

    let removed = before_count - nodes_map.len();
    if removed > 0 {
        println!("[CLEANUP] {} nœud(s) supprimé(s)", removed);
    }
}

/// Affiche les statistiques du réseau
async fn print_stats(nodes: &NodesMap) {
    let nodes_map = nodes.lock().await;
    let total = nodes_map.len();
    let relays = nodes_map.values().filter(|n| n.is_relay).count();
    let clients = total - relays;

    println!("\n[STATS] {} nœuds actifs → {} relay(s) + {} client(s)\n", total, relays, clients);
}

// Extension pour Message pour faciliter l'affichage
impl Message {
    fn nodes_count(&self) -> usize {
        match self {
            Message::AllNodesList { nodes } => nodes.len(),
            _ => 0,
        }
    }
}
