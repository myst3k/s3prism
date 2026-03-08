use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub key: String,
    pub bucket: String,
    pub size: u64,
    pub etag: String,
    pub content_type: String,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub user_metadata: HashMap<String, String>,
    pub storage_mode: ObjectStorageMode,
    pub chunks: Vec<ChunkInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectStorageMode {
    Replicated,
    ErasureCoded {
        data_chunks: usize,
        parity_chunks: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub index: usize,
    pub chunk_type: ChunkType,
    pub site: String,
    pub bucket: String,
    pub s3_key: String,
    pub size: u64,
    pub checksum: String,
    pub status: ChunkStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChunkType {
    Data,
    Parity,
    Replica,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChunkStatus {
    Confirmed,
    Pending,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketMeta {
    pub name: String,
    pub created: DateTime<Utc>,
    pub storage_mode: crate::config::StorageMode,
    pub data_chunks: Option<usize>,
    pub parity_chunks: Option<usize>,
    pub backend_buckets: Vec<BackendBucket>,
    // TODO: per-bucket read_strategy override
    // TODO: per-bucket hybrid_threshold override
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendBucket {
    pub site: String,
    pub bucket_name: String,
    pub created: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeEntry {
    pub id: String,
    pub bucket: String,
    pub key: String,
    pub chunks: Vec<PurgeChunk>,
    pub queued_at: DateTime<Utc>,
    pub attempts: u32,
    pub last_attempt: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeChunk {
    pub site: String,
    pub bucket: String,
    pub s3_key: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEntry {
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub last_modified: DateTime<Utc>,
    pub storage_class: String,
}

impl PurgeEntry {
    pub fn from_object_meta(meta: &ObjectMeta) -> Self {
        let chunks = meta
            .chunks
            .iter()
            .map(|c| PurgeChunk {
                site: c.site.clone(),
                bucket: c.bucket.clone(),
                s3_key: c.s3_key.clone(),
                deleted: false,
            })
            .collect();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            bucket: meta.bucket.clone(),
            key: meta.key.clone(),
            chunks,
            queued_at: chrono::Utc::now(),
            attempts: 0,
            last_attempt: None,
        }
    }

    pub fn all_deleted(&self) -> bool {
        self.chunks.iter().all(|c| c.deleted)
    }
}

impl From<&ObjectMeta> for ListEntry {
    fn from(meta: &ObjectMeta) -> Self {
        Self {
            key: meta.key.clone(),
            size: meta.size,
            etag: meta.etag.clone(),
            last_modified: meta.modified,
            storage_class: "STANDARD".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartUpload {
    pub upload_id: String,
    pub bucket: String,
    pub key: String,
    pub created: DateTime<Utc>,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartMeta {
    pub part_number: u16,
    pub etag: String,
    pub size: u64,
    pub backend_key: String,
    pub site: String,
    pub backend_bucket: String,
}
