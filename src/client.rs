use tokio::net::{TcpStream, UdpSocket, TcpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, Duration, timeout};
use std::net::SocketAddr;
use std::io::{self, Write};
use crate::{Opts, Mode};

use stunclient::StunClient;


pub async fn main_client(opts: Opts) {
    // Connexion au relai
    let ip_relay = opts.relay_ip.expect("--relay-ip est requis");
    let port_relay = opts.relay_port.expect("--relay-port est requis");
    let socket_relay = format!("{}:{}", ip_relay, port_relay);
    
    let mut relay_stream = TcpStream::connect(&socket_relay).await
        .expect(&format!("[ERROR] Can't connect to relay {}", socket_relay));
    let local_addr = relay_stream.local_addr().unwrap();
    println!("\nConnected to relay {}", socket_relay);    

    // Pas de Hole Punching pour les noeuds derrière un NAT symétrique
    if peer_hole_punchable().await {
        println!("[INFO] This pair can become a relay");
    } else {
        println!("[WARNING] This pair can't become a relay (Symmetric NAT, Hole Punching impossible)");
        // return; // On essaye quand même au cas où
    }


    println!("\n\n## Let's create direct connection with other peers ##\nOur local address: {}", local_addr);
    match opts.mode {
        Mode::Listen => {
            listen_mode(&mut relay_stream, local_addr).await;
        }
        Mode::Dial => {
            let remote_peer_ip = opts.remote_peer_ip.expect("--remote-peer-ip requis");
            let remote_peer_port = opts.remote_peer_port.expect("--remote-peer-port requis");
            dial_mode(&mut relay_stream, local_addr, &remote_peer_ip, &remote_peer_port).await;
        }
        _ => unreachable!()
    }
}

// ==================== MODE LISTEN ====================
async fn listen_mode(relay_stream: &mut TcpStream, local_addr: SocketAddr) {
	// Étape 0 : séparer le flux de données du canal vers le relay
	let (mut relay_read, mut relay_write) = relay_stream.split();
    
    // Étape 1 : Écouter jusqu'à recevoir l'adresse du peer Dial via le relai
    println!("Waiting for the dial's address (LISTEN MODE)...");
    let dial_peer_addr: SocketAddr = loop { 
    	// Récupération des message reçu
    	let mut buf = [0; 1024];
    	let n = relay_read.read(&mut buf).await.unwrap();
	    if n == 0 {
	    	println!("This pair is disconnected from the relay"); 
	    	return; 
	    }
	    let message = String::from_utf8_lossy(&buf[..n]).trim().to_string();

	    // Recherche de l'adresse dans le message du dial
	    if let Some(addr_str) = message.split('[').nth(1).and_then(|s| s.split(']').next()) {
	        if let Ok(addr) = addr_str.parse::<SocketAddr>() {
	            println!("Received dial peer address: {}", addr);
	            break addr;
	        }
	    }

	    // Affichage du message reçu, si ce n'est pas celui attendu
	    println!("[RECEIVED] '{}'", message);
	    io::stdout().flush().unwrap();
	};

    // Étape 2 : Annoncer au relai qu'on est prêt à recevoir
    let msg = format!("LISTEN_READY:{}\n", dial_peer_addr);
    relay_write.write_all(msg.as_bytes()).await.unwrap();
    println!("Sent 'LISTEN_READY:{}' to relay", dial_peer_addr);
    
    // Étape 3 : Test de connexion directe (avant hole punching)
    // let listener = TcpListener::bind(local_addr).await.unwrap();
    // println!("Listening...");

    // let (_, new_peer_address) = listener.accept().await.unwrap();
	// println!("New peer connected as {}", new_peer_address);

	let socket = TcpSocket::new_v4().unwrap();
	socket.set_reuseaddr(true).unwrap();
	socket.bind(local_addr).unwrap();
	let listener = socket.listen(1).unwrap();

	match timeout(Duration::from_secs(15), listener.accept()).await {
	    Ok(Ok((stream, addr))) => {
	        println!("✓ Direct connection from {}", addr);
	        return;
	    }
	    _ => println!("Direct connection failed, starting hole punching...")
	}
	
	// Étape 4 : Hole Punching - connect() simultané
	println!("🔨 Starting HOLE PUNCHING...");

	let socket = TcpSocket::new_v4().unwrap();
	socket.set_reuseaddr(true).unwrap();
	socket.bind(local_addr).unwrap(); // même port que relay
	let p2p_stream = socket.connect(dial_peer_addr).await;

	// Étape 5 : Résultat
	match p2p_stream {
	    Ok(mut stream) => {
	        println!("✓ Hole punching SUCCESS, direct connection established with {}", dial_peer_addr);
	        // utiliser stream pour communiquer...
	    }
	    Err(e) => {
	        println!("✗ Hole punching failed: {}", e);
	    }
	}
}

