use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, warn};

use super::signing::{S3Signer, SignableBodyRef};
use crate::config::{SiteConfig, UrlStyle};

#[derive(Clone)]
pub struct SiteClient {
    pub site_name: String,
    pub region: String,
    pub endpoint: String,
    url_style: UrlStyle,
    http: Client,
    signer: std::sync::Arc<S3Signer>,
}

impl SiteClient {
    pub fn new(config: &SiteConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(32)
            .build()
            .context("failed to build HTTP client")?;

        let signer = S3Signer::new(&config.access_key, &config.secret_key, &config.region);

        Ok(Self {
            site_name: config.name.clone(),
            region: config.region.clone(),
            endpoint: config.endpoint.clone(),
            url_style: config.url_style,
            http,
            signer: std::sync::Arc::new(signer),
        })
    }

    fn url(&self, bucket: &str, key: &str) -> String {
        match self.url_style {
            UrlStyle::Path => format!("{}/{}/{}", self.endpoint, bucket, key),
            UrlStyle::VirtualHost => {
                let base = self.host_base();
                let scheme = self.scheme();
                format!("{scheme}://{bucket}.{base}/{key}")
            }
        }
    }

    fn bucket_url(&self, bucket: &str) -> String {
        match self.url_style {
            UrlStyle::Path => format!("{}/{}", self.endpoint, bucket),
            UrlStyle::VirtualHost => {
                let base = self.host_base();
                let scheme = self.scheme();
                format!("{scheme}://{bucket}.{base}")
            }
        }
    }

    fn scheme(&self) -> &str {
        if self.endpoint.starts_with("https://") {
            "https"
        } else {
            "http"
        }
    }

    fn host_base(&self) -> &str {
        self.endpoint
            .strip_prefix("https://")
            .or_else(|| self.endpoint.strip_prefix("http://"))
            .unwrap_or(&self.endpoint)
    }

    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<PutObjectOutput> {
        let url = self.url(bucket, key);
        let ct = content_type.unwrap_or("application/octet-stream");
        let host = self.host_for_bucket(bucket);

        let mut headers = vec![
            ("content-type", ct),
            ("host", host.as_str()),
        ];
        let content_length = data.len().to_string();
        headers.push(("content-length", &content_length));

        let signed = self
            .signer
            .sign_request("PUT", &url, &headers, SignableBodyRef::Bytes(&data))?;

        let resp = self
            .http
            .put(&url)
            .headers(signed.into_header_map())
            .header("content-type", ct)
            .body(data)
            .send()
            .await
            .context("PUT request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "PUT {}/{} to {} failed ({}): {}",
                bucket, key, self.site_name, status, body
            );
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        debug!("PUT {}/{} -> {} (etag={})", bucket, key, self.site_name, etag);

        Ok(PutObjectOutput { etag })
    }

    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<GetObjectOutput> {
        let url = self.url(bucket, key);
        let host = self.host_for_bucket(bucket);
        let headers = vec![("host", host.as_str())];

        let signed = self
            .signer
            .sign_request("GET", &url, &headers, SignableBodyRef::UnsignedPayload)?;

        let resp = self
            .http
            .get(&url)
            .headers(signed.into_header_map())
            .send()
            .await
            .context("GET request failed")?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(GetObjectOutput {
                data: None,
                etag: String::new(),
                content_type: String::new(),
                content_length: 0,
            });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "GET {}/{} from {} failed ({}): {}",
                bucket, key, self.site_name, status, body
            );
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let content_length = resp.content_length().unwrap_or(0);

        let data = resp.bytes().await.context("failed to read response body")?;

        debug!(
            "GET {}/{} <- {} ({} bytes)",
            bucket, key, self.site_name, data.len()
        );

