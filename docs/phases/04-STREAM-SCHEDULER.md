# Phase 04 — 검증 피스 기반 스트리밍·버퍼 스케줄러

**의존:** 03. **목표:** 아직 파일 전체를 받기 전에 단일 인코딩 영상을 재생하면서 seek, rebuffer, 실패 복구를 제어한다. **다른 인코딩 조합 전의 반드시 성공할 베이스라인**이다.

## 데이터 파이프라인

~~~mermaid
flowchart LR
 P[Playback clock] --> W[Sliding time window]
 W --> I[Container index/time-to-byte map]
 I --> R[File byte-range demands]
 R --> M[Piece mapper / deadline queue]
 M --> B[TorrentBackend]
 B --> V[VerifiedStore]
 V --> H[HTTP Range / initial player]
 H --> P
~~~

## 핵심 구현

1. 컨테이너의 moov/mdat 위치와 keyframe/index 정보를 ffprobe로 조사한다. 파일 메타데이터가 끝부분에 있으면 초기 우선 요청 목록에 그 범위도 포함한다.
2. 시간 → 바이트 범위 매핑은 컨테이너·VFR·키프레임에 의존한다. **시간을 전체 길이로 나눈 선형 비율**은 fallback 추정에만 사용하고 정확성 주장 금지.
3. 요구 구간을 즉시 재생(앞 0~6초), 다음 버퍼(6~30초), seek 후보(과거/미래)로 구분하고 priority/deadline을 다르게 설정.
4. torrent v1에서 피스는 여러 파일 경계를 걸칠 수 있다. 선택 파일 외 주변 바이트를 포함해 타깃 피스 무결성 검사가 가능하도록 내부 스토어에서 처리한다.
5. 다운로드가 늦을 때 미검증 영역을 200 OK로 내보내지 않는다. Range 처리 중 readiness 대기 또는 명시적 503/206 계약을 고정한다.
6. 백프레셔: queued segment/piece read 작업, in-flight reads, disk cache quota, network peer slots 상한.
7. 사용자가 seek하면 generation 증가, 이전 deadline 취소/강등, 현재 시간 window 재계산, 기존 유효 버퍼는 lease 후 재활용.
8. 실패한 스웜 탐색은 플레이어 요청 스레드를 막지 않는다. 단일 소스 부하 측정 baseline 별도 저장.
9. 플레이어 연속성이 확보되면 single-source baseline을 freeze하고 추후 MultiSwarm이 최소한 그 성능을 유지하는지 비교.

## Scheduler 입력·출력 (설계)

~~~text
Input:
  PlaybackDemand(time_ms, ahead_ms, preferred_profile, generation)
  ContainerIndex(keyframes, timestamps, byte_ranges)
  SourceHealth(verified_goodput, range_coverage, error_rate)
Output:
  PieceDemand(source, piece_index, priority, deadline, generation)
  BufferState(ready_until_ms, waiting_ranges, stall_reason)
  PlaybackDecision(serve | buffer | seek | fail)
~~~

## 실패와 경계

- 0피어, 피스 영구 누락, 다운로드 대기 timeout, 중간 pause, 반복 seek 50회, 나쁜 메타데이터, MP4 헤더가 끝에 있는 경우.
- 컨테이너에 인덱스가 없는 경우: remux 또는 명시적 미지원; 허위 시간 매핑 금지.
- 전송량 측정은 실제 client가 받은 bytes가 아니라 verified data delivery와 플레이어 소비율을 별도 집계.
- source playback 취소 후 느리게 도착한 callback은 이전 generation이라 무시한다.

## 테스트 게이트

- seed 2개 제어 환경에서 완성 다운로드보다 앞서 동영상 재생 가능.
- seek 이후 요청된 새 구간의 검증된 바이트만 제공.
- torrent hash mismatch를 감지해 corrupt 데이터 재생 0건.
- 테스트 기록에 initial startup p50/p95, stall_seconds, goodput, RAM, seek latency 기록.
- baseline의 원본 세그먼트와 재생된 출력 hash/PTS가 참조와 일치하는지 확인.
