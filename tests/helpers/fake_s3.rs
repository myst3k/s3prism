use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, head, put},
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct FakeS3State {
    pub objects: Arc<Mutex<HashMap<String, Bytes>>>,
    buckets: Arc<Mutex<std::collections::HashSet<String>>>,
}

pub struct FakeS3 {
    pub addr: SocketAddr,
    pub name: String,
    pub state: FakeS3State,
    shutdown: tokio::sync::oneshot::Sender<()>,
}

impl FakeS3 {
    pub async fn start(name: &str) -> Self {
        let state = FakeS3State {
            objects: Arc::new(Mutex::new(HashMap::new())),
            buckets: Arc::new(Mutex::new(std::collections::HashSet::new())),
        };

        let app = Router::new()
            .route("/{bucket}", put(create_bucket))
            .route("/{bucket}", delete(delete_bucket))
            .route("/{bucket}", head(head_bucket))
            .route("/{bucket}", get(list_objects))
            .route("/{bucket}/{*key}", put(put_object))
            .route("/{bucket}/{*key}", get(get_object))
            .route("/{bucket}/{*key}", head(head_object))
            .route("/{bucket}/{*key}", delete(delete_object))
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async { rx.await.ok(); })
                .await
                .unwrap();
        });

        FakeS3 {
            addr,
            name: name.to_string(),
            state,
            shutdown: tx,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn to_site_config(&self) -> s3prism::config::SiteConfig {
        s3prism::config::SiteConfig {
            name: self.name.clone(),
            region: "us-east-1".to_string(),
            endpoint: self.endpoint(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            priority: 0,
            url_style: s3prism::config::UrlStyle::Path,
        }
    }

    pub fn has_object(&self, bucket: &str, key: &str) -> bool {
        let obj_key = format!("{bucket}/{key}");
        self.state.objects.lock().unwrap().contains_key(&obj_key)
    }

    pub fn object_count(&self) -> usize {
        self.state.objects.lock().unwrap().len()
    }

    pub fn stop(self) {
        let _ = self.shutdown.send(());
    }
}

async fn create_bucket(
    State(state): State<FakeS3State>,
    Path(bucket): Path<String>,
) -> StatusCode {
    state.buckets.lock().unwrap().insert(bucket);
    StatusCode::OK
}

async fn delete_bucket(
    State(state): State<FakeS3State>,
    Path(bucket): Path<String>,
) -> StatusCode {
    state.buckets.lock().unwrap().remove(&bucket);
    StatusCode::NO_CONTENT
}

async fn head_bucket(
    State(state): State<FakeS3State>,
    Path(bucket): Path<String>,
) -> StatusCode {
    if state.buckets.lock().unwrap().contains(&bucket) {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn put_object(
    State(state): State<FakeS3State>,
    Path((bucket, key)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let obj_key = format!("{bucket}/{key}");
    state.objects.lock().unwrap().insert(obj_key, body);
    (
        StatusCode::OK,
        [("etag", "\"fake-etag\"")],
        "",
    )
        .into_response()
}

async fn get_object(
    State(state): State<FakeS3State>,
    Path((bucket, key)): Path<(String, String)>,
) -> Response {
    let obj_key = format!("{bucket}/{key}");
    match state.objects.lock().unwrap().get(&obj_key).cloned() {
        Some(data) => (
            StatusCode::OK,
            [
                ("etag", "\"fake-etag\"".to_string()),
                ("content-type", "application/octet-stream".to_string()),
                ("content-length", data.len().to_string()),
            ],
            data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn head_object(
    State(state): State<FakeS3State>,
    Path((bucket, key)): Path<(String, String)>,
) -> Response {
    let obj_key = format!("{bucket}/{key}");
    match state.objects.lock().unwrap().get(&obj_key) {
        Some(data) => (
            StatusCode::OK,
            [
                ("etag", "\"fake-etag\"".to_string()),
                ("content-length", data.len().to_string()),
            ],
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn delete_object(
    State(state): State<FakeS3State>,
    Path((bucket, key)): Path<(String, String)>,
) -> StatusCode {
    let obj_key = format!("{bucket}/{key}");
    state.objects.lock().unwrap().remove(&obj_key);
    StatusCode::NO_CONTENT
}

async fn list_objects(
    State(state): State<FakeS3State>,
    Path(bucket): Path<String>,
) -> Response {
    let objects = state.objects.lock().unwrap();
    let prefix = format!("{bucket}/");
    let keys: Vec<&str> = objects
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .map(|k| k.strip_prefix(&prefix).unwrap_or(k))
        .collect();

    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push_str("<ListBucketResult>");
    for key in keys {
        xml.push_str(&format!("<Contents><Key>{key}</Key><Size>0</Size></Contents>"));
    }
    xml.push_str("</ListBucketResult>");

    (
        StatusCode::OK,
        [("content-type", "application/xml")],
        xml,
    )
        .into_response()
}
