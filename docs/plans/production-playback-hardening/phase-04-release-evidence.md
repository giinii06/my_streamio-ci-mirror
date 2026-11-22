# Phase 04 — Real platform E2E, security review and production decision
**State:** not_started | Implemented=no | Tested=no | Verified=no | Committed=no
**Blocked by:** Phases 01–03 exact-SHA evidence.

## Release gates
- Browser tests on Chrome and Safari: user magnet -> select source -> playback -> seek -> pause -> disconnect -> clear error -> resume; actual audio/video clocks and no silent truncation. Record independently observed logs, browser build/platform and SHA.
- Real independent swarms, verified cache after restart, corrupt data fail closed. Dynamic mixed-encode switching is a **separate required v1 user flow**: safe align + segment switch + continuous audio, measurable with synthetic edit variants; offline HLS demo alone is not enough.
- Linux ARM64 Docker build, nonroot health + restart + read-only FS and cache limits. Mac M5 native VideoToolbox/CPU comparisons and fallback must be measured on actual device.
- GitHub Actions actual job logs, checked-out SHA, Rust format/clippy/build/tests, FFmpeg decode, E2E browser & platform jobs. No dispatch-only inferred pass.
- Measured startup p50/p95, stall ratio, useful verified goodput, memory/disk/cpu budgets; baseline vs optional multi-swarm with no unexplained regressions.
- Two-lens security/code reviews (independent reviewer if actually available), no critical/important unresolved defect. Record failures honestly with `NOT_RUN` rather than 0/success.

## Go/no-go
If any `docs/phases/13-RELEASE.md` No-Go condition remains, keep Draft / not production. When all gates independently proven, request separate authorization for merge and deployment.
