# Phase 03 — Recovery, hostile inputs, quota and safe localhost
**State:** not_started | Implemented=no | Tested=no | Verified=no | Committed=no
**Blocked by:** Phase 02 exact-SHA evidence.

## User outcome
When peer disappears, payload corrupts, browser seeks repeatedly or daemon restarts, no incorrect bytes/media manifests are ever served. Recovery errors are bounded and understandable; work may be canceled; no remote web origin may control the local daemon.

## Test-first plan
1. Controlled N07/F10/F12 fixtures: missing peer, invalid torrent piece, 30 seeks, source cut and restart. Compare full content hashes after restart. Errors: 503/no successful payload, bounded retry, correct browser states.
2. Security: hostile tracker URI and redirect/DNS resolution, local host/origin with malicious punctuation, path traversal, oversized magnet/JSON, CSRF preflight and browser embedding; isolate non-loopback access and avoid leaking raw magnet/peer endpoint.
3. Resource caps: bytes buffered per playback, concurrent requests, persistent quota, session count, CPU/network flood, disk-full. Add configurable limits with documented safe values AFTER real measurement, not guessed production SLO.
4. Per-failure user-visible state and timeout; closedown/cleanup and repeatability; GitHub exact-SHA tests, requirements and security self-review.
Do not weaken localhost-only production constraint. No schema change without rollback plan.
