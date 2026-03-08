use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, info};

use std::collections::HashMap as SiteMap;

use crate::api::error::S3Error;
use crate::api::state::AppState;
use crate::backend::fanout::{GetTarget, PutTarget};
use crate::config::StorageMode;
use crate::erasure_coding;
use crate::metadata::models::{BucketMeta, ChunkInfo, ChunkStatus, ChunkType, ObjectMeta, ObjectStorageMode};

pub async fn put_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, S3Error> {
    let body = crate::api::chunked::decode_aws_chunked(&headers, body);

    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    // CopyObject: PUT with x-amz-copy-source header
    if let Some(copy_source) = headers.get("x-amz-copy-source") {
        let source = copy_source
            .to_str()
            .map_err(|_| S3Error::InvalidArgument("invalid x-amz-copy-source".into()))?;
        return copy_object(&state, source, &bucket, &key, &headers).await;
    }

    let bucket_meta = state
        .store
        .get_bucket(&bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    let config = state.config.read();
    let erasure = &config.erasure;

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let user_metadata: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| {
            let name = k.as_str();
            if let Some(meta_key) = name.strip_prefix("x-amz-meta-") {
                Some((meta_key.to_string(), v.to_str().unwrap_or("").to_string()))
            } else {
                None
            }
        })
        .collect();

    let data_chunks = bucket_meta.data_chunks.unwrap_or(erasure.data_chunks);
    let parity_chunks = bucket_meta.parity_chunks.unwrap_or(erasure.parity_chunks);
    let storage_mode = bucket_meta.storage_mode;
    let object_size = body.len() as u64;

    let effective_mode = match storage_mode {
        StorageMode::Hybrid => {
            if object_size < erasure.hybrid_threshold_bytes {
                StorageMode::Replica
            } else {
                StorageMode::Erasure
            }
        }
        other => other,
    };

    let backend_key = uuid::Uuid::new_v4().to_string();

    let (chunks, obj_storage_mode) = match effective_mode {
        StorageMode::Replica => put_replicated(&state, &bucket_meta, &backend_key, &body, &content_type).await?,
        StorageMode::Erasure => {
            put_erasure_coded(&state, &bucket_meta, &backend_key, &body, &content_type, data_chunks, parity_chunks).await?
        }
        StorageMode::Hybrid => unreachable!(),
    };

    let body_for_hash = body.clone();
    let etag = tokio::task::spawn_blocking(move || {
        use md5::{Md5, Digest};
        format!("{:x}", Md5::digest(&body_for_hash))
    }).await.map_err(|e| S3Error::InternalError(e.into()))?;

    let meta = ObjectMeta {
        key: key.clone(),
        bucket: bucket.clone(),
        size: object_size,
        etag: etag.clone(),
        content_type,
        created: Utc::now(),
        modified: Utc::now(),
        user_metadata,
        storage_mode: obj_storage_mode,
        chunks,
    };

    state.store.put_object(&meta)?;
    info!("PUT {bucket}/{key} ({object_size} bytes, {effective_mode:?})");

    Ok((
        StatusCode::OK,
        [("etag", format!("\"{etag}\""))],
        "",
    )
        .into_response())
}

fn backend_bucket_map(bucket_meta: &BucketMeta) -> SiteMap<String, String> {
    bucket_meta
        .backend_buckets
        .iter()
        .map(|bb| (bb.site.clone(), bb.bucket_name.clone()))
        .collect()
}

pub async fn put_replicated(
    state: &AppState,
    bucket_meta: &BucketMeta,
    key: &str,
    data: &Bytes,
    content_type: &str,
) -> Result<(Vec<ChunkInfo>, ObjectStorageMode), S3Error> {
    let bb_map = backend_bucket_map(bucket_meta);

    let targets: Vec<PutTarget> = state
        .clients
        .iter()
        .map(|c| {
            let backend_bucket = bb_map
                .get(&c.site_name)
                .cloned()
                .unwrap_or_else(|| bucket_meta.name.clone());
            PutTarget {
                site: c.site_name.clone(),
                bucket: backend_bucket,
                key: key.to_string(),
                data: data.clone(),
                content_type: Some(content_type.to_string()),
            }
        })
        .collect();

    let quorum = 1;
    let results =
        crate::backend::fanout::fanout_put(&state.clients, targets, quorum).await?;

    let chunks: Vec<ChunkInfo> = state
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let backend_bucket = bb_map
                .get(&c.site_name)
                .cloned()
                .unwrap_or_else(|| bucket_meta.name.clone());
            let succeeded = results
                .iter()
                .any(|r| r.site == c.site_name && r.result.is_ok());
            ChunkInfo {
                index: i,
                chunk_type: ChunkType::Replica,
                site: c.site_name.clone(),
                bucket: backend_bucket,
                s3_key: key.to_string(),
                size: data.len() as u64,
                checksum: String::new(),
                status: if succeeded {
                    ChunkStatus::Confirmed
                } else {
                    ChunkStatus::Pending
                },
            }
        })
        .collect();

    Ok((chunks, ObjectStorageMode::Replicated))
}

