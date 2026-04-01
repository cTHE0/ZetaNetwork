use std::fmt;
use tokio::net::UdpSocket;
use std::net::{SocketAddr, ToSocketAddrs};
use clap::{Parser, ValueEnum};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use anyhow::Result;

use crate::post::Post;

#[derive(Debug, Parser, Clone)]
pub struct Opts {
    // Si ce noeud est celui qui initie la connection, celui qui la recoit, voir le relai
    #[arg(long, value_enum)]
    pub mode: Mode,

    #[arg(long)]
    pub peer_id: String
}

#[derive(Clone, Debug, PartialEq, ValueEnum)]
pub enum Mode {
    Client,
    HubRelay
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // ========== Messages réseau social décentralisé ==========

    PublishPost {  // Diffuser un nouveau post aux peers
        post: Post,
    },

    RequestPosts {  // Demander les posts récents de certains auteurs
        src_addr: SocketAddr,
        since: u64,           // Timestamp depuis lequel on veut les posts
        pubkeys: Vec<String>, // Liste des clés publiques des auteurs recherchés
    },

    PostsBatch {  // Réponse à RequestPosts
        posts: Vec<Post>,
    },

    NodeAnnounce {  // Annonce d'un noeud avec sa clé publique
        addr: SocketAddr,
        pubkey: String,
        is_relay: bool,
        time: u64,
    },

    GetAllNodes {  // Demande la liste de tous les nœuds du réseau au Hub
        src_addr: SocketAddr,
        time: u64,
    },

    AllNodesList {  // Liste complète des nœuds du réseau
        nodes: Vec<NetworkNode>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkNode {
    pub addr: SocketAddr,
    pub pubkey: String,
    pub is_relay: bool,
    pub last_seen: u64,
}

#[async_trait::async_trait]
pub trait UdpSocketExt {
    async fn send_msg(&self, msg: &Message, next_hop: SocketAddr) -> Result<usize>;
}

#[async_trait::async_trait]
impl UdpSocketExt for UdpSocket {
    async fn send_msg(&self, msg: &Message, next_hop: SocketAddr) -> Result<usize> {
        let encoded = bincode::serialize(&msg)?;
        Ok(self.send_to(&encoded, next_hop).await?)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::PublishPost { post } => {
                write!(f, "[PublishPost] {}", post)
            }
            Message::RequestPosts { src_addr, since, pubkeys } => {
                let count = pubkeys.len();
                write!(f, "[RequestPosts] {} demande posts depuis {} pour {} auteurs", src_addr, since, count)
            }
            Message::PostsBatch { posts } => {
                write!(f, "[PostsBatch] {} posts", posts.len())
            }
            Message::NodeAnnounce { addr, pubkey, is_relay, time } => {
                let pk_short = if pubkey.len() > 12 { &pubkey[..12] } else { pubkey };
                let node_type = if *is_relay { "relay" } else { "client" };
                let time_str = DateTime::<Utc>::from_timestamp(*time as i64, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| format!("t={}", time));
                write!(f, "[NodeAnnounce] {} ({}...) [{}] ({})", addr, pk_short, node_type, time_str)
            }
            Message::GetAllNodes { src_addr, time } => {
                let time_str = DateTime::<Utc>::from_timestamp(*time as i64, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_else(|| format!("t={}", time));
                write!(f, "[GetAllNodes] {} ({})", src_addr, time_str)
            }
            Message::AllNodesList { nodes } => {
                let relays = nodes.iter().filter(|n| n.is_relay).count();
                write!(f, "[AllNodesList] {} nodes ({} relays)", nodes.len(), relays)
            }
        }
    }
}

pub async fn get_public_ip(socket: &UdpSocket) -> Result<SocketAddr> {
    let stun_addr = "stun.l.google.com:19302"
        .to_socket_addrs()?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| anyhow::anyhow!("Cannot resolve STUN server"))?;

    let client = stunclient::StunClient::new(stun_addr);
    let public_addr = client.query_external_address_async(socket).await?;
    Ok(public_addr)
}

pub async fn recv_msg(socket: &UdpSocket) -> Option<(Message, SocketAddr)> {
    let mut buf = [0; 65536];  // 64KB au lieu de 4KB (pour PostsBatch de 100 posts)
    let (size, sender_addr) = socket.recv_from(&mut buf).await.expect("Nothing received");
    if size == 0 || size >= 65536 {
        println!("The message's size is incorrect({})", size);
        return None;
    }
    let msg: Message = bincode::deserialize(&buf[..size]).expect("[ERROR] Deserialization failed");
    Some((msg, sender_addr))
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}