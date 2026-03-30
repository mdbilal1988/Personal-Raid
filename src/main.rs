pub mod crypto;
pub mod network;
pub mod storage;
pub mod sync;
pub mod daemon;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new admin node
    Init {
        #[arg(short, long)]
        storage_dir: String,
    },
    /// Join a network via an invitation string
    Join {
        #[arg(short, long)]
        invitation: String,
        #[arg(short, long)]
        storage_dir: String,
    },
    /// Rotate the master key
    RotateKey {
        #[arg(short, long)]
        new_seed: String,
        #[arg(short, long)]
        storage_dir: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { storage_dir } => {
            info!("Initializing ShardNet as Admin...");
            let seed = crypto::generate_mnemonic()?;
            println!("=== ADMIN RECOVERY SEED ===");
            println!("{}", seed);
            println!("===========================");
            println!("Store this safely! You need it to recover the network.");

            let d = daemon::Daemon::new_admin(&seed, storage_dir).await?;
            let inv = d.generate_invitation("/ip4/127.0.0.1/tcp/4001")?;
            println!("Invite others using: {}", inv.to_base64()?);

            d.run().await?;
        }
        Commands::Join { invitation, storage_dir } => {
            info!("Joining ShardNet from Invitation...");
            let d = daemon::Daemon::join_from_invitation(invitation, storage_dir).await?;
            d.run().await?;
        }
        Commands::RotateKey { new_seed, storage_dir } => {
            info!("Rotating Master Key...");
            let d = daemon::Daemon::new_admin(new_seed, storage_dir).await?;
            d.rotate_master_key(new_seed).await?;
        }
    }

    Ok(())
}
