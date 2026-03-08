use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use tracing::info;

use super::auth::auth_middleware;
use super::state::MgmtState;
use crate::backend::client::SiteClient;
use crate::config::{
    ReadStrategy, RuntimeConfig, SiteConfig, StorageMode, TlsConfig, UrlStyle, WriteDistribution,
};

pub fn routes(state: MgmtState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/check", post(auth_check))
        .route("/api/status", get(get_status))
        // Site management
        .route("/api/sites", get(list_sites))
        .route("/api/sites", post(add_site))
        .route("/api/sites/{name}", put(update_site))
        .route("/api/sites/{name}", delete(remove_site))
        .route("/api/sites/{name}/test", post(test_site))
        // Erasure config
        .route("/api/erasure", get(get_erasure_config))
        .route("/api/erasure", put(update_erasure_config))
        // TLS config
        .route("/api/tls", get(get_tls_config))
        .route("/api/tls", put(update_tls_config))
        .route("/api/tls", delete(remove_tls_config))
        // Full config (export/import)
        .route("/api/config", get(get_full_config))
        .route("/api/config", put(save_full_config))
        // Bucket management
        .route("/api/buckets", get(list_buckets))
        // Purge queue stats
        .route("/api/purge", get(get_purge_stats))
        // Wasabi region discovery
        .route("/api/wasabi/regions", get(discover_wasabi_regions))
        .route("/api/wasabi/ping/{region}", post(ping_wasabi_region))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}

// ── Health / Status ──

async fn health() -> &'static str {
    "ok"
}

async fn auth_check(State(state): State<MgmtState>) -> Response {
    if state.mgmt_password.is_none() {
        return (StatusCode::OK, Json(serde_json::json!({"auth_required": false}))).into_response();
    }
    (StatusCode::OK, Json(serde_json::json!({"auth_required": true}))).into_response()
}

#[derive(Serialize)]
struct StatusResponse {
    configured: bool,
    site_count: usize,
    storage_mode: String,
    data_chunks: usize,
    parity_chunks: usize,
    bucket_count: usize,
    purge_queue_depth: usize,
}

async fn get_status(State(state): State<MgmtState>) -> Result<Json<StatusResponse>, MgmtError> {
    let cfg = state.config.read();
    let bucket_count = state.store.list_buckets()?.len();
    let purge_depth = state.store.purge_queue_depth()?;

    Ok(Json(StatusResponse {
        configured: cfg.is_configured(),
        site_count: cfg.sites.len(),
        storage_mode: format!("{:?}", cfg.erasure.default_storage_mode),
        data_chunks: cfg.erasure.data_chunks,
        parity_chunks: cfg.erasure.parity_chunks,
        bucket_count,
        purge_queue_depth: purge_depth,
    }))
}

// ── Site Management ──

#[derive(Serialize)]
struct SiteResponse {
    name: String,
    region: String,
    endpoint: String,
    priority: u8,
    url_style: String,
}

impl From<&SiteConfig> for SiteResponse {
    fn from(s: &SiteConfig) -> Self {
        Self {
            name: s.name.clone(),
            region: s.region.clone(),
            endpoint: s.endpoint.clone(),
            priority: s.priority,
            url_style: format!("{:?}", s.url_style),
        }
    }
}

async fn list_sites(State(state): State<MgmtState>) -> Json<Vec<SiteResponse>> {
    let cfg = state.config.read();
    Json(cfg.sites.iter().map(SiteResponse::from).collect())
}

#[derive(Deserialize)]
struct AddSiteRequest {
    name: String,
    region: String,
    endpoint: String,
    access_key: String,
    secret_key: String,
    #[serde(default)]
    priority: u8,
    #[serde(default)]
    url_style: Option<String>,
}

async fn add_site(
    State(state): State<MgmtState>,
    Json(req): Json<AddSiteRequest>,
) -> Result<Response, MgmtError> {
    let mut cfg = state.config.read().as_ref().clone();

    if cfg.sites.iter().any(|s| s.name == req.name) {
        return Err(MgmtError::Conflict(format!(
            "site '{}' already exists",
            req.name
        )));
    }

    let url_style = match req.url_style.as_deref() {
        Some("virtualhost") | Some("virtual_host") => UrlStyle::VirtualHost,
        _ => UrlStyle::Path,
    };

    cfg.sites.push(SiteConfig {
        name: req.name.clone(),
        region: req.region,
        endpoint: req.endpoint,
        access_key: req.access_key,
        secret_key: req.secret_key,
        priority: req.priority,
        url_style,
    });

    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Added site: {}", req.name);

    Ok((StatusCode::CREATED, Json(serde_json::json!({"ok": true}))).into_response())
}

