# 자동화·재현 가능한 테스트 매트릭스

이 문서는 구현 테스트의 **요구사항**이다. 아직 테스트 바이너리나 스크립트가 존재한다는 뜻이 아니다.

## 합법적 테스트 미디어 만들기

기본 소스는 FFmpeg의 testsrc2 및 sine source로 생성하는 **자체 제작 합성 영상**을 사용한다. 실습을 위한 개념 명령(환경별 FFmpeg 빌드 및 encoder 지원 확인 후 실행):

~~~bash
mkdir -p tests/fixtures/generated
ffmpeg -hide_banner -y -f lavfi -i "testsrc2=size=1280x720:rate=24:duration=120" \
       -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=120" \
       -c:v libx264 -pix_fmt yuv420p -g 48 -c:a aac -shortest \
       -movflags +faststart tests/fixtures/generated/reference.mp4
ffprobe -v error -show_streams -show_format -of json tests/fixtures/generated/reference.mp4
shasum -a 256 tests/fixtures/generated/reference.mp4
~~~

실제 CI에는 대용량 바이너리를 커밋하지 않는다. fixtures generator를 버전 관리하고 재생성한 입력/출력 SHA와 FFmpeg 버전을 결과에 기록한다. 타임라인의 ground truth를 고정하려면 기본 소스에 timecode/frame counter 및 간헐적 고유 이벤트를 넣는다.

## Fixture 세트

| ID | 설명 | 검증 |
|---|---|---|
| F01 | 동일 파일 바이트, 다른 tracker + Infohash/piece size | file bytes reuse, 타깃 piece 재검증 |
| F02 | 같은 제목·러닝타임, 다른 encoding | 같은 byte로 간주하지 않음 |
| F03 | H.264 24fps / HEVC 23.976fps 동일 컷 | alignment drift + 정상 switch |
| F04 | B 버전에 +350ms 로고 | offset mapping |
| F05 | B 버전에 2초 삽입/5초 삭제 | cut boundary 차단 |
| F06 | source B는 영상은 같고 audio가 120ms 지연 | primary audio continuity |
| F07 | 다른 SDR/HDR 색공간 | color normalize 및 metadata |
| F08 | ISO 24fps vs variable frame rate | PTS mapping |
| F09 | MP4 moov atom at end, MKV | index/cue priority |
| F10 | 일부 torrent pieces missing/corrupted | verified-only, no corrupted output |
| F11 | 악성 파일 경로 및 metadata 폭탄 | 파서 제한/샌드박스 |
| F12 | 재생 중 다중 seek, kill/restart | session generation, cache integrity |

## 네트워크 시나리오

- N01: 1 seed, 1 downloader / 정상 연결.
- N02: 같은 torrent에서 5 peers, 각 대역폭 다름.
- N03: 별도 Infohash의 서로 다른 source에서 각 1~5 peers.
- N04: primary의 다음 30초 다운로드 차단, alternate는 정상.
- N05: tracker unreachable, DHT-only / private policy 분리.
- N06: packet loss 1%/5%, RTT 100ms, bandwidth 1Mbps.
- N07: 모든 소스 0 peers, timeout 후 사용자에게 명시된 error.
- N08: tracker 일시 429/503 및 자원 재시도 제한.
- N09: IPv4-only/IPv6-only, NAT inbound 실패.
- N10: 3회 반복 재실행/seek 및 한 소스의 비정상 종료.

## 테스트 계층과 게이트

1. **Unit:** magnet parser, normalization, hash identity, timestamp transforms, source ranking, stale generation.
2. **Property/fuzz:** untrusted metadata/path/URL, time offset overflows, monotonic timeline, malformed chunks.
3. **Integration:** 제어되는 tracker/seed 환경에서 verified byte read & alternate failover.
4. **Media:** ffprobe로 output PTS/DTS 및 stream format, audio lag/crossfade, frame matching 검증.
5. **End-to-end:** localhost player로 movie start/pause/seek/source loss/manifest update.
6. **Platform:** Linux ARM64 Docker vs Mac M5 native FFmpeg/VideoToolbox 독립 증명.
7. **Perf:** baseline vs multi-source, 중앙값·p95, 악화된 지표 함께 공개.

## 실험 결과 포맷 예시

~~~json
{
  "schema_version": 1,
  "fixture_id": "F03",
  "network_profile": "N04",
  "platform": "macos-arm64",
  "mode": "multi-encode-switch",
  "git_sha": "<exact source SHA>",
  "ffmpeg_version": "<runtime reported version>",
  "startup_p95_ms": null,
  "stall_seconds": null,
  "useful_peers": null,
  "verified_goodput_bps": null,
  "source_switch_count": null,
  "av_sync_p95_ms": null,
  "cpu_p95_percent": null,
  "peak_memory_bytes": null,
  "result": "NOT_RUN"
}
~~~

모든 결과에서 NOT_RUN, SKIPPED, FAIL, PASS를 분리하고 테스트하지 않은 지표를 0으로 기입하지 않는다.
