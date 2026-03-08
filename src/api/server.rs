use super::state::AppState;
use anyhow::Result;
use std::net::SocketAddr;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;
use tracing::info;

pub async fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    let app = NormalizePathLayer::trim_trailing_slash()
        .layer(super::router::build(state));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("S3 API listening on {addr}");
    axum::serve(
        listener,
        axum::ServiceExt::<axum::http::Request<axum::body::Body>>::into_make_service(app),
    )
    .await?;
    Ok(())
}
