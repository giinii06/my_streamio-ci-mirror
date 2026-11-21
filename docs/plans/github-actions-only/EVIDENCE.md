# Exact-SHA CI evidence and review — GitHub-only migration

`gate_version=2.2` — source evidence recorded for a historical candidate.
This record is NOT permission to mark unobserved GitHub jobs successful.

## Pinned candidate and provider

- GitLab source-of-truth: `blond11qq/my_streamio`
- Branch: `ci/github-parallel-verification`
- Draft MR: [!12](https://gitlab.com/blond11qq/my_streamio/-/merge_requests/12)
- **Checked candidate code SHA:** `b5ad25e06a8aba9798de5270d90b366067b8daeb`
- Effective GitLab project CI: `.gitlab/ci-hub-request.yml@blond11qq/ci-hub-smoke`
- GitLab [pipeline #3098278029](https://gitlab.com/blond11qq/my_streamio/-/pipelines/3098278029): `success`, source SHA matched, `dispatch_github_actions=success`, **zero GitLab Runner jobs**.
- External GitLab commit context `github-actions/ci-hub-request=success`, completed `2026-11-21T21:49:57.859Z`, same source SHA and pipeline ID, provider run [GitHub #44953323573](https://github.com/giinii06/my_streamio-ci-mirror/actions/runs/44953323573).
- GitHub `rust-hub` / `rust-policy` / `media-hls` individual job execution, checked-out SHA and logs: **UNKNOWN** (not readable through currently connected GitLab tooling, public web fetching blocked, GitHub app not connected).
- Local Cargo build/test/format: **NOT RUN** (no authenticated local repo checkout); no inferred pass.
- Mac M5 VideoToolbox E2E: **NOT RUN** (outside CI migration acceptance).

## Phase state (do not infer tests from callbacks)

| Phase | v2.2 state | What is grounded | Open acceptance |
|---|---|---|---|
| 01: Remove GitLab Runner CI | verified | `.gitlab-ci.yml` absent, effective external CI path present, matching-SHA pipeline reports zero runner jobs | Historical runners can still exist; no settings altered |
| 02: Align GitHub workflow and docs | implemented | .github workflow and README/doc source reviewed on checked SHA | GitHub individual Rust, Clippy, HLS jobs must be inspected |
| 03: GitHub provider execution proof | implemented | exact-SHA dispatch and external provider callback succeeded | GitHub job detail, checkout ref and steps not independently verified |

**Append-only transitions/evidence:**
- `phase-01: not_started → in_progress → implemented → tested → verified` based on commit `b5ad25e0`, source-tree check, effective GitLab CI configuration and zero-job pipeline.
- `phase-02: not_started → in_progress → implemented` based on same candidate's source/MR inspection, **no claim of provider job proof**.
- `phase-03: not_started → in_progress → implemented` based on the matching-SHA GitHub dispatch and `success` provider callback, **jobs_verified remains false**.
- No transition to `completed` is recorded. Missing reviews/required job evidence prevent completion.

## Two-lens source review (self-review, NOT independent agent)

**Requirements lens:** The deleted `.gitlab-ci.yml` contained `runner_smoke`, `rust_verify` and `media_hls_verify`; the effective external GitLab config remains the established GitHub bridge. No alternate GitLab application checks were selected by the matching pipeline. **Observed workflow meets the GitHub-only-compute requirement.** Existing GitLab API pipeline remains a dispatch/status controller, intentionally not removed.

**Quality/security lens:** Github caller workflow has `permissions: contents: read`; no new secrets, visibility or mirror changes; GitHub jobs are separate without serial `needs`. Open evidence risks: `ci-hub` implementation and actual provider job logs are not independently inspected; `rust-policy` runs the *mutating* `cargo fmt --all` rather than a strict formatting check; some actions use mutable tags, which merits follow-up supply-chain pinning. This review is **not** an external independent review.

## Resume and exit condition

A subsequent session must re-read actual source HEAD and MR SHA. This checkpoint refers to **code SHA `b5ad25e0`**, not to the later documentation checkpoint commit. If a new docs commit changes HEAD, obtain new matching-SHA CI evidence before claiming it is verified.

The final outstanding step is direct GitHub run/job/step inspection with a connected GitHub provider or authorized external GitHub viewer. The external `success` status by itself is not proof of each Rust/HLS/Clippy command.
