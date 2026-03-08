mod helpers;

use anyhow::Result;
use reqwest::Client;
use s3prism::config::{ErasureConfig, RuntimeConfig, SharedRuntimeConfig, StorageMode};
use s3prism::metadata::{MetadataStore, RocksDbBackend};
use std::sync::Arc;
use tempfile::TempDir;

struct MgmtHarness {
    base_url: String,
    client: Client,
    _store: MetadataStore,
    _tmp: TempDir,
}

impl MgmtHarness {
    async fn start() -> Self {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.rocksdb");

        let store: MetadataStore =
            Arc::new(RocksDbBackend::open(db_path.to_str().unwrap()).unwrap());
        let config = SharedRuntimeConfig::new(RuntimeConfig {
            sites: vec![],
            erasure: ErasureConfig {
                data_chunks: 2,
                parity_chunks: 1,
                default_storage_mode: StorageMode::Replica,
                hybrid_threshold_bytes: 1024,
                block_size_bytes: 64 * 1024 * 1024,
                read_strategy: Default::default(),
                write_distribution: Default::default(),
            },
            server: Default::default(),
            tls: None,
        });

        let app = s3prism::web::build_router(config, store.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        MgmtHarness {
            base_url,
            client: Client::new(),
            _store: store,
            _tmp: tmp,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

#[tokio::test]
async fn test_health() -> Result<()> {
    let h = MgmtHarness::start().await;
    let resp = h.client.get(h.url("/api/health")).send().await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await?, "ok");
    Ok(())
}

#[tokio::test]
async fn test_status() -> Result<()> {
    let h = MgmtHarness::start().await;
    let resp = h.client.get(h.url("/api/status")).send().await?;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await?;
    assert_eq!(body["site_count"], 0);
    assert_eq!(body["configured"], false);
    assert_eq!(body["data_chunks"], 2);
    assert_eq!(body["parity_chunks"], 1);
    Ok(())
}

#[tokio::test]
async fn test_add_and_list_sites() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Add a site
    let resp = h
        .client
        .post(h.url("/api/sites"))
        .json(&serde_json::json!({
            "name": "us-east-1",
            "region": "us-east-1",
            "endpoint": "https://s3.us-east-1.wasabisys.com",
            "access_key": "AKTEST",
            "secret_key": "secret123",
            "url_style": "path",
            "priority": 1
        }))
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    // List sites
    let resp = h.client.get(h.url("/api/sites")).send().await?;
    assert_eq!(resp.status(), 200);
    let sites: Vec<serde_json::Value> = resp.json().await?;
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0]["name"], "us-east-1");
    assert_eq!(sites[0]["region"], "us-east-1");
    assert_eq!(sites[0]["priority"], 1);

    Ok(())
}

#[tokio::test]
async fn test_add_duplicate_site() -> Result<()> {
    let h = MgmtHarness::start().await;

    let site = serde_json::json!({
        "name": "site-a",
        "region": "us-east-1",
        "endpoint": "https://example.com",
        "access_key": "AK",
        "secret_key": "SK"
    });

    let resp = h.client.post(h.url("/api/sites")).json(&site).send().await?;
    assert_eq!(resp.status(), 201);

    let resp = h.client.post(h.url("/api/sites")).json(&site).send().await?;
    assert_eq!(resp.status(), 409);

    Ok(())
}