pub async fn put_erasure_coded(
    state: &AppState,
    bucket_meta: &BucketMeta,
    _key: &str,
    data: &Bytes,
    content_type: &str,
    data_chunks: usize,
    parity_chunks: usize,
) -> Result<(Vec<ChunkInfo>, ObjectStorageMode), S3Error> {
    let total_sites = state.clients.len();
    let total_chunks = data_chunks + parity_chunks;
    if total_chunks > total_sites {
        return Err(S3Error::InternalError(anyhow::anyhow!(
            "need {} sites for {data_chunks}+{parity_chunks} EC, but only {total_sites} configured",
            total_chunks
        )));
    }

    let bb_map = backend_bucket_map(bucket_meta);

    // Build site ordering based on write distribution config
    let config = state.config.read();
    let mut site_indices: Vec<usize> = (0..total_sites).collect();
    match config.erasure.write_distribution {
        crate::config::WriteDistribution::Shuffle => {
            site_indices.shuffle(&mut rand::rng());
        }
        crate::config::WriteDistribution::Priority => {
            // Already ordered by config insertion order (priority)
        }
    }
    drop(config);

    let ec_data = data.to_vec();
    let encoded = tokio::task::spawn_blocking(move || {
        erasure_coding::encode(&ec_data, data_chunks, parity_chunks)
    })
    .await
    .map_err(|e| S3Error::InternalError(e.into()))?
    .map_err(|e| S3Error::InternalError(e))?;

    // Generate a UUID key for each chunk
    let chunk_keys: Vec<String> = (0..encoded.chunks.len())
        .map(|_| uuid::Uuid::new_v4().to_string())
        .collect();

    let targets: Vec<PutTarget> = encoded
        .chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let site_idx = site_indices[i % total_sites];
            let site_name = &state.clients[site_idx].site_name;
            let backend_bucket = bb_map
                .get(site_name)
                .cloned()
                .unwrap_or_else(|| bucket_meta.name.clone());
            PutTarget {
                site: site_name.clone(),
                bucket: backend_bucket,
                key: chunk_keys[i].clone(),
                data: Bytes::from(chunk.data.clone()),
                content_type: Some(content_type.to_string()),
            }
        })
        .collect();

    let quorum = data_chunks;
    let results =
        crate::backend::fanout::fanout_put(&state.clients, targets, quorum).await?;

    let chunks: Vec<ChunkInfo> = encoded
        .chunks
        .iter()
        .enumerate()
        .map(|(i, ec_chunk)| {
            let site_idx = site_indices[i % total_sites];
            let site_name = &state.clients[site_idx].site_name;
            let backend_bucket = bb_map
                .get(site_name)
                .cloned()
                .unwrap_or_else(|| bucket_meta.name.clone());
            let succeeded = results
                .iter()
                .any(|r| r.site == *site_name && r.result.is_ok());

            ChunkInfo {
                index: ec_chunk.index,
                chunk_type: if ec_chunk.is_parity {
                    ChunkType::Parity
                } else {
                    ChunkType::Data
                },
                site: site_name.clone(),
                bucket: backend_bucket,
                s3_key: chunk_keys[i].clone(),
                size: ec_chunk.data.len() as u64,
                checksum: ec_chunk.checksum.clone(),
                status: if succeeded {
                    ChunkStatus::Confirmed
                } else {
                    ChunkStatus::Pending
                },
            }
        })
        .collect();

    Ok((
        chunks,
        ObjectStorageMode::ErasureCoded {
            data_chunks,
            parity_chunks,
        },
    ))
}

