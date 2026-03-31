use anyhow::{anyhow, Result};
use reed_solomon_erasure::galois_8::ReedSolomon;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// A local shard that holds a chunk of encrypted data and its checksum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shard {
    pub index: usize,
    pub data: Vec<u8>,
    pub checksum: [u8; 32],
}

impl Shard {
    /// Creates a new Shard and computes its SHA-256 checksum.
    pub fn new(index: usize, data: Vec<u8>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let checksum: [u8; 32] = hasher.finalize().into();
        Self {
            index,
            data,
            checksum,
        }
    }

    /// Verifies the shard data against its stored checksum.
    pub fn verify(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        let computed: [u8; 32] = hasher.finalize().into();
        self.checksum == computed
    }
}

/// The core StorageEngine trait defining standard operations for reading/writing files into shards.
pub trait StorageEngine {
    /// Shards the given file content into N total shards, requiring K to reconstruct.
    fn shard_file(&self, content: &[u8], k: usize, n: usize) -> Result<Vec<Shard>>;

    /// Reconstructs the original file content from a subset of shards.
    fn reconstruct_file(
        &self,
        shards: &[Option<Shard>],
        k: usize,
        n: usize,
        original_len: usize,
    ) -> Result<Vec<u8>>;

    /// Saves a shard to local disk.
    fn save_shard(&self, file_id: &str, shard: &Shard) -> Result<()>;

    /// Loads a shard from local disk, returning None if not found or corrupted.
    fn load_shard(&self, file_id: &str, index: usize) -> Result<Option<Shard>>;
}

/// A local disk-backed implementation of the StorageEngine.
pub struct LocalStorageEngine {
    base_dir: PathBuf,
}

impl LocalStorageEngine {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let path = base_dir.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;
        Ok(Self { base_dir: path })
    }

    fn shard_path(&self, file_id: &str, index: usize) -> PathBuf {
        self.base_dir.join(format!("{}.shard.{}", file_id, index))
    }
}

impl StorageEngine for LocalStorageEngine {
    fn shard_file(&self, content: &[u8], k: usize, n: usize) -> Result<Vec<Shard>> {
        if k == 0 || n == 0 || k > n {
            return Err(anyhow!("Invalid K/N parameters"));
        }

        let parity_count = n - k;
        let rs = ReedSolomon::new(k, parity_count)
            .map_err(|e| anyhow!("Failed to initialize Reed-Solomon: {:?}", e))?;

        // Padding content to be a multiple of K
        let mut padded_content = content.to_vec();
        let rem = content.len() % k;
        if rem != 0 {
            let pad_len = k - rem;
            padded_content.extend(vec![0u8; pad_len]);
        }

        let shard_size = padded_content.len() / k;
        let mut shards: Vec<Vec<u8>> = padded_content
            .chunks_exact(shard_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Allocate empty parity shards
        for _ in 0..parity_count {
            shards.push(vec![0u8; shard_size]);
        }

        rs.encode(&mut shards)
            .map_err(|e| anyhow!("Reed-Solomon encoding failed: {:?}", e))?;

        let final_shards = shards
            .into_iter()
            .enumerate()
            .map(|(i, data)| Shard::new(i, data))
            .collect();

        Ok(final_shards)
    }

    fn reconstruct_file(
        &self,
        shards: &[Option<Shard>],
        k: usize,
        n: usize,
        original_len: usize,
    ) -> Result<Vec<u8>> {
        if shards.len() != n {
            return Err(anyhow!("Expected {} shards array slots", n));
        }

        let parity_count = n - k;
        let rs = ReedSolomon::new(k, parity_count)
            .map_err(|e| anyhow!("Failed to initialize Reed-Solomon: {:?}", e))?;

        // Determine shard size from the first present shard
        let mut shard_size = 0;
        for shard_opt in shards {
            if let Some(shard) = shard_opt {
                shard_size = shard.data.len();
                break;
            }
        }

        if shard_size == 0 {
            return Err(anyhow!("No valid shards provided for reconstruction"));
        }

        // Prepare data for reconstruction
        let mut rs_shards: Vec<Option<Vec<u8>>> = vec![None; n];
        for (i, shard_opt) in shards.iter().enumerate() {
            if let Some(shard) = shard_opt {
                if !shard.verify() {
                    return Err(anyhow!("Shard {} failed checksum verification", i));
                }
                rs_shards[i] = Some(shard.data.clone());
            }
        }

        rs.reconstruct(&mut rs_shards)
            .map_err(|e| anyhow!("Reed-Solomon reconstruction failed: {:?}", e))?;

        // Reassemble data from data shards (first K shards)
        let mut reconstructed_data = Vec::with_capacity(k * shard_size);
        for i in 0..k {
            if let Some(ref data) = rs_shards[i] {
                reconstructed_data.extend_from_slice(data);
            } else {
                return Err(anyhow!("Failed to recover data shard {}", i));
            }
        }

        // Truncate padded bytes
        reconstructed_data.truncate(original_len);

        Ok(reconstructed_data)
    }

    fn save_shard(&self, file_id: &str, shard: &Shard) -> Result<()> {
        let path = self.shard_path(file_id, shard.index);

        // Serialize the shard containing checksum and data
        let encoded =
            bincode::serialize(shard).map_err(|e| anyhow!("Failed to serialize shard: {}", e))?;

        fs::write(&path, encoded)?;
        Ok(())
    }

    fn load_shard(&self, file_id: &str, index: usize) -> Result<Option<Shard>> {
        let path = self.shard_path(file_id, index);
        if !path.exists() {
            return Ok(None);
        }

        let data = fs::read(&path)?;
        let shard: Shard = bincode::deserialize(&data)
            .map_err(|e| anyhow!("Failed to deserialize shard: {}", e))?;

        if !shard.verify() {
            // Bit rot detected
            return Ok(None);
        }

        Ok(Some(shard))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_shard_and_reconstruct() {
        let engine = LocalStorageEngine::new(tempdir().unwrap().path()).unwrap();
        let content = b"This is some super secret file content that needs to be sharded and reconstructed safely.";
        let k = 3;
        let n = 5;

        // Shard
        let shards = engine.shard_file(content, k, n).unwrap();
        assert_eq!(shards.len(), n);

        // Simulate missing shards (lose 2)
        let mut recovered_shards: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();
        recovered_shards[0] = None;
        recovered_shards[3] = None;

        // Reconstruct
        let reconstructed = engine
            .reconstruct_file(&recovered_shards, k, n, content.len())
            .unwrap();
        assert_eq!(content.as_slice(), reconstructed.as_slice());
    }

    #[test]
    fn test_save_and_load_shard() {
        let dir = tempdir().unwrap();
        let engine = LocalStorageEngine::new(dir.path()).unwrap();
        let file_id = "test_file_123";
        let shard = Shard::new(0, b"shard_data".to_vec());

        engine.save_shard(file_id, &shard).unwrap();

        let loaded = engine.load_shard(file_id, 0).unwrap().unwrap();
        assert_eq!(shard.data, loaded.data);
        assert_eq!(shard.checksum, loaded.checksum);
    }
}
