use tokio::net::UdpSocket;
use tokio::time::{sleep, Duration};
use std::net::SocketAddr;
use crate::{Opts, Mode};

use crate::nat_detector::nat_detector;


pub async fn main_client(opts: Opts) {
	// Description de ce noeud
	println!("\nLooking for NAT type and public IP address...");
	let (nat_type, public_addr) = nat_detector().await
		.expect("[ERROR] NAT type not detected");
    println!("   -> {:?}\n   -> {}", nat_type, public_addr);

    // Envoi du premier message au relai
    let ip_relay = opts.relay_ip.expect("--relay-ip est requis");
    let port_relay = opts.relay_port.expect("--relay-port est requis");
    let addr_relay = format!("{}:{}", ip_relay, port_relay).parse().expect("Wrong address format");
    let socket_relay = UdpSocket::bind("0.0.0.0:0").await.expect("Failed to bind");
    let local_addr = socket_relay.local_addr().expect("Failed to get local address");
    let message = "HELLO_RELAY";
    let _ = socket_relay.send_to(message.as_bytes(), &addr_relay).await;

    println!("\n\n## Let's create direct connection with other peers ##");
    match opts.mode {
        Mode::Listen => {
            listen_mode(socket_relay, addr_relay,
                local_addr
            	).await;
        }
        Mode::Dial => {
            dial_mode(
                socket_relay, addr_relay, 
                local_addr,
                &opts.remote_peer_ip.expect("--remote-peer-ip requis"), 
                &opts.remote_peer_port.expect("--remote-peer-port requis")
                ).await;
        }
        _ => unreachable!()
    }
}

// ==================== MODE LISTEN ====================
async fn listen_mode(socket_relay: UdpSocket, addr_relay: SocketAddr, local_addr: SocketAddr) {
	// Étape 1 : Écouter jusqu'à recevoir l'adresse du peer Dial via le relai
    println!("Waiting for the dial's address (LISTEN MODE)...");
    let dial_peer_addr: SocketAddr = loop {
    	// Récupération des message reçu
    	let mut buf = [0; 1024];
    	let (size, peer_addr) = socket_relay.recv_from(&mut buf).await.expect("Nothing received");
	    if size <= 0 || size >= 1024  {
	    	println!("The message's size is incorrect({})", size); 
	    	return; 
	    }
	    let message = String::from_utf8_lossy(&buf[..size]).trim().to_string();

	    // Recherche de l'adresse dans le message du dial
	    if let Some(addr_str) = message.split('[').nth(1).and_then(|s| s.split(']').next()) {
	        if let Ok(addr) = addr_str.parse::<SocketAddr>() {
	            println!("Received dial peer address: {}", addr);
	            break addr;
	        }
	    }

	    // Affichage du message reçu, si ce n'est pas celui attendu
	    println!("[{}] {}", peer_addr, message);
	};

    // Étape 2 : Annoncer au relai qu'on est prêt à recevoir
    let msg = format!("LISTEN_READY:{}", dial_peer_addr);
    let _ = socket_relay.send_to(msg.as_bytes(), addr_relay).await.unwrap();
    println!("Sent '{}' to relay", msg);
    
    // Étape 3 : Test de connexion directe (avant hole punching)
    loop {
    	// Récupération des message reçu
    	let mut buf = [0; 1024];
    	let (size, addr_last_jump) = socket_relay.recv_from(&mut buf).await.expect("Nothing received");
	    if size <= 0 || size >= 1024  {
	    	println!("The message's size is incorrect({})", size); 
	    	return; 
	    }
	    let message = String::from_utf8_lossy(&buf[..size]).trim().to_string();

	    // Recherche de l'adresse dans le message du dial
	    if let Some(addr_last_jump) = message.split('[').nth(1).and_then(|s| s.split(']').next()) {
	        if let Ok(addr_sender) = addr_last_jump.parse::<SocketAddr>() {
	        	if addr_sender == dial_peer_addr {
	            	println!("Direction connection possible ({})", message);
	            	break;
	            }
	        }
	    }

	    // Affichage du message reçu, si ce n'est pas celui attendu
	    println!("[{}] {}", addr_last_jump, message);
	};
	
	// Étape 4 : Hole Punching - connect() simultané
	println!("🔨 Starting HOLE PUNCHING...");

}

// ==================== MODE DIAL ====================
async fn dial_mode(socket_relay: UdpSocket, addr_relay: SocketAddr, local_addr: SocketAddr, remote_peer_ip: &str, remote_peer_port: &str) {
    // Étape 1 : Demander au relai de nous connecter au peer Listen
    println!("\nInitiating connection to {}:{} (DIAL MODE)...", remote_peer_ip, remote_peer_port);
    let msg = format!("DIAL_REQUEST:{}:{}", remote_peer_ip, remote_peer_port);
    let _ = socket_relay.send_to(msg.as_bytes(), addr_relay).await.unwrap();
    println!("Sent '{}' to relay", msg);
    
    // Étape 2 : Recevoir l'adresse du peer Listen via le relai
    let listen_peer_addr: SocketAddr = loop { 
    	// Récupération des message reçu
    	let mut buf = [0; 1024];
    	let (size, _) = socket_relay.recv_from(&mut buf).await.expect("Nothing received");
   	    if size <= 0 || size >= 1024  {
	    	println!("The message's size is incorrect({})", size); 
	    	return; 
	    }
	    let message = String::from_utf8_lossy(&buf[..size]).trim().to_string();

	    // Recherche de l'adresse dans le message du dial
	    if let Some(addr_str) = message.split('[').nth(1).and_then(|s| s.split(']').next()) {
	        if let Ok(addr) = addr_str.parse::<SocketAddr>() {
	            println!("Received listen peer address: {}", addr);
	            break addr;
	        }
	    }

	    // Affichage du message reçu, si ce n'est pas celui attendu
	    println!("[{}] '{}'",addr_relay, message);
	};

    // Étape 3 : Test de connexion directe (avant hole punching)
    sleep(Duration::from_secs(1)).await;
    socket_relay.send_to("Direct connection".as_bytes(), listen_peer_addr).await.unwrap();
    println!("Sent '{}' to relay", msg);

	// Étape 4 : Hole Punching - connect() simultané
	println!("🔨 Starting HOLE PUNCHING...");
}