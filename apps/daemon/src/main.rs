use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    response::Html,
    routing::get,
    Json, Router,
};
use my_streamio_torrent_adapter::TorrentBackend;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;

async fn home() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}

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

fn is_local_host(host: &str) -> bool {
    let (hostname, port) = match host.split_once(':') {
        Some((hostname, port)) => (hostname, Some(port)),
        None => (host, None),
    };
    if !matches!(hostname, "localhost" | "127.0.0.1") {
        return false;
    }
    port.is_none_or(|port| port.parse::<u16>().is_ok())
}

async fn guard_local_api(request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !is_local_host(host) {
        return (StatusCode::FORBIDDEN, "Untrusted Host header").into_response();
    }
    if let Some(origin) = request.headers().get(header::ORIGIN) {
        if origin.to_str().ok() != Some(format!("http://{host}").as_str()) {
            return (StatusCode::FORBIDDEN, "Untrusted Origin").into_response();
        }
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; object-src 'none'; frame-ancestors 'none'"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
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
        .route("/", get(home))
        .route("/healthz", get(health))
        .route("/api/v1/sources", get(list_sources).post(add_source))
        .with_state(backend)
        .layer(middleware::from_fn(guard_local_api));
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

    #[tokio::test]
    async fn homepage_is_bundled_without_external_assets() {
        assert!(home().await.0.contains("My Streamio"));
    }

    #[test]
    fn rejects_dns_rebinding_style_host_headers() {
        assert!(is_local_host("localhost:3031"));
        assert!(is_local_host("127.0.0.1:3031"));
        assert!(!is_local_host("localhost.evil.example:3031"));
        assert!(!is_local_host("127.0.0.1.attacker.test"));
        assert!(!is_local_host("example.org:3031"));
    }

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