async fn update_site(
    State(state): State<MgmtState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<AddSiteRequest>,
) -> Result<Response, MgmtError> {
    let mut cfg = state.config.read().as_ref().clone();

    let site = cfg
        .sites
        .iter_mut()
        .find(|s| s.name == name)
        .ok_or_else(|| MgmtError::NotFound(format!("site '{name}' not found")))?;

    let url_style = match req.url_style.as_deref() {
        Some("virtualhost") | Some("virtual_host") => UrlStyle::VirtualHost,
        _ => UrlStyle::Path,
    };

    site.name = req.name;
    site.region = req.region;
    site.endpoint = req.endpoint;
    site.access_key = req.access_key;
    site.secret_key = req.secret_key;
    site.priority = req.priority;
    site.url_style = url_style;

    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Updated site: {name}");

    Ok(StatusCode::OK.into_response())
}

async fn remove_site(
    State(state): State<MgmtState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Response, MgmtError> {
    let mut cfg = state.config.read().as_ref().clone();
    let before = cfg.sites.len();
    cfg.sites.retain(|s| s.name != name);

    if cfg.sites.len() == before {
        return Err(MgmtError::NotFound(format!("site '{name}' not found")));
    }

    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Removed site: {name}");

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Serialize)]
struct TestSiteResponse {
    reachable: bool,
    latency_ms: u64,
    error: Option<String>,
}

async fn test_site(
    State(state): State<MgmtState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<TestSiteResponse>, MgmtError> {
    let cfg = state.config.read();
    let site = cfg
        .sites
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| MgmtError::NotFound(format!("site '{name}' not found")))?;

    let client = SiteClient::new(site).map_err(|e| MgmtError::Internal(e.to_string()))?;
    let start = std::time::Instant::now();

    // Test by listing buckets (HEAD on the endpoint)
    match client.list_objects("__test__", "").await {
        Ok(_) => Ok(Json(TestSiteResponse {
            reachable: true,
            latency_ms: start.elapsed().as_millis() as u64,
            error: None,
        })),
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let err_str = e.to_string();
            // Connection succeeded if we got an HTTP error (like 404/403)
            let reachable = !err_str.contains("connect") && !err_str.contains("dns");
            Ok(Json(TestSiteResponse {
                reachable,
                latency_ms: elapsed,
                error: Some(err_str),
            }))
        }
    }
}

// ── Erasure Config ──

#[derive(Serialize, Deserialize)]
struct ErasureConfigRequest {
    data_chunks: Option<usize>,
    parity_chunks: Option<usize>,
    default_storage_mode: Option<String>,
    hybrid_threshold_bytes: Option<u64>,
    block_size_bytes: Option<usize>,
    read_strategy: Option<String>,
    write_distribution: Option<String>,
}

async fn get_erasure_config(State(state): State<MgmtState>) -> Json<serde_json::Value> {
    let cfg = state.config.read();
    Json(serde_json::to_value(&cfg.erasure).unwrap())
}

