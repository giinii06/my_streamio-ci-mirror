# Phase 10 — 피어·대역폭·버퍼 최적화 및 벤치마크

**의존:** 03~09. **목표:** 많은 피어 수가 실제 스트리밍 품질 향상으로 이어지는지 재현 가능한 시험으로 판단한다.

## 핵심 지표 분리

- Found peers: discovery 결과 endpoint 수. 중복·연결 불가 포함.
- Connected peers: 실제 프로토콜 접속 성공. 해당 구간 데이터 소유를 보장하지 않음.
- Useful peers: 최근 window에서 **검증된, 실제 필요한 미디어 바이트**에 기여한 연결.
- Network goodput: 실제 유효 네트워크 수신 바이트/초. 동일 캐시 재사용 바이트 포함하지 않음.
- Segment readiness: 마감 시간 전에 검증·변환·공개된 segment 비율.
- Startup latency, rebuffer count/duration, seek p95, active source switch count, failed switch %, A/V sync residual.
- CPU%, RSS/peak unified memory, swap, disk read/write amplification, cache hit rate, energy/thermal state.

## 실험 설계

비교군 A/B/C의 샘플 수와 반복 횟수를 일정하게 한다. 시드·네트워크 조건과 모든 설정을 기록.

| 실험군 | 구성 | 기대 검증 |
|---|---|---|
| A | 단일 정보 해시, 기본 piece deadline | baseline |
| B | 단일 정보 해시, 튜닝된 scheduler | piece scheduling 순수 이득 |
| C | 동일 바이트 파일, 복수 Infohash | 추가 스웜의 실질 기여 |
| D | 서로 다른 인코딩, 정렬·switch off | 병렬 연결 overhead |
| E | 서로 다른 인코딩, 정렬·switch on | 영상 결합 순이득 |
| F | E + M5 하드웨어 경로 | 처리량 및 에너지 순이득 |

- 각 실험은 5회 이상 반복을 초기값으로 두고, seed/나쁜 네트워크 스케줄을 고정해 비교한다.
- 네트워크 프로필: 1/3/10/30 Mbps, 50/100/200ms RTT, 0/1/5% loss, 특정 source 30초 단절.
- controlled test peers에서 각 source의 다운로드 기여를 별도로 계측.
- policy 변경 전/후 throughput뿐 아니라 stall, start latency, CPU, disk, memory 함께 비교.
- 노이즈가 큰 public swarm에서 수행한 시험은 통제된 실험 결과와 별도 표시.

## 최적화 항목(순서)

1. metadata 획득 병목/비효율적인 tracker 재시도 제거.
2. 재생 window 내 piece availability와 delivery deadline 예측 개선.
3. useful peer 기반 pool resize, 가중치·hysteresis 튜닝.
4. 중복 다운로드/검증·fingerprint 반복 최소화.
5. warm source 수, segment 크기, transcode concurrency 최적화.
6. disk cache eviction 및 오버다운로드 비율 관리.
7. 재생 실패를 악화시키는 최적화는 롤백.

## 통과 조건

- 실험군 E가 A보다 성능 개선을 보인다는 검증 가능한 데이터를 제공하거나, 이점이 없음을 정직하게 기록하여 feature를 제한.
- 사용자 UX 개선 없이 단순 peer count만 증가한 결과는 **성공으로 분류하지 않음**.
- 각 정책 버전의 benchmark JSON, 시스템 정보, source fixture checksum, raw events와 분석 보고서를 보존.
