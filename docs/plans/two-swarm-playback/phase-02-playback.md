# Phase 02 — Verified peer bytes → localhost HTTP Range

- Preconditions: Phase 01 seeded torrent handles and reachable loopback peer port(s).
- Observe RED: against existing daemon the test cannot supply controlled initial peers without a test-only seam; production magnet validation rejects localhost tracker literals intentionally.
- Minimum production change: only an opt-in `#[cfg(feature = "network-fixture")]` test seam in `TorrentBackend` to supply a fixed loopback initial peer. The feature MUST NOT be enabled in normal release builds. Reject non-loopback IPs or arbitrary caller-supplied peer endpoints.
- Reuse the real `POST /api/v1/sources` and `GET /api/v1/sources/{id}/files/{file_id}/stream` handlers. Extract router builder for an in-process HTTP integration test if needed. Test full requested ranges, byte equality/sha256, and EOF/Range rejections.
- Negative: no seed returns bounded failure, stream never serves raw local file without lib piece proofs, no public Listen or tracker changes.
- Record discrepancy between HTTP response and data extent. Fix only evidenced failures, keep max per-read cap and bounded runtime.
- Rollback: gate/remove test-only feature and related integration test.
- State: `not_started`. Implemented=no; Tested=no; Committed=no.
