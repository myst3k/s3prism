mod api;
mod backend;
mod balancer;
mod config;
mod coordination;
mod erasure_coding;
mod health;
mod metadata;
mod metrics;
mod ratelimit;
mod reconciler;
mod retry;
mod stats;
mod web;

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser)]
#[command(name = "s3prism", version, about = "Multi-Region Erasure-Coded S3 Gateway")]
enum Cli {
    /// Start the S3Prism server
    Serve {
        /// Path to bootstrap configuration file
        #[arg(short, long, default_value = "s3prism.toml")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli {
        Cli::Serve {
            config: config_path,
        } => {
            let bootstrap = config::load_bootstrap(&config_path).await?;

            tracing_subscriber::registry()
                .with(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new(&bootstrap.log_level)),
                )
                .with(tracing_subscriber::fmt::layer())
                .init();

            info!("S3Prism starting...");

            let store = metadata::RocksDbBackend::open(&bootstrap.db_path)?;
            let store: metadata::MetadataStore = Arc::new(store);

            let runtime_config = config::SharedRuntimeConfig::load_from_db(store.as_ref())?;

            // Build per-site HTTP clients from runtime config
            let clients: Vec<backend::client::SiteClient> = {
                let cfg = runtime_config.read();
                cfg.sites
                    .iter()
                    .map(|site| backend::client::SiteClient::new(site))
                    .collect::<Result<Vec<_>>>()?
            };

            let max_uploads = runtime_config.read().server.max_concurrent_uploads;
            let purge_notify = Arc::new(tokio::sync::Notify::new());
            let app_state = api::AppState {
                config: runtime_config.clone(),
                store: store.clone(),
                clients: clients.clone(),
                upload_semaphore: Arc::new(tokio::sync::Semaphore::new(max_uploads)),
                purge_notify: purge_notify.clone(),
            };

            // Start purge reaper for async backend deletes
            let reaper = metadata::purge_queue::PurgeReaper::new(
                store.clone(),
                clients,
                purge_notify,
            );
            let reaper_shutdown = reaper.shutdown_handle();

            let mgmt_addr: SocketAddr =
                format!("{}:{}", bootstrap.bind_addr, bootstrap.mgmt_port).parse()?;
            let s3_addr: SocketAddr =
                format!("{}:{}", bootstrap.bind_addr, bootstrap.s3_port).parse()?;

            info!("Management UI: http://{mgmt_addr}");

            if !runtime_config.read().is_configured() {
                info!("No configuration found — open the management UI to complete setup");
                info!("S3 endpoint will return 503 until setup is complete");
            } else {
                info!("S3 endpoint: https://{s3_addr}");
            }

            let (mgmt_server, s3_server, _) = tokio::join!(
                web::serve(mgmt_addr, runtime_config.clone(), store.clone(), bootstrap.mgmt_password.clone()),
                api::serve(s3_addr, app_state),
                reaper.run(),
            );
            mgmt_server?;
            s3_server?;
            drop(reaper_shutdown);
        }
    }

    Ok(())
}
