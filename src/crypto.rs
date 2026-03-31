use secp256k1::{Secp256k1, SecretKey, PublicKey, Message as SecpMessage};
use secp256k1::ecdsa::Signature;
use sha2::{Sha256, Digest};

#[derive(Clone)]
pub struct KeyPair {
    pub secret: SecretKey,
    pub public: PublicKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (secret, public) = secp.generate_keypair(&mut rand::thread_rng());
        KeyPair { secret, public }
    }

    pub fn from_secret_hex(hex_str: &str) -> anyhow::Result<Self> {
        let bytes = hex::decode(hex_str)?;
        let secret = SecretKey::from_slice(&bytes)?;
        let secp = Secp256k1::new();
        let public = PublicKey::from_secret_key(&secp, &secret);
        Ok(KeyPair { secret, public })
    }

    pub fn secret_hex(&self) -> String {
        hex::encode(self.secret.secret_bytes())
    }

    pub fn public_hex(&self) -> String {
        hex::encode(self.public.serialize())
    }
}

pub fn pubkey_from_hex(hex_str: &str) -> anyhow::Result<PublicKey> {
    let bytes = hex::decode(hex_str)?;
    let pubkey = PublicKey::from_slice(&bytes)?;
    Ok(pubkey)
}

pub fn hash_message(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub fn sign(data: &[u8], keypair: &KeyPair) -> String {
    let hash = hash_message(data);
    let secp = Secp256k1::new();
    let msg = SecpMessage::from_digest(hash);
    let sig = secp.sign_ecdsa(&msg, &keypair.secret);
    hex::encode(sig.serialize_compact())
}

pub fn verify(data: &[u8], signature_hex: &str, pubkey_hex: &str) -> bool {
    let Ok(sig_bytes) = hex::decode(signature_hex) else { return false };
    let Ok(sig) = Signature::from_compact(&sig_bytes) else { return false };
    let Ok(pubkey) = pubkey_from_hex(pubkey_hex) else { return false };

    let hash = hash_message(data);
    let msg = SecpMessage::from_digest(hash);
    let secp = Secp256k1::new();
    secp.verify_ecdsa(&msg, &sig, &pubkey).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        let keypair = KeyPair::generate();
        let data = b"Hello Zeta Network!";
        let signature = sign(data, &keypair);
        assert!(verify(data, &signature, &keypair.public_hex()));
    }

    #[test]
    fn test_keypair_serialization() {
        let keypair = KeyPair::generate();
        let secret_hex = keypair.secret_hex();
        let restored = KeyPair::from_secret_hex(&secret_hex).unwrap();
        assert_eq!(keypair.public_hex(), restored.public_hex());
    }
}
