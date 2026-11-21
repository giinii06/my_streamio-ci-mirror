# Phase 03 — Peer Discovery, Multi-Swarm 등록 및 예산

**의존:** 02. **목표:** 사용자가 제공한 복수 magnet에서 DHT/Tracker/PEX/LSD 등 실제 지원되는 검색 수단을 활용하고, '찾은 피어'가 아닌 **유효 다운로드 공급량**을 측정한다.

## 소스 등록

- magnet URI 파싱: 허용 scheme/xt/Infohash 버전, 최대 길이, tracker URL 수, encoding/percent escapes, 중복을 제한한다.
- 동일 Infohash는 하나의 session으로 병합하고, 개별 원본 magnet에서 추가 tracker만 정책 검토 후 반영한다.
- 파일 제목, media metadata, 사용자 검증 없는 작품 자동 병합은 금지. Source → ContentGroup 연결은 사용자의 명시 선택 또는 Phase 06 검증 후에만 가능하다.
- tracker 인증 토큰이 포함된 magnet은 로그에서 query를 마스킹하고 외부 API에 원문을 표시하지 않는다.

## 검색 메커니즘과 제약

1. Metadata bootstrap: DHT 및 magnet 내 tracker → metadata 획득 → 크기/파일/조각 수 제한 검증.
2. Peer discovery: tracker announces, DHT lookup, 지원되는 peer exchange(PEX), 같은 LAN에서만 LSD, IPv4/IPv6, TCP/µTP.
3. 각 수단의 기능 지원 여부는 선택 Backend 실제 API에 따라 활성화. 무조건 모든 수단 지원이라고 주장하지 않는다.
4. 인바운드: 기본은 기존 OS/공유기 설정 유지. NAT-PMP/UPnP/PCP로 포트를 자동 개방하는 기능은 **명시적 opt-in**, 기본 off; 실패해도 아웃바운드 동작.
5. 트래커/피어 대상 rate limit, exponential backoff + jitter, 동시 연결 상한, 요청 실패 회로 차단.
6. private torrent flag 또는 비공개 tracker 정책을 우회하기 위해 DHT/PEX를 강제로 켜지 않는다.

## MultiSwarmManager 알고리즘(초기 제안)

- 상태: Dormant → Discovering → Connected → Supplying → Cooldown/Failed, 세션별 timeout/cancel.
- 주기적으로 다음 정보를 추적: discovered endpoints, connected peers, peers sending verified bytes/10s, verified goodput EWMA, piece coverage near playback window, RTT estimate(가능할 때), reconnect error %, cost/FD count.
- 사용자 소스 수가 100개여도 전부 상시 다운로드하지 않도록 탐색과 활성 다운로드를 분리.
- 시작 가설: 최대 8 active torrents, 3 hot contributors, 1 discovery cycle당 최대 N개; 설정 가능한 수치로 두고 Phase 10에서 결정.
- 승격: 다음 재생 구간의 verified throughput 및 coverage가 높은 스웜 우선. 강등: 의미 있는 기여가 없고 유지비가 높은 스웜.
- 공정성: 탐색 budget을 한 토렌트가 독점하지 못하도록 per-swarm quota와 aging 포함.

~~~text
every scheduler_tick:
  for each source under connection cap:
    snapshot = backend.stats(source)
    usable = verified_goodput + needed_range_availability - connection_cost
  keep best contributors with hysteresis
  promote a standby only if improvement exceeds threshold for hold time
  enforce hard peer/session/FD/RAM/bandwidth limits
~~~

## 테스트 및 수용

- 3개의 Infohash·서로 다른 seed 가용성 테스트에서 각 스웜의 피어와 바이트 기여를 **별도** 기록.
- 100개의 중복 magnet 입력에도 같은 Infohash 세션이 100개 뜨지 않음.
- tracker 429/503/timeout, DHT 응답 없음, IPv6-only, NAT 실패, private torrent 제한 시 무한 재시도·누수 없음.
- rate limit를 넘지 않고 스웜을 동적으로 줄이며 단일 스웜으로 안전하게 폴백.
- metric에 '발견 250명 = 다운로드 가능한 250명' 식의 수치 혼동이 없음.

## 산출물

crates/discovery, session registry 및 metric endpoint; source별 상태 전이 테스트; 실제 backend가 노출할 수 없는 지표를 문서에 명시한다.
