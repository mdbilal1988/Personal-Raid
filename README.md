# ShardNet

ShardNet is a decentralized, RAID-style storage daemon built in Rust. It shards data across a dynamic number of peer nodes ($N$) with a configurable recovery threshold ($K$). This project provides the core logic and a Tauri-based dashboard for visualizing network health and bandwidth usage.

## Architecture

*   **Identity**: Uses Ed25519 identities derived deterministically from a BIP-39 mnemonic seed via HKDF-SHA256. The seed also derives the Symmetric Master Key.
*   **Cryptography**: Implements a Key Wrap system. File Keys are generated per file (AES-256-GCM) and then wrapped (encrypted) by the Symmetric Master Key. The Master Key can be split using Shamir's Secret Sharing (SSS).
*   **Storage**: Uses Reed-Solomon Erasure coding to split files into $N$ shards (requiring $K$ shards to reconstruct). Shards are stored locally and checksummed (SHA-256) to detect bit rot.
*   **Networking**: Core logic for a custom libp2p `NetworkBehaviour` combining Gossipsub, Kademlia, Rendezvous, and a Request-Response Quorum Handshake. Includes an Elastic Onboarding `Invitation` system.
*   **Sync & Self-Healing**: Implements a global Tokio rate-limiter (Throttled Sync Loop) capped at 1MB/s for "invisible" background processing. Includes "Lazy Re-encryption" logic that triggers when the Master Key rotates.
*   **UI Dashboard**: A React + Tailwind CSS dashboard built with Tauri. Features a Radial Health Graph indicating the $K/N$ Quorum status and a Live Bandwidth SVG monitor.

---

## 🚀 Getting Started

### Prerequisites

*   [Rust & Cargo](https://rustup.rs/) (latest stable)
*   [Node.js](https://nodejs.org/) & `npm` (v20+ recommended)
*   Tauri Prerequisites (See [Tauri Docs](https://tauri.app/v1/guides/getting-started/prerequisites) for OS-specific dependencies like `webkit2gtk` on Linux).

### 1. Building the Rust Daemon Core

To build and run the daemon CLI:

```bash
cargo build --release
cargo run -- --help
```

#### CLI Commands

**Initialize a new Admin node:**
This will generate a new 24-word BIP-39 seed and print an invitation link.
```bash
cargo run -- init --storage-dir ./data/admin
```

**Join the network:**
Use the Base64 invitation string printed from the `init` command to join as a peer.
```bash
cargo run -- join --invitation <BASE64_STRING> --storage-dir ./data/peer1
```

**Rotate the Master Key:**
If an admin's laptop is compromised, use the 24-word seed on a new machine to reclaim the network. This will trigger the Lazy Re-encryption flow to slowly rotate all file keys in the background at 1MB/s.
```bash
cargo run -- rotate-key --new-seed "your 24 words here..." --storage-dir ./data/admin
```

---

### 2. Running the Tauri Dashboard (UI)

The UI visualizes the state of the local daemon (currently using mock data loops for demonstration).

1. Navigate to the `ui` directory:
   ```bash
   cd ui
   ```
2. Install Node dependencies:
   ```bash
   npm install
   ```
3. Run the development server (browser-only mode):
   ```bash
   npm run dev
   ```
4. Or, build the full Tauri desktop application:
   ```bash
   npm run tauri dev
   ```

## Development & Testing

To run the Rust core unit tests (which cover key wrapping, mnemonic generation, SSS splitting, and file sharding):

```bash
cargo test
```
