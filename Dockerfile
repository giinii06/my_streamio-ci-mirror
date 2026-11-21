# Linux ARM64 runtime. Mac VideoToolbox requires a separate native macOS build.
FROM rust:1.91.1-bookworm AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release --bin my-streamio-daemon

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates ffmpeg curl && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 streamio \
    && useradd --uid 10001 --gid 10001 --system --home /data streamio \
    && mkdir -p /data && chown 10001:10001 /data
COPY --from=builder /workspace/target/release/my-streamio-daemon /usr/local/bin/my-streamio-daemon
USER 10001:10001
WORKDIR /data
ENV STREAMIO_PORT=3031 STREAMIO_DATA_DIR=/data/torrents
EXPOSE 3031/tcp
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=4s --start-period=60s --retries=3 \
  CMD curl --fail --silent http://127.0.0.1:3031/healthz >/dev/null || exit 1
ENTRYPOINT ["/usr/local/bin/my-streamio-daemon"]
