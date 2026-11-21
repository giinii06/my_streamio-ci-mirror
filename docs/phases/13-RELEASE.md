# Phase 13 — 통합 검증, 릴리스 게이트 및 후속 로드맵

**의존:** 00~12의 필수 검증. **목표:** 단일 소스와 다중 소스를 제대로 구별하고, 데이터 무결성·A/V 연속성·사용자 진단이 모두 작동하는 배포 가능 버전.

## 릴리스 단계

- **Alpha A:** 사용자 입력 magnet/.torrent + 단일 소스 다운로드/Range playback, seek. 단일 인코딩만.
- **Alpha B:** 여러 Infohash의 source catalog/peer 관측, active source selection, 동일 바이트 검증 cache 공유.
- **Beta C:** 이종 인코딩 정렬, 안전한 별도 segment 선택, FFmpeg 공통 출력; 단일 source 폴백.
- **Beta D:** macOS VideoToolbox 조건부 가속 및 Linux ARM64 Docker 재현.
- **v1:** 사용성·에러 복구·보안 게이트 통과, 성능 실험 결과 공개.
- **후속 연구:** 장편 4K HDR, 언어/오디오 트랙 switching, GPUI native shell, 원격 멀티 디바이스. 실험 결과가 선행.

## 필수 릴리스 체크

1. docs/TEST_MATRIX.md의 모든 필수 시나리오 green; flaky 통과를 성공으로 치지 않는다.
2. CI: Linux ARM64 build/test, integration fixtures, FFI가 있으면 sanitizer/메모리 테스트, container build.
3. macOS: native build 및 실제 M5 장치의 hardware capability, 1080p encode pipeline logs.
4. 스웜 간 verified byte 재사용이 **Infohash 관계를 위반하지 않는지** 테스트.
5. 파일의 해시/영상 정렬 실패 시 재생이 자동 손상되지 않는지.
6. 30회 반복 seek, tracker 단절, 소스 교체, cache eviction, restart stress.
7. 비디오 frame gap/PTS mismatch/오디오 drift/검은 화면 오류 검증.
8. redistributable test assets의 라이선스 및 생성 스크립트 유지.
9. feature flag로 multi-source, fingerprint, transcode, hardware acceleration을 각각 비활성화할 수 있어야 함.
10. 최신 소스 SHA의 GitLab pipeline/job trace와 성능 결과를 release note에 링크.
11. Crash/bug report의 magnet URL, peer IP, 원본 경로 등 민감정보가 마스킹되어야 함.

## Go / No-Go

- **No-Go**: unverified bytes output, 틀린 컷으로 이어 붙임, 세션 권한 누출, stale seek manifest 갱신, 재생 중 재현성 있는 data corruption.
- **Conditional:** 일부 codec/HDR 미지원, 제한된 peer 수, VideoToolbox unavailable — 명시적 폴백이면 Alpha/Beta의 일부로 허용.
- **Go:** 인코딩 전환 실패가 사용자에게 명확히 표시되고, 최소 baseline인 단일 소스 재생 품질을 훼손하지 않음.

## 운영 이후 개선 사이클

- 메트릭: startup p95, stall_seconds/hour, verified useful peer throughput, decode/encode p95, segment deadline miss%, crash-free session%.
- 사용량 분석은 옵트인; PII/peer IP redaction 유지.
- 사용자 체감 개선과 비교 baseline이 동반되지 않은 최적화는 출시 근거로 사용하지 않는다.
- 서로 다른 소스 조합이 실제로 이득 없는 경우 source switching을 자동으로 줄이는 정책을 허용한다.