// ==================== MODE DIAL ====================
async fn dial_mode(relay_stream: &mut TcpStream, local_addr: SocketAddr, remote_peer_ip: &str, remote_peer_port: &str) {

	// Étape 0 : séparer le flux de données du canal vers le relay
	let (mut relay_read, mut relay_write) = relay_stream.split();
    
    // Étape 1 : Demander au relai de nous connecter au peer Listen
    println!("\nInitiating connection to {}:{} (DIAL MODE)...", remote_peer_ip, remote_peer_port);
    let msg = format!("DIAL_REQUEST:{}:{}\n", remote_peer_ip, remote_peer_port);
    relay_write.write_all(msg.as_bytes()).await.unwrap();
    println!("Sent 'DIAL_REQUEST:{}:{}' to relay", remote_peer_ip, remote_peer_port);
    
    // Étape 2 : Recevoir l'adresse du peer Listen via le relai
    let listen_peer_addr: SocketAddr = loop { 
    	// Récupération des message reçu
    	let mut buf = [0; 1024];
    	let n = relay_read.read(&mut buf).await.unwrap();
	    if n == 0 {
	    	println!("This pair is disconnected from the relay"); 
	    	return; 
	    }
	    let message = String::from_utf8_lossy(&buf[..n]).trim().to_string();

	    // Recherche de l'adresse dans le message du dial
	    if let Some(addr_str) = message.split('[').nth(1).and_then(|s| s.split(']').next()) {
	        if let Ok(addr) = addr_str.parse::<SocketAddr>() {
	            println!("Received listen peer address: {}", addr);
	            break addr;
	        }
	    }

	    // Affichage du message reçu, si ce n'est pas celui attendu
	    println!("[RECEIVED] '{}'", message);
	    io::stdout().flush().unwrap();
	};

    // Étape 3 : TEST 1 - Connexion directe AVANT hole punching
    sleep(Duration::from_secs(3)).await;
    match timeout(Duration::from_secs(5), TcpStream::connect(listen_peer_addr)).await {
	    Ok(Ok(stream)) => {
	        println!("✓ Direct connection to {}", listen_peer_addr);
	        return;
	    }
	    _ => println!("Direct connection failed, starting hole punching...")
	}

	// Étape 4 : Hole Punching - connect() simultané
	println!("🔨 Starting HOLE PUNCHING...");

	let socket = TcpSocket::new_v4().unwrap();
	socket.set_reuseaddr(true).unwrap();
	socket.bind(local_addr).unwrap(); // même port que relay
	let p2p_stream = socket.connect(listen_peer_addr).await;

	// Étape 5 : Résultat
	match p2p_stream {
	    Ok(mut stream) => {
	        println!("✓ Hole punching SUCCESS, direct connection established with {}", listen_peer_addr);
	        // utiliser stream pour communiquer...
	    }
	    Err(e) => {
	        println!("✗ Hole punching failed: {}", e);
	    }
	}
}

// ==================== TESTS & UTILS ====================
async fn peer_hole_punchable() -> bool {
    // Création du socket pour accéder au serveur STUN
    let udp = UdpSocket::bind("0.0.0.0:0").await.unwrap();

    // Création des clients et envoie de la requête aux serveurs STUN
    let client1 = StunClient::new("74.125.250.129:3478".parse().unwrap());  // Serveur STUN stun.l.google.com 3478
    let client2 = StunClient::new("46.225.95.169:3478".parse().unwrap());  // Serveur STUN stun.nextcloud.com 3478

    // Récupération des adresses publiques de notre noeud, en fonction du serveur public contacté
    let public_addr1 = match client1.query_external_address_async(&udp).await {
        Ok(addr) => Some(addr),
        Err(e) => {
        	println!("Erreur STUN : {:?}", e);
        	None
        },
    };
    let public_addr2 = match client2.query_external_address_async(&udp).await {
        Ok(addr) => Some(addr),
        Err(e) => {
        	println!("Erreur STUN : {:?}", e);
        	None
        },
    };

    public_addr1.is_some() && public_addr1 == public_addr2
}