use anyhow::{bail, Result};
use reed_solomon_simd::ReedSolomonDecoder;

pub struct ShardInput {
    pub index: usize,
    pub is_parity: bool,
    pub data: Vec<u8>,
}

pub fn decode(
    shards: Vec<ShardInput>,
    data_chunks: usize,
    parity_chunks: usize,
    shard_size: usize,
    original_size: u64,
) -> Result<Vec<u8>> {
    if data_chunks == 0 || parity_chunks == 0 {
        bail!("data_chunks and parity_chunks must be > 0");
    }
    if shards.len() < data_chunks {
        bail!(
            "need at least {} shards to decode, got {}",
            data_chunks,
            shards.len()
        );
    }

    let mut decoder = ReedSolomonDecoder::new(data_chunks, parity_chunks, shard_size)?;

    let mut have_originals = vec![false; data_chunks];
    let mut original_data: Vec<Option<Vec<u8>>> = vec![None; data_chunks];

    for shard in &shards {
        if shard.is_parity {
            let recovery_index = shard.index.checked_sub(data_chunks).unwrap_or(shard.index);
            decoder.add_recovery_shard(recovery_index, &shard.data)?;
        } else {
            if shard.index < data_chunks {
                have_originals[shard.index] = true;
                original_data[shard.index] = Some(shard.data.clone());
            }
            decoder.add_original_shard(shard.index, &shard.data)?;
        }
    }

    let result = decoder.decode()?;

    // Reassemble original data in order
    let mut output = Vec::with_capacity(original_size as usize);
    for i in 0..data_chunks {
        let shard = if have_originals[i] {
            original_data[i].as_deref().unwrap()
        } else {
            result.restored_original(i).ok_or_else(|| {
                anyhow::anyhow!("failed to restore shard {i}")
            })?
        };
        output.extend_from_slice(shard);
    }

    output.truncate(original_size as usize);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::erasure_coding::encoder;

    #[test]
    fn decode_with_all_data_shards() {
        let data = b"Hello, erasure coding roundtrip test!";
        let encoded = encoder::encode(data, 2, 1).unwrap();

        let shards: Vec<ShardInput> = encoded
            .chunks
            .iter()
            .filter(|c| !c.is_parity)
            .map(|c| ShardInput {
                index: c.index,
                is_parity: false,
                data: c.data.clone(),
            })
            .collect();

        let decoded = decode(shards, 2, 1, encoded.shard_size, encoded.original_size).unwrap();
        assert_eq!(&decoded, data);
    }

    #[test]
    fn decode_with_missing_data_shard() {
        let data = b"Reconstruct me from parity!";
        let encoded = encoder::encode(data, 2, 1).unwrap();

        // Drop data shard 0, keep data shard 1 + parity shard
        let shards: Vec<ShardInput> = encoded
            .chunks
            .iter()
            .filter(|c| c.index != 0)
            .map(|c| ShardInput {
                index: c.index,
                is_parity: c.is_parity,
                data: c.data.clone(),
            })
            .collect();

        let decoded = decode(shards, 2, 1, encoded.shard_size, encoded.original_size).unwrap();
        assert_eq!(&decoded, data);
    }

    #[test]
    fn decode_with_missing_second_data_shard() {
        let data = b"Another test with shard 1 missing";
        let encoded = encoder::encode(data, 2, 1).unwrap();

        // Drop data shard 1, keep data shard 0 + parity shard
        let shards: Vec<ShardInput> = encoded
            .chunks
            .iter()
            .filter(|c| c.index != 1)
            .map(|c| ShardInput {
                index: c.index,
                is_parity: c.is_parity,
                data: c.data.clone(),
            })
            .collect();

        let decoded = decode(shards, 2, 1, encoded.shard_size, encoded.original_size).unwrap();
        assert_eq!(&decoded, data);
    }

    #[test]
    fn decode_fails_with_insufficient_shards() {
        let data = b"Not enough shards";
        let encoded = encoder::encode(data, 3, 2).unwrap();

        // Only provide 2 shards when we need 3
        let shards: Vec<ShardInput> = encoded
            .chunks
            .iter()
            .take(2)
            .map(|c| ShardInput {
                index: c.index,
                is_parity: c.is_parity,
                data: c.data.clone(),
            })
            .collect();

        assert!(decode(shards, 3, 2, encoded.shard_size, encoded.original_size).is_err());
    }

    #[test]
    fn roundtrip_3_plus_2() {
        let data = vec![42u8; 10_000];
        let encoded = encoder::encode(&data, 3, 2).unwrap();
        assert_eq!(encoded.chunks.len(), 5);

        // Drop 2 data shards (max tolerable with 2 parity)
        let shards: Vec<ShardInput> = encoded
            .chunks
            .iter()
            .filter(|c| c.index != 0 && c.index != 2)
            .map(|c| ShardInput {
                index: c.index,
                is_parity: c.is_parity,
                data: c.data.clone(),
            })
            .collect();

        let decoded = decode(shards, 3, 2, encoded.shard_size, encoded.original_size).unwrap();
        assert_eq!(decoded, data);
    }
}
