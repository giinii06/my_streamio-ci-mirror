# Phase 01 — Rust workspace, CI, 개발 환경

**의존:** 00. **목표:** Mac M5 24GB 개발과 기존 GitLab ARM64 Linux Runner에서 동일 핵심 로직을 검증할 최소 Cargo 프로젝트.

## 구현 작업

1. 루트 Cargo workspace, Rust stable toolchain pin(rust-toolchain.toml), fmt/clippy 및 정확한 lockfile를 추가한다.
2. apps/daemon에는 graceful shutdown, 임시 스토리지 루트, bind 127.0.0.1, GET /healthz만 구현한다.
3. crates/domain에는 SourceId/ContentId/SwarmKey/PlaybackId newtype과 입력 검증만 구현한다.
4. 추적 로그는 tracing + JSON/human 개발 옵션. CI에는 secret 없는 환경 변수와 deterministic 출력만 허용.
5. 최소 구성: config schema version, max active torrents, cache quota, peer limits, listen port, metrics opt-in, media worker concurrency.
6. API 에러 envelope, cancellation token, task shutdown timeout, Ctrl+C 회수, 종료 후 temp file 청소를 검증한다.
7. README에 macOS 명령과 Linux ARM64 명령을 분리한다. FFmpeg는 향후 Phase에 추가.
8. CI의 기존 runner_smoke 유지. 신규 lint, unit, build (Linux aarch64, Docker image가 해당 아키텍처를 지원하는지 확인) jobs. PR/MR 시 중복 실행 방지 rules 검토.
9. RUSTFLAGS, linker, container manifest 등 버전을 고정. 캐시의 lockfile 해시를 키로 사용. 대형 바이너리와 개인 credentials을 저장하지 않는다.

## 예상 파일

- Cargo.toml / Cargo.lock / rust-toolchain.toml / rustfmt.toml
- apps/daemon/src/main.rs, crates/domain/src/lib.rs
- .gitlab-ci.yml 의 Rust lint/test/build jobs
- docs/CI_RUNNER.md 업데이트(실제 CI job IDs, Runner의 aarch64 OS 확인 결과)

## 검증

- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets -- -D warnings
- cargo test --workspace
- cargo build --workspace --locked
- localhost /healthz 200; 비로컬 bind 기본 차단.
- SIGTERM/CTRL+C 종료 후 열린 포트/임시 작업이 남지 않는 테스트.
- 매칭 commit SHA가 포함된 GitLab Runner job log 실재 확인.

## 완료 게이트

- Oracle-A1-ARM64에서 소스 체크아웃 후 Rust build/test가 **실제로 성공**.
- macOS native 빌드(최소 로컬) 성공, ARM64 Linux와 경계 분리.
- 이 시점에는 토렌트 다운로드나 영화 스트리밍 지원이라고 주장하지 않는다.

## 롤백

Docker/Runner 호환성 문제면 smoke job 유지, 신규 Rust job만 분리. Runner 인프라 장애를 소스 코드 버그로 추측해 고치지 않는다.