async fn copy_object(
    state: &AppState,
    source: &str,
    dest_bucket: &str,
    dest_key: &str,
    headers: &HeaderMap,
) -> Result<Response, S3Error> {
    // Parse source: /bucket/key or bucket/key (with optional leading /)
    let source = source.strip_prefix('/').unwrap_or(source);
    let (src_bucket, src_key) = source
        .split_once('/')
        .ok_or_else(|| S3Error::InvalidArgument("x-amz-copy-source must be /bucket/key".into()))?;

    // Verify destination bucket exists
    state
        .store
        .get_bucket(dest_bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    // Read source object metadata
    let src_meta = state
        .store
        .get_object(src_bucket, src_key)?
        .ok_or(S3Error::NoSuchKey)?;

    // Fetch the source data from backends
    let data = match &src_meta.storage_mode {
        ObjectStorageMode::Replicated => get_replicated(state, &src_meta).await?,
        ObjectStorageMode::ErasureCoded {
            data_chunks,
            parity_chunks,
        } => get_erasure_coded(state, &src_meta, *data_chunks, *parity_chunks).await?,
    };

    // Determine metadata: replace or copy
    let metadata_directive = headers
        .get("x-amz-metadata-directive")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("COPY");

    let (content_type, user_metadata) = if metadata_directive == "REPLACE" {
        let ct = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let meta: HashMap<String, String> = headers
            .iter()
            .filter_map(|(k, v)| {
                k.as_str()
                    .strip_prefix("x-amz-meta-")
                    .map(|mk| (mk.to_string(), v.to_str().unwrap_or("").to_string()))
            })
            .collect();
        (ct, meta)
    } else {
        (src_meta.content_type.clone(), src_meta.user_metadata.clone())
    };

    // Write to destination using the same put logic
    let dest_bucket_meta = state
        .store
        .get_bucket(dest_bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    let config = state.config.read();
    let erasure = &config.erasure;
    let data_chunks = dest_bucket_meta.data_chunks.unwrap_or(erasure.data_chunks);
    let parity_chunks = dest_bucket_meta.parity_chunks.unwrap_or(erasure.parity_chunks);
    let storage_mode = dest_bucket_meta.storage_mode;
    let object_size = data.len() as u64;

    let effective_mode = match storage_mode {
        StorageMode::Hybrid => {
            if object_size < erasure.hybrid_threshold_bytes {
                StorageMode::Replica
            } else {
                StorageMode::Erasure
            }
        }
        other => other,
    };

    let body = Bytes::from(data.to_vec());
    let copy_backend_key = uuid::Uuid::new_v4().to_string();
    let (chunks, obj_storage_mode) = match effective_mode {
        StorageMode::Replica => {
            put_replicated(state, &dest_bucket_meta, &copy_backend_key, &body, &content_type).await?
        }
        StorageMode::Erasure => {
            put_erasure_coded(
                state, &dest_bucket_meta, &copy_backend_key, &body, &content_type, data_chunks, parity_chunks,
            )
            .await?
        }
        StorageMode::Hybrid => unreachable!(),
    };

    let body_for_hash = body.clone();
    let etag = tokio::task::spawn_blocking(move || {
        use md5::{Md5, Digest};
        format!("{:x}", Md5::digest(&body_for_hash))
    }).await.map_err(|e| S3Error::InternalError(e.into()))?;
    let now = Utc::now();

    let meta = ObjectMeta {
        key: dest_key.to_string(),
        bucket: dest_bucket.to_string(),
        size: object_size,
        etag: etag.clone(),
        content_type,
        created: now,
        modified: now,
        user_metadata,
        storage_mode: obj_storage_mode,
        chunks,
    };

    state.store.put_object(&meta)?;
    info!("COPY {src_bucket}/{src_key} -> {dest_bucket}/{dest_key} ({object_size} bytes)");

    // CopyObject returns XML, not empty body
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CopyObjectResult>
  <ETag>"{etag}"</ETag>
  <LastModified>{}</LastModified>
</CopyObjectResult>"#,
        now.to_rfc3339()
    );

    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}

pub async fn get_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    let meta = state
        .store
        .get_object(&bucket, &key)?
        .ok_or(S3Error::NoSuchKey)?;

    let body = match &meta.storage_mode {
        ObjectStorageMode::Replicated => get_replicated(&state, &meta).await?,
        ObjectStorageMode::ErasureCoded {
            data_chunks,
            parity_chunks,
        } => get_erasure_coded(&state, &meta, *data_chunks, *parity_chunks).await?,
    };

    let mut headers = HeaderMap::new();
    headers.insert("content-type", meta.content_type.parse().unwrap());
    headers.insert("etag", format!("\"{}\"", meta.etag).parse().unwrap());
    headers.insert("content-length", meta.size.to_string().parse().unwrap());
    headers.insert(
        "last-modified",
        meta.modified
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string()
            .parse()
            .unwrap(),
    );

    for (k, v) in &meta.user_metadata {
        if let (Ok(name), Ok(val)) = (
            format!("x-amz-meta-{k}").parse::<axum::http::HeaderName>(),
            v.parse::<axum::http::HeaderValue>(),
        ) {
            headers.insert(name, val);
        }
    }

    if body.len() as u64 != meta.size {
        tracing::error!(
            "GET {bucket}/{key} SIZE MISMATCH: meta.size={} body.len={} mode={:?}",
            meta.size, body.len(), meta.storage_mode
        );
    }

    debug!("GET {bucket}/{key} ({} bytes)", meta.size);

    Ok((StatusCode::OK, headers, body).into_response())
}

