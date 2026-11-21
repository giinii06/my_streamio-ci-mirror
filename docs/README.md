# my_streamio — 구현 설계 문서 인덱스

> 상태: 설계안(구현 전). 작성 기준: 2026-11-22. 실제 코드, 성능, 런타임 API 적합성은 각 Phase의 검증 게이트를 통과해야 확정된다.
> 범위: **자신이 제작했거나 적법하게 배포·사용할 권리가 있는 미디어** 및 재현 가능한 테스트 토렌트. 콘텐츠 검색/저작권 우회 서비스는 범위 밖.

## 목표와 핵심 사실

목표는 영화/영상 1개를 여러 개의 Infohash/스웜에서 제공할 때 **가용 피어를 넓게 발견하고, 사용 가능한 전송량을 최대화해 버퍼링을 낮추는 로컬 우선 스트리밍 엔진**이다. 핵심 지표는 발견 피어 수가 아니라 검증된 유효 바이트/초, 시작 지연, 리버퍼링 시간이다.

- **동일 스웜**: 기존 BitTorrent에서도 다수 피어가 병렬 전송 가능.
- **다른 스웜, 동일 바이트 파일**: 검증된 바이트를 공용 저장소에서 재사용하고 각 토렌트의 해시 검증 규칙을 만족시킬 수 있다. 피어가 다른 Infohash에 자동 합류하는 것은 아니다.
- **다른 인코딩**: 원본 바이트를 섞으면 안 된다. 정렬된 재생 시간 구간 단위로 별개 스트림에서 비디오를 디코딩·변환·교체하여 하나의 출력 스트림을 만든다. 시간 길이/해상도 일치는 충분 조건이 아니다.
- **안전한 폴백**: 정렬 확신도, 다운로드 마감, 오디오/색공간 제약을 만족하지 않으면 단일 소스를 유지한다.
- **현실적인 성능 판단**: Apple M5/24GB의 실시간 합성/변환 능력은 코덱·해상도·동시 세션에 의존한다. 벤치마크 없이 '항상 된다'고 주장하지 않는다.

## 개발 스택(가설; PoC 후 결정)

- Rust stable + Cargo workspace, tokio, serde, tracing, axum(로컬 HTTP API); SQLite는 영속 메타데이터에만.
- TorrentBackend 추상화 → **librqbit(순수 Rust)** 또는 **libtorrent 2.x(C++ FFI)** 중 **Phase 02**에서 측정 후 결정. 알려지지 않은 바인딩/기능을 가정하지 않는다.
- FFmpeg/ffprobe subprocess 기반으로 시작. macOS는 VideoToolbox 하드웨어 경로를 계측하고, Linux는 CPU/설치된 인코더로 안전 폴백.
- M5 macOS 네이티브 실행 + 기존 GitLab Linux ARM64 Docker Runner에서 독립적인 CI. 사용자 UI는 먼저 브라우저/localhost, GPUI는 별도 선택 사항.
- Docker는 Linux daemon 배포용이다. macOS Docker 컨테이너에서 VideoToolbox 사용이 가능하다고 가정하지 않는다.

## 실행 순서 / 종속성

| Phase | 문서 | 산출물 / 차단 게이트 |
|---|---|---|
| 00 | [제품 정의](phases/00-SCOPE.md) | 비목표, 권리 범위, SLO, 테스트 픽스처 확정 |
| 01 | [개발 기반](phases/01-FOUNDATION.md) | Cargo workspace + Runner의 Rust 검증 |
| 02 | [토렌트 엔진](phases/02-TORRENT-BACKEND.md) | 두 후보 중 1개 선택, piece read + SHA 확인 |
| 03 | [피어·스웜 검색](phases/03-DISCOVERY.md) | 피어 측정, 제약 있는 multi-swarm 등록 |
| 04 | [피스 예약·버퍼](phases/04-STREAM-SCHEDULER.md) | 단일 소스 재생·seek·복구 |
| 05 | [동일 파일 교차 재사용](phases/05-EXACT-CONTENT.md) | 별도 Infohash 간 해시 검증된 바이트 재사용 |
| 06 | [영상 정렬](phases/06-TIMELINE-ALIGNMENT.md) | 다른 인코딩 간 타임라인·신뢰도 |
| 07 | [구간 조합·트랜스코딩](phases/07-SEGMENT-SWITCHING.md) | 품질/오디오 유지하며 조건부 소스 전환 |
| 08 | [재생 API·UI](phases/08-PLAYBACK-UX.md) | 플레이어, 진단, 수동 소스 선택 |
| 09 | [M5 가속](phases/09-MACOS-ACCELERATION.md) | VideoToolbox 사용/폴백 실측 |
| 10 | [측정·최적화](phases/10-PERFORMANCE.md) | 단일 소스 대비 개선 증거 |
| 11 | [보안·프라이버시](phases/11-SECURITY.md) | 악성 입력·네트워크 경계 테스트 |
| 12 | [Docker·운영](phases/12-DEPLOYMENT.md) | ARM64 Linux compose/회복 검증 |
| 13 | [통합·릴리스](phases/13-RELEASE.md) | 수용 테스트와 단계별 릴리스 게이트 |

## 횡단 문서

- [아키텍처와 경계](ARCHITECTURE.md)
- [데이터 모델·상태 전이·API 계약](DATA_CONTRACTS.md)
- [테스트 매트릭스 및 실험 프로토콜](TEST_MATRIX.md)
- [결정 기록과 기술 부채](DECISIONS.md)
- [기존 Runner 검증](CI_RUNNER.md)

## 단계 수행 규칙

1. 모든 구현 Phase마다 분리 브랜치 → 실패 사례 우선 테스트 → 구현 → CI 실제 실행 → 로그 검증 → Draft MR. 선행 Phase의 미완료 기능은 더미로 위장하지 않는다.
2. 테스트는 본인이 생성한 오픈 라이선스/합성 영상으로만 수행. 악성 magnet, 무한 트래커 호출, 포트 무단 개방 방지.
3. 문서상의 타입/경로/목표 수치는 **제안 또는 초기 실험 기준**이지 현재 구현 사실이 아니다.
4. 각 Phase의 통과 기준을 충족하지 못하면 다음 Phase가 아니라 원인 분석과 설계 수정으로 돌아간다.
5. 대형 최적화보다 항상 '재생 가능한 단일 소스' 폴백을 유지한다.
