use anyhow::Result;

use super::models::{BucketMeta, MultipartUpload, ObjectMeta, PartMeta, PurgeEntry};

pub struct BucketStats {
    pub object_count: u64,
    pub total_size: u64,
}

pub struct ListObjectsResult {
    pub entries: Vec<super::models::ListEntry>,
    pub common_prefixes: Vec<String>,
    pub truncated: bool,
}

pub trait MetadataBackend: Send + Sync {
    // -- Object operations --
    fn put_object(&self, meta: &ObjectMeta) -> Result<()>;
    fn get_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectMeta>>;
    fn delete_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectMeta>>;
    fn delete_object_and_enqueue_purge(&self, bucket: &str, key: &str) -> Result<bool>;
    fn batch_delete_and_enqueue_purge(&self, bucket: &str, keys: &[String]) -> Result<Vec<(String, bool)>>;
    fn head_object(&self, bucket: &str, key: &str) -> Result<bool>;
    fn list_objects(
        &self,
        bucket: &str,
        prefix: &str,
        delimiter: Option<&str>,
        start_after: Option<&str>,
        max_keys: usize,
    ) -> Result<ListObjectsResult>;

    // -- Bucket operations --
    fn create_bucket(&self, meta: &BucketMeta) -> Result<()>;
    fn get_bucket(&self, name: &str) -> Result<Option<BucketMeta>>;
    fn delete_bucket(&self, name: &str) -> Result<bool>;
    fn list_buckets(&self) -> Result<Vec<BucketMeta>>;
    fn bucket_exists(&self, name: &str) -> Result<bool>;
    fn bucket_is_empty(&self, name: &str) -> Result<bool>;

    // -- Purge queue operations --
    fn enqueue_purge(&self, entry: &PurgeEntry) -> Result<()>;
    fn list_purge_entries(&self, limit: usize) -> Result<Vec<PurgeEntry>>;
    fn update_purge_entry(&self, entry: &PurgeEntry) -> Result<()>;
    fn remove_purge_entry(&self, id: &str) -> Result<()>;
    fn purge_queue_depth(&self) -> Result<usize>;

    // -- Multipart upload operations --
    fn create_multipart_upload(&self, upload: &MultipartUpload) -> Result<()>;
    fn get_multipart_upload(&self, upload_id: &str) -> Result<Option<MultipartUpload>>;
    fn delete_multipart_upload(&self, upload_id: &str) -> Result<()>;
    fn list_multipart_uploads(&self, bucket: &str) -> Result<Vec<MultipartUpload>>;
    fn put_part(&self, upload_id: &str, part: &PartMeta) -> Result<()>;
    fn list_parts(&self, upload_id: &str) -> Result<Vec<PartMeta>>;
    fn delete_parts(&self, upload_id: &str) -> Result<()>;

    // -- Bucket stats --
    fn bucket_stats(&self, bucket: &str) -> Result<BucketStats>;

    // -- Runtime config --
    fn get_runtime_config(&self) -> Result<Option<crate::config::RuntimeConfig>>;
    fn put_runtime_config(&self, config: &crate::config::RuntimeConfig) -> Result<()>;

    // -- Snapshots --
    fn create_checkpoint(&self, path: &str) -> Result<()>;
}

pub type MetadataStore = std::sync::Arc<dyn MetadataBackend>;
