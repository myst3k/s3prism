use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::error;

#[derive(Debug)]
pub enum S3Error {
    NoSuchKey,
    NoSuchBucket,
    BucketAlreadyExists,
    BucketNotEmpty,
    NoSuchUpload,
    AccessDenied,
    InvalidArgument(String),
    MalformedXML,
    InternalError(anyhow::Error),
    NotImplemented,
    ServiceUnavailable,
}

impl IntoResponse for S3Error {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            S3Error::NoSuchKey => (
                StatusCode::NOT_FOUND,
                "NoSuchKey",
                "The specified key does not exist.".to_string(),
            ),
            S3Error::NoSuchBucket => (
                StatusCode::NOT_FOUND,
                "NoSuchBucket",
                "The specified bucket does not exist.".to_string(),
            ),
            S3Error::BucketAlreadyExists => (
                StatusCode::CONFLICT,
                "BucketAlreadyOwnedByYou",
                "Your previous request to create the named bucket succeeded.".to_string(),
            ),
            S3Error::BucketNotEmpty => (
                StatusCode::CONFLICT,
                "BucketNotEmpty",
                "The bucket you tried to delete is not empty.".to_string(),
            ),
            S3Error::NoSuchUpload => (
                StatusCode::NOT_FOUND,
                "NoSuchUpload",
                "The specified multipart upload does not exist.".to_string(),
            ),
            S3Error::AccessDenied => (
                StatusCode::FORBIDDEN,
                "AccessDenied",
                "Access Denied".to_string(),
            ),
            S3Error::InvalidArgument(msg) => (
                StatusCode::BAD_REQUEST,
                "InvalidArgument",
                msg.clone(),
            ),
            S3Error::MalformedXML => (
                StatusCode::BAD_REQUEST,
                "MalformedXML",
                "The XML you provided was not well-formed.".to_string(),
            ),
            S3Error::InternalError(e) => {
                error!("Internal error: {e:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalError",
                    "We encountered an internal error. Please try again.".to_string(),
                )
            }
            S3Error::NotImplemented => (
                StatusCode::NOT_IMPLEMENTED,
                "NotImplemented",
                "This operation is not yet implemented.".to_string(),
            ),
            S3Error::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "ServiceUnavailable",
                "Service is not configured. Complete setup via the management UI.".to_string(),
            ),
        };

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
  <Code>{code}</Code>
  <Message>{message}</Message>
</Error>"#
        );

        (status, [("content-type", "application/xml")], body).into_response()
    }
}

impl From<anyhow::Error> for S3Error {
    fn from(e: anyhow::Error) -> Self {
        S3Error::InternalError(e)
    }
}
