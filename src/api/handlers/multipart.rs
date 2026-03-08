use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use tracing::info;

use crate::api::error::S3Error;
use crate::api::state::AppState;
use crate::metadata::models::{MultipartUpload, PartMeta};

#[derive(Deserialize)]
pub struct MultipartQuery {
    #[serde(rename = "uploadId")]
    pub upload_id: Option<String>,
    #[serde(rename = "partNumber")]
    pub part_number: Option<u16>,
    pub uploads: Option<String>,
}

/// POST /{bucket}/{key}?uploads → CreateMultipartUpload
pub async fn create_multipart_upload(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    if !state.store.bucket_exists(&bucket)? {
        return Err(S3Error::NoSuchBucket);
    }

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let upload_id = uuid::Uuid::new_v4().to_string();

    let upload = MultipartUpload {
        upload_id: upload_id.clone(),
        bucket: bucket.clone(),
        key: key.clone(),
        created: Utc::now(),
        content_type,
    };

    state.store.create_multipart_upload(&upload)?;

    info!("CreateMultipartUpload {bucket}/{key} -> {upload_id}");

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<InitiateMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Bucket>{}</Bucket>
  <Key>{}</Key>
  <UploadId>{}</UploadId>
</InitiateMultipartUploadResult>"#,
        crate::api::xml::escape_xml(&bucket),
        crate::api::xml::escape_xml(&key),
        crate::api::xml::escape_xml(&upload_id),
    );

    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}

