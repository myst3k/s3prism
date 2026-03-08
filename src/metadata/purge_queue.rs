use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

use crate::backend::client::SiteClient;
use super::store::MetadataStore;

pub struct PurgeReaper {
    store: MetadataStore,
    clients: Vec<SiteClient>,
    wake: Arc<Notify>,
    shutdown: Arc<Notify>,
}

impl PurgeReaper {
    pub fn new(
        store: MetadataStore,
        clients: Vec<SiteClient>,
        wake: Arc<Notify>,
    ) -> Self {
        Self {
            store,
            clients,
            wake,
            shutdown: Arc::new(Notify::new()),
        }
    }

    pub fn shutdown_handle(&self) -> Arc<Notify> {
        self.shutdown.clone()
    }

    pub async fn run(&self) {
        info!("Purge reaper started");

        // Process any leftover entries from before restart
        self.drain().await;

        loop {
            tokio::select! {
                _ = self.wake.notified() => {
                    // Brief delay to batch up multiple rapid deletes
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.drain().await;
                }
                _ = self.shutdown.notified() => {
                    info!("Purge reaper shutting down");
                    break;
                }
            }
        }
    }

    async fn drain(&self) {
        loop {
            match self.process_batch().await {
                Ok(processed) => {
                    if processed == 0 {
                        break;
                    }
                }
                Err(e) => {
                    error!("Purge reaper error: {e}");
                    // Back off on errors to avoid tight loop
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    break;
                }
            }
        }
    }

    async fn process_batch(&self) -> anyhow::Result<usize> {
        let entries = self.store.list_purge_entries(1000)?;

        if entries.is_empty() {
            return Ok(0);
        }

        let count = entries.len();
        debug!("Processing {count} purge entries");

        // Group all pending chunks by (site, bucket) for batch delete
        let mut batches: HashMap<(String, String), Vec<(usize, usize, String)>> = HashMap::new();
        let mut entries: Vec<_> = entries.into_iter().map(|mut e| {
            e.attempts += 1;
            e.last_attempt = Some(chrono::Utc::now());
            e
        }).collect();

        for (entry_idx, entry) in entries.iter().enumerate() {
            for (chunk_idx, chunk) in entry.chunks.iter().enumerate() {
                if chunk.deleted {
                    continue;
                }
                batches
                    .entry((chunk.site.clone(), chunk.bucket.clone()))
                    .or_default()
                    .push((entry_idx, chunk_idx, chunk.s3_key.clone()));
            }
        }

        // Fire batch deletes in parallel across all (site, bucket) groups
        let mut handles = Vec::new();
        for ((site, bucket), items) in &batches {
            let client = match self.clients.iter().find(|c| c.site_name == *site) {
                Some(c) => c.clone(),
                None => {
                    warn!("No client for site {site} — skipping batch purge");
                    continue;
                }
            };
            let task_bucket = bucket.clone();
            let keys: Vec<String> = items.iter().map(|(_, _, k)| k.clone()).collect();
            let handle = tokio::spawn(async move {
                let mut deleted = std::collections::HashSet::new();
                for chunk in keys.chunks(1000) {
                    let chunk_vec: Vec<String> = chunk.to_vec();
                    match client.delete_objects(&task_bucket, &chunk_vec).await {
                        Ok(keys) => {
                            deleted.extend(keys);
                        }
                        Err(e) => {
                            warn!("Batch delete from {}/{} failed: {e}", client.site_name, task_bucket);
                        }
                    }
                }
                deleted
            });
            handles.push((site.clone(), bucket.clone(), handle));
        }

        // Collect results and mark chunks as deleted
        for (site, bucket, handle) in handles {
            match handle.await {
                Ok(deleted_keys) => {
                    if let Some(items) = batches.get(&(site.clone(), bucket.clone())) {
                        for (entry_idx, chunk_idx, key) in items {
                            if deleted_keys.contains(key) {
                                entries[*entry_idx].chunks[*chunk_idx].deleted = true;
                            }
                        }
                    }
                    if !deleted_keys.is_empty() {
                        debug!("Purged {} keys from {}/{}", deleted_keys.len(), site, bucket);
                    }
                }
                Err(e) => {
                    error!("Purge task panicked for {site}/{bucket}: {e}");
                }
            }
        }

        // Update or remove entries
        for entry in &entries {
            if entry.all_deleted() {
                self.store.remove_purge_entry(&entry.id)?;
                debug!(
                    "Purge complete: {}/{} ({} chunks)",
                    entry.bucket, entry.key, entry.chunks.len()
                );
            } else {
                self.store.update_purge_entry(entry)?;
            }
        }

        let completed = entries.iter().filter(|e| e.all_deleted()).count();
        if completed > 0 {
            info!("Purged {completed}/{count} entries");
        }

        Ok(count)
    }
}
