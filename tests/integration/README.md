# Local API integration smoke

After `cargo build --workspace`, run `sh tests/integration/smoke.sh`. This exercises a local daemon and rejects a malicious Host header; it does **not** prove torrent streaming or M5 acceleration.
