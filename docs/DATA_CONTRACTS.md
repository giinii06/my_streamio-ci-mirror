# 데이터 타입, 상태 기계, API 계약 (설계 초안)

**주의**: 아래 스키마는 구현 요구사항이지 이미 존재하는 Rust 타입이 아니다. Phase 01~08에서 타입 정의와 호환성 테스트로 고정한다.

## 필수 도메인 ID

- ContentId: 사용자가 묶은 동일 작품의 논리적 ID; 파일 바이트 해시와 다르다.
- SourceId: 하나의 torrent 내 선택된 미디어 파일 ID. TorrentId + file index와 무관하게 안정화.
- SwarmKey: (protocol version, infohash), 격리된 피어 검색 단위.
- FileIdentity: 확정된 동일 바이트 파일을 나타내는 content digest 또는 검증 가능한 v2 file root와 length; provisional/verified 구분.
- PlaybackId: 재생 세션/seek generation.
- TimelineRevision: 정렬 결과 버전. 콘텐츠별 다중 인코딩 맵 변경 시 증가.
- SegmentId: (PlaybackId, generation, start_time, duration, profile, rendition). 외부 공개 뒤 변경 금지.

## 예시 Rust 타입

~~~rust
struct ContentGroup { id: ContentId, title: String, sources: Vec<SourceId> }
struct TorrentSource {
  id: SourceId, swarm: SwarmKey, file_index: u32,
  file_size: u64, piece_length: u32,
  codec_hint: Option<CodecHint>, trust: SourceTrust,
}
enum SourceTrust { Unknown, Candidate, VerifiedFileBytes }
enum AlignmentState { Unknown, Sampling, Aligned, Rejected }
struct TimelineMap {
  source: SourceId,
  reference: SourceId,
  segments: Vec<AffineMapSegment>, // ref_ms = a * src_ms + b
  confidence: f32,
  revision: u64,
}
enum PlaybackState { Created, Buffering, Playing, Stalled, Seeking, Recovering, Ended, Failed }
struct SegmentPlan {
  playback_generation: u64, source: SourceId,
  source_start_ms: i64, duration_ms: u32,
  output_profile: OutputProfile, deadline_ms: u64,
}
~~~

f32 confidence 하나만으로 안전을 판단하지 않는다. 겹친 샘플 수, offset residual p95, 타임라인 coverage, ambiguity flag도 필수 저장한다.

## 검증된 ChunkStore 프로토콜

- 시작 단계의 VerifiedChunk는 (SourceId, torrent-piece index, piece hash, length, storage ref)로 식별한다.
- 교차 토렌트 재사용 시 파일 전체 digest 또는 BEP52 root가 적합한지 판별하고, **타깃 피스 경계에 맞게 필요한 바이트를 재구성하여 타깃 검증 해시로 다시 검사**한다.
- 쓰기 경로: 임시 파일 → checksum 검사 → fsync 수준의 내구성 정책 적용 → atomic rename → DB metadata commit.
- 읽기 경로: 확인된 range만 read lease 허용. Eviction은 lease를 참조해 안전하게 삭제.
- 재생 중 완료되지 않은 피스/미검증 partial block은 segment에 제공하지 않는다.

## 주요 상태 전이

~~~mermaid
stateDiagram-v2
 [*] --> Created
 Created --> Buffering
 Buffering --> Playing: initial validated segment ready
 Buffering --> Failed: no viable source and timeout
 Playing --> Stalled: buffer below threshold
 Playing --> Seeking: seek received
 Stalled --> Recovering: alternative validated source ready
 Recovering --> Playing: new segment continuity verified
 Recovering --> Buffering: fallback buffering
 Seeking --> Buffering: generation increment
 Playing --> Ended: EOF reached
 Stalled --> Failed: all sources unavailable
 Ended --> [*]
 Failed --> [*]
~~~

동시에 처리되는 seek와 transcoding completion은 generation 비교로 구별한다. 예전 generation이 최근 세션의 HLS 매니페스트를 변경할 수 없다.

## HTTP 계약 (localhost 전용, v1 제안)

| Method | Path | 요청 | 성공 응답 / 오류 |
|---|---|---|---|
| POST | /api/v1/contents | title, explicit approved sources | 201 ContentId; 400/422 |
| GET | /api/v1/contents/{id} | none | source summary and status |
| POST | /api/v1/playbacks | content_id, profile, source_mode | 201 playback_id, urls |
| POST | /api/v1/playbacks/{id}/seek | position_ms, expected_generation | 202 new_generation; 409 stale |
| GET | /api/v1/playbacks/{id}/manifest.m3u8 | playback generation | 200 HLS; 404/409/503 |
| GET | /api/v1/playbacks/{id}/segments/{segment_id} | valid segment_id | immutable bytes; 404/410 |
| GET | /api/v1/playbacks/{id}/events | SSE | state, progress, reason |
| GET | /api/v1/metrics/summary | local auth | metrics snapshot |
| DELETE | /api/v1/playbacks/{id} | none | 204 cancellation |

보안: loopback bind, origin allowlist, local token/CSRF model, bounded request sizes. 원격 배포에서는 기본적으로 API를 공개하지 않는다. UI 서버와 API CORS는 명시적 출처만 허용.

## Event 계약 / 예시

~~~json
{
  "schema_version": 1,
  "event": "segment_ready",
  "playback_id": "p-123",
  "generation": 4,
  "segment_id": "s-456",
  "source_id": "src-b",
  "start_ms": 18000,
  "end_ms": 22000,
  "verified": true,
  "transcoded": true,
  "alignment_revision": 2
}
~~~

실시간 진행 스트림에는 peer IP, magnet 전체 URI 또는 트래커 인증 토큰을 보내지 않는다. 테스트 가능한 오류코드: INVALID_MAGNET, NO_METADATA, NO_VERIFIED_BYTES, ALIGNMENT_LOW_CONFIDENCE, CODEC_UNSUPPORTED, TRANSCODE_DEADLINE_MISSED, SEEK_SUPERSEDED, STORAGE_EXHAUSTED.

## 저장 계층

- SQLite: catalog, sources, alignment manifest, benchmark run, playback journal(선택).
- 디스크 캐시: 토렌트 다운로드 바이트와 완성된 segment, 별도 quota/GC.
- RAM: hot segment index/buffer, 제한된 decode queue. 상태가 변경되면 tracing span과 계측을 함께 발행.
