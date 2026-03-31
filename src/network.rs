use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use libp2p::{
    gossipsub, identify, kad, ping, rendezvous, request_response, swarm::NetworkBehaviour,
};
use serde::{Deserialize, Serialize};

/// Represents an Elastic Onboarding Invitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub admin_peer_id: String,
    pub bootstrap_addr: String,
    pub namespace: String,
    pub psk: Vec<u8>,
}

impl Invitation {
    /// Encodes the invitation to a Base64 string for easy sharing.
    pub fn to_base64(&self) -> Result<String> {
        let json = serde_json::to_string(self)?;
        Ok(STANDARD.encode(json.as_bytes()))
    }

    /// Decodes an invitation from a Base64 string.
    pub fn from_base64(encoded: &str) -> Result<Self> {
        let bytes = STANDARD.decode(encoded)?;
        let json = String::from_utf8(bytes)?;
        let inv = serde_json::from_str(&json)?;
        Ok(inv)
    }
}

/// The Quorum Handshake request protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumRequest {
    pub file_id: String,
    pub signature: Vec<u8>, // Proof of being on the access list
}

/// The Quorum Handshake response protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumResponse {
    pub share: Option<Vec<u8>>, // The requested key-share (if authorized)
}

/// Custom network behaviour combining Gossipsub, Kademlia, Rendezvous, and Request-Response.
#[derive(NetworkBehaviour)]
pub struct ShardNetBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub rendezvous_client: rendezvous::client::Behaviour,
    pub rendezvous_server: rendezvous::server::Behaviour,
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub quorum_req_res: request_response::cbor::Behaviour<QuorumRequest, QuorumResponse>,
}

impl ShardNetBehaviour {
    // We'll add initialization logic in `src/main.rs` where the swarm is built.
}
