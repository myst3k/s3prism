mod helpers;

use anyhow::Result;
use helpers::fake_s3::FakeS3;
use reqwest::Client;
use s3prism::api::AppState;
use s3prism::backend::client::SiteClient;
use s3prism::config::{
    ErasureConfig, RuntimeConfig, SharedRuntimeConfig, StorageMode,
};
use s3prism::metadata::{MetadataStore, RocksDbBackend};
use s3prism::metadata::purge_queue::PurgeReaper;
use std::sync::Arc;
use tempfile::TempDir;

struct TestHarness {
    s3_addr: String,
    client: Client,
    fake_sites: Vec<FakeS3>,
    store: MetadataStore,
    site_clients: Vec<SiteClient>,
    _tmp: TempDir,
}

impl TestHarness {
    async fn start(site_count: usize, storage_mode: StorageMode) -> Self {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.rocksdb");

        // Start fake S3 backends
        let mut fake_sites = Vec::new();
        let mut site_configs = Vec::new();
        for i in 0..site_count {
            let name = format!("site-{i}");
            let fake = FakeS3::start(&name).await;
            site_configs.push(fake.to_site_config());
            fake_sites.push(fake);
        }

        // Build runtime config
        let runtime_config = RuntimeConfig {
            sites: site_configs.clone(),
            erasure: ErasureConfig {
                data_chunks: 2,
                parity_chunks: 1,
                default_storage_mode: storage_mode,
                hybrid_threshold_bytes: 1024,
                block_size_bytes: 64 * 1024 * 1024,
                read_strategy: Default::default(),
                write_distribution: Default::default(),
            },
            server: Default::default(),
            tls: None,
        };

        let store: MetadataStore =
            Arc::new(RocksDbBackend::open(db_path.to_str().unwrap()).unwrap());
        let shared_config = SharedRuntimeConfig::new(runtime_config);

        let clients: Vec<SiteClient> = site_configs
            .iter()
            .map(|s| SiteClient::new(s).unwrap())
            .collect();

        let app_state = AppState {
            config: shared_config,
            store: store.clone(),
            clients: clients.clone(),
            upload_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(64)),
        };

        // Start S3Prism on a random port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let s3_addr = format!("http://{addr}");

        let app = s3prism::api::router::build(app_state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        TestHarness {
            s3_addr,
            client: Client::new(),
            fake_sites,
            store,
            site_clients: clients,
            _tmp: tmp,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.s3_addr, path)
    }
}

// ── Bucket operations ──

#[tokio::test]
async fn test_create_and_list_buckets() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    // Create a bucket
    let resp = h.client.put(h.url("/test-bucket")).send().await?;
    assert_eq!(resp.status(), 200, "create bucket should return 200");

    // Head bucket should return 200
    let resp = h.client.head(h.url("/test-bucket")).send().await?;
    assert_eq!(resp.status(), 200, "head bucket should return 200");

    // List buckets should include it
    let resp = h.client.get(h.url("/")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("test-bucket"), "list buckets should include test-bucket");

    // Create duplicate should return 409
    let resp = h.client.put(h.url("/test-bucket")).send().await?;
    assert_eq!(resp.status(), 409, "duplicate bucket should return 409");

    Ok(())
}

#[tokio::test]
async fn test_delete_bucket() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/del-bucket")).send().await?;

    let resp = h.client.delete(h.url("/del-bucket")).send().await?;
    assert_eq!(resp.status(), 204, "delete empty bucket should return 204");

    // Head should now return 404
    let resp = h.client.head(h.url("/del-bucket")).send().await?;
    assert_eq!(resp.status(), 404, "deleted bucket should return 404");

    Ok(())
}

#[tokio::test]
async fn test_head_nonexistent_bucket() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    let resp = h.client.head(h.url("/no-such-bucket")).send().await?;
    assert_eq!(resp.status(), 404);

    Ok(())
}

// ── Object operations (Replica mode) ──

