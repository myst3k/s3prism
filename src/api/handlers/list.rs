use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::api::error::S3Error;
use crate::api::state::AppState;

#[derive(Deserialize, Default)]
#[allow(dead_code)]
pub struct ListObjectsV2Params {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    #[serde(rename = "start-after")]
    pub start_after: Option<String>,
    #[serde(rename = "continuation-token")]
    pub continuation_token: Option<String>,
    #[serde(rename = "max-keys")]
    pub max_keys: Option<usize>,
    #[serde(rename = "list-type")]
    pub list_type: Option<String>,
}

pub async fn list_objects_v2(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    Query(params): Query<ListObjectsV2Params>,
) -> Result<Response, S3Error> {
    if !state.config.read().is_configured() {
        return Err(S3Error::ServiceUnavailable);
    }

    if !state.store.bucket_exists(&bucket)? {
        return Err(S3Error::NoSuchBucket);
    }

    let prefix = params.prefix.as_deref().unwrap_or("");
    let delimiter = params.delimiter.as_deref();
    // continuation-token takes precedence over start-after
    let start_after = params
        .continuation_token
        .as_deref()
        .or(params.start_after.as_deref());
    let max_keys = params.max_keys.unwrap_or(1000).min(1000);

    let result = state
        .store
        .list_objects(&bucket, prefix, delimiter, start_after, max_keys)?;

    let xml = crate::api::xml::list_objects_v2_response(
        &bucket, prefix, delimiter, max_keys, &result,
    );

    Ok((
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response())
}
