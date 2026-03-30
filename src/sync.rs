use crate::crypto::KeyManager;
use crate::storage::StorageEngine;
use anyhow::{anyhow, Result};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

/// 1 MB/s = 1,048,576 bytes per second
const MAX_BYTES_PER_SECOND: u32 = 1_048_576;

/// Throttled synchronizer for background rebalancing and lazy re-encryption.
pub struct ThrottledSync {
    rate_limiter: Arc<RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, governor::clock::DefaultClock>>,
    storage_engine: Arc<dyn StorageEngine + Send + Sync>,
}

impl ThrottledSync {
    /// Creates a new ThrottledSync with a 1MB/s global rate limit.
    pub fn new(storage_engine: Arc<dyn StorageEngine + Send + Sync>) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(MAX_BYTES_PER_SECOND).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));
        Self {
            rate_limiter,
            storage_engine,
        }
    }

    /// Simulates waiting until there's enough token bucket capacity for `bytes` amount of network transfer.
    pub async fn throttle(&self, bytes: usize) {
        let mut remaining = bytes as u32;
        while remaining > 0 {
            let chunk = std::cmp::min(remaining, MAX_BYTES_PER_SECOND);
            // In a real implementation we would wait on the exact byte count.
            // Governor provides `until_n_ready`, but we'll use a simpler wait for larger chunks here
            if let Some(n) = NonZeroU32::new(chunk) {
                if let Err(e) = self.rate_limiter.until_n_ready(n).await {
                    // If the chunk is larger than burst size, we might need to sleep manually
                    warn!("Rate limiter error (chunk too big?): {:?}", e);
                    sleep(Duration::from_millis(100)).await;
                }
            }
            remaining -= chunk;
        }
    }

    /// Performs a Lazy Re-encryption on a file.
    /// 1. Throttles the download of K shards.
    /// 2. Reconstructs the old file key and decrypts the file.
    /// 3. Wraps the file key with the NEW Master Key.
    /// 4. Re-shards and throttles the upload of N new shards.
    pub async fn lazy_reencrypt_file(
        &self,
        file_id: &str,
        k: usize,
        n: usize,
        original_len: usize,
        old_master_key: &[u8; 32],
        new_master_key: &[u8; 32],
    ) -> Result<()> {
        info!("Starting lazy re-encryption for file: {}", file_id);

        // 1. Load shards locally (in a real system, we might need to fetch them over P2P, so we throttle).
        let mut loaded_shards = Vec::new();
        let mut total_bytes_read = 0;

        for i in 0..n {
            if let Ok(Some(shard)) = self.storage_engine.load_shard(file_id, i) {
                total_bytes_read += shard.data.len();
                loaded_shards.push(Some(shard));
            } else {
                loaded_shards.push(None);
            }
        }

        // Simulate network throttling for reading shards
        self.throttle(total_bytes_read).await;

        // 2. Reconstruct file data
        let reconstructed_data = self.storage_engine.reconstruct_file(&loaded_shards, k, n, original_len)?;

        // Assuming the file content has the wrapped file key prepended
        // 32 bytes wrapped -> 60 bytes with 12 byte nonce + 16 byte MAC (Authentication Tag)
        // In a real scenario, the file key might be stored in a metadata manifest.
        // For demonstration, let's assume the first 60 bytes of the data is the wrapped file key.
        if reconstructed_data.len() < 60 {
            return Err(anyhow!("Reconstructed data too small to contain wrapped key"));
        }

        let old_wrapped_key = &reconstructed_data[0..60];
        let encrypted_payload = &reconstructed_data[60..];

        // Unwrap the file key with the old master key
        let file_key = KeyManager::unwrap_key(old_master_key, old_wrapped_key)?;

        // We don't actually need to decrypt and re-encrypt the entire file!
        // This is the beauty of Key Wrap. We only need to wrap the file key with the NEW master key.
        let new_wrapped_key = KeyManager::wrap_key(new_master_key, &file_key)?;

        // Construct the new data package
        let mut new_data = Vec::with_capacity(60 + encrypted_payload.len());
        new_data.extend_from_slice(&new_wrapped_key);
        new_data.extend_from_slice(encrypted_payload);

        // 3. Re-shard the new data
        let new_shards = self.storage_engine.shard_file(&new_data, k, n)?;

        // 4. Save (and broadcast) new shards, throttled.
        let mut total_bytes_written = 0;
        for shard in new_shards {
            total_bytes_written += shard.data.len();
            self.storage_engine.save_shard(file_id, &shard)?;
        }

        self.throttle(total_bytes_written).await;

        info!("Finished lazy re-encryption for file: {}", file_id);
        Ok(())
    }
}
