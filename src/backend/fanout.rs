use anyhow::Result;
use bytes::Bytes;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::client::{GetObjectOutput, PutObjectOutput, SiteClient};

pub struct FanoutResult<T> {
    pub site: String,
    pub result: Result<T>,
}

pub async fn fanout_put(
    clients: &[SiteClient],
    targets: Vec<PutTarget>,
    quorum: usize,
) -> Result<Vec<FanoutResult<PutObjectOutput>>> {
    let (tx, mut rx) = mpsc::channel(targets.len());

    for target in &targets {
        let client = clients
            .iter()
            .find(|c| c.site_name == target.site)
            .expect("site client not found for target")
            .clone();
        let bucket = target.bucket.clone();
        let key = target.key.clone();
        let data = target.data.clone();
        let content_type = target.content_type.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let result = client
                .put_object(&bucket, &key, data, content_type.as_deref())
                .await;
            let _ = tx
                .send(FanoutResult {
                    site: client.site_name.clone(),
                    result,
                })
                .await;
        });
    }

    drop(tx);

    let mut results = Vec::with_capacity(targets.len());
    let mut successes = 0;

    while let Some(result) = rx.recv().await {
        match &result.result {
            Ok(_) => {
                successes += 1;
                debug!("PUT succeeded at {}", result.site);
            }
            Err(e) => {
                warn!("PUT failed at {}: {e}", result.site);
            }
        }
        results.push(result);

        if successes >= quorum {
            debug!("Quorum reached ({}/{})", successes, quorum);
            break;
        }
    }

    if successes < quorum {
        anyhow::bail!(
            "Failed to reach write quorum: {successes}/{quorum} succeeded"
        );
    }

    // Collect any remaining results that arrived
    while let Ok(result) = rx.try_recv() {
        results.push(result);
    }

    Ok(results)
}

pub async fn fanout_get(
    clients: &[SiteClient],
    targets: Vec<GetTarget>,
    needed: usize,
) -> Result<Vec<FanoutResult<GetObjectOutput>>> {
    let (tx, mut rx) = mpsc::channel(targets.len());

    for target in &targets {
        let client = clients
            .iter()
            .find(|c| c.site_name == target.site)
            .expect("site client not found for target")
            .clone();
        let bucket = target.bucket.clone();
        let key = target.key.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let result = client.get_object(&bucket, &key).await;
            let _ = tx
                .send(FanoutResult {
                    site: client.site_name.clone(),
                    result,
                })
                .await;
        });
    }

    drop(tx);

    let mut results = Vec::new();
    let mut successes = 0;

    while let Some(result) = rx.recv().await {
        match &result.result {
            Ok(output) if output.data.is_some() => {
                successes += 1;
                debug!("GET succeeded from {}", result.site);
            }
            Ok(_) => {
                debug!("GET returned not found from {}", result.site);
            }
            Err(e) => {
                warn!("GET failed from {}: {e}", result.site);
            }
        }
        results.push(result);

        if successes >= needed {
            debug!("Got enough chunks ({}/{})", successes, needed);
            break;
        }
    }

    while let Ok(result) = rx.try_recv() {
        results.push(result);
    }

    if successes < needed {
        anyhow::bail!(
            "Failed to get enough chunks: {successes}/{needed} retrieved"
        );
    }

    Ok(results)
}

pub async fn fanout_delete(
    clients: &[SiteClient],
    targets: Vec<DeleteTarget>,
) -> Vec<FanoutResult<()>> {
    let (tx, mut rx) = mpsc::channel(targets.len());

    for target in &targets {
        let client = clients
            .iter()
            .find(|c| c.site_name == target.site)
            .expect("site client not found for target")
            .clone();
        let bucket = target.bucket.clone();
        let key = target.key.clone();
        let tx = tx.clone();

        tokio::spawn(async move {
            let result = client.delete_object(&bucket, &key).await;
            let _ = tx
                .send(FanoutResult {
                    site: client.site_name.clone(),
                    result,
                })
                .await;
        });
    }

    drop(tx);

    let mut results = Vec::new();
    while let Some(result) = rx.recv().await {
        results.push(result);
    }
    results
}

pub struct PutTarget {
    pub site: String,
    pub bucket: String,
    pub key: String,
    pub data: Bytes,
    pub content_type: Option<String>,
}

pub struct GetTarget {
    pub site: String,
    pub bucket: String,
    pub key: String,
}

pub struct DeleteTarget {
    pub site: String,
    pub bucket: String,
    pub key: String,
}
