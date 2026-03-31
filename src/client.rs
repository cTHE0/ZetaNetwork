use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::nat_detector::nat_detector;
use crate::nat_detector::util::NatType::*;
use crate::lib_p2p::*;
use crate::storage::Storage;
use crate::network::NetworkState;
use crate::web::start_web_server;

const DB_PATH: &str = "zeta_data.db";
const WEB_PORT: u16 = 8080;

pub async fn main_client(peer_id: String, hubRelay_addr: SocketAddr) {
    // Initialisation du noeud
    println!("\nLooking for NAT type...");
    let (nat_type, _) = nat_detector().await
        .expect("[ERROR] NAT type not detected");
    println!("   -> {:?}\n", nat_type);

    // Vérifie l'accès réseau dès le début
    if matches!(nat_type, Unknown | UdpBlocked) {
        println!("This node can't access the network ({:?})", nat_type);
        return;
    }

    // Créer/charger le storage et le keypair
    let storage = Storage::new(DB_PATH).expect("[ERROR] Failed to init database");
    let keypair = storage.get_or_create_keypair().expect("[ERROR] Failed to get keypair");
    println!("Public key: {}", keypair.public_hex());

    // Crée le socket pour envoyer des messages
    let socket = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind");
    let public_addr: SocketAddr = get_public_ip(&socket).await
        .expect("Public IP not obtained.");
    println!("Socket created on public address {:?}", public_addr);

    // Demande un relais disponible au hubRelay
    println!("Asking the hub relay an available relay...");
    let msg = Message::NeedRelay {
        src_addr: public_addr,
        src_id: peer_id.clone(),
        time: now_secs(),
    };
    let _ = socket.send_msg(&msg, hubRelay_addr).await;

    // Attends l'adresse d'un relais, de la part du hub relais
    let relay_addr: Option<SocketAddr> = loop {
        let Some((msg, _)) = recv_msg(&socket).await else { return };
        match &msg {
            Message::PeerInfo { peer_addr, peer_id, .. } => {
                println!("Received relay address {} ({})", peer_addr, peer_id);
                break Some(*peer_addr);
            }
            Message::NoRelayAvailable { .. } => {
                println!("[WARN] No relays available on the network");
                break None;
            }
            _ => println!("Unexpected message: '{}'", msg),
        }
    };

    // Déterminer si ce nœud peut être un relay
    let is_relay = matches!(nat_type, OpenInternet | FullCone | RestrictedCone | PortRestrictedCone);

    // Créer le NetworkState
    let network = Arc::new(NetworkState::new(socket, storage, keypair, public_addr, hubRelay_addr, is_relay));

    // Enregistrer auprès du relay si disponible
    if let Some(relay_addr) = relay_addr {
        let msg = Message::Register {
            src_addr: public_addr,
            src_id: peer_id.clone(),
            dst_addr: relay_addr,
            dst_id: "relay".to_string(),
            time: now_secs(),
        };
        let _ = network.socket.send_msg(&msg, relay_addr).await;
        network.add_peer(relay_addr, String::new()).await;
    } else {
        println!("[WARN] No relay, skipping registration");
    }

    // Ajout de ce noeud au réseau Zeta Network
    if is_relay {
        println!("This node is a RELAY ({:?}) - will propagate messages", nat_type);
        // S'annoncer comme relay au HubRelay
        let msg = Message::BeNewRelay {
            src_addr: public_addr,
            src_id: peer_id.clone(),
            time: now_secs(),
        };
        let _ = network.socket.send_msg(&msg, hubRelay_addr).await;
    } else {
        println!("This node is a CLIENT ({:?}) - won't propagate messages", nat_type);
    }

    // Démarrer les services en parallèle
    let network_clone = Arc::clone(&network);
    let network_web = Arc::clone(&network);

    // Service web
    tokio::spawn(async move {
        start_web_server(network_web, WEB_PORT).await;
    });

    // Service de nettoyage des peers
    let network_cleanup = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            network_cleanup.cleanup_old_peers().await;
        }
    });

    // Service de récupération périodique des posts
    let network_fetch = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            let storage = network_fetch.storage.lock().await;
            let subs = storage.get_subscriptions().unwrap_or_default();
            drop(storage);
            if !subs.is_empty() {
                let since = now_secs().saturating_sub(3600); // Posts de la dernière heure
                network_fetch.request_posts_from_peers(since, subs).await;
            }
        }
    });

    // Service d'annonce périodique
    let network_announce = Arc::clone(&network);
    let hubRelay_addr_clone = hubRelay_addr;
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            network_announce.announce_self().await;
            // Keep-alive au HubRelay
            let msg = Message::NodeAnnounce {
                addr: public_addr,
                pubkey: network_announce.keypair.public_hex(),
                time: now_secs(),
            };
            let _ = network_announce.socket.send_msg(&msg, hubRelay_addr_clone).await;
        }
    });

    // Service de récupération périodique des nœuds du réseau
    let network_nodes = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            network_nodes.request_network_nodes().await;
            sleep(Duration::from_secs(30)).await;
        }
    });

    // Boucle principale de réception des messages
    println!("\n[NET] Starting main network loop...");
    run_network_loop(network_clone).await;
}

async fn run_network_loop(network: Arc<NetworkState>) {
    let mut buf = vec![0; 4096];
    loop {
        match network.socket.recv_from(&mut buf).await {
            Ok((size, sender_addr)) => {
                if size == 0 || size >= 4096 {
                    eprintln!("[WARN] Invalid message size: {}", size);
                    continue;
                }
                match bincode::deserialize::<Message>(&buf[..size]) {
                    Ok(msg) => {
                        network.handle_message(msg, sender_addr).await;
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Deserialization failed: {}", e);
                    }
                }
            }
            Err(e) => eprintln!("[ERROR] recv_from: {}", e),
        }
    }
}
