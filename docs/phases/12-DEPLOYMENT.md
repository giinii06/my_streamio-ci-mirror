# Phase 12 — Linux ARM64 Docker 배포·운영 및 macOS 분리

**의존:** 08~11. **목표:** Oracle ARM64 환경 등 Linux 서버에 daemon을 안전하게 배포하되, macOS 전용 영상 가속과 CI Runner의 역할을 혼동하지 않는다.

## 배포 프로파일

A. **macOS native local:** Mac M5에서 my-streamiod 실행, VideoToolbox/FFmpeg 경로, localhost 브라우저 연결. macOS Docker가 아닌 호스트 네이티브 구성을 기본으로.

B. **Linux ARM64 docker:** Dockerfile multi-stage build --locked, nonroot uid, read-only rootfs 가능한 범위, /data cache/config volume, healthcheck, explicit resource limits. GPU 가속은 호스트에서 실제 지원이 입증된 경우에만 별도 설정.

C. **CI runner:** 기존 Oracle-A1-ARM64 project runner는 소스 빌드/테스트 실행용. 프로덕션 데몬을 CI job으로 24시간 띄우는 것은 금지.

## Docker 배포 작업

1. rust build stage와 slim 런타임 stage로 나누고 image digest 및 CPU 아키텍처를 고정.
2. 런타임은 필요한 media binary만 설치; FFmpeg binary 버전/라이선스/codec 표기.
3. .dockerignore로 fixtures/다운로드/cache/secrets 제외. .env는 Git 추적 제외.
4. Docker volume에 verified cache, profile DB, transcode temp를 분리할지 disk benchmark 후 결정.
5. 컨테이너 재시작 때 dangling temp cleanup, incomplete segment discard, verified bytes rehash/consistent cache index 복원.
6. compose.yml: 최소 공개 포트(기본 로컬), 제한된 네트워크, no privileged, user credentials/paths 명시.
7. Torrent용 TCP/UDP 포트가 필요한 경우 사용자가 승인한 범위만 열고, reverse proxy와 관리 API를 분리.
8. graceful SIGTERM 종료와 interruptible FFmpeg; 종료 도중 재생 상태 DB commit 및 임시 작업 폐기.
9. log rotation·quota, cache LRU GC, SMART 여유 공간/디스크 쓰기 폭주 감시.
10. 업데이트: 이미지 롤백, DB migration version/백업, 사전 무결성 검사, startup health check.
11. 호스팅 네트워크·트래커 규약을 준수하고 공개 인바운드 접근은 명시적으로만 활성화.

## 수용 테스트

- linux/arm64 image 빌드, 기본 healthz, root 권한 없이 정상 start.
- 제한된 메모리/디스크에서 Graceful degrade; disk full 시 데이터 손상/무한 재시도 없음.
- 갑작스런 kill -9 후 재시작: 기존 검증 파일만 사용, 임시 segment는 노출하지 않음.
- 2회 연속 재배포 및 롤백: cache schema 호환성과 영향 범위 기록.
- Docker와 Mac M5 네이티브의 기능 차이를 명시한 운영 매트릭스 발행.

## 성공 기준

ARM64 Linux에서 end-to-end **합법 테스트 비디오** 재생, 로그/상태/데이터가 재시작 후 합리적으로 복구. macOS VideoToolbox 기능을 Linux CI 성공의 일부로 표기하지 않음.
