use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use tracing::{info, warn};

use crate::api::error::S3Error;
use crate::api::state::AppState;
use crate::metadata::models::{BackendBucket, BucketMeta};

fn generate_backend_bucket_name(virtual_name: &str) -> String {
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    format!("{virtual_name}-{short_id}")
}

pub async fn create_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    if state.store.bucket_exists(&bucket)? {
        return Err(S3Error::BucketAlreadyExists);
    }

    // Generate unique backend bucket names per site and create them
    let mut created = Vec::new();
    for client in &state.clients {
        let backend_name = generate_backend_bucket_name(&bucket);
        match client.create_bucket(&backend_name).await {
            Ok(()) => {
                created.push((client.site_name.clone(), backend_name));
            }
            Err(e) => {
                warn!(
                    "Failed to create backend bucket {backend_name} at {}: {e}",
                    client.site_name
                );
                // Rollback: delete buckets from sites where they were created
                for (site_name, bk_name) in &created {
                    if let Some(client) = state.clients.iter().find(|c| &c.site_name == site_name) {
                        if let Err(re) = client.delete_bucket(bk_name).await {
                            warn!("Rollback failed for {bk_name} at {site_name}: {re}");
                        }
                    }
                }
                return Err(S3Error::InternalError(anyhow::anyhow!(
                    "failed to create bucket on all sites: {e}"
                )));
            }
        }
    }

    let config = state.config.read();
    let meta = BucketMeta {
        name: bucket.clone(),
        created: Utc::now(),
        storage_mode: config.erasure.default_storage_mode,
        data_chunks: None,
        parity_chunks: None,
        backend_buckets: created
            .into_iter()
            .map(|(site, backend_name)| BackendBucket {
                site,
                bucket_name: backend_name,
                created: Utc::now(),
            })
            .collect(),
    };

    state.store.create_bucket(&meta)?;
    info!("Created virtual bucket {bucket} with backend buckets: {:?}",
          meta.backend_buckets.iter().map(|b| format!("{}@{}", b.bucket_name, b.site)).collect::<Vec<_>>());

    Ok(StatusCode::OK.into_response())
}

pub async fn delete_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    if !state.store.bucket_exists(&bucket)? {
        return Err(S3Error::NoSuchBucket);
    }

    if !state.store.bucket_is_empty(&bucket)? {
        return Err(S3Error::BucketNotEmpty);
    }

    // Look up backend bucket names from metadata
    let bucket_meta = state
        .store
        .get_bucket(&bucket)?
        .ok_or(S3Error::NoSuchBucket)?;

    // Delete actual backend buckets
    for bb in &bucket_meta.backend_buckets {
        if let Some(client) = state.clients.iter().find(|c| c.site_name == bb.site) {
            if let Err(e) = client.delete_bucket(&bb.bucket_name).await {
                warn!("Failed to delete backend bucket {} at {}: {e}", bb.bucket_name, bb.site);
            }
        }
    }

    state.store.delete_bucket(&bucket)?;
    info!("Deleted virtual bucket {bucket}");

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn head_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    if state.store.bucket_exists(&bucket)? {
        Ok(StatusCode::OK.into_response())
    } else {
        Err(S3Error::NoSuchBucket)
    }
}

pub async fn list_buckets(
    State(state): State<AppState>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    let buckets = state.store.list_buckets()?;
    let xml = crate::api::xml::list_buckets_response(&buckets);

    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}
