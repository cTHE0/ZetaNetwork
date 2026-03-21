use clap::Parser;
mod relay;
mod client;
mod nat_detector;
mod lib_p2p;

use crate::lib_p2p::*;


#[tokio::main]
async fn main() {
	// Récupération du type de noeud (dial/listen/relay)
    let opts = Opts::parse();

    match opts.mode {
        Mode::Relay => relay::main_relay().await,
        Mode::Listen | Mode::Dial => client::main_client(opts.clone()).await,
    }
    
    
}
