# Developer deployment quick start (not release-certified)

The Docker image is intended for Linux arm64/x86_64. It deliberately uses **host networking** so the daemon remains bound to `127.0.0.1:3031`. That means it is usable only by applications on the same Linux host, unless an independently secured reverse proxy is configured later. Host networking ignores normal Compose port mappings.

## Linux

```bash
docker compose build
docker compose up -d
curl --fail http://127.0.0.1:3031/healthz
docker compose logs -f --tail=100
```

Persistent cache resides in a named Docker volume. Run as nonroot with a read-only root filesystem. This is a prototype until the Docker image is built, smoke-tested and security reviewed on a real host.

## macOS native (recommended for M5 VideoToolbox research)

```bash
cargo run --bin my-streamio-daemon
open http://127.0.0.1:3031
```

The macOS native daemon uses the library's Rust torrent engine. FFmpeg must be installed separately before using the standalone media segment worker. Linux Docker cannot use Apple's VideoToolbox framework.

## Limits

- No automated media switching or full video streaming integrated into the UI yet.
- No public-cloud or WAN endpoint; no authentication for remote users.
- Never map or expose the API to all interfaces without implementing and verifying proper authentication/TLS first.
- CI has not yet validated this image. Runtime verification requires a functioning GitLab runner and a Docker build test.
