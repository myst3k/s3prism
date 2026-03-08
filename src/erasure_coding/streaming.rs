use anyhow::{bail, Result};
use sha2::{Digest, Sha256};

use super::encoder::EncodeOutput;
use crate::config::ErasureConfig;

pub struct StreamingEncoder {
    data_chunks: usize,
    parity_chunks: usize,
    block_size: usize,
}

pub struct BlockEncodeOutput {
    pub blocks: Vec<EncodeOutput>,
    pub total_chunks: usize,
    pub original_size: u64,
}

impl StreamingEncoder {
    pub fn new(data_chunks: usize, parity_chunks: usize) -> Self {
        Self {
            data_chunks,
            parity_chunks,
            block_size: crate::config::ErasureConfig::default().block_size_bytes,
        }
    }

    pub fn from_config(config: &ErasureConfig) -> Self {
        Self {
            data_chunks: config.data_chunks,
            parity_chunks: config.parity_chunks,
            block_size: config.block_size_bytes,
        }
    }

    pub fn with_block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size;
        self
    }

    pub fn encode_blocks(&self, data: &[u8]) -> Result<BlockEncodeOutput> {
        if data.is_empty() {
            bail!("cannot encode empty data");
        }

        let original_size = data.len() as u64;
        let mut blocks = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            let end = (offset + self.block_size).min(data.len());
            let block = &data[offset..end];
            let encoded = super::encoder::encode(block, self.data_chunks, self.parity_chunks)?;
            blocks.push(encoded);
            offset = end;
        }

        let total_chunks = blocks.len() * (self.data_chunks + self.parity_chunks);

        Ok(BlockEncodeOutput {
            blocks,
            total_chunks,
            original_size,
        })
    }
}

pub struct StreamingDecoder {
    data_chunks: usize,
    parity_chunks: usize,
}

impl StreamingDecoder {
    pub fn new(data_chunks: usize, parity_chunks: usize) -> Self {
        Self {
            data_chunks,
            parity_chunks,
        }
    }

    pub fn decode_blocks(
        &self,
        block_shards: Vec<Vec<super::decoder::ShardInput>>,
        shard_sizes: &[usize],
        block_sizes: &[u64],
    ) -> Result<Vec<u8>> {
        if block_shards.len() != shard_sizes.len() || block_shards.len() != block_sizes.len() {
            bail!("mismatched block_shards, shard_sizes, and block_sizes lengths");
        }

        let total_size: u64 = block_sizes.iter().sum();
        let mut output = Vec::with_capacity(total_size as usize);

        for (i, shards) in block_shards.into_iter().enumerate() {
            let decoded = super::decoder::decode(
                shards,
                self.data_chunks,
                self.parity_chunks,
                shard_sizes[i],
                block_sizes[i],
            )?;
            output.extend_from_slice(&decoded);
        }

        Ok(output)
    }
}

pub fn compute_checksum(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::decoder::ShardInput;

    #[test]
    fn streaming_roundtrip_single_block() {
        let data = vec![0xABu8; 1000];
        let enc = StreamingEncoder::new(2, 1).with_block_size(2000);
        let encoded = enc.encode_blocks(&data).unwrap();
        assert_eq!(encoded.blocks.len(), 1);
        assert_eq!(encoded.original_size, 1000);

        let block = &encoded.blocks[0];
        let shards: Vec<ShardInput> = block
            .chunks
            .iter()
            .map(|c| ShardInput {
                index: c.index,
                is_parity: c.is_parity,
                data: c.data.clone(),
            })
            .collect();

        let dec = StreamingDecoder::new(2, 1);
        let decoded = dec
            .decode_blocks(
                vec![shards],
                &[block.shard_size],
                &[block.original_size],
            )
            .unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn streaming_roundtrip_multiple_blocks() {
        let data = vec![0xCDu8; 5000];
        let enc = StreamingEncoder::new(2, 1).with_block_size(2000);
        let encoded = enc.encode_blocks(&data).unwrap();
        assert_eq!(encoded.blocks.len(), 3); // 2000 + 2000 + 1000

        let shard_sizes: Vec<usize> = encoded.blocks.iter().map(|b| b.shard_size).collect();
        let block_sizes: Vec<u64> = encoded.blocks.iter().map(|b| b.original_size).collect();
        let block_shards: Vec<Vec<ShardInput>> = encoded
            .blocks
            .iter()
            .map(|block| {
                block
                    .chunks
                    .iter()
                    .map(|c| ShardInput {
                        index: c.index,
                        is_parity: c.is_parity,
                        data: c.data.clone(),
                    })
                    .collect()
            })
            .collect();

        let dec = StreamingDecoder::new(2, 1);
        let decoded = dec
            .decode_blocks(block_shards, &shard_sizes, &block_sizes)
            .unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn streaming_decode_with_missing_shard_per_block() {
        let data = vec![0xEFu8; 3000];
        let enc = StreamingEncoder::new(2, 1).with_block_size(1500);
        let encoded = enc.encode_blocks(&data).unwrap();
        assert_eq!(encoded.blocks.len(), 2);

        let shard_sizes: Vec<usize> = encoded.blocks.iter().map(|b| b.shard_size).collect();
        let block_sizes: Vec<u64> = encoded.blocks.iter().map(|b| b.original_size).collect();

        // Drop shard 0 from each block — should still reconstruct from shard 1 + parity
        let block_shards: Vec<Vec<ShardInput>> = encoded
            .blocks
            .iter()
            .map(|block| {
                block
                    .chunks
                    .iter()
                    .filter(|c| c.index != 0)
                    .map(|c| ShardInput {
                        index: c.index,
                        is_parity: c.is_parity,
                        data: c.data.clone(),
                    })
                    .collect()
            })
            .collect();

        let dec = StreamingDecoder::new(2, 1);
        let decoded = dec
            .decode_blocks(block_shards, &shard_sizes, &block_sizes)
            .unwrap();
        assert_eq!(decoded, data);
    }
}
