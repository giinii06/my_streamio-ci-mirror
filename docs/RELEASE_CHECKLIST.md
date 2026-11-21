# Implementation and release verification

## Code/build evidence

- Every changed source commit is checked at its exact SHA.
- `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace` run on GitLab ARM64 Runner.
- `tests/integration/smoke.sh` runs on the built daemon and validates health, UI, sources and hostile host rejection.
- Unavailable runner/registry failures are not reported as source compilation failures.

## Functional evidence still required

- Build real v1/v2-compatible torrent fixtures from generated video. Confirm add, peer discovery, verified reads, seek and EOF.
- Introduce two different Infohashes with identical file bytes; prove actual cache reuse and target piece hash validation.
- Introduce two differently encoded cuts; compute true video/audio fingerprints and alignment confidence, reject inserted/deleted scene.
- Prove the final HLS/audio/video pipeline can switch source without frame gaps or A/V sync regressions.
- Benchmark Mac M5 native hardware encoder and prove Linux ARM64 Docker image boot on actual target.
- Verify security/resource limits, import/deletion lifecycle and no arbitrary peer or tracker SSRF.

## Release rule

A compiler green pipeline is necessary but not sufficient. No v1 release until the complete media and network integration matrix has passed. Staged alpha may ship only separately verified capabilities; no claim of finished multi-swarm streaming before end-to-end evidence.
