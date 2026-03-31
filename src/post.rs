use serde::{Serialize, Deserialize};
use crate::crypto::{KeyPair, sign, verify, hash_message};
use crate::lib_p2p::now_secs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,              // Hash unique du post
    pub author_pubkey: String,   // Clé publique de l'auteur
    pub content: String,         // Contenu du message
    pub signature: String,       // Signature du contenu
    pub timestamp: u64,          // Timestamp Unix
}

impl Post {
    pub fn new(content: String, keypair: &KeyPair) -> Self {
        let timestamp = now_secs();
        let author_pubkey = keypair.public_hex();

        // Données à signer: content + timestamp + author
        let data_to_sign = format!("{}:{}:{}", content, timestamp, author_pubkey);
        let signature = sign(data_to_sign.as_bytes(), keypair);

        // ID = hash de (content + timestamp + author)
        let hash = hash_message(data_to_sign.as_bytes());
        let id = hex::encode(&hash[..16]); // 16 premiers bytes pour un ID court

        Post {
            id,
            author_pubkey,
            content,
            signature,
            timestamp,
        }
    }

    pub fn verify(&self) -> bool {
        let data_to_verify = format!("{}:{}:{}", self.content, self.timestamp, self.author_pubkey);
        verify(data_to_verify.as_bytes(), &self.signature, &self.author_pubkey)
    }

    pub fn short_author(&self) -> String {
        if self.author_pubkey.len() > 12 {
            format!("{}...", &self.author_pubkey[..12])
        } else {
            self.author_pubkey.clone()
        }
    }
}

impl std::fmt::Display for Post {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let datetime = chrono::DateTime::from_timestamp(self.timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| format!("t={}", self.timestamp));
        write!(f, "[{}] {} : {}", datetime, self.short_author(), self.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_creation_and_verification() {
        let keypair = KeyPair::generate();
        let post = Post::new("Hello Zeta!".to_string(), &keypair);

        assert!(post.verify());
        assert_eq!(post.author_pubkey, keypair.public_hex());
    }

    #[test]
    fn test_tampered_post_fails_verification() {
        let keypair = KeyPair::generate();
        let mut post = Post::new("Hello Zeta!".to_string(), &keypair);

        // Tamper with content
        post.content = "Hacked!".to_string();
        assert!(!post.verify());
    }
}
