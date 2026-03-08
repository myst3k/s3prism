use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    sign, PayloadChecksumKind, SignableBody, SignableRequest, SigningSettings,
};
use aws_sigv4::sign::v4;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::time::SystemTime;

pub struct S3Signer {
    credentials: Credentials,
    region: String,
}

impl S3Signer {
    pub fn new(access_key: &str, secret_key: &str, region: &str) -> Self {
        let credentials = Credentials::new(
            access_key,
            secret_key,
            None,
            None,
            "s3prism",
        );
        Self {
            credentials,
            region: region.to_string(),
        }
    }

    pub fn sign_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: SignableBodyRef<'_>,
    ) -> Result<SignedHeaders> {
        let mut signing_settings = SigningSettings::default();
        signing_settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;

        let identity = self.credentials.clone().into();
        let signing_params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name("s3")
            .time(SystemTime::now())
            .settings(signing_settings)
            .build()
            .context("failed to build signing params")?
            .into();

        let signable_body = match body {
            SignableBodyRef::Bytes(b) => SignableBody::Bytes(b),
            SignableBodyRef::UnsignedPayload => SignableBody::UnsignedPayload,
            SignableBodyRef::Precomputed(hash) => SignableBody::Precomputed(hash.to_string()),
        };

        let signable_request = SignableRequest::new(
            method,
            url,
            headers.iter().copied(),
            signable_body,
        )
        .context("failed to create signable request")?;

        let (signing_instructions, _signature) =
            sign(signable_request, &signing_params)?.into_parts();

        let mut signed_headers = HeaderMap::new();
        for (name, value) in signing_instructions.headers() {
            signed_headers.insert(
                HeaderName::from_bytes(name.as_bytes())?,
                HeaderValue::from_str(value)?,
            );
        }

        Ok(SignedHeaders(signed_headers))
    }
}

pub enum SignableBodyRef<'a> {
    Bytes(&'a [u8]),
    UnsignedPayload,
    Precomputed(&'a str),
}

pub struct SignedHeaders(pub HeaderMap);

impl SignedHeaders {
    pub fn into_header_map(self) -> HeaderMap {
        self.0
    }
}
