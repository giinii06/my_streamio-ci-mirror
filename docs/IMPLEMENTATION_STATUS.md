# Implementation status and evidence

This file distinguishes planning from functionality. Documentation in `docs/phases/` is a roadmap, not a statement of completed behavior.

| Phase | State | Verification |
|---|---|---|
| 00 | Fixture generator committed; run on FFmpeg-capable machine pending | shell syntax in CI |
| 01 | Not started | — |
| 02 | Not started | — |
| 03–13 | Not started | — |

Rules: commit one functional slice per phase; inspect matching SHA GitLab job logs, do not treat a smoke-only success as a Rust build, and keep failed/incomplete phases labelled. Do not merge or deploy automatically.
