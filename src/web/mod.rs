mod api;
mod auth;
pub mod state;
mod websocket;

use self::state::MgmtState;
use crate::config::SharedRuntimeConfig;
use crate::metadata::MetadataStore;
use anyhow::Result;
use axum::Router;
use axum::response::{Html, IntoResponse};
use std::net::SocketAddr;
use tracing::info;

pub fn build_router(config: SharedRuntimeConfig, store: MetadataStore) -> Router {
    build_router_with_password(config, store, None)
}

pub fn build_router_with_password(
    config: SharedRuntimeConfig,
    store: MetadataStore,
    mgmt_password: Option<String>,
) -> Router {
    let state = MgmtState {
        config,
        store,
        mgmt_password,
    };
    Router::new()
        .merge(api::routes(state))
        .fallback(serve_ui)
}

pub async fn serve(
    addr: SocketAddr,
    config: SharedRuntimeConfig,
    store: MetadataStore,
    mgmt_password: Option<String>,
) -> Result<()> {
    let app = build_router_with_password(config, store, mgmt_password);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Management UI listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn serve_ui() -> impl IntoResponse {
    Html(include_str!("assets/index.html"))
}
