use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::{delete, get, head, put},
};
use serde::Deserialize;
use tracing::info;

use super::error::S3Error;
use super::state::AppState;

#[derive(Deserialize, Default)]
#[allow(dead_code)]
struct BucketQuery {
    uploads: Option<String>,
    delete: Option<String>,
    location: Option<String>,
    versioning: Option<String>,
    // ListObjectsV2 params forwarded to list handler
    prefix: Option<String>,
    delimiter: Option<String>,
    #[serde(rename = "start-after")]
    start_after: Option<String>,
    #[serde(rename = "continuation-token")]
    continuation_token: Option<String>,
    #[serde(rename = "max-keys")]
    max_keys: Option<usize>,
    #[serde(rename = "list-type")]
    list_type: Option<String>,
}

#[derive(Deserialize, Default)]
struct ObjectQuery {
    #[serde(rename = "uploadId")]
    upload_id: Option<String>,
    #[serde(rename = "partNumber")]
    part_number: Option<u16>,
    uploads: Option<String>,
}

/// GET /{bucket} — dispatches between ListObjects, ListMultipartUploads, GetBucketLocation, etc.
async fn bucket_get_dispatch(
    state: State<AppState>,
    path: Path<String>,
    query: Query<BucketQuery>,
) -> Result<Response, S3Error> {
    if query.location.is_some() {
        info!("GetBucketLocation {}", path.0);
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><LocationConstraint xmlns="http://s3.amazonaws.com/doc/2006-03-01/">us-east-1</LocationConstraint>"#;
        return Ok((
            axum::http::StatusCode::OK,
            [("content-type", "application/xml")],
            xml,
        ).into_response());
    }
    if query.versioning.is_some() {
        info!("GetBucketVersioning {}", path.0);
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?><VersioningConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/"/>"#;
        return Ok((
            axum::http::StatusCode::OK,
            [("content-type", "application/xml")],
            xml,
        ).into_response());
    }
    if query.uploads.is_some() {
        super::handlers::multipart::list_multipart_uploads(state, path).await
    } else {
        let q = query.0;
        super::handlers::list::list_objects_v2(state, path, axum::extract::Query(
            super::handlers::list::ListObjectsV2Params {
                prefix: q.prefix,
                delimiter: q.delimiter,
                start_after: q.start_after,
                continuation_token: q.continuation_token,
                max_keys: q.max_keys,
                list_type: q.list_type,
            }
        )).await
    }
}

/// POST /{bucket} — batch delete
async fn bucket_post_dispatch(
    state: State<AppState>,
    path: Path<String>,
    query: Query<BucketQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, S3Error> {
    // Only batch delete for now
    super::handlers::object::delete_objects(state, path, axum::extract::Query(
        super::handlers::object::DeleteObjectsQuery { delete: query.0.delete }
    ), headers, body).await
}

/// PUT /{bucket}/{key} — dispatches between PutObject, UploadPart
async fn object_put_dispatch(
    state: State<AppState>,
    path: Path<(String, String)>,
    query: Query<ObjectQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, S3Error> {
    if query.upload_id.is_some() && query.part_number.is_some() {
        super::handlers::multipart::upload_part(
            state, path,
            axum::extract::Query(super::handlers::multipart::MultipartQuery {
                upload_id: query.0.upload_id,
                part_number: query.0.part_number,
                uploads: None,
            }),
            headers,
            body,
        ).await
    } else {
        super::handlers::object::put_object(state, path, headers, body).await
    }
}

/// POST /{bucket}/{key} — dispatches between CreateMultipartUpload, CompleteMultipartUpload
async fn object_post_dispatch(
    state: State<AppState>,
    path: Path<(String, String)>,
    query: Query<ObjectQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, S3Error> {
    if query.uploads.is_some() {
        super::handlers::multipart::create_multipart_upload(state, path, headers).await
    } else if query.upload_id.is_some() {
        super::handlers::multipart::complete_multipart_upload(
            state, path,
            axum::extract::Query(super::handlers::multipart::MultipartQuery {
                upload_id: query.0.upload_id,
                part_number: None,
                uploads: None,
            }),
            body,
        ).await
    } else {
        Err(S3Error::InvalidArgument("missing uploads or uploadId query parameter".into()))
    }
}

/// DELETE /{bucket}/{key} — dispatches between DeleteObject, AbortMultipartUpload
async fn object_delete_dispatch(
    state: State<AppState>,
    path: Path<(String, String)>,
    query: Query<ObjectQuery>,
) -> Result<Response, S3Error> {
    if query.upload_id.is_some() {
        super::handlers::multipart::abort_multipart_upload(
            state, path,
            axum::extract::Query(super::handlers::multipart::MultipartQuery {
                upload_id: query.0.upload_id,
                part_number: None,
                uploads: None,
            }),
        ).await
    } else {
        super::handlers::object::delete_object(state, path).await
    }
}

pub fn build(state: AppState) -> Router {
    Router::new()
        // Bucket operations
        .route("/{bucket}", put(super::handlers::bucket::create_bucket))
        .route("/{bucket}", delete(super::handlers::bucket::delete_bucket))
        .route("/{bucket}", head(super::handlers::bucket::head_bucket))
        .route("/{bucket}", get(bucket_get_dispatch))
        .route("/{bucket}", axum::routing::post(bucket_post_dispatch))
        // Object operations (with multipart dispatch)
        .route("/{bucket}/{*key}", put(object_put_dispatch))
        .route("/{bucket}/{*key}", get(super::handlers::object::get_object))
        .route("/{bucket}/{*key}", head(super::handlers::object::head_object))
        .route("/{bucket}/{*key}", delete(object_delete_dispatch))
        .route("/{bucket}/{*key}", axum::routing::post(object_post_dispatch))
        // Service-level: list all buckets
        .route("/", get(super::handlers::bucket::list_buckets))
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024 * 1024)) // 5GB
        .with_state(state)
}
