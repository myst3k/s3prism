use anyhow::{Context, Result};
use rocksdb::{
    ColumnFamilyDescriptor, DBWithThreadMode, IteratorMode, MultiThreaded, Options, WriteBatch,
};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

use super::models::{BucketMeta, MultipartUpload, ObjectMeta, PartMeta, PurgeEntry};
use super::models::AdminUser;
use super::store::{BucketStats, ListObjectsResult, MetadataBackend, SiteStats};

const CF_OBJECTS: &str = "objects";
const CF_BUCKETS: &str = "buckets";
const CF_PURGE: &str = "purge";
const CF_CONFIG: &str = "config";
const CF_MULTIPART: &str = "multipart";
const CF_CREDENTIALS: &str = "credentials";

const RUNTIME_CONFIG_KEY: &[u8] = b"__runtime_config__";

type DB = DBWithThreadMode<MultiThreaded>;

pub struct RocksDbBackend {
    db: Arc<DB>,
}

impl RocksDbBackend {
    pub fn open(path: &str) -> Result<Self> {
        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create database directory")?;
        }

        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        opts.set_max_background_jobs(4);
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB

        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_OBJECTS, Options::default()),
            ColumnFamilyDescriptor::new(CF_BUCKETS, Options::default()),
            ColumnFamilyDescriptor::new(CF_PURGE, Options::default()),
            ColumnFamilyDescriptor::new(CF_CONFIG, Options::default()),
            ColumnFamilyDescriptor::new(CF_MULTIPART, Options::default()),
            ColumnFamilyDescriptor::new(CF_CREDENTIALS, Options::default()),
        ];

        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)
            .context("failed to open RocksDB")?;

        info!("Metadata store opened at {}", path.display());

        Ok(Self { db: Arc::new(db) })
    }

    fn object_key(bucket: &str, key: &str) -> Vec<u8> {
        format!("{bucket}\0{key}").into_bytes()
    }
}