async fn update_erasure_config(
    State(state): State<MgmtState>,
    Json(req): Json<ErasureConfigRequest>,
) -> Result<Response, MgmtError> {
    let mut cfg = state.config.read().as_ref().clone();

    if let Some(dc) = req.data_chunks {
        cfg.erasure.data_chunks = dc;
    }
    if let Some(pc) = req.parity_chunks {
        cfg.erasure.parity_chunks = pc;
    }
    if let Some(mode) = &req.default_storage_mode {
        cfg.erasure.default_storage_mode = match mode.to_lowercase().as_str() {
            "replica" => StorageMode::Replica,
            "erasure" => StorageMode::Erasure,
            "hybrid" => StorageMode::Hybrid,
            _ => return Err(MgmtError::BadRequest("invalid storage mode".into())),
        };
    }
    if let Some(ht) = req.hybrid_threshold_bytes {
        cfg.erasure.hybrid_threshold_bytes = ht;
    }
    if let Some(bs) = req.block_size_bytes {
        cfg.erasure.block_size_bytes = bs;
    }
    if let Some(rs) = &req.read_strategy {
        cfg.erasure.read_strategy = match rs.to_lowercase().as_str() {
            "fan_out_all" | "fanoutall" => ReadStrategy::FanOutAll,
            "fetch_minimum" | "fetchminimum" => ReadStrategy::FetchMinimum,
            _ => return Err(MgmtError::BadRequest("invalid read strategy".into())),
        };
    }
    if let Some(wd) = &req.write_distribution {
        cfg.erasure.write_distribution = match wd.to_lowercase().as_str() {
            "shuffle" => WriteDistribution::Shuffle,
            "priority" => WriteDistribution::Priority,
            _ => return Err(MgmtError::BadRequest("invalid write distribution".into())),
        };
    }

    // Validate
    if cfg.erasure.data_chunks == 0 || cfg.erasure.parity_chunks == 0 {
        return Err(MgmtError::BadRequest(
            "data_chunks and parity_chunks must be > 0".into(),
        ));
    }
    let total = cfg.erasure.data_chunks + cfg.erasure.parity_chunks;
    if total > cfg.sites.len() && !cfg.sites.is_empty() {
        return Err(MgmtError::BadRequest(format!(
            "data_chunks + parity_chunks ({total}) exceeds site count ({})",
            cfg.sites.len()
        )));
    }

    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Updated erasure config");

    Ok(StatusCode::OK.into_response())
}

// ── TLS Config ──

async fn get_tls_config(State(state): State<MgmtState>) -> Json<serde_json::Value> {
    let cfg = state.config.read();
    Json(serde_json::to_value(&cfg.tls).unwrap())
}

async fn update_tls_config(
    State(state): State<MgmtState>,
    Json(tls): Json<TlsConfig>,
) -> Result<Response, MgmtError> {
    let mut cfg = state.config.read().as_ref().clone();
    cfg.tls = Some(tls);
    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Updated TLS config");
    Ok(StatusCode::OK.into_response())
}

async fn remove_tls_config(
    State(state): State<MgmtState>,
) -> Result<Response, MgmtError> {
    let mut cfg = state.config.read().as_ref().clone();
    cfg.tls = None;
    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Removed TLS config");
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ── Full Config ──

async fn get_full_config(State(state): State<MgmtState>) -> Json<serde_json::Value> {
    let cfg = state.config.read();
    Json(serde_json::to_value(cfg.as_ref()).unwrap())
}

async fn save_full_config(
    State(state): State<MgmtState>,
    Json(cfg): Json<RuntimeConfig>,
) -> Result<Response, MgmtError> {
    state.config.update(cfg);
    state.config.save_to_db(state.store.as_ref())?;
    info!("Saved full runtime config");
    Ok(StatusCode::OK.into_response())
}

// ── Buckets ──

async fn list_buckets(State(state): State<MgmtState>) -> Result<Json<serde_json::Value>, MgmtError> {
    let buckets = state.store.list_buckets()?;
    let list: Vec<serde_json::Value> = buckets
        .iter()
        .map(|b| {
            let stats = state.store.bucket_stats(&b.name).unwrap_or(
                crate::metadata::store::BucketStats { object_count: 0, total_size: 0 }
            );
            serde_json::json!({
                "name": b.name,
                "created": b.created.to_rfc3339(),
                "storage_mode": format!("{:?}", b.storage_mode),
                "sites": b.backend_buckets.len(),
                "object_count": stats.object_count,
                "total_size": stats.total_size,
            })
        })
        .collect();
    Ok(Json(serde_json::json!(list)))
}

// ── Purge Queue ──

async fn get_purge_stats(State(state): State<MgmtState>) -> Result<Json<serde_json::Value>, MgmtError> {
    let depth = state.store.purge_queue_depth()?;
    let entries = state.store.list_purge_entries(10)?;
    let recent: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "bucket": e.bucket,
                "key": e.key,
                "chunks": e.chunks.len(),
                "deleted": e.chunks.iter().filter(|c| c.deleted).count(),
                "attempts": e.attempts,
                "queued_at": e.queued_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "depth": depth,
        "recent": recent,
    })))
}