#[tokio::test]
async fn test_put_get_object_replica() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/mybucket")).send().await?;

    let data = b"Hello, S3Prism!";
    let resp = h
        .client
        .put(h.url("/mybucket/hello.txt"))
        .header("content-type", "text/plain")
        .body(data.to_vec())
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "PUT object should return 200");
    assert!(resp.headers().get("etag").is_some(), "should return etag");

    // GET it back
    let resp = h.client.get(h.url("/mybucket/hello.txt")).send().await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/plain"
    );
    let body = resp.bytes().await?;
    assert_eq!(&body[..], data, "GET should return the same data");

    Ok(())
}

#[tokio::test]
async fn test_head_object() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/mybucket")).send().await?;
    h.client
        .put(h.url("/mybucket/file.bin"))
        .body(vec![0u8; 1000])
        .send()
        .await?;

    let resp = h.client.head(h.url("/mybucket/file.bin")).send().await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-length").unwrap(),
        "1000"
    );

    Ok(())
}

#[tokio::test]
async fn test_get_nonexistent_object() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/mybucket")).send().await?;

    let resp = h.client.get(h.url("/mybucket/nope.txt")).send().await?;
    assert_eq!(resp.status(), 404);
    let body = resp.text().await?;
    assert!(body.contains("NoSuchKey"));

    Ok(())
}

#[tokio::test]
async fn test_delete_object() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/mybucket")).send().await?;
    h.client
        .put(h.url("/mybucket/to-delete.txt"))
        .body("delete me")
        .send()
        .await?;

    let resp = h
        .client
        .delete(h.url("/mybucket/to-delete.txt"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // Should be gone
    let resp = h.client.get(h.url("/mybucket/to-delete.txt")).send().await?;
    assert_eq!(resp.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_delete_nonempty_bucket() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/mybucket")).send().await?;
    h.client
        .put(h.url("/mybucket/file.txt"))
        .body("data")
        .send()
        .await?;

    let resp = h.client.delete(h.url("/mybucket")).send().await?;
    assert_eq!(resp.status(), 409, "delete non-empty bucket should return 409");

    Ok(())
}

// ── Object operations (Erasure Coded mode) ──

#[tokio::test]
async fn test_put_get_object_erasure() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Erasure).await;

    h.client.put(h.url("/ecbucket")).send().await?;

    let data = vec![42u8; 5000];
    let resp = h
        .client
        .put(h.url("/ecbucket/bigfile.bin"))
        .body(data.clone())
        .send()
        .await?;
    assert_eq!(resp.status(), 200, "PUT EC object should return 200");

    // GET it back and verify data integrity
    let resp = h.client.get(h.url("/ecbucket/bigfile.bin")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await?;
    assert_eq!(body.len(), 5000);
    assert_eq!(&body[..], &data[..], "EC roundtrip should preserve data");

    Ok(())
}

// ── ListObjectsV2 ──

#[tokio::test]
async fn test_list_objects() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/listbucket")).send().await?;
    h.client
        .put(h.url("/listbucket/a.txt"))
        .body("a")
        .send()
        .await?;
    h.client
        .put(h.url("/listbucket/b.txt"))
        .body("b")
        .send()
        .await?;
    h.client
        .put(h.url("/listbucket/sub/c.txt"))
        .body("c")
        .send()
        .await?;

    // List all
    let resp = h.client.get(h.url("/listbucket?list-type=2")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("a.txt"));
    assert!(body.contains("b.txt"));
    assert!(body.contains("sub/c.txt"));
    assert!(body.contains("<KeyCount>3</KeyCount>"));

    // List with prefix
    let resp = h
        .client
        .get(h.url("/listbucket?list-type=2&prefix=sub/"))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("sub/c.txt"));
    assert!(!body.contains("a.txt"));

    Ok(())
}

// ── Batch delete ──

#[tokio::test]
async fn test_batch_delete() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/batchbucket")).send().await?;
    h.client
        .put(h.url("/batchbucket/f1.txt"))
        .body("1")
        .send()
        .await?;
    h.client
        .put(h.url("/batchbucket/f2.txt"))
        .body("2")
        .send()
        .await?;

    let delete_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete>
  <Object><Key>f1.txt</Key></Object>
  <Object><Key>f2.txt</Key></Object>
