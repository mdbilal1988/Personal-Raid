use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use bip39::Mnemonic;
use hkdf::Hkdf;
use libp2p::identity::{ed25519 as p2p_ed25519, Keypair as P2pKeypair, PeerId};
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use sharks::{Share, Sharks};

/// Generates a new 24-word BIP-39 mnemonic seed.
pub fn generate_mnemonic() -> Result<String> {
    let mut entropy = [0u8; 32]; // 256 bits of entropy for 24 words
    OsRng.fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)?;
    Ok(mnemonic.to_string())
}

/// Derives deterministic keys and libp2p identity from a mnemonic.
pub struct Identity {
    pub peer_id: PeerId,
    pub p2p_keypair: P2pKeypair,
    pub master_key: [u8; 32],
}

impl Identity {
    /// Derives identity from a 24-word mnemonic.
    pub fn from_mnemonic(phrase: &str) -> Result<Self> {
        let mnemonic = Mnemonic::parse(phrase)?;
        let seed = mnemonic.to_seed("");

        // Derive libp2p Ed25519 Keypair
        let hk = Hkdf::<Sha256>::new(None, &seed);
        let mut ed25519_seed = [0u8; 32];
        hk.expand(b"shardnet-identity", &mut ed25519_seed)
            .map_err(|e| anyhow!("HKDF expand failed: {}", e))?;

        let p2p_ed_secret = p2p_ed25519::SecretKey::try_from_bytes(ed25519_seed)?;
        let p2p_ed_keypair = p2p_ed25519::Keypair::from(p2p_ed_secret);
        let p2p_keypair = P2pKeypair::from(p2p_ed_keypair);
        let peer_id = PeerId::from(p2p_keypair.public());

        // Derive Symmetric Master Key
        let mut master_key = [0u8; 32];
        hk.expand(b"shardnet-master-key", &mut master_key)
            .map_err(|e| anyhow!("HKDF expand failed: {}", e))?;

        Ok(Self {
            peer_id,
            p2p_keypair,
            master_key,
        })
    }
}

/// AES-256-GCM Encryption wrapper.
pub struct Cipher;

impl Cipher {
    /// Encrypts data using a 32-byte key. A random 12-byte nonce is prepended to the ciphertext.
    pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new(key.into());
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypts data using a 32-byte key. Expects a 12-byte nonce prepended to the ciphertext.
    pub fn decrypt(key: &[u8; 32], encrypted_data: &[u8]) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            return Err(anyhow!("Encrypted data too short"));
        }
        let cipher = Aes256Gcm::new(key.into());
        let nonce = Nonce::from_slice(&encrypted_data[0..12]);
        let ciphertext = &encrypted_data[12..];

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }
}

/// Key management for files, implementing the Key Wrap strategy.
pub struct KeyManager;

impl KeyManager {
    /// Generates a random 32-byte file key.
    pub fn generate_file_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Wraps (encrypts) the file key with the master key.
    pub fn wrap_key(master_key: &[u8; 32], file_key: &[u8; 32]) -> Result<Vec<u8>> {
        Cipher::encrypt(master_key, file_key)
    }

    /// Unwraps (decrypts) the file key with the master key.
    pub fn unwrap_key(master_key: &[u8; 32], wrapped_key: &[u8]) -> Result<[u8; 32]> {
        let decrypted = Cipher::decrypt(master_key, wrapped_key)?;
        if decrypted.len() != 32 {
            return Err(anyhow!("Unwrapped key has invalid length"));
        }
        let mut file_key = [0u8; 32];
        file_key.copy_from_slice(&decrypted);
        Ok(file_key)
    }
}

/// Shamir's Secret Sharing wrapper for splitting keys.
pub struct KeySplitter;

impl KeySplitter {
    /// Splits a secret into N shares with a threshold of K.
    pub fn split(secret: &[u8], n: u8, k: u8) -> Result<Vec<Vec<u8>>> {
        let sharks = Sharks(k);
        let dealer = sharks.dealer(secret);
        let shares: Vec<Share> = dealer.take(n as usize).collect();
        // Convert to bytes for easier transmission/storage
        let encoded_shares: Vec<Vec<u8>> = shares.into_iter().map(|s| {
            // sharks `Share` implements `Into<Vec<u8>>` and `From<Vec<u8>>`
            Vec::from(&s)
        }).collect();
        Ok(encoded_shares)
    }

    /// Recovers a secret from a set of shares.
    pub fn recover(shares: &[Vec<u8>], k: u8) -> Result<Vec<u8>> {
        let sharks = Sharks(k);
        let decoded_shares: Result<Vec<Share>, _> = shares.iter().map(|s| {
            if s.is_empty() {
                return Err(anyhow!("Empty share"));
            }
            // In the sharks crate, Share is created using from
            Share::try_from(s.as_slice()).map_err(|_| anyhow!("Invalid share format"))
        }).collect();

        let secret = sharks.recover(&decoded_shares?)
            .map_err(|e| anyhow!("Failed to recover secret: {:?}", e))?;
        Ok(secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnemonic_generation_and_derivation() {
        let mnemonic = generate_mnemonic().unwrap();
        assert_eq!(mnemonic.split_whitespace().count(), 24);

        let id1 = Identity::from_mnemonic(&mnemonic).unwrap();
        let id2 = Identity::from_mnemonic(&mnemonic).unwrap();

        assert_eq!(id1.peer_id, id2.peer_id);
        assert_eq!(id1.master_key, id2.master_key);
    }

    #[test]
    fn test_encryption_decryption() {
        let key = KeyManager::generate_file_key();
        let plaintext = b"hello shardnet";
        let encrypted = Cipher::encrypt(&key, plaintext).unwrap();
        let decrypted = Cipher::decrypt(&key, &encrypted).unwrap();
        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_key_wrap() {
        let master_key = KeyManager::generate_file_key();
        let file_key = KeyManager::generate_file_key();

        let wrapped = KeyManager::wrap_key(&master_key, &file_key).unwrap();
        let unwrapped = KeyManager::unwrap_key(&master_key, &wrapped).unwrap();

        assert_eq!(file_key, unwrapped);
    }

    #[test]
    fn test_shamir_secret_sharing() {
        let secret = b"my super secret master key or something else";
        let n = 5;
        let k = 3;

        let shares = KeySplitter::split(secret, n, k).unwrap();
        assert_eq!(shares.len(), 5);

        // Recover with 3 shares
        let subset1 = vec![shares[0].clone(), shares[2].clone(), shares[4].clone()];
        let recovered1 = KeySplitter::recover(&subset1, k).unwrap();
        assert_eq!(secret.as_slice(), recovered1.as_slice());

        // Recover with 4 shares
        let subset2 = vec![shares[0].clone(), shares[1].clone(), shares[2].clone(), shares[3].clone()];
        let recovered2 = KeySplitter::recover(&subset2, k).unwrap();
        assert_eq!(secret.as_slice(), recovered2.as_slice());
    }
}
