# GitLab Runner bootstrap

This repository starts with the `runner_smoke` job in `.gitlab-ci.yml`, before adding application build jobs.

## Verified runner (2026-11-22 KST)

- Existing project runner: **Oracle-A1-ARM64** (runner ID **59776971**)
- Platform: **Linux ARM64**, GitLab Runner **19.6.0**
- Executor: **Docker**, successfully pulled **alpine:3.21**
- Successful smoke pipeline: [#3097758566](https://gitlab.com/blond11qq/my_streamio/-/pipelines/3097758566)
- Tested commit: `5aaf52d0936cad04b66663585ef2c56e4d3826be`

The runner checked out the repository and executed the smoke script. The project API reports `shared_runners_enabled: false`, but that **does not prevent this active dedicated project runner from executing jobs**. No GitLab-hosted runner enablement or new runner registration is needed for these jobs.

## What the smoke test verifies

- A GitLab CI job is scheduled onto an available runner.
- The runner can retrieve the repository and launch an Alpine Linux Docker image.
- Built-in runner and commit variables are present.
- The checked-out repository contains `README.md`.

The job intentionally has **no tags**, no privileged services, no deployments, and no secrets.

## If jobs become pending

1. In **GitLab > my_streamio > Settings > CI/CD > Runners**, confirm `Oracle-A1-ARM64` is online, active, and assigned to the project.
2. Check that it accepts **untagged jobs** (this job has no `tags`).
3. Confirm the runner is permitted on the source branch (protected runner restrictions).
4. Inspect the job's pending/stuck reason and the Docker executor service.
5. Optionally enable GitLab-hosted **instance runners** under the same settings if additional runners are needed and account compute minutes allow it.

## Future: native Apple M5 benchmarks

This Linux ARM64 runner is suitable for CI smoke checks and potentially ARM64 software builds, but **not** for native Apple VideoToolbox or M5 media benchmarks. For those, separately register a macOS project runner, give it a distinctive tag, and route macOS-only jobs explicitly to it. Never place runner authentication tokens in Git or CI logs.

## Scope

Only runner availability is tested here. This pipeline does not yet build or test Rust, libtorrent, FFmpeg, streaming, or transcoding code.