</Delete>"#;

    let resp = h
        .client
        .post(h.url("/batchbucket?delete"))
        .header("content-type", "application/xml")
        .body(delete_xml)
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("<Deleted>"));
    assert!(body.contains("f1.txt"));
    assert!(body.contains("f2.txt"));

    // Verify both are gone
    let resp = h.client.get(h.url("/batchbucket/f1.txt")).send().await?;
    assert_eq!(resp.status(), 404);
    let resp = h.client.get(h.url("/batchbucket/f2.txt")).send().await?;
    assert_eq!(resp.status(), 404);

    Ok(())
}

// ── Hybrid mode ──

#[tokio::test]
async fn test_hybrid_mode_small_object_replicated() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Hybrid).await;

    h.client.put(h.url("/hybridbucket")).send().await?;

    // Small object (< 1024 byte threshold) should be replicated
    let small_data = b"small object";
    let resp = h
        .client
        .put(h.url("/hybridbucket/small.txt"))
        .body(small_data.to_vec())
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let resp = h.client.get(h.url("/hybridbucket/small.txt")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await?;
    assert_eq!(&body[..], small_data);

    Ok(())
}

#[tokio::test]
async fn test_hybrid_mode_large_object_erasure_coded() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Hybrid).await;

    h.client.put(h.url("/hybridbucket")).send().await?;

    // Large object (> 1024 byte threshold) should be erasure coded
    let large_data = vec![0xABu8; 5000];
    let resp = h
        .client
        .put(h.url("/hybridbucket/large.bin"))
        .body(large_data.clone())
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let resp = h.client.get(h.url("/hybridbucket/large.bin")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await?;
    assert_eq!(&body[..], &large_data[..]);

    Ok(())
}

// ── CopyObject ──

#[tokio::test]
async fn test_copy_object_same_bucket() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/copybucket")).send().await?;

    let data = b"copy me please";
    h.client
        .put(h.url("/copybucket/original.txt"))
        .header("content-type", "text/plain")
        .header("x-amz-meta-tag", "original")
        .body(data.to_vec())
        .send()
        .await?;

    // Copy within same bucket
    let resp = h
        .client
        .put(h.url("/copybucket/copied.txt"))
        .header("x-amz-copy-source", "/copybucket/original.txt")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    let body = resp.text().await?;
    assert!(body.contains("<CopyObjectResult>"));
    assert!(body.contains("<ETag>"));

    // Verify copied object has same data
    let resp = h.client.get(h.url("/copybucket/copied.txt")).send().await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "text/plain");
    assert_eq!(resp.headers().get("x-amz-meta-tag").unwrap(), "original");
    let got = resp.bytes().await?;
    assert_eq!(&got[..], data);

    Ok(())
}

#[tokio::test]
async fn test_copy_object_cross_bucket() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/srcbucket")).send().await?;
    h.client.put(h.url("/dstbucket")).send().await?;

    h.client
        .put(h.url("/srcbucket/file.bin"))
        .body(vec![0xFFu8; 2000])
        .send()
        .await?;

    let resp = h
        .client
        .put(h.url("/dstbucket/file-copy.bin"))
        .header("x-amz-copy-source", "/srcbucket/file.bin")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let resp = h.client.get(h.url("/dstbucket/file-copy.bin")).send().await?;
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await?;
    assert_eq!(body.len(), 2000);
    assert!(body.iter().all(|&b| b == 0xFF));

    Ok(())
}

#[tokio::test]
async fn test_copy_object_replace_metadata() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/copybucket")).send().await?;

    h.client
        .put(h.url("/copybucket/src.txt"))
        .header("content-type", "text/plain")
        .header("x-amz-meta-old", "value")
        .body("data")
        .send()
        .await?;

    // Copy with REPLACE metadata directive
    let resp = h
        .client
        .put(h.url("/copybucket/dst.txt"))
        .header("x-amz-copy-source", "/copybucket/src.txt")
        .header("x-amz-metadata-directive", "REPLACE")
        .header("content-type", "application/json")
        .header("x-amz-meta-new", "replaced")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let resp = h.client.get(h.url("/copybucket/dst.txt")).send().await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "application/json");
    assert_eq!(resp.headers().get("x-amz-meta-new").unwrap(), "replaced");
    assert!(resp.headers().get("x-amz-meta-old").is_none());

    Ok(())
}