// ── Wasabi Region Discovery ──

#[derive(Serialize)]
struct WasabiRegion {
    region: String,
    name: String,
    endpoint: String,
    status: String,
    is_default: bool,
    geo: String,
}

fn region_to_geo(region: &str) -> &'static str {
    if region.starts_with("us-") || region.starts_with("ca-") {
        "Americas"
    } else if region.starts_with("eu-") {
        "Europe"
    } else if region.starts_with("ap-") {
        "Asia Pacific"
    } else {
        "Other"
    }
}

async fn discover_wasabi_regions() -> Result<Json<Vec<WasabiRegion>>, MgmtError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| MgmtError::Internal(e.to_string()))?;

    let resp = client
        .get("https://s3.wasabisys.com/?describeRegions")
        .send()
        .await
        .map_err(|e| MgmtError::Internal(format!("failed to fetch regions: {e}")))?;

    let body = resp
        .text()
        .await
        .map_err(|e| MgmtError::Internal(e.to_string()))?;

    let mut regions = Vec::new();
    // Simple XML parsing — each <item> has Region, RegionName, Endpoint, Status, IsDefault
    for item in body.split("<item>").skip(1) {
        let extract = |tag: &str| -> String {
            item.split(&format!("<{tag}>"))
                .nth(1)
                .and_then(|s| s.split(&format!("</{tag}>")).next())
                .unwrap_or("")
                .to_string()
        };
        let region = extract("Region");
        let geo = region_to_geo(&region).to_string();
        regions.push(WasabiRegion {
            region,
            name: extract("RegionName"),
            endpoint: extract("Endpoint"),
            status: extract("Status"),
            is_default: extract("IsDefault") == "true",
            geo,
        });
    }

    Ok(Json(regions))
}

#[derive(Serialize)]
struct PingResult {
    region: String,
    endpoint: String,
    pings: Vec<u64>,
    min_ms: u64,
    avg_ms: u64,
    max_ms: u64,
    reachable: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct PingQuery {
    #[serde(default = "default_ping_count")]
    count: u8,
}

fn default_ping_count() -> u8 { 3 }

async fn ping_wasabi_region(
    axum::extract::Path(region): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<PingQuery>,
) -> Result<Json<PingResult>, MgmtError> {
    let count = query.count.clamp(1, 20);
    let endpoint = if region == "us-east-1" {
        "s3.wasabisys.com".to_string()
    } else {
        format!("s3.{region}.wasabisys.com")
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| MgmtError::Internal(e.to_string()))?;

    let url = format!("https://{endpoint}/**ping**");
    let mut pings = Vec::new();
    let mut last_error = None;

    for _ in 0..count {
        let start = std::time::Instant::now();
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                pings.push(start.elapsed().as_millis() as u64);
            }
            Ok(resp) => {
                pings.push(start.elapsed().as_millis() as u64);
                last_error = Some(format!("HTTP {}", resp.status()));
            }
            Err(e) => {
                last_error = Some(e.to_string());
            }
        }
    }

    if pings.is_empty() {
        return Ok(Json(PingResult {
            region,
            endpoint,
            pings: vec![],
            min_ms: 0,
            avg_ms: 0,
            max_ms: 0,
            reachable: false,
            error: last_error,
        }));
    }

    let min_ms = *pings.iter().min().unwrap();
    let max_ms = *pings.iter().max().unwrap();
    let avg_ms = pings.iter().sum::<u64>() / pings.len() as u64;

    Ok(Json(PingResult {
        region,
        endpoint,
        pings,
        min_ms,
        avg_ms,
        max_ms,
        reachable: true,
        error: last_error,
    }))
}

// ── Error type ──

#[derive(Debug)]
enum MgmtError {
    NotFound(String),
    Conflict(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for MgmtError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            MgmtError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            MgmtError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            MgmtError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            MgmtError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}

impl From<anyhow::Error> for MgmtError {
    fn from(e: anyhow::Error) -> Self {
        MgmtError::Internal(e.to_string())
    }
}
