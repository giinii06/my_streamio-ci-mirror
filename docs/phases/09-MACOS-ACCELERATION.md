# Phase 09 — Apple M5 / 24GB 하드웨어 미디어 가속

**의존:** 07, 08. **목표:** macOS 네이티브에서 VideoToolbox 가속 여부와 실제 성능 이득을 계측하며, 가속이 안 되는 조합은 안전 폴백.

## 중요한 구분

- macOS M5의 하드웨어 비디오 디코더/인코더를 사용하려면 macOS native FFmpeg/VideoToolbox 또는 네이티브 API 경로가 필요하다.
- GitLab Linux ARM64 Runner와 Docker Linux 이미지는 VideoToolbox를 제공하지 않는다. Linux CI green은 M5 하드웨어 검증의 증거가 아니다.
- 지원 코덱/픽셀포맷/프로파일/HDR/동시 encoder session은 모델명만으로 단정 금지. 실행 중 capability probe 결과로 판단.
- 24GB 통합 메모리는 여러 세션에 공유되며, 다중 4K/HDR transcode 성능을 보장하지 않는다.

## 개발 작업

1. macOS arm64 FFmpeg 버전/빌드 설정 출력 수집: encoders, decoders, hwaccels, videotoolbox 지원, 실제 encode/session 성공.
2. 짧은 합성 영상의 H.264, HEVC, AV1 decode와 H.264/HEVC encode 조합을 **지원하는 경우에만** 실행하고 결과 프레임 검증.
3. 첫 버전은 FFmpeg subprocess를 제어하고, 필요 시 VideoToolbox native FFI 여부를 별도 ADR로 논의.
4. GPU/CPU 경로가 출력 프로필에서 동일한 timestamps와 색 공간의 결과를 만들 수 있는지 검증. 색공간/transfer/matrix 태그 체크.
5. source sample 여러 개 동시 decode, 1080p streaming encode, output remux를 따로 측정하여 병목 위치 식별.
6. encoder memory queue/copy count, process lifetime, thermal throttling, 전원 연결 여부를 계측한다.
7. low-power 정책: 배터리/고온/낮은 여유 RAM일 때 동시 decode/encode 숫자를 낮춤; 메모리 압박 시 standby buffer를 선제 축소.
8. 하드웨어 경로 실패 시 CPU encode가 deadline을 충족하면 폴백; 불가능하면 기존 소스 재생/일시 버퍼링.
9. 네이티브 macOS Runner를 추가할 경우 별도 tag(macos-arm64)와 protected credentials, 로그인 세션 요구 여부 및 유지보수를 검토. 기존 Oracle Runner를 변경하지 않는다.

## 벤치마크

| 측정 항목 | 테스트 |
|---|---|
| Decode speed | 1080p 24fps H.264/HEVC/AV1, 소스별 실측 |
| Encode latency | H.264 1080p 4s segment p50/p95 및 최대 |
| 2→1 merge | A/B 서로 다른 codec에서 동시 decode 및 one output encode |
| Switching | 10분 동안 여러 번 source switch, frame gap과 stall 확인 |
| 4K HDR | 별도 stretch goal, 테스트 불합격이어도 기본 1080p 실패 아님 |
| Memory/heat | unified memory peak, swap, CPU, energy, thermal state |

가설: 초기 목표는 1080p 24~30fps 단일 재생. 둘 이상 동시 4K encode를 제품 필수 기능으로 지정하지 않는다.

## 완료 게이트

- 해당 Mac M5에서 h/w path가 실제 선택됐다는 실행 로그와 ffprobe 결과.
- 동일 조건 CPU baseline 대비 latency, quality, power를 비교해 결과에 기록.
- macOS에서 지원하지 않는 codec/profile은 명시적으로 회피.
- AV sync, segment timestamp, HDR→SDR 톤매핑 오류 없는 fixture 검증.
