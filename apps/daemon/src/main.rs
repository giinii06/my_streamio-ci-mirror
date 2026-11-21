use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use std::{env, net::SocketAddr};
use tokio::net::TcpListener;

async fn health() -> Json<Value> {
    Json(json!({"status":"ok","service":"my-streamio","version":env!("CARGO_PKG_VERSION")}))
}

fn listen_address() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let port = env::var("STREAMIO_PORT").unwrap_or_else(|_| "3031".to_string());
    let port: u16 = port.parse()?;
    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = listen_address()?;
    let listener = TcpListener::bind(address).await?;
    eprintln!("my-streamio: listening on http://{address}");
    axum::serve(listener, Router::new().route("/healthz", get(health)))
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