#[tokio::test]
async fn test_copy_nonexistent_source() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/copybucket")).send().await?;

    let resp = h
        .client
        .put(h.url("/copybucket/dst.txt"))
        .header("x-amz-copy-source", "/copybucket/doesnt-exist.txt")
        .send()
        .await?;
    assert_eq!(resp.status(), 404);

    Ok(())
}

// ── User metadata ──

#[tokio::test]
async fn test_user_metadata_roundtrip() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/metabucket")).send().await?;

    let resp = h
        .client
        .put(h.url("/metabucket/with-meta.txt"))
        .header("x-amz-meta-custom", "my-value")
        .header("x-amz-meta-another", "second-value")
        .body("metadata test")
        .send()
        .await?;
    assert_eq!(resp.status(), 200);

    let resp = h.client.get(h.url("/metabucket/with-meta.txt")).send().await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("x-amz-meta-custom").unwrap(),
        "my-value"
    );
    assert_eq!(
        resp.headers().get("x-amz-meta-another").unwrap(),
        "second-value"
    );

    Ok(())
}

// ── Purge reaper ──

#[tokio::test]
async fn test_purge_reaper_deletes_from_backends() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Replica).await;

    h.client.put(h.url("/purgebucket")).send().await?;

    // Put an object (gets replicated to all 3 fake sites)
    h.client
        .put(h.url("/purgebucket/to-purge.txt"))
        .body("purge me")
        .send()
        .await?;

    // Verify objects exist on all fake backends
    for site in &h.fake_sites {
        assert!(
            site.has_object("purgebucket", "to-purge.txt"),
            "object should exist on {}",
            site.name
        );
    }

    // Delete via S3Prism (tombstones + enqueues purge)
    let resp = h
        .client
        .delete(h.url("/purgebucket/to-purge.txt"))
        .send()
        .await?;
    assert_eq!(resp.status(), 204);

    // Object should be gone from S3Prism's metadata
    let resp = h.client.get(h.url("/purgebucket/to-purge.txt")).send().await?;
    assert_eq!(resp.status(), 404);

    // But objects should still exist on fake backends (purge hasn't run yet)
    for site in &h.fake_sites {
        assert!(
            site.has_object("purgebucket", "to-purge.txt"),
            "object should still be on {} before purge",
            site.name
        );
    }

    // Run purge reaper with short interval
    let reaper = PurgeReaper::new(h.store.clone(), h.site_clients.clone(), std::sync::Arc::new(tokio::sync::Notify::new()));
    let shutdown = reaper.shutdown_handle();
    tokio::spawn(async move { reaper.run().await });

    // Wait for reaper to tick
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    shutdown.notify_one();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Objects should be purged from all fake backends
    for site in &h.fake_sites {
        assert!(
            !site.has_object("purgebucket", "to-purge.txt"),
            "object should be purged from {}",
            site.name
        );
    }

    // Purge queue should be empty
    let depth = h.store.purge_queue_depth()?;
    assert_eq!(depth, 0, "purge queue should be empty after reaper runs");

    Ok(())
}

#[tokio::test]
async fn test_purge_reaper_erasure_coded() -> Result<()> {
    let h = TestHarness::start(3, StorageMode::Erasure).await;

    h.client.put(h.url("/ecpurge")).send().await?;

    h.client
        .put(h.url("/ecpurge/chunked.bin"))
        .body(vec![0xABu8; 5000])
        .send()
        .await?;

    // Count total objects across all fake backends (should be EC chunks)
    let total_before: usize = h.fake_sites.iter().map(|s| s.object_count()).sum();
    assert!(total_before > 0, "chunks should exist on backends");

    // Delete the object
    h.client.delete(h.url("/ecpurge/chunked.bin")).send().await?;

    // Run purge reaper
    let reaper = PurgeReaper::new(h.store.clone(), h.site_clients.clone(), std::sync::Arc::new(tokio::sync::Notify::new()));
    let shutdown = reaper.shutdown_handle();
    tokio::spawn(async move { reaper.run().await });
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    shutdown.notify_one();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // All EC chunks should be purged
    let total_after: usize = h.fake_sites.iter().map(|s| s.object_count()).sum();
    assert_eq!(total_after, 0, "all EC chunks should be purged from backends");

    let depth = h.store.purge_queue_depth()?;
    assert_eq!(depth, 0);

    Ok(())
}
