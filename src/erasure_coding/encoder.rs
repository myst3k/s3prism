use anyhow::{bail, Result};
use reed_solomon_simd::ReedSolomonEncoder;
use sha2::{Digest, Sha256};

pub struct EncodedChunk {
    pub index: usize,
    pub is_parity: bool,
    pub data: Vec<u8>,
    pub checksum: String,
}

pub struct EncodeOutput {
    pub chunks: Vec<EncodedChunk>,
    pub shard_size: usize,
    pub original_size: u64,
}

pub fn encode(data: &[u8], data_chunks: usize, parity_chunks: usize) -> Result<EncodeOutput> {
    if data_chunks == 0 {
        bail!("data_chunks must be > 0");
    }
    if parity_chunks == 0 {
        bail!("parity_chunks must be > 0");
    }
    if data.is_empty() {
        bail!("cannot encode empty data");
    }

    let original_size = data.len() as u64;
    let shard_size = (data.len() + data_chunks - 1) / data_chunks;
    // reed-solomon-simd requires shard_size >= 2 and even
    let shard_size = shard_size.max(2);
    let shard_size = (shard_size + 1) & !1; // round up to even

    let mut encoder = ReedSolomonEncoder::new(data_chunks, parity_chunks, shard_size)?;

    // Split data into shards, padding the last one with zeros if needed
    for i in 0..data_chunks {
        let start = i * shard_size;
        if start >= data.len() {
            // This shard is entirely padding
            encoder.add_original_shard(vec![0u8; shard_size])?;
        } else {
            let end = (start + shard_size).min(data.len());
            let slice = &data[start..end];
            if slice.len() < shard_size {
                let mut padded = vec![0u8; shard_size];
                padded[..slice.len()].copy_from_slice(slice);
                encoder.add_original_shard(padded)?;
            } else {
                encoder.add_original_shard(slice)?;
            }
        }
    }

    let result = encoder.encode()?;

    let mut chunks = Vec::with_capacity(data_chunks + parity_chunks);

    // Collect data shards
    for i in 0..data_chunks {
        let start = i * shard_size;
        let shard = if start >= data.len() {
            vec![0u8; shard_size]
        } else {
            let end = (start + shard_size).min(data.len());
            let slice = &data[start..end];
            if slice.len() < shard_size {
                let mut padded = vec![0u8; shard_size];
                padded[..slice.len()].copy_from_slice(slice);
                padded
            } else {
                slice.to_vec()
            }
        };
        let checksum = hex::encode(Sha256::digest(&shard));
        chunks.push(EncodedChunk {
            index: i,
            is_parity: false,
            data: shard,
            checksum,
        });
    }

    // Collect parity shards
    for (i, parity) in result.recovery_iter().enumerate() {
        let checksum = hex::encode(Sha256::digest(parity));
        chunks.push(EncodedChunk {
            index: data_chunks + i,
            is_parity: true,
            data: parity.to_vec(),
            checksum,
        });
    }

    Ok(EncodeOutput {
        chunks,
        shard_size,
        original_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let data = b"Hello, erasure coding world! This is a test of the S3Prism system.";
        let output = encode(data, 2, 1).unwrap();
        assert_eq!(output.chunks.len(), 3);
        assert_eq!(output.original_size, data.len() as u64);
        assert!(!output.chunks[0].is_parity);
        assert!(!output.chunks[1].is_parity);
        assert!(output.chunks[2].is_parity);
    }

    #[test]
    fn data_can_be_reconstructed_from_shards() {
        let data = b"ABCDEFGHIJKLMNOP";
        let output = encode(data, 2, 1).unwrap();
        // Data shards should contain the original data (possibly padded)
        let mut reconstructed = Vec::new();
        for chunk in output.chunks.iter().filter(|c| !c.is_parity) {
            reconstructed.extend_from_slice(&chunk.data);
        }
        reconstructed.truncate(output.original_size as usize);
        assert_eq!(&reconstructed, data);
    }

    #[test]
    fn rejects_empty_data() {
        assert!(encode(&[], 2, 1).is_err());
    }

    #[test]
    fn rejects_zero_chunks() {
        assert!(encode(b"test", 0, 1).is_err());
        assert!(encode(b"test", 2, 0).is_err());
    }
}
