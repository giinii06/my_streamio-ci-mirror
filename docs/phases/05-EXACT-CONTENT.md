# Phase 05 — 별도 Infohash 간 동일 바이트 파일 재사용

**의존:** 04. **목표:** 토렌트 A, B가 서로 다른 스웜이더라도 **파일 자체가 완전히 동일**하다면 한쪽에서 내려받은 검증 바이트를 다른 토렌트의 검증 요구에 맞춰 재사용한다.

## 반드시 구분할 것

- **동일 영화**: 시청 콘텐츠 의미상 같을 수 있다. 파일 바이트·인코딩은 다를 수 있음.
- **동일 해상도/러닝타임**: 동일 파일 증거가 절대 아님.
- **동일 file content digest**: 동일 바이트 가능성을 강하게 입증. v2 file root + length는 프로토콜 맥락에서 파일 식별에 유용.
- **동일 토렌트 Infohash**: 같은 torrent swarm에 속하며 일반 BitTorrent의 기본 멀티 피어 다운로드가 적합.

## 인식·검증 절차

1. 다른 토렌트에 같은 파일명이 있다는 이유만으로 alias 하지 않는다.
2. v2-only/hybrid 파일은 BEP52의 file root와 file length 비교. 단, 실제 지원 여부와 메타데이터 정합성 검증 필수.
3. v1 또는 mixed 케이스에서는 무결성 검증된 바이트를 점진적으로 수집하고 **전체 파일 digest**를 계산한 후 강한 alias를 형성하는 경로를 제공한다. 전체 파일을 아직 확보하지 못했으면 speculative 동일성 표시만 하고 검증 없이 공유하지 않는다.
4. torrent A에서 verify된 실제 구간을 저장. torrent B가 그 범위를 요청하면 B의 piece alignment 및 (v1의 인접 파일 포함) piece proof를 충족할 만큼만 가져와 재검증한다.
5. 타깃 조각 전체가 검증되기 전까지 B의 '완료' 상태를 올리지 않는다. 스트리밍 범위의 sparse store는 최종 해시 증거와 provenance를 함께 관리.
6. 파일 version/hash mismatch나 읽기 race 시 shared alias를 폐기하고 소스별 독립 경로로 되돌린다.
7. 소스별 카운터로 network bytes와 reused verified bytes를 구분한다. 캐시 재사용량을 네트워크 다운로드 속도 향상으로 중복 계산하지 않는다.

## 제안 데이터 키

~~~text
VerifiedObjectId = hash(algorithm, digest, length)
TorrentPieceKey = (swarm_key, piece_index, expected_piece_hash, piece_length)
ReusableSpan = (verified_object_id, offset, length, provenance)
SourceToFileMap = (source_id, verified_object_id, mapping_confidence)
~~~

## 테스트 매트릭스

- 동일 바이트 영상 파일을 torrent A(다른 announce list/패딩)와 B(다른 piece length)로 생성.
- v1 multi-file torrent와 v2 file-aligned torrent(지원하는 backend)에 동일 파일이 들어 있는 경우.
- 같은 이름/길이·다른 바이트 1개 변형, SHA 일부만 알려진 경우, 해시가 다르게 계산된 경로 조작.
- A에만 0~20%, B에만 70~100% 구간이 있어도 검증 전 부분 결합을 허용하지 않는지 확인.
- alias 후 seek, cache eviction, process restart, 일부 파일 삭제, checksum 검증 실패.
- 두 토렌트의 피어가 자동 합쳐지는 것처럼 metric이 표현되지 않도록 테스트.

## 완료 게이트

- 동일 파일로 확정된 둘 이상의 스웜에서 cross-swarm verified cache hit가 실제 관측.
- 불일치 파일, 불완전 piece, 잘못된 캐시 index에서 corrupt output 0건.
- 재사용 이득(net network bytes saved)과 저장/검증 비용을 각각 계측.
- 원리상 재사용이 불가능한 경우 기존 Phase 04 단일 소스 재생을 그대로 유지.
