# Phase 02 — Complete no-Range GET, bounded memory and backpressure
**State:** not_started | Implemented=no | Tested=no | Verified=no | Committed=no
**Blocked by:** Phase 01 exact-SHA evidence.

## Outcome
`GET .../stream` without Range must stream a **full, correct HTTP 200** representation, not a misleading single 1 MiB 206 slice. A client abort must stop needless reads; peak response buffering remains bounded by <=1 MiB per in-flight request (plus network overhead), and slow peer/timeout cannot turn into fabricated success.

## Interfaces and approach
- `apps/daemon/src/main.rs` `stream_source_file`: use bounded, demand-driven async response body; delay successful headers until first verified chunk when practical.
- `crates/torrent-adapter/src/lib.rs` `read_verified_range`: retain piece-verified reads and bounded timeouts; no raw file path shortcut.
- `apps/daemon/src/network_e2e.rs`: verify GET no Range, full SHA256 equality and response size after two actual swarms; abort or peer disconnect must terminate stream without unverified bytes.
- Benchmark seek latency and memory at multiple fixture sizes; do not assume library's stream is always ready.

## Execution
RED HTTP integration with no Range vs expected full 200/length/hash; implement streaming body with explicit backpressure and cancellation; GREEN on GitHub real jobs; self-review requirements/security; avoid adding extra async/dependency wrappers without a measured need. No release until real browser smoke test.