async fn get_replicated(state: &AppState, meta: &ObjectMeta) -> Result<Bytes, S3Error> {
    let targets: Vec<GetTarget> = meta
        .chunks
        .iter()
        .map(|c| GetTarget {
            site: c.site.clone(),
            bucket: c.bucket.clone(),
            key: c.s3_key.clone(),
        })
        .collect();

    let results =
        crate::backend::fanout::fanout_get(&state.clients, targets, 1).await?;

    for r in &results {
        if let Ok(output) = &r.result {
            if let Some(data) = &output.data {
                return Ok(data.clone());
            }
        }
    }

    Err(S3Error::InternalError(anyhow::anyhow!(
        "no replica returned data"
    )))
}

async fn get_erasure_coded(
    state: &AppState,
    meta: &ObjectMeta,
    data_chunks: usize,
    parity_chunks: usize,
) -> Result<Bytes, S3Error> {
    let targets: Vec<GetTarget> = meta
        .chunks
        .iter()
        .map(|c| GetTarget {
            site: c.site.clone(),
            bucket: c.bucket.clone(),
            key: c.s3_key.clone(),
        })
        .collect();

    let results =
        crate::backend::fanout::fanout_get(&state.clients, targets, data_chunks).await?;

    let mut shards = Vec::new();
    let mut shard_size = 0usize;

    for r in &results {
        if let Ok(output) = &r.result {
            if let Some(data) = &output.data {
                let chunk_info = meta
                    .chunks
                    .iter()
                    .find(|c| c.site == r.site)
                    .expect("chunk info not found for site");

                shard_size = data.len();
                shards.push(erasure_coding::ShardInput {
                    index: chunk_info.index,
                    is_parity: chunk_info.chunk_type == ChunkType::Parity,
                    data: data.to_vec(),
                });
            }
        }
    }

    let original_size = meta.size;
    let decoded = tokio::task::spawn_blocking(move || {
        erasure_coding::decode(shards, data_chunks, parity_chunks, shard_size, original_size)
    })
    .await
    .map_err(|e| S3Error::InternalError(e.into()))??;
    Ok(Bytes::from(decoded))
}

pub async fn head_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    let meta = state
        .store
        .get_object(&bucket, &key)?
        .ok_or(S3Error::NoSuchKey)?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", meta.content_type.parse().unwrap());
    headers.insert("etag", format!("\"{}\"", meta.etag).parse().unwrap());
    headers.insert("content-length", meta.size.to_string().parse().unwrap());
    headers.insert(
        "last-modified",
        meta.modified
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string()
            .parse()
            .unwrap(),
    );

    Ok((StatusCode::OK, headers).into_response())
}

pub async fn delete_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    state.store.delete_object_and_enqueue_purge(&bucket, &key)?;
    state.purge_notify.notify_one();
    debug!("DELETE {bucket}/{key}");

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Deserialize)]
pub struct DeleteObjectsQuery {
    pub delete: Option<String>,
}

pub async fn delete_objects(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    Query(_query): Query<DeleteObjectsQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, S3Error> {
    let body = crate::api::chunked::decode_aws_chunked(&headers, body);

    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    let body_str = String::from_utf8(body.to_vec())
        .map_err(|_| S3Error::MalformedXML)?;
    let keys = crate::api::xml::parse_delete_objects_request(&body_str)?;

    let results = state.store.batch_delete_and_enqueue_purge(&bucket, &keys)?;
    state.purge_notify.notify_one();
    info!("DELETE {bucket} batch: {} keys", keys.len());

    let xml = crate::api::xml::delete_result_response(&results);
    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}