#[tokio::test]
async fn test_remove_site() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Add then remove
    h.client
        .post(h.url("/api/sites"))
        .json(&serde_json::json!({
            "name": "removeme",
            "region": "us-west-1",
            "endpoint": "https://example.com",
            "access_key": "AK",
            "secret_key": "SK"
        }))
        .send()
        .await?;

    let resp = h
        .client
        .delete(h.url("/api/sites/removeme"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // Verify it's gone
    let sites: Vec<serde_json::Value> = h
        .client
        .get(h.url("/api/sites"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(sites.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_remove_nonexistent_site() -> Result<()> {
    let h = MgmtHarness::start().await;
    let resp = h
        .client
        .delete(h.url("/api/sites/nope"))
        .send()
        .await?;
    assert_eq!(resp.status(), 404);
    Ok(())
}

#[tokio::test]
async fn test_update_site() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Add site
    h.client
        .post(h.url("/api/sites"))
        .json(&serde_json::json!({
            "name": "updatable",
            "region": "us-east-1",
            "endpoint": "https://old.example.com",
            "access_key": "AK",
            "secret_key": "SK"
        }))
        .send()
        .await?;

    // Update it
    let resp = h
        .client
        .put(h.url("/api/sites/updatable"))
        .json(&serde_json::json!({
            "name": "updatable",
            "region": "eu-west-1",
            "endpoint": "https://new.example.com",
            "access_key": "AK2",
            "secret_key": "SK2",
            "url_style": "virtualhost",
            "priority": 5
        }))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    // Verify update
    let sites: Vec<serde_json::Value> = h
        .client
        .get(h.url("/api/sites"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(sites[0]["region"], "eu-west-1");
    assert_eq!(sites[0]["priority"], 5);

    Ok(())
}

#[tokio::test]
async fn test_erasure_config_get_and_update() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Get defaults
    let resp = h.client.get(h.url("/api/erasure")).send().await?;
    assert_eq!(resp.status(), 200);
    let ec: serde_json::Value = resp.json().await?;
    assert_eq!(ec["data_chunks"], 2);
    assert_eq!(ec["parity_chunks"], 1);

    // Update
    let resp = h
        .client
        .put(h.url("/api/erasure"))
        .json(&serde_json::json!({
            "data_chunks": 3,
            "parity_chunks": 2,
            "default_storage_mode": "erasure",
            "read_strategy": "fetch_minimum"
        }))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    // Verify
    let ec: serde_json::Value = h
        .client
        .get(h.url("/api/erasure"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(ec["data_chunks"], 3);
    assert_eq!(ec["parity_chunks"], 2);

    Ok(())
}

#[tokio::test]
async fn test_erasure_config_validation() -> Result<()> {
    let h = MgmtHarness::start().await;

    // data_chunks = 0 should fail
    let resp = h
        .client
        .put(h.url("/api/erasure"))
        .json(&serde_json::json!({
            "data_chunks": 0
        }))
        .send()
        .await?;
    assert_eq!(resp.status(), 400);

    Ok(())
}

#[tokio::test]
async fn test_tls_config_crud() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Initially null
    let resp = h.client.get(h.url("/api/tls")).send().await?;
    assert_eq!(resp.status(), 200);
    let tls: serde_json::Value = resp.json().await?;
    assert!(tls.is_null());

    // Set TLS config
    let resp = h
        .client
        .put(h.url("/api/tls"))
        .json(&serde_json::json!({
            "cert_path": "/etc/certs/cert.pem",
            "key_path": "/etc/certs/key.pem"
        }))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    // Verify
    let tls: serde_json::Value = h
        .client
        .get(h.url("/api/tls"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(tls["cert_path"], "/etc/certs/cert.pem");

    // Remove
    let resp = h.client.delete(h.url("/api/tls")).send().await?;
    assert_eq!(resp.status(), 204);

    // Verify removed
    let tls: serde_json::Value = h
        .client
        .get(h.url("/api/tls"))
        .send()
        .await?
        .json()
        .await?;
    assert!(tls.is_null());

    Ok(())
}

#[tokio::test]
async fn test_full_config_export_import() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Add a site first
    h.client
        .post(h.url("/api/sites"))
        .json(&serde_json::json!({
            "name": "test-site",
            "region": "us-east-1",
            "endpoint": "https://example.com",
            "access_key": "AK",
            "secret_key": "SK"
        }))
        .send()
        .await?;

    // Export
    let cfg: serde_json::Value = h
        .client
        .get(h.url("/api/config"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(cfg["sites"].as_array().unwrap().len(), 1);

    // Import (overwrite with empty sites)
    let mut modified = cfg.clone();
    modified["sites"] = serde_json::json!([]);
    let resp = h
        .client
        .put(h.url("/api/config"))
        .json(&modified)
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    // Verify
    let sites: Vec<serde_json::Value> = h
        .client
        .get(h.url("/api/sites"))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(sites.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_buckets_list_empty() -> Result<()> {
    let h = MgmtHarness::start().await;
    let resp = h.client.get(h.url("/api/buckets")).send().await?;
    assert_eq!(resp.status(), 200);
    let buckets: Vec<serde_json::Value> = resp.json().await?;
    assert_eq!(buckets.len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_purge_stats() -> Result<()> {
    let h = MgmtHarness::start().await;
    let resp = h.client.get(h.url("/api/purge")).send().await?;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await?;
    assert_eq!(body["depth"], 0);
    assert_eq!(body["recent"].as_array().unwrap().len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_ui_fallback() -> Result<()> {
    let h = MgmtHarness::start().await;
    let resp = h.client.get(h.url("/")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("S3Prism"));
    assert!(body.contains("<!DOCTYPE html>"));
    Ok(())
}

#[tokio::test]
async fn test_config_persists_via_db() -> Result<()> {
    let h = MgmtHarness::start().await;

    // Add a site via API
    let resp = h
        .client
        .post(h.url("/api/sites"))
        .json(&serde_json::json!({
            "name": "persisted-site",
            "region": "us-east-1",
            "endpoint": "https://example.com",
            "access_key": "AK",
            "secret_key": "SK"
        }))
        .send()
        .await?;
    assert_eq!(resp.status(), 201);

    // Load config from the same DB (simulating restart)
    let reloaded = SharedRuntimeConfig::load_from_db(h._store.as_ref())?;
    let cfg = reloaded.read();
    assert_eq!(cfg.sites.len(), 1);
    assert_eq!(cfg.sites[0].name, "persisted-site");
    assert_eq!(cfg.sites[0].region, "us-east-1");

    Ok(())
}
