# Production playback hardening — Alchemy 2.2
gate_version=2.2

## Approved scope and boundaries
The product remains **local-only, single-user**, with a Rust daemon and browser player. The current stacked source branch `feat/controlled-two-swarm-playback-e2e` is the integration baseline; this is a separate Draft MR to preserve that work and its CI evidence. Do not merge, deploy, expose API publicly, change mirror/runner settings, or advertise a v1 release.

**Final outcome (not yet implemented):** a user enters a lawful magnet, selects compatible media, starts/stops/seeks it in a local browser, with verified-only bytes, correct HTTP semantics, predictable errors after peer loss, bounded RAM/network/disk usage, and no cross-origin command injection. Multi-encode source switching is required for a future full v1, but is NOT proved by the existing static HLS demo or this hardening patch.

## Observed initial defects (baseline 3bf8a5948edd10f0c392c3ab7799e84d497235fe)
1. `parse_http_range("bytes=-N", size)` rejects a common suffix range; no end-of-file media seek.
2. GET with no Range serves just one <=1 MiB chunk as HTTP 206 even though the client never asked for partial content.
3. Invalid/unsatisfiable ranges return 416 without the `Content-Range: bytes */<size>` response field needed for client recovery.
4. Browser startup/seek, peer corruption, restart, bounded workloads, independent GitHub job logs and native macOS/Linux target verification remain unproven.

## Phase sequence (do not advance past missing gates)
| Phase | State | User-visible gate | Consumer |
| --- | --- | --- | --- |
| 01 [RFC byte-range parsing and error headers](phase-01-http-range.md) | not_started | client can request last N bytes; bad ranges report representation size | source HTTP playback/seek |
| 02 [bounded full-response and backpressure](phase-02-full-response.md) | not_started | a no-Range GET returns a complete HTTP 200 body without unbounded buffering | source playback and download |
| 03 [network failure, lifecycle and security](phase-03-resilience-security.md) | not_started | no corrupt/stale bytes, bounded peer-loss/restart and safe localhost origin | usable playback session |
| 04 [platform, CI and release decision](phase-04-release-evidence.md) | not_started | full browser/peer/platform acceptance with exact-SHA logs | production release |

Append-only event log:
- `not_started`: 2026-11-22, baseline source SHA above, new stacked review branch; all phases remain not started until evidence supports transitions.
- This is a PLAN, not a declaration of product readiness or permission to merge/deploy.

## Acceptance and evidence policy
- Only self-generated/suitably licensed media. No automatic public content catalog, DRM bypass, unrestricted proxy, shared multi-user SaaS or arbitrary remote management.
- Keep `librqbit` content verification as sole path to the stream; no unverified disk reads. Multiple infohashes must never be treated as identical solely from length, filename or duration.
- Test: `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace`, `sh tests/integration/hls.sh`, `sh tests/integration/smoke.sh <built-daemon>`; actual execution must be evidenced on SAME source SHA by GitHub Actions.
- Existing effective GitLab CI `.gitlab/ci-hub-request.yml@blond11qq/ci-hub-smoke` dispatches GitHub Actions; zero GitLab Runner compute. A `github-actions/ci-hub-request=success` callback alone does NOT establish `jobs_verified` or `e2e_verified`.
- No local authenticated Rust checkout/toolchain is available from this ChatGPT session; remote CI is mandatory for Rust evidence.
- Exact-SHA requirements and quality/security reviews recorded separately. No phase may move to `verified` on indirect evidence alone.
- Rollback = do not merge this stacked branch; reverting per-phase commits is possible without data-schema changes.
- Mac M5 VideoToolbox and Linux ARM64 Docker are separate acceptance targets; no evidence from Ubuntu CI substitutes for actual target runs.
- The broader 14-phase release program and `docs/phases/13-RELEASE.md` remain authoritative for full v1. A locally correct HTTP Range endpoint is **necessary, not sufficient**.
