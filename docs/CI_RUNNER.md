# GitLab Runner bootstrap

This repository uses a minimal smoke-test pipeline in `.gitlab-ci.yml` before adding application build jobs.

## What the smoke test verifies

- A GitLab CI job is scheduled onto an available runner.
- The runner can retrieve the repository and start an Alpine Linux job.
- Built-in runner and commit variables are present.
- The checked-out repository contains `README.md`.

The job intentionally has **no tags**, no privileged services, no deployment steps, and no secrets. It can run on a compatible GitLab-hosted Linux runner or a suitable untagged custom runner.

## Enable a GitLab-hosted runner

At the time this bootstrap was created, the GitLab project API reported `shared_runners_enabled: false`. The GitLab connector available for this task cannot change that project setting.

1. Open **GitLab > my_streamio > Settings > CI/CD**.
2. Expand **Runners**.
3. Under **Instance runners**, enable instance runners for this project (label may vary).
4. Open **Build > Pipelines** or the merge request's pipeline.
5. Confirm `runner_smoke` reaches **success**; inspect the job trace.

GitLab-hosted runner usage is subject to GitLab's namespace compute-minute limits and project eligibility.

## If jobs stay pending

- Confirm instance runners are enabled and an eligible runner is online.
- Confirm the runner can execute **untagged jobs**; this job uses no `tags`.
- Check whether the runner is restricted to protected branches.
- Check GitLab's CI/CD quota and usage information.
- Read the job's *stuck/pending reason* before changing CI code.

## Dedicated macOS runner later

For Apple M5 media-engine benchmarks, use a separate project runner on the Mac and explicitly tag macOS-specific jobs. Do not assume GitLab-hosted Linux runners provide Apple VideoToolbox or M5 hardware. Keep runner authentication tokens out of Git and CI logs.

## Scope

Only runner availability is tested here. This pipeline does not yet build or test Rust, libtorrent, FFmpeg, streaming, or transcoding code.
