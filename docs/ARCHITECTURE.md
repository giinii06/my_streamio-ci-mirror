# 시스템 아키텍처 (설계안)

## 시스템 경계

사용자가 합법적으로 확보한 magnet/.torrent 또는 직접 제작한 소스 목록을 입력한다. 하나의 작품(ContentGroup) 아래 복수 토렌트(Source)를 등록한다. 각 토렌트는 **고유 Infohash별 독립 스웜**을 가진다. Peer Discovery가 찾은 피어를 합산해 하나의 BitTorrent 스웜으로 만드는 것은 프로토콜상 지원하지 않는다.

~~~mermaid
flowchart TD
 U[Browser Player / Local API] --> A[Playback Controller]
 A --> C[Content Catalog + Identity]
 C --> S[Multi-Swarm Manager]
 S --> T1[Torrent session hash A]
 S --> T2[Torrent session hash B]
 S --> T3[Torrent session hash C]
 T1 --> P[Verified Chunk Store]
 T2 --> P
 T3 --> P
 A --> D[Deadline/Buffer Scheduler]
 D --> S
 A --> L[Timeline Alignment]
 L --> Q[Segment Planner]
 Q --> X[FFmpeg / VideoToolbox if necessary]
 P --> Q
 X --> H[HLS / fMP4 or Range HTTP]
 H --> U
 S --> O[Telemetry and Backpressure]
 D --> O
 L --> O
 X --> O
~~~

## 권장 프로세스·모듈 레이아웃

~~~text
my_streamio/
  Cargo.toml
  crates/
    domain/           # 콘텐츠 정체성, 시간, 이벤트, 상태
    torrent-adapter/  # 구현체 독립 TorrentBackend trait
    discovery/        # magnet validation, DHT/tracker/PEX 관측
    catalog/          # ContentGroup 및 source metadata
    verified-store/   # 검증된 범위, cache/lease/eviction
    scheduler/        # deadline, fairness, seek, backpressure
    timeline/         # fingerprint, offsets, alignment confidence
    media-pipeline/   # ffprobe, segment plan, ffmpeg process
    playback/         # manifest/segment session and fallback
    api/              # localhost axum endpoints
    telemetry/        # structured metrics/events
  apps/
    daemon/           # my-streamiod 실행 파일
    web/              # 선택: 정적 웹 플레이어 UI
  tests/fixtures/     # 합성 비디오 원본/생성 스크립트(대형 바이너리 제외)
  docs/
    phases/
~~~

이 구조는 **제안**이다. 실제 파일은 해당 Phase 구현 시 생성한다. Cargo workspace에 모든 모듈을 선행 생성하지 않는다.

## 책임 구분과 비동기 경계

| 구성요소 | 소유 상태 | 입력 → 출력 | 실패 시 조치 |
|---|---|---|---|
| Catalog | 작품·소스 메타데이터 | 사용자 입력 → Source | 잘못된 매칭 거부 |
| TorrentBackend | session/peer/piece/metadata | infohash+deadline → VerifiedBytes | 해당 소스만 격리·재시도 |
| MultiSwarmManager | 활성 소스·budget | 소스 후보 → 활성 세션 | LRU / 우선순위 동적 축소 |
| VerifiedStore | 해시 검증된 바이트 | 쓰기 검증 → 읽기 lease | 불일치 폐기 |
| Scheduler | timeline demand → source request | playback clock → piece deadlines | 즉시 단일 소스 폴백 |
| Alignment | 시간대응 맵 | decoded samples → offset/drift map | confidence 낮으면 전환 금지 |
| Media Pipeline | ffmpeg process + segments | 입력 A/B → 출력 segment | 작업 취소, 기존 segment 유지 |
| Playback | 세션/manifest | ready segments → HTTP response | 중단된 세션만 복구 |
| Telemetry | 벤치마크·오류 보고 | 상태 이벤트 → 집계 | 측정 오류가 재생 방해 금지 |

## '피어 최대화' 해석

- 발견된 peer endpoint 수와 실제 connected peer 수를 분리한다.
- useful peer는 지난 관측 창에서 **필요한 조각의 검증 바이트를 전달**한 피어다.
- 같은 IP:port라 해도 Infohash가 다르면 자격/가용 piece가 독립적이다.
- 더 많은 스웜 등록은 DNS·tracker·DHT·메모리·FD·CPU·디스크 지연을 증가시킨다.
- Scheduler는 활성 스웜을 무제한 늘리지 않는다. **접속할 스웜은 유효 goodput 기대값과 필수 시간 구간 가용성**을 기준으로 선택한다.

## 출력 미디어 전략

1. Phase 04: 원본 파일의 검증 구간을 Range HTTP로 제공(재생 가능한 MP4 등만). 헤더나 인덱스가 파일 끝에 있는 경우 별도 처리.
2. Phase 07: 이종 인코딩/컨테이너는 segment boundaries에서 **하나의 정상 출력 프로필**(예: H.264/AAC SDR 1080p)로 변환한 HLS/fMP4를 제공. 각 구간의 PTS/DTS, keyframe, color, audio 기준을 강제.
3. 트랜스코딩이 필요 없는 호환 세그먼트라도 타임스탬프·컨테이너 재패키징 검증을 거친다.
4. VideoToolbox 미지원, FPS 부족, alignment 미충족 → **동일 소스 연속 재생 또는 중단을 명시**. 깨진 비디오를 조용히 이어 붙이지 않는다.

## 리소스 예산·플랫폼

최초 실험 가설: 기본 활성 스웜 3개/최대 8개(설정값), player ahead buffer 15~45초(네트워크에 따라 적응), RAM 상한 및 disk cache 상한을 별도 부여. 모두 벤치마크에서 조정해야 할 **초기값**이고 SLA가 아니다. 배터리 사용 중, 저전력, 로컬 SSD 여유 부족이면 자동 감축.

Linux ARM64 GitLab Runner는 Cargo unit/integration 및 합성 네트워크 시험용. macOS 네이티브 VideoToolbox는 Mac M5 전용 별도 실행/Runner에서만 검증. Docker Linux 컨테이너와 macOS 하드웨어 미디어 엔진은 독립 경로다.

## 필수 불변식

- 렌더링에 투입되는 바이트는 반드시 소스 별 BitTorrent 무결성 검사 또는 검증된 로컬 콘텐츠 해시를 통과한다.
- 서로 다른 인코딩의 압축 비트스트림은 torrent-piece 수준으로 혼합하지 않는다.
- 같은 파일이더라도 각 타깃 torrent의 piece/hash 조건을 통과하기 전에는 재사용 성공으로 간주하지 않는다.
- 재생 중 이미 클라이언트에 공개한 segment ID와 바이트는 불변이다.
- 실패한 재인코딩이나 잘못된 정렬은 재생 성공 이벤트를 발생시키지 않는다.
- seek나 session cancel 이후 구 작업이 새 세션에 segment를 덮어쓰지 못한다(generation fencing).
