# Two independent BitTorrent swarms → verified video playback (Alchemy v2.2)

**Source:** private GitLab `blond11qq/my_streamio`. This work branches from `ci/github-parallel-verification`; GitHub Actions is the ONLY build/test provider. New Draft MR, no merges/deploys/visibility/mirroring/runner configuration changes.

## User-facing outcome

With two independently created BitTorrent v1 infohashes seeded from self-generated reference H.264 and alternate HEVC footage, open the local daemon, admit both magnets and retrieve verified video byte ranges from BOTH sources. Seeking must return the exact requested bytes. Without the seed a request must NOT silently return unrelated or unverified bytes.

**Important scope distinction:** This is a real P2P → verified Range playback E2E milestone, not proof of automatic mid-stream encoding transitions; multi-encoding live HLS planner/cross-hash reuse remains a distinct future phase.

## Architectural choice

Use `librqbit::Session::create_and_serve_torrent` to seed legally generated fixture MP4 files on loopback in one separate seeder session. Use `AddTorrentOptions.initial_peers` *only* behind an explicitly compiled test feature or test-only API to connect the downloader to that exact loopback seeder without public trackers. Never accept caller-controlled IP addresses in the released API and never permit loopback tracker URLs in normal magnet validation.

## Phase state machine — gate_version=2.2

| Phase | Current | Implemented | Tested | Committed |
|---|---|---|---|---|
| 01 plan + deterministic local seed fixture | not_started | no | no | no |
| 02 connect live downloader and validate positive/negative Range | not_started | no | no | no |
| 03 run GitHub Actions exact SHA and review | not_started | no | no | no |

Append-only log:
- `not_started` — plan created before implementation at source baseline `ad3bf77f6a45edc03ce99999b32d8d1f07452d6e`.

## Acceptance gates

1. Two distinct magnet Infohashes and two real seeder torrents created from independently encoded, synthetic MP4 inputs; log only fixture hashes/metadata, no sensitive endpoints.
2. Real TCP BitTorrent peer handshake, BEP9 metadata acquisition and piece-verified download occur (no copying fixture files into the downloader directory).
3. For EACH source: HTTP `POST /api/v1/sources`, `GET /files`, `GET /files/0/stream` partial first bytes and mid-file seek return correct status/content-range/body, with SHA-256 equality against original file.
4. Negative cases: malformed magnets 422, Range outside EOF 416, unavailable pieces/peer down fail closed with 503 or bounded timeout; no unverified bytes or metadata paths exposed.
5. No GitLab Runner compute. GitHub workflow explicitly invokes this test. Exact git SHA and actual job logs are the only proof; callback `success` is not sufficient.
6. Never claim production readiness, browser A/V smoothness, failover, or M5 hardware acceleration from this limited E2E.

## Phases

- [Phase 01 — Seeding and fixtures](phase-01-seeding.md)
- [Phase 02 — Real verified playback](phase-02-playback.md)
- [Phase 03 — CI gates and review](phase-03-ci-review.md)

Risk: `librqbit` startup/ports/BEP9 may require source-backed API adjustments; fail if actual peer metadata is not obtained. No artificial stubbed peer bytes may satisfy the test. GitHub public mirror has the already-established GitLab request bridge; no new token is checked into source.
