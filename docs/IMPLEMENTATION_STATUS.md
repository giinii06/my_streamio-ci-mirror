# Implementation evidence ledger

Updated 2026-11-22. All entries marked **unverified** until an exact HEAD SHA GitLab Rust pipeline actually executes and passes. Most phases are still incomplete end-to-end.

| Phase | Committed code | Status / missing acceptance gates |
|---|---|---|
| 00 | Synthetic FFmpeg fixture generator; `tests/fixtures/generate.sh` | GitLab Runner smoke passed at `8f54c136`; actual FFmpeg fixture generation not executed in CI |
| 01 | Rust workspace, domain types, loopback daemon, Rust CI | Rust build/test pending runner |
| 02 | librqbit session, magnet validation, add/list API | Source files and Range API added, but no actual verified-byte streaming test or backend benchmark |
| 03 | Budgeted multi-swarm registry, useful peer scoring | Not wired to actual librqbit peer stats or DHT/tracker events |
| 04 | Time-indexed demands, piece mapping, byte range, seek generations | Includes HTTP Range endpoint calling librqbit verified file stream; uncompiled and unverified against real media |
| 05 | Complete single-file v1 identity proof, bounded sparse piece store, target-piece SHA1 recheck | Pure in-memory code; no v2 Merkle proof or live two-torrent cache integration |
| 06 | Affine timeline alignment from provided evidence | No video fingerprint extraction or edited-cut piecewise alignment |
| 07 | FFmpeg video segment renderer with output validation | Offline HLS assembler with a single primary audio encoder now committed; end-to-end CI / running player verification pending, not live torrent switching |
| 08 | Local source manager UI | Local player has single-source byte-range URL; real video startup/seek test pending |
| 09 | VideoToolbox encoder discovery probe | Not benchmarked or integrated as active encoder on user's Mac M5 |
| 10 | Playback event metric aggregation | Not connected to running daemon or comparative benchmark |
| 11 | Magnet restrictions, loopback, origin/host guard | Security review, fuzzing, limits and production hardening incomplete |
| 12 | Dockerfile/Compose prototype | Image has not been built/run on real Linux ARM64 host |
| 13 | End-to-end API smoke script | Needs live Runner and complete streaming acceptance suite |

## Current verification

- Latest confirmed existing runner: Oracle-A1-ARM64, verified by prior smoke pipelines.
- After Rust implementation commits, current GitLab CI jobs are **pending** and have not executed. A pending/smoke-only pipeline is not a compiled or tested application.
- Project runner connection/availability should be checked under Settings → CI/CD → Runners if the queue persists.
- No real multi-encode streaming, peer pooling improvements or Apple M5 hardware acceleration can be claimed at this stage.

## Prioritization to reach actual product

1. Restore CI and clear format/compile/lint issues on exact source commit.
2. Wire librqbit's verified file stream into authenticated, bounded Range HTTP endpoint; prove one fixture plays.
3. Wire live source discovery stats into the registry, prove two distinct swarm sources yield usable data.
4. Add controlled synthetic movie timeline fingerprints from real decoded frames.
5. Implement HLS segment planner + audio continuity + safe source switching, verify true end-to-end.
6. Run macOS M5 native FFmpeg/VideoToolbox benchmarks and Linux ARM64 Docker image build.
7. Run security, failure recovery and performance gates, then release only tested capabilities.

**Do not mark a phase complete from a passing unrelated unit test or from this document.**
