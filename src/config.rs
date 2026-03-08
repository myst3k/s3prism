use anyhow::Result;
use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

// ── Bootstrap config (TOML/env/CLI — needed before DB opens) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapConfig {
    #[serde(default = "default_s3_port")]
    pub s3_port: u16,
    #[serde(default = "default_mgmt_port")]
    pub mgmt_port: u16,
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub mgmt_password: Option<String>,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            s3_port: default_s3_port(),
            mgmt_port: default_mgmt_port(),
            bind_addr: default_bind_addr(),
            db_path: default_db_path(),
            log_level: default_log_level(),
            mgmt_password: None,
        }
    }
}

fn default_s3_port() -> u16 { 8443 }
fn default_mgmt_port() -> u16 { 9090 }
fn default_bind_addr() -> String { "0.0.0.0".to_string() }
fn default_db_path() -> String { "data/s3prism.rocksdb".to_string() }
fn default_log_level() -> String { "info".to_string() }

pub async fn load_bootstrap(path: &str) -> Result<BootstrapConfig> {
    let path = Path::new(path);
    if path.exists() {
        let contents = tokio::fs::read_to_string(path).await?;
        let config: BootstrapConfig = toml::from_str(&contents)?;
        info!("Loaded bootstrap config from {}", path.display());
        Ok(config)
    } else {
        info!("No bootstrap config at {}, using defaults", path.display());
        Ok(BootstrapConfig::default())
    }
}

// ── Runtime config (stored in RocksDB, managed via web UI) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub sites: Vec<SiteConfig>,
    #[serde(default)]
    pub erasure: ErasureConfig,
    #[serde(default)]
    pub server: ServerRuntimeConfig,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub region: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub url_style: UrlStyle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UrlStyle {
    #[default]
    Path,
    VirtualHost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureConfig {
    #[serde(default = "default_data_chunks")]
    pub data_chunks: usize,
    #[serde(default = "default_parity_chunks")]
    pub parity_chunks: usize,
    #[serde(default = "default_storage_mode")]
    pub default_storage_mode: StorageMode,
    #[serde(default = "default_hybrid_threshold")]
    pub hybrid_threshold_bytes: u64,
    #[serde(default = "default_block_size")]
    pub block_size_bytes: usize,
    #[serde(default)]
    pub read_strategy: ReadStrategy,
    #[serde(default)]
    pub write_distribution: WriteDistribution,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Replica,
    Erasure,
    Hybrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReadStrategy {
    #[default]
    FanOutAll,
    FetchMinimum,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WriteDistribution {
    #[default]
    Shuffle,
    Priority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRuntimeConfig {
    #[serde(default = "default_snapshot_interval_secs")]
    pub snapshot_interval_secs: u64,
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_max_concurrent_uploads")]
    pub max_concurrent_uploads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

impl RuntimeConfig {
    pub fn is_configured(&self) -> bool {
        !self.sites.is_empty()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            sites: Vec::new(),
            erasure: ErasureConfig::default(),
            server: ServerRuntimeConfig::default(),
            tls: None,
        }
    }
}

impl Default for ErasureConfig {
    fn default() -> Self {
        Self {
            data_chunks: default_data_chunks(),
            parity_chunks: default_parity_chunks(),
            default_storage_mode: default_storage_mode(),
            hybrid_threshold_bytes: default_hybrid_threshold(),
            block_size_bytes: default_block_size(),
            read_strategy: ReadStrategy::default(),
            write_distribution: WriteDistribution::default(),
        }
    }
}

impl Default for ServerRuntimeConfig {
    fn default() -> Self {
        Self {
            snapshot_interval_secs: default_snapshot_interval_secs(),
            health_check_interval_secs: default_health_check_interval_secs(),
            max_concurrent_uploads: default_max_concurrent_uploads(),
        }
    }
}

fn default_data_chunks() -> usize { 2 }
fn default_parity_chunks() -> usize { 1 }
fn default_storage_mode() -> StorageMode { StorageMode::Hybrid }
fn default_hybrid_threshold() -> u64 { 1_048_576 } // 1MB
fn default_block_size() -> usize { 67_108_864 } // 64MB
fn default_snapshot_interval_secs() -> u64 { 300 }
fn default_health_check_interval_secs() -> u64 { 30 }
fn default_max_concurrent_uploads() -> usize { 64 }

// ── Shared runtime config (arc-swap for lock-free hot-reload) ──

#[derive(Clone)]
pub struct SharedRuntimeConfig {
    inner: Arc<ArcSwap<RuntimeConfig>>,
}

impl SharedRuntimeConfig {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            inner: Arc::new(ArcSwap::from_pointee(config)),
        }
    }

    pub fn read(&self) -> arc_swap::Guard<Arc<RuntimeConfig>> {
        self.inner.load()
    }

    pub fn update(&self, config: RuntimeConfig) {
        self.inner.store(Arc::new(config));
    }

    pub fn load_from_db(store: &dyn crate::metadata::MetadataBackend) -> Result<Self> {
        match store.get_runtime_config()? {
            Some(config) => {
                info!("Loaded runtime config from database");
                Ok(Self::new(config))
            }
            None => {
                info!("No runtime config in database, starting unconfigured");
                Ok(Self::new(RuntimeConfig::default()))
            }
        }
    }

    pub fn save_to_db(&self, store: &dyn crate::metadata::MetadataBackend) -> Result<()> {
        let config = self.read();
        store.put_runtime_config(&config)?;
        info!("Saved runtime config to database");
        Ok(())
    }
}