/// PUT /{bucket}/{key}?partNumber=N&uploadId=ID → UploadPart
pub async fn upload_part(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(query): Query<MultipartQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, S3Error> {
    let body = crate::api::chunked::decode_aws_chunked(&headers, body);
    let upload_id = query.upload_id.ok_or(S3Error::InvalidArgument("missing uploadId".into()))?;
    let part_number = query.part_number.ok_or(S3Error::InvalidArgument("missing partNumber".into()))?;

    let _upload = state
        .store
        .get_multipart_upload(&upload_id)?
        .ok_or(S3Error::NoSuchUpload)?;

    let _permit = state
        .upload_semaphore
        .acquire()
        .await
        .map_err(|_| S3Error::InternalError(anyhow::anyhow!("upload semaphore closed")))?;

    let bucket_meta = state
        .store
        .get_bucket(&bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    // Store part as a replicated object across all sites, same as put_object
    let backend_key = format!("_multipart/{upload_id}/{}", uuid::Uuid::new_v4());
    let (chunks, _) = super::object::put_replicated(&state, &bucket_meta, &backend_key, &body, "application/octet-stream").await?;

    // Use the site that actually confirmed the write (quorum=1 means only one may be done)
    let confirmed = chunks
        .iter()
        .find(|c| c.status == crate::metadata::models::ChunkStatus::Confirmed)
        .ok_or_else(|| S3Error::InternalError(anyhow::anyhow!("no confirmed replica for part")))?;

    let body_for_hash = body.clone();
    let etag = tokio::task::spawn_blocking(move || {
        use md5::{Digest, Md5};
        format!("{:x}", Md5::digest(&body_for_hash))
    }).await.map_err(|e| S3Error::InternalError(e.into()))?;

    let part_meta = PartMeta {
        part_number,
        etag: etag.clone(),
        size: body.len() as u64,
        backend_key,
        site: confirmed.site.clone(),
        backend_bucket: confirmed.bucket.clone(),
    };

    state.store.put_part(&upload_id, &part_meta)?;

    info!("UploadPart {bucket}/{key} upload={upload_id} part={part_number} ({} bytes)", body.len());

    Ok((
        StatusCode::OK,
        [("etag", format!("\"{etag}\""))],
        "",
    )
        .into_response())
}

/// POST /{bucket}/{key}?uploadId=ID → CompleteMultipartUpload
pub async fn complete_multipart_upload(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(query): Query<MultipartQuery>,
    body: Bytes,
) -> Result<Response, S3Error> {
    let upload_id = query.upload_id.ok_or(S3Error::InvalidArgument("missing uploadId".into()))?;

    let upload = state
        .store
        .get_multipart_upload(&upload_id)?
        .ok_or(S3Error::NoSuchUpload)?;

    // Parse the completion XML to get ordered part list
    let body_str = String::from_utf8(body.to_vec())
        .map_err(|_| S3Error::MalformedXML)?;
    let requested_parts = parse_complete_multipart_xml(&body_str)?;

    // Read stored parts metadata
    let stored_parts = state.store.list_parts(&upload_id)?;

    // Validate all requested parts exist
    for rp in &requested_parts {
        if !stored_parts.iter().any(|sp| sp.part_number == rp.part_number) {
            return Err(S3Error::InvalidArgument(
                format!("part {} not found", rp.part_number),
            ));
        }
    }

    // Fetch part data from backend and concatenate in order
    let mut assembled = Vec::new();
    for rp in &requested_parts {
        let part = stored_parts
            .iter()
            .find(|sp| sp.part_number == rp.part_number)
            .unwrap();

        let client = state
            .clients
            .iter()
            .find(|c| c.site_name == part.site)
            .ok_or_else(|| S3Error::InternalError(anyhow::anyhow!("no client for site {}", part.site)))?;

        let output = client
            .get_object(&part.backend_bucket, &part.backend_key)
            .await
            .map_err(|e| S3Error::InternalError(e))?;

        let data = output.data.ok_or_else(|| {
            S3Error::InternalError(anyhow::anyhow!("part {} data not found on backend", rp.part_number))
        })?;
        assembled.extend_from_slice(&data);
    }

    let assembled_bytes = Bytes::from(assembled);

    // Store the assembled object using the normal put logic
    let bucket_meta = state
        .store
        .get_bucket(&bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    let config = state.config.read();
    let erasure = &config.erasure;
    let data_chunks = bucket_meta.data_chunks.unwrap_or(erasure.data_chunks);
    let parity_chunks = bucket_meta.parity_chunks.unwrap_or(erasure.parity_chunks);
    let storage_mode = bucket_meta.storage_mode;
    let object_size = assembled_bytes.len() as u64;

    let effective_mode = match storage_mode {
        crate::config::StorageMode::Hybrid => {
            if object_size < erasure.hybrid_threshold_bytes {
                crate::config::StorageMode::Replica
            } else {
                crate::config::StorageMode::Erasure
            }
        }
        other => other,
    };
    drop(config);

    let content_type = upload.content_type.clone();

    let final_backend_key = uuid::Uuid::new_v4().to_string();
    let (chunks, obj_storage_mode) = match effective_mode {
        crate::config::StorageMode::Replica => {
            super::object::put_replicated(&state, &bucket_meta, &final_backend_key, &assembled_bytes, &content_type).await?
        }
        crate::config::StorageMode::Erasure => {
            super::object::put_erasure_coded(
                &state, &bucket_meta, &final_backend_key, &assembled_bytes, &content_type, data_chunks, parity_chunks,
            ).await?
        }
        crate::config::StorageMode::Hybrid => unreachable!(),
    };

    let hash_bytes = assembled_bytes.clone();
    let num_parts = requested_parts.len();
    let etag = tokio::task::spawn_blocking(move || {
        use md5::{Digest, Md5};
        format!("{:x}-{}", Md5::digest(&hash_bytes), num_parts)
    }).await.map_err(|e| S3Error::InternalError(e.into()))?;

    let meta = crate::metadata::models::ObjectMeta {
        key: key.clone(),
        bucket: bucket.clone(),
        size: object_size,
        etag: etag.clone(),
        content_type,
        created: Utc::now(),
        modified: Utc::now(),
        user_metadata: std::collections::HashMap::new(),
        storage_mode: obj_storage_mode,
        chunks,
    };

    state.store.put_object(&meta)?;

    // Clean up: delete part objects from all backend sites and metadata
    cleanup_parts(&state, &bucket_meta, &stored_parts).await;
    state.store.delete_multipart_upload(&upload_id)?;

    info!("CompleteMultipartUpload {bucket}/{key} ({object_size} bytes, {} parts)", requested_parts.len());

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Bucket>{}</Bucket>
  <Key>{}</Key>
  <ETag>"{}"</ETag>
</CompleteMultipartUploadResult>"#,
        crate::api::xml::escape_xml(&bucket),
        crate::api::xml::escape_xml(&key),
        crate::api::xml::escape_xml(&etag),
    );

    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}

/// DELETE /{bucket}/{key}?uploadId=ID → AbortMultipartUpload
pub async fn abort_multipart_upload(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(query): Query<MultipartQuery>,
) -> Result<Response, S3Error> {
    let upload_id = query.upload_id.ok_or(S3Error::InvalidArgument("missing uploadId".into()))?;

    let upload = state
        .store
        .get_multipart_upload(&upload_id)?
        .ok_or(S3Error::NoSuchUpload)?;

    let bucket_meta = state
        .store
        .get_bucket(&upload.bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    // Delete part objects from all backend sites
    let parts = state.store.list_parts(&upload_id)?;
    cleanup_parts(&state, &bucket_meta, &parts).await;

    state.store.delete_multipart_upload(&upload_id)?;

    info!("AbortMultipartUpload {bucket}/{key} upload={upload_id}");

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// GET /{bucket}?uploads → ListMultipartUploads
pub async fn list_multipart_uploads(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    let uploads = state.store.list_multipart_uploads(&bucket)?;

    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str(r#"<ListMultipartUploadsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);
    xml.push_str(&format!("<Bucket>{}</Bucket>", crate::api::xml::escape_xml(&bucket)));

    for u in &uploads {
        xml.push_str("<Upload>");
        xml.push_str(&format!("<Key>{}</Key>", crate::api::xml::escape_xml(&u.key)));
        xml.push_str(&format!("<UploadId>{}</UploadId>", crate::api::xml::escape_xml(&u.upload_id)));
        xml.push_str(&format!("<Initiated>{}</Initiated>", u.created.to_rfc3339()));
        xml.push_str("</Upload>");
    }

    xml.push_str("</ListMultipartUploadsResult>");

    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}

async fn cleanup_parts(state: &AppState, bucket_meta: &crate::metadata::models::BucketMeta, parts: &[PartMeta]) {
    for part in parts {
        for bb in &bucket_meta.backend_buckets {
            if let Some(client) = state.clients.iter().find(|c| c.site_name == bb.site) {
                let _ = client.delete_object(&bb.bucket_name, &part.backend_key).await;
            }
        }
    }
}

struct RequestedPart {
    part_number: u16,
}

fn parse_complete_multipart_xml(body: &str) -> Result<Vec<RequestedPart>, S3Error> {
    let mut parts = Vec::new();
    for segment in body.split("<PartNumber>") {
        if let Some(end) = segment.find("</PartNumber>") {
            let num_str = &segment[..end];
            let part_number: u16 = num_str
                .trim()
                .parse()
                .map_err(|_| S3Error::MalformedXML)?;
            parts.push(RequestedPart { part_number });
        }
    }
    if parts.is_empty() {
        return Err(S3Error::MalformedXML);
    }
    parts.sort_by_key(|p| p.part_number);
    Ok(parts)
}
