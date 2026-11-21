# GitHub + GitLab parallel CI closeout — Alchemy poteto-mode

Target: `blond11qq/my_streamio`, branch `ci/github-parallel-verification`, Draft MR !12. This is a continuation of `docs/GITHUB_PARALLEL_CI.md`; no merging, deployment, repository-visibility changes or mirror-setting changes are in scope.

## Acceptance contract

Run GitLab's ARM64 Rust and HLS jobs independently of the GitHub Actions bridge. Each provider must be checked against an **identical source SHA**, with request/dispatch status separated from actual completed Rust/HLS job evidence. Do not publish unrelated private branches.

## Phase 01 — Investigate and establish baseline

- [x] Resolve GitLab branch and latest pipeline; record exact SHA.
- [x] Confirm `main` has the GitHub pipeline request bridge, and two `github-actions/ci-hub-request` contexts exist for the feature SHA.
- [x] Inspect GitLab ARM64 logs: `rust_verify` success, `media_hls_verify` failed at `ffmpeg -encoders | grep -q libx264` with exit 141.
- [x] Confirm the successful GitLab API pipeline `dispatch_github_actions` context alone is not proof of individual GitHub jobs.

## Phase 02 — Fix evidenced media CI defect

- [ ] Replace all affected FFmpeg encoder probes with a SIGPIPE-safe check that consumes the complete output.
- [ ] Add a regression check verifying the expected encoder selection when output contains many later encoder lines.
- [ ] Commit corrected CI/config and run GitLab real ARM64 Rust + FFmpeg synthetic HLS on the matching commit.
- [ ] Correct any observed source failure from real job logs; do not guess or claim unexecuted checks.

## Phase 03 — GitHub run and phase evidence

- [ ] Invoke the supported GitLab API pipeline on the exact implementation ref; verify matching SHA and `dispatch_github_actions` success.
- [ ] Check matching pipeline's `github-actions/ci-hub-request` status and run link.
- [ ] Verify GitHub Rust, Clippy and HLS **actual job conclusions** when the connected provider exposes them. Otherwise mark unverified explicitly.
- [ ] Update MR !12 status and this ledger with exact pipeline URLs, checked commits and remaining access limits.

## Execution rules

GitLab's `needs: []` permits both jobs to be scheduled together; the self-hosted runner may execute only one job at a time. GitHub's reusable Rust, HLS and Clippy jobs are separate. No phase is declared verified on mere source creation, dispatch acceptance, or stale SHA evidence.
