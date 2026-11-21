# Phase 03 — GitHub Actions CI and evidence

- Preconditions: fixture and Range test are committed, passing Rust compiler check available.
- Run via GitHub Actions **only**, integrating the test into an actual executed GitHub workflow (not the dormant GitLab source workflow).
- Pin every verdict to the GitLab HEAD 40-character SHA and GitHub checked-out SHA. Verify actual job logs for fixture creation, real peer connections, Rust tests and media assertions.
- Positive: both torrents and each verified Range test succeed, no skip.
- Negative: intentionally unreachable seed / invalid Range errors are asserted.
- Two-lens self-review of security and user-visible requirements, no independent review unless real agent present. Save exact-SHA, provider and open blockers; keep MR Draft.
- Any missing Job log => `jobs_verified=unknown`, regardless of bridge callback.
- State: `not_started`. Implemented=no; Tested=no; Committed=no.