impl MetadataBackend for RocksDbBackend {
    fn put_object(&self, meta: &ObjectMeta) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let db_key = Self::object_key(&meta.bucket, &meta.key);
        let value = serde_json::to_vec(meta)?;
        self.db.put_cf(&cf, &db_key, &value)?;
        Ok(())
    }

    fn get_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectMeta>> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let db_key = Self::object_key(bucket, key);
        match self.db.get_cf(&cf, &db_key)? {
            Some(value) => Ok(Some(serde_json::from_slice(&value)?)),
            None => Ok(None),
        }
    }

    fn delete_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectMeta>> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let db_key = Self::object_key(bucket, key);

        let meta = match self.db.get_cf(&cf, &db_key)? {
            Some(value) => Some(serde_json::from_slice::<ObjectMeta>(&value)?),
            None => None,
        };

        if meta.is_some() {
            self.db.delete_cf(&cf, &db_key)?;
        }

        Ok(meta)
    }

    fn delete_object_and_enqueue_purge(&self, bucket: &str, key: &str) -> Result<bool> {
        let obj_cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let purge_cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;

        let db_key = Self::object_key(bucket, key);

        let meta = match self.db.get_cf(&obj_cf, &db_key)? {
            Some(value) => serde_json::from_slice::<ObjectMeta>(&value)?,
            None => return Ok(false),
        };

        let purge_entry = PurgeEntry::from_object_meta(&meta);
        let purge_value = serde_json::to_vec(&purge_entry)?;

        let mut batch = WriteBatch::default();
        batch.delete_cf(&obj_cf, &db_key);
        batch.put_cf(&purge_cf, purge_entry.id.as_bytes(), &purge_value);
        self.db.write(batch)?;

        Ok(true)
    }

    fn batch_delete_and_enqueue_purge(
        &self,
        bucket: &str,
        keys: &[String],
    ) -> Result<Vec<(String, bool)>> {
        let obj_cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let purge_cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;

        let mut batch = WriteBatch::default();
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            let db_key = Self::object_key(bucket, key);
            match self.db.get_cf(&obj_cf, &db_key)? {
                Some(value) => {
                    let meta: ObjectMeta = serde_json::from_slice(&value)?;
                    let purge_entry = PurgeEntry::from_object_meta(&meta);
                    let purge_value = serde_json::to_vec(&purge_entry)?;

                    batch.delete_cf(&obj_cf, &db_key);
                    batch.put_cf(&purge_cf, purge_entry.id.as_bytes(), &purge_value);
                    results.push((key.clone(), true));
                }
                None => {
                    // S3 semantics: deleting a non-existent key is not an error
                    results.push((key.clone(), true));
                }
            }
        }

        self.db.write(batch)?;
        Ok(results)
    }

    fn head_object(&self, bucket: &str, key: &str) -> Result<bool> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let db_key = Self::object_key(bucket, key);
        Ok(self.db.get_cf(&cf, &db_key)?.is_some())
    }

    fn list_objects(
        &self,
        bucket: &str,
        prefix: &str,
        delimiter: Option<&str>,
        start_after: Option<&str>,
        max_keys: usize,
    ) -> Result<ListObjectsResult> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;

        let scan_prefix = if prefix.is_empty() {
            format!("{bucket}\0")
        } else {
            format!("{bucket}\0{prefix}")
        };

        let iter = self.db.iterator_cf(
            &cf,
            IteratorMode::From(scan_prefix.as_bytes(), rocksdb::Direction::Forward),
        );

        let bucket_prefix = format!("{bucket}\0");
        let mut entries = Vec::new();
        let mut common_prefixes: Vec<String> = Vec::new();
        let mut truncated = false;

        for item in iter {
            let (db_key, value) = item?;
            let key_str = std::str::from_utf8(&db_key)?;

            if !key_str.starts_with(&bucket_prefix) {
                break;
            }

            let object_key = &key_str[bucket_prefix.len()..];

            if !object_key.starts_with(prefix) {
                break;
            }

            if let Some(start) = start_after {
                if object_key <= start {
                    continue;
                }
            }

            if let Some(delim) = delimiter {
                let after_prefix = &object_key[prefix.len()..];
                if let Some(pos) = after_prefix.find(delim) {
                    let common_prefix = format!("{prefix}{}{delim}", &after_prefix[..pos]);
                    if !common_prefixes.contains(&common_prefix) {
                        common_prefixes.push(common_prefix);
                    }
                    continue;
                }
            }

            if entries.len() >= max_keys {
                truncated = true;
                break;
            }

            let meta: ObjectMeta = serde_json::from_slice(&value)?;
            entries.push(super::models::ListEntry::from(&meta));
        }

        Ok(ListObjectsResult {
            entries,
            common_prefixes,
            truncated,
        })
    }

    fn create_bucket(&self, meta: &BucketMeta) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_BUCKETS)
            .context("buckets column family missing")?;
        let value = serde_json::to_vec(meta)?;
        self.db.put_cf(&cf, meta.name.as_bytes(), &value)?;
        Ok(())
    }

    fn get_bucket(&self, name: &str) -> Result<Option<BucketMeta>> {
        let cf = self
            .db
            .cf_handle(CF_BUCKETS)
            .context("buckets column family missing")?;
        match self.db.get_cf(&cf, name.as_bytes())? {
            Some(value) => Ok(Some(serde_json::from_slice(&value)?)),
            None => Ok(None),
        }
    }

    fn delete_bucket(&self, name: &str) -> Result<bool> {
        let cf = self
            .db
            .cf_handle(CF_BUCKETS)
            .context("buckets column family missing")?;
        let exists = self.db.get_cf(&cf, name.as_bytes())?.is_some();
        if exists {
            self.db.delete_cf(&cf, name.as_bytes())?;
        }
        Ok(exists)
    }

    fn list_buckets(&self) -> Result<Vec<BucketMeta>> {
        let cf = self
            .db
            .cf_handle(CF_BUCKETS)
            .context("buckets column family missing")?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        let mut buckets = Vec::new();
        for item in iter {
            let (_, value) = item?;
            let meta: BucketMeta = serde_json::from_slice(&value)?;
            buckets.push(meta);
        }
        Ok(buckets)
    }

    fn bucket_exists(&self, name: &str) -> Result<bool> {
        let cf = self
            .db
            .cf_handle(CF_BUCKETS)
            .context("buckets column family missing")?;
        Ok(self.db.get_cf(&cf, name.as_bytes())?.is_some())
    }

    fn bucket_is_empty(&self, name: &str) -> Result<bool> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let prefix = format!("{name}\0");
        let mut iter = self.db.iterator_cf(
            &cf,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        match iter.next() {
            Some(Ok((key, _))) => {
                let key_str = std::str::from_utf8(&key)?;
                Ok(!key_str.starts_with(&prefix))
            }
            _ => Ok(true),
        }
    }

    fn enqueue_purge(&self, entry: &PurgeEntry) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;
        let value = serde_json::to_vec(entry)?;
        self.db.put_cf(&cf, entry.id.as_bytes(), &value)?;
        Ok(())
    }

    fn list_purge_entries(&self, limit: usize) -> Result<Vec<PurgeEntry>> {
        let cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        let mut entries = Vec::new();
        for item in iter {
            if entries.len() >= limit {
                break;
            }
            let (_, value) = item?;
            let entry: PurgeEntry = serde_json::from_slice(&value)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    fn update_purge_entry(&self, entry: &PurgeEntry) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;
        let value = serde_json::to_vec(entry)?;
        self.db.put_cf(&cf, entry.id.as_bytes(), &value)?;
        Ok(())
    }

    fn remove_purge_entry(&self, id: &str) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;
        self.db.delete_cf(&cf, id.as_bytes())?;
        Ok(())
    }

    fn purge_queue_depth(&self) -> Result<usize> {
        let cf = self
            .db
            .cf_handle(CF_PURGE)
            .context("purge column family missing")?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        Ok(iter.count())
    }

    fn get_runtime_config(&self) -> Result<Option<crate::config::RuntimeConfig>> {
        let cf = self
            .db
            .cf_handle(CF_CONFIG)
            .context("config column family missing")?;
        match self.db.get_cf(&cf, RUNTIME_CONFIG_KEY)? {
            Some(value) => Ok(Some(serde_json::from_slice(&value)?)),
            None => Ok(None),
        }
    }

    fn put_runtime_config(&self, config: &crate::config::RuntimeConfig) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_CONFIG)
            .context("config column family missing")?;
        let value = serde_json::to_vec(config)?;
        self.db.put_cf(&cf, RUNTIME_CONFIG_KEY, &value)?;
        Ok(())
    }

    fn create_multipart_upload(&self, upload: &MultipartUpload) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let db_key = format!("upload:{}", upload.upload_id);
        let value = serde_json::to_vec(upload)?;
        self.db.put_cf(&cf, db_key.as_bytes(), &value)?;
        Ok(())
    }

    fn get_multipart_upload(&self, upload_id: &str) -> Result<Option<MultipartUpload>> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let db_key = format!("upload:{upload_id}");
        match self.db.get_cf(&cf, db_key.as_bytes())? {
            Some(value) => Ok(Some(serde_json::from_slice(&value)?)),
            None => Ok(None),
        }
    }

    fn delete_multipart_upload(&self, upload_id: &str) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let db_key = format!("upload:{upload_id}");
        self.db.delete_cf(&cf, db_key.as_bytes())?;
        self.delete_parts(upload_id)?;
        Ok(())
    }

    fn list_multipart_uploads(&self, bucket: &str) -> Result<Vec<MultipartUpload>> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let prefix = "upload:";
        let iter = self.db.iterator_cf(
            &cf,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut uploads = Vec::new();
        for item in iter {
            let (key, value) = item?;
            let key_str = std::str::from_utf8(&key)?;
            if !key_str.starts_with(prefix) {
                break;
            }
            let upload: MultipartUpload = serde_json::from_slice(&value)?;
            if upload.bucket == bucket {
                uploads.push(upload);
            }
        }
        Ok(uploads)
    }

    fn put_part(&self, upload_id: &str, part: &PartMeta) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let db_key = format!("part:{upload_id}:{:05}", part.part_number);
        let value = serde_json::to_vec(part)?;
        self.db.put_cf(&cf, db_key.as_bytes(), &value)?;
        Ok(())
    }

    fn list_parts(&self, upload_id: &str) -> Result<Vec<PartMeta>> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let prefix = format!("part:{upload_id}:");
        let iter = self.db.iterator_cf(
            &cf,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut parts = Vec::new();
        for item in iter {
            let (key, value) = item?;
            let key_str = std::str::from_utf8(&key)?;
            if !key_str.starts_with(&prefix) {
                break;
            }
            let part: PartMeta = serde_json::from_slice(&value)?;
            parts.push(part);
        }
        Ok(parts)
    }

    fn delete_parts(&self, upload_id: &str) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_MULTIPART)
            .context("multipart column family missing")?;
        let prefix = format!("part:{upload_id}:");
        let iter = self.db.iterator_cf(
            &cf,
            IteratorMode::From(prefix.as_bytes(), rocksdb::Direction::Forward),
        );
        let mut batch = WriteBatch::default();
        for item in iter {
            let (key, _) = item?;
            let key_str = std::str::from_utf8(&key)?;
            if !key_str.starts_with(&prefix) {
                break;
            }
            batch.delete_cf(&cf, &key);
        }
        self.db.write(batch)?;
        Ok(())
    }

    fn put_admin_user(&self, user: &AdminUser) -> Result<()> {
        let cf = self.db.cf_handle(CF_CREDENTIALS).context("credentials CF missing")?;
        let value = serde_json::to_vec(user)?;
        self.db.put_cf(&cf, user.username.as_bytes(), &value)?;
        Ok(())
    }

    fn get_admin_user(&self, username: &str) -> Result<Option<AdminUser>> {
        let cf = self.db.cf_handle(CF_CREDENTIALS).context("credentials CF missing")?;
        match self.db.get_cf(&cf, username.as_bytes())? {
            Some(value) => Ok(Some(serde_json::from_slice(&value)?)),
            None => Ok(None),
        }
    }

    fn delete_admin_user(&self, username: &str) -> Result<bool> {
        let cf = self.db.cf_handle(CF_CREDENTIALS).context("credentials CF missing")?;
        if self.db.get_cf(&cf, username.as_bytes())?.is_none() {
            return Ok(false);
        }
        self.db.delete_cf(&cf, username.as_bytes())?;
        Ok(true)
    }

    fn list_admin_users(&self) -> Result<Vec<AdminUser>> {
        let cf = self.db.cf_handle(CF_CREDENTIALS).context("credentials CF missing")?;
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        let mut users = Vec::new();
        for item in iter {
            let (_k, v) = item?;
            users.push(serde_json::from_slice(&v)?);
        }
        Ok(users)
    }

    fn bucket_stats(&self, bucket: &str) -> Result<BucketStats> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let prefix = format!("{bucket}\0");
        let iter = self.db.prefix_iterator_cf(&cf, prefix.as_bytes());
        let mut object_count = 0u64;
        let mut total_size = 0u64;
        for item in iter {
            let (k, v) = item?;
            if !k.starts_with(prefix.as_bytes()) {
                break;
            }
            object_count += 1;
            if let Ok(meta) = serde_json::from_slice::<ObjectMeta>(&v) {
                total_size += meta.size;
            }
        }
        Ok(BucketStats {
            object_count,
            total_size,
        })
    }

    fn bucket_site_stats(&self, bucket: &str) -> Result<Vec<SiteStats>> {
        let cf = self
            .db
            .cf_handle(CF_OBJECTS)
            .context("objects column family missing")?;
        let prefix = format!("{bucket}\0");
        let iter = self.db.prefix_iterator_cf(&cf, prefix.as_bytes());

        let mut site_map: std::collections::HashMap<String, SiteStats> = std::collections::HashMap::new();

        for item in iter {
            let (k, v) = item?;
            if !k.starts_with(prefix.as_bytes()) {
                break;
            }
            if let Ok(meta) = serde_json::from_slice::<ObjectMeta>(&v) {
                for chunk in &meta.chunks {
                    let entry = site_map.entry(chunk.site.clone()).or_insert_with(|| SiteStats {
                        site: chunk.site.clone(),
                        backend_bucket: chunk.bucket.clone(),
                        chunk_count: 0,
                        total_size: 0,
                    });
                    entry.chunk_count += 1;
                    entry.total_size += chunk.size;
                }
            }
        }

        let mut stats: Vec<SiteStats> = site_map.into_values().collect();
        stats.sort_by(|a, b| a.site.cmp(&b.site));
        Ok(stats)
    }

    fn create_checkpoint(&self, path: &str) -> Result<()> {
        let checkpoint = rocksdb::checkpoint::Checkpoint::new(&self.db)?;
        checkpoint.create_checkpoint(path)?;
        Ok(())
    }
}
