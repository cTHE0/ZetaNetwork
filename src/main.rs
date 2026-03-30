use clap::Parser;
mod client;
mod nat_detector;
mod lib_p2p;
mod hubRelay;

use crate::lib_p2p::*;


#[tokio::main]
async fn main() {
	// Récupération du type de noeud (client/hubRelay)
    let opts = Opts::parse();

    match opts.mode {
        Mode::HubRelay => hubRelay::main_hubRelay(opts.peer_id).await,
        Mode::Client => client::main_client(opts.peer_id).await,
    }
    
    println!("See you soon!");

}
