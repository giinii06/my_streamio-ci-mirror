# Verified sparse reuse between different torrent piece layouts

The `crates/verified-store/src/sparse.rs` module implements a small, bounded and strictly v1/single-file proof-of-concept.

## Safety requirements

1. Two torrents must have a **proven identical complete file identity** (not just the same title, runtime or resolution). The current constructor accepts a SHA-256 identity produced by an already complete `VerifiedObject` whose v1 pieces were verified.
2. Every inserted source span is a **whole v1 source piece** that passes the **source** torrent's expected SHA-1.
3. When returning a piece for the target infohash, the complete byte range must already be present, and **the target torrent's SHA-1** is checked again.
4. The sparse store never returns bytes for gaps, conflicting overlapping spans, invalid source hashes, missing data, or an exceeded memory budget.
5. It supports only pieces of a v1 **single-file torrent that starts at torrent offset zero**. Multi-file v1 pieces spanning neighboring files, BEP52/v2 Merkle proofs, hybrid torrents and partially verified blocks remain unsupported.
6. Peer connections stay isolated by Infohash: this reuses verified bytes, not P2P peer identities.

## Test cases

- Same 10-byte file with source piece length 4 and target length 6.
- Missing a source piece → target piece is not available.
- Source corruption, target digest mismatch, and conflicting overlap rejected.
- Memory quota and integer overflow guards.

## Remaining integration work

- Verify source/target torrent metadata and authenticated file identity from the running `librqbit` session (do not infer from names).
- Support sparse verified BEP52 proof and cross-file v1 piece reconstruction.
- Integrate a lease-managed on-disk cache keyed by file identity, not by Infohash alone.
- Wire the store to actual playback demand, switch deadlines and torrent source progress.
- Benchmark whether cross-swarm byte sharing increases useful throughput and reduces stalls; cache reuse is not network goodput.

This module is deliberately fail-closed and is **not** a claim of finished live multi-swarm streaming.
