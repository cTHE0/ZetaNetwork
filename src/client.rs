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

/// Point d'entrée principal pour un nœud client du réseau social décentralisé
///
/// Fonctionnement :
/// 1. Détecte le type de NAT pour déterminer le rôle (relay ou simple client)
/// 2. S'enregistre auprès du HubRelay avec NodeAnnounce
/// 3. Récupère la liste des relays disponibles via GetAllNodes
/// 4. Lance les services (web, réception, synchronisation)
///
/// Relay : Peut recevoir des connexions entrantes et propager les messages
/// Client : Ne peut qu'envoyer aux relays pour publier/récupérer des posts
pub async fn main_client(_peer_id: String, hub_relay_addr: SocketAddr) {
    println!("\n=== Initialisation du nœud ===");

    // 1. Détection du type de NAT
    println!("Détection du type de NAT...");
    let (nat_type, _) = nat_detector().await
        .expect("[ERREUR] Impossible de détecter le type de NAT");
    println!("  → Type NAT : {:?}", nat_type);

    if matches!(nat_type, Unknown | UdpBlocked) {
        eprintln!("[ERREUR] Ce nœud ne peut pas accéder au réseau (NAT : {:?})", nat_type);
        return;
    }

    // 2. Initialisation du stockage et des clés
    let storage = Storage::new(DB_PATH)
        .expect("[ERREUR] Échec d'initialisation de la base de données");
    let keypair = storage.get_or_create_keypair()
        .expect("[ERREUR] Échec de récupération de la paire de clés");
    println!("  → Clé publique : {}", keypair.public_hex());

    // 3. Création du socket UDP
    let socket = UdpSocket::bind("0.0.0.0:0").await
        .expect("[ERREUR] Échec de création du socket");
    let public_addr = get_public_ip(&socket).await
        .expect("[ERREUR] Impossible d'obtenir l'IP publique");
    println!("  → Adresse publique : {}", public_addr);

    // 4. Déterminer le rôle : relay ou simple client
    let is_relay = matches!(nat_type, OpenInternet | FullCone | RestrictedCone | PortRestrictedCone);

    if is_relay {
        println!("\n[RÔLE] RELAY - Ce nœud peut recevoir des connexions et propager les messages");
    } else {
        println!("\n[RÔLE] CLIENT - Ce nœud ne peut qu'envoyer aux relays");
    }

    // 5. Créer l'état du réseau
    let network = Arc::new(NetworkState::new(
        socket,
        storage,
        keypair,
        public_addr,
        hub_relay_addr,
        is_relay
    ));

    // 6. S'enregistrer auprès du HubRelay
    println!("\nEnregistrement auprès du HubRelay...");
    let announce_msg = Message::NodeAnnounce {
        addr: public_addr,
        pubkey: network.keypair.public_hex(),
        is_relay,
        time: now_secs(),
    };
    if let Err(e) = network.socket.send_msg(&announce_msg, hub_relay_addr).await {
        eprintln!("[ERREUR] Échec d'enregistrement : {}", e);
    } else {
        println!("  → Enregistré avec succès");
    }

    // 7. Récupérer la liste des nœuds du réseau (notamment les relays)
    println!("Récupération de la liste des nœuds...");
    network.request_network_nodes().await;
    sleep(Duration::from_secs(2)).await; // Attendre la réponse

    let nodes = network.get_network_nodes().await;
    let relay_count = nodes.iter().filter(|n| n.is_relay).count();
    println!("  → {} nœuds trouvés ({} relays)", nodes.len(), relay_count);

    // 8. Lancer les services
    println!("\nDémarrage des services...");
    start_services(network.clone(), hub_relay_addr, public_addr).await;

    // 9. Boucle principale de réception des messages
    println!("[NET] Prêt à recevoir des messages\n");
    run_network_loop(network).await;
}

/// Lance tous les services du nœud en arrière-plan
async fn start_services(network: Arc<NetworkState>, hub_relay_addr: SocketAddr, public_addr: SocketAddr) {
    // Service 1 : Interface web
    let network_web = Arc::clone(&network);
    tokio::spawn(async move {
        println!("  → Service web : http://localhost:{}", WEB_PORT);
        start_web_server(network_web, WEB_PORT).await;
    });

    // Service 2 : Keep-alive au HubRelay (toutes les 60 secondes)
    let network_keepalive = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            let msg = Message::NodeAnnounce {
                addr: public_addr,
                pubkey: network_keepalive.keypair.public_hex(),
                is_relay: network_keepalive.is_relay,
                time: now_secs(),
            };
            let _ = network_keepalive.socket.send_msg(&msg, hub_relay_addr).await;
        }
    });

    // Service 3 : Mise à jour de la liste des nœuds (toutes les 30 secondes)
    let network_nodes = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;
            network_nodes.request_network_nodes().await;
        }
    });

    // Service 4 : Synchronisation des posts (toutes les 30 secondes)
    // Les clients simples demandent aux relays les posts de leurs abonnements
    let network_sync = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(30)).await;

            let storage = network_sync.storage.lock().await;
            let subscriptions = storage.get_subscriptions().unwrap_or_default();
            drop(storage);

            if !subscriptions.is_empty() {
                let since = now_secs().saturating_sub(3600); // Posts de la dernière heure
                network_sync.request_posts_from_peers(since, subscriptions).await;
            }
        }
    });

    // Service 5 : Annonce périodique aux pairs (toutes les 60 secondes)
    let network_announce = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            network_announce.announce_self().await;
        }
    });

    // Service 6 : Nettoyage des peers inactifs (toutes les 60 secondes)
    let network_cleanup = Arc::clone(&network);
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60)).await;
            network_cleanup.cleanup_old_peers().await;
        }
    });
}

/// Boucle principale de réception et traitement des messages
async fn run_network_loop(network: Arc<NetworkState>) {
    let mut buf = vec![0; 4096];
    loop {
        match network.socket.recv_from(&mut buf).await {
            Ok((size, sender_addr)) => {
                if size == 0 || size >= 4096 {
                    continue;
                }

                match bincode::deserialize::<Message>(&buf[..size]) {
                    Ok(msg) => {
                        network.handle_message(msg, sender_addr).await;
                    }
                    Err(e) => {
                        eprintln!("[ERREUR] Désérialisation : {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERREUR] Réception : {}", e);
            }
        }
    }
}
