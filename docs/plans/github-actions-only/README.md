# GitHub Actions only — CI consolidation

Target: `blond11qq/my_streamio`, branch `ci/github-parallel-verification`, Draft MR !12.

## User-owned decision gate

- **Resolved from user's explicit instruction:** GitHub Actions is the **only compute provider** for build, test, Clippy and HLS. No GitLab-hosted or Oracle ARM64 Runner jobs.
- **Existing deployment/source topology remains:** private GitLab repository as source of truth; hosted GitLab pipeline bridge only dispatches to public GitHub CI mirror `giinii06/my_streamio-ci-mirror` and relays the status. No change to mirror visibility, protected branches or hosted controller configuration.
- No automatic merge, deployment or CI/runner settings change.

## Phase 01 — Replace inactive GitLab Runner configuration

- [x] Delete the stale repository `.gitlab-ci.yml` containing runner_smoke, rust_verify and media_hls_verify.
- [x] Preserve the already-configured project CI path `.gitlab/ci-hub-request.yml@blond11qq/ci-hub-smoke`, whose jobs only request GitHub Actions and do not run Rust/FFmpeg.
- [x] Record that changing GitLab CI settings back to default would require explicit review and restoration of the bridge.
- Implemented: yes | Committed: yes | Tested: yes (source tree and matching-SHA zero-runner pipeline)

## Phase 02 — GitHub workflow and documentation consistency

- [x] Keep GitHub reusable Rust ci-hub, Clippy and HLS jobs independently runnable.
- [x] Align documentation to the actual externally configured GitLab→GitHub dispatch, not legacy protected-branch mirroring assumptions.
- [x] Update README and CI document with GitHub-only policy and exact-SHA verification.
- Implemented: yes | Committed: yes | Tested: static source review only (individual GitHub job logs not available)

## Phase 03 — Exact SHA remote verification

- [x] Inspect resulting branch SHA, workflow file, GitLab project's external CI configuration path and MR diff.
- [x] Trigger GitHub CI through a GitLab API pipeline for that exact branch SHA, capture `github-actions/ci-hub-request` result and run URL.
- [x] Check actual individual GitHub jobs when provider data is available, else explicitly mark their conclusions unverified.
- [x] Update Draft MR !12 with evidence. Do not merge/deploy.
- Implemented: yes | Committed: yes | Provider callback: success | Jobs: unknown (not directly inspectable)

## Acceptance

The **only** paths that compile or test Rust and media are under `.github/workflows/` and the public `ci-hub` reusable workflow. The GitLab trigger is an orchestration entry point, not a GitLab build. No GitLab Runner job should be defined by the active CI config. A successful dispatch alone is not proof that the final build/test passed.

**Known limitation:** the current GitLab connector cannot change repository-level CI config settings and does not provide GitHub job logs. These do not justify fabricating results.
