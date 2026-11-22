# Phase 01 — HTTP Range parsing and 416 representation size
**State:** not_started | Implemented=no | Tested=no | Verified=no | Committed=no

## Behavior contract
Given length L > 0: `bytes=N-M` yields [N, min(M,L-1)] inclusive, `bytes=N-` yields [N,L-1], and suffix `bytes=-K` (K>0) yields [max(0,L-K), L-1]. Suffix larger than file returns entire representation. Strictly reject suffix zero, overflow, invalid syntax, multiple ranges and all requests on a length-0 file. HTTP 416 must expose `Content-Range: bytes */L`. Valid partial reads must expose exact 206 status and `Content-Range` and never return unverified pieces.

## Source interfaces
- `crates/scheduler/src/lib.rs` : `parse_http_range(&str,u64)->Result<ByteRange,&'static str>`.
- `apps/daemon/src/main.rs`: `stream_source_file` bad Range HTTP response.
- `apps/daemon/src/network_e2e.rs`: actual 416 header assertions, successful tail seek through BitTorrent.
No new Cargo dependencies, routes, public network surface, torrent-payload storage, or schema.

## Execution microsteps
1. Commit tests only: assert `bytes=-1`, `bytes=-1024`, `bytes=-larger-than-file`, malformed/overflow/suffix zero and 416 `Content-Range: bytes */length`. Include real BitTorrent tail HTTP seek.
2. Trigger matching-SHA GitHub Actions through the established GitLab bridge and observe expected RED; distinguish expected assertion failure from build/infra failures.
3. Implement minimal, overflow-safe parser suffix range; set the 416 range-size header. Do not silently weaken tests or alter returned bytes.
4. Run `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `sh tests/integration/hls.sh` via actual executor/GitHub Actions; record commands and SHA. Independent job/checkout evidence is required for `verified`.
5. Review behavior and quality/security with distinct checklists; pin to code SHA, append genuine transition and checkpoint.

## Negative gates / rollback
Invalid range must not provoke panic, overflow or large allocation. Any missing verified bytes must remain HTTP 503 without a successful Range payload. Revert phase code without changing persisted torrent files.