        Ok(GetObjectOutput {
            data: Some(data),
            etag,
            content_type,
            content_length,
        })
    }

    pub async fn head_object(&self, bucket: &str, key: &str) -> Result<Option<HeadObjectOutput>> {
        let url = self.url(bucket, key);
        let host = self.host_for_bucket(bucket);
        let headers = vec![("host", host.as_str())];

        let signed = self
            .signer
            .sign_request("HEAD", &url, &headers, SignableBodyRef::UnsignedPayload)?;

        let resp = self
            .http
            .head(&url)
            .headers(signed.into_header_map())
            .send()
            .await
            .context("HEAD request failed")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            anyhow::bail!(
                "HEAD {}/{} from {} failed ({})",
                bucket, key, self.site_name, resp.status()
            );
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        let content_length = resp.content_length().unwrap_or(0);

        Ok(Some(HeadObjectOutput {
            etag,
            content_length,
        }))
    }

    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        let url = self.url(bucket, key);
        let host = self.host_for_bucket(bucket);
        let headers = vec![("host", host.as_str())];

        let signed = self
            .signer
            .sign_request("DELETE", &url, &headers, SignableBodyRef::UnsignedPayload)?;

        let resp = self
            .http
            .delete(&url)
            .headers(signed.into_header_map())
            .send()
            .await
            .context("DELETE request failed")?;

        let status = resp.status();
        if !status.is_success() && status != reqwest::StatusCode::NOT_FOUND {
            let body = resp.text().await.unwrap_or_default();
            warn!(
                "DELETE {}/{} from {} failed ({}): {}",
                bucket, key, self.site_name, status, body
            );
            anyhow::bail!("DELETE failed ({})", status);
        }

        debug!("DELETE {}/{} -> {}", bucket, key, self.site_name);
        Ok(())
    }

    /// S3 multi-object delete: POST /{bucket}?delete
    pub async fn delete_objects(&self, bucket: &str, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Build the XML body
        let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><Delete><Quiet>true</Quiet>"#);
        for key in keys {
            xml.push_str(&format!(
                "<Object><Key>{}</Key></Object>",
                key.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
            ));
        }
        xml.push_str("</Delete>");

        let body = Bytes::from(xml);

        // S3 requires Content-MD5 for delete requests
        use md5::{Digest, Md5};
        use base64::Engine;
        let md5_digest = Md5::digest(&body);
        let content_md5 = base64::engine::general_purpose::STANDARD.encode(md5_digest);

        let url = match self.url_style {
            UrlStyle::Path => format!("{}/{}?delete", self.endpoint, bucket),
            UrlStyle::VirtualHost => {
                let base = self.host_base();
                let scheme = self.scheme();
                format!("{scheme}://{bucket}.{base}?delete")
            }
        };
        let host = self.host_for_bucket(bucket);
        let content_length = body.len().to_string();
        let headers = vec![
            ("host", host.as_str()),
            ("content-type", "application/xml"),
            ("content-md5", &content_md5),
            ("content-length", &content_length),
        ];

        let signed = self
            .signer
            .sign_request("POST", &url, &headers, SignableBodyRef::Bytes(&body))?;

        let resp = self
            .http
            .post(&url)
            .headers(signed.into_header_map())
            .header("content-type", "application/xml")
            .header("content-md5", &content_md5)
            .body(body)
            .send()
            .await
            .context("batch DELETE request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let resp_body = resp.text().await.unwrap_or_default();
            warn!(
                "Batch DELETE {} keys from {}/{} failed ({}): {}",
                keys.len(), bucket, self.site_name, status, resp_body
            );
            anyhow::bail!("batch DELETE failed ({})", status);
        }

        debug!("Batch DELETE {} keys from {}/{}", keys.len(), bucket, self.site_name);
        Ok(keys.to_vec())
    }

    pub async fn create_bucket(&self, bucket: &str) -> Result<()> {
        let url = self.bucket_url(bucket);
        let host = self.host_for_bucket(bucket);
        let headers = vec![("host", host.as_str())];

        let signed = self
            .signer
            .sign_request("PUT", &url, &headers, SignableBodyRef::Bytes(&[]))?;

        let resp = self
            .http
            .put(&url)
            .headers(signed.into_header_map())
            .send()
            .await
            .context("create bucket request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Create bucket {} at {} failed ({}): {}",
                bucket, self.site_name, status, body
            );
        }

        debug!("Created bucket {} at {}", bucket, self.site_name);
        Ok(())
    }

    pub async fn delete_bucket(&self, bucket: &str) -> Result<()> {
        let url = self.bucket_url(bucket);
        let host = self.host_for_bucket(bucket);
        let headers = vec![("host", host.as_str())];

        let signed = self
            .signer
            .sign_request("DELETE", &url, &headers, SignableBodyRef::UnsignedPayload)?;

        let resp = self
            .http
            .delete(&url)
            .headers(signed.into_header_map())
            .send()
            .await
            .context("delete bucket request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "Delete bucket {} at {} failed ({}): {}",
                bucket, self.site_name, status, body
            );
        }

        debug!("Deleted bucket {} at {}", bucket, self.site_name);
        Ok(())
    }

    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: &str,
    ) -> Result<Vec<ListObjectEntry>> {
        let url = match self.url_style {
            UrlStyle::Path => format!(
                "{}/{}?list-type=2&prefix={}",
                self.endpoint, bucket, prefix
            ),
            UrlStyle::VirtualHost => {
                let base = self.host_base();
                let scheme = self.scheme();
                format!("{scheme}://{bucket}.{base}?list-type=2&prefix={prefix}")
            }
        };
        let host = self.host_for_bucket(bucket);
        let headers = vec![("host", host.as_str())];

        let signed = self
            .signer
            .sign_request("GET", &url, &headers, SignableBodyRef::UnsignedPayload)?;

        let resp = self
            .http
            .get(&url)
            .headers(signed.into_header_map())
            .send()
            .await
            .context("list objects request failed")?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("List objects in {} failed: {}", bucket, body);
        }

        // TODO: parse XML response into ListObjectEntry vec
        let _body = resp.text().await?;
        Ok(Vec::new())
    }

    fn host_for_bucket(&self, bucket: &str) -> String {
        let base = self.host_base();
        match self.url_style {
            UrlStyle::Path => base.to_string(),
            UrlStyle::VirtualHost => format!("{bucket}.{base}"),
        }
    }
}

pub struct PutObjectOutput {
    pub etag: String,
}

pub struct GetObjectOutput {
    pub data: Option<Bytes>,
    pub etag: String,
    pub content_type: String,
    pub content_length: u64,
}

pub struct HeadObjectOutput {
    pub etag: String,
    pub content_length: u64,
}

pub struct ListObjectEntry {
    pub key: String,
    pub size: u64,
    pub etag: String,
}
