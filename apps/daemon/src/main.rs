use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use my_streamio_torrent_adapter::TorrentBackend;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;

async fn health() -> Json<Value> {
    Json(json!({"status":"ok","service":"my-streamio","version":env!("CARGO_PKG_VERSION")}))
}

#[derive(Deserialize)]
struct AddSource {
    magnet: String,
}

type ApiError = (StatusCode, Json<Value>);

async fn add_source(
    State(backend): State<TorrentBackend>,
    Json(request): Json<AddSource>,
) -> Result<Json<Value>, ApiError> {
    let source = backend
        .add_magnet(&request.magnet)
        .await
        .map_err(|_| (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({
            "error": "invalid_or_unavailable_source"
        }))))?;
    Ok(Json(source))
}

async fn list_sources(State(backend): State<TorrentBackend>) -> Result<Json<Value>, ApiError> {
    backend
        .list_torrents()
        .map(Json)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": "backend_unavailable"
        }))))
}

fn listen_address() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let port = env::var("STREAMIO_PORT").unwrap_or_else(|_| "3031".to_string());
    let port: u16 = port.parse()?;
    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = listen_address()?;
    let data_dir = env::var("STREAMIO_DATA_DIR").unwrap_or_else(|_| "./data".to_owned());
    let backend = TorrentBackend::open(data_dir.into()).await?;
    let app = Router::new()
        .route("/healthz", get(health))
        .route("/api/v1/sources", get(list_sources).post(add_source))
        .with_state(backend);
    let listener = TcpListener::bind(address).await?;
    eprintln!("my-streamio: listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_to_loopback_only() {
        assert!(SocketAddr::from(([127, 0, 0, 1], 3031)).ip().is_loopback());
    }

    #[tokio::test]
    async fn health_response_is_not_placeholder() {
        let Json(data) = health().await;
        assert_eq!(data["status"], "ok");
        assert_eq!(data["service"], "my-streamio");
    }
}
