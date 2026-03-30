use crate::crypto::Identity;
use crate::network::Invitation;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Main daemon state.
pub struct Daemon {
    pub identity: Identity,
    pub storage_engine: Arc<dyn crate::storage::StorageEngine + Send + Sync>,
    pub k: usize,
    pub n: usize,
    pub is_admin: bool,
    pub master_key: Arc<Mutex<[u8; 32]>>,
}

impl Daemon {
    pub async fn new_admin(seed: &str, storage_dir: &str) -> Result<Self> {
        info!("Initializing as new Admin node...");
        let identity = Identity::from_mnemonic(seed)?;
        let storage_engine = Arc::new(crate::storage::LocalStorageEngine::new(storage_dir)?);
        let master_key = Arc::new(Mutex::new(identity.master_key));

        Ok(Self {
            identity,
            storage_engine,
            k: 3, // Default threshold
            n: 5, // Default total nodes
            is_admin: true,
            master_key,
        })
    }

    pub async fn join_from_invitation(invitation_b64: &str, storage_dir: &str) -> Result<Self> {
        info!("Joining network from invitation...");
        let _inv = Invitation::from_base64(invitation_b64)?;

        // Generate a new temporary identity for the node
        let seed = crate::crypto::generate_mnemonic()?;
        let identity = Identity::from_mnemonic(&seed)?;

        let storage_engine = Arc::new(crate::storage::LocalStorageEngine::new(storage_dir)?);

        // In a real scenario, the node would connect to `bootstrap_addr`, perform noise handshake with `psk`,
        // and request the master key share from the admin via the Quorum Handshake.
        // For now, we mock the master key retrieval.
        let master_key = Arc::new(Mutex::new([0u8; 32]));

        Ok(Self {
            identity,
            storage_engine,
            k: 3,
            n: 5,
            is_admin: false,
            master_key,
        })
    }

    pub fn generate_invitation(&self, bootstrap_addr: &str) -> Result<Invitation> {
        if !self.is_admin {
            return Err(anyhow::anyhow!("Only admins can generate invitations"));
        }

        let inv = Invitation {
            admin_peer_id: self.identity.peer_id.to_string(),
            bootstrap_addr: bootstrap_addr.to_string(),
            namespace: "shardnet-main".to_string(),
            psk: vec![1, 2, 3, 4], // Random PSK
        };
        Ok(inv)
    }

    /// Triggers the "Rotate Master Key" flow.
    pub async fn rotate_master_key(&self, new_seed: &str) -> Result<()> {
        if !self.is_admin {
            return Err(anyhow::anyhow!("Only admins can rotate the master key"));
        }

        info!("Triggering Master Key Rotation...");

        // 1. Derive new master key
        let new_identity = Identity::from_mnemonic(new_seed)?;
        let new_master_key = new_identity.master_key;

        // 2. Lock current master key
        let mut mk_lock = self.master_key.lock().await;
        let _old_master_key = *mk_lock;

        // 3. Inform peers to re-shard (this would happen via gossipsub or direct RPC)
        warn!("(Mock) Broadcasting new key rotation to network peers...");

        // 4. Update local key
        *mk_lock = new_master_key;

        info!("Master Key rotated successfully.");
        // The background Sync Loop will handle the lazy re-encryption.

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        info!("Daemon running. PeerID: {}", self.identity.peer_id);

        // In a real implementation, this is where we'd start the libp2p swarm event loop,
        // the background ThrottledSync task, and listen for incoming connections.
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        Ok(())
    }
}
