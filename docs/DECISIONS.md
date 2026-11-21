# 아키텍처 결정 기록(ADR-lite)

이 기록의 **Proposed**는 확정이 아닌 검토 제안이다. Phase 게이트 통과 후 Accepted/Rejected 및 근거 커밋/벤치마크 ID를 기입한다.

| ID | 제안 | 상태 | 근거 / 검증 |
|---|---|---|---|
| D-001 | Rust daemon + 로컬 HTTP player-first | Proposed | 플랫폼 분리, 테스트 편의; GPUI는 미래 선택 |
| D-002 | TorrentBackend 추상화, librqbit vs libtorrent 비교 | Proposed | libtorrent piece deadlines vs librqbit Rust 네이티브 |
| D-003 | 스웜당 독립 연결 풀 유지 | Proposed | BitTorrent infohash 경계 준수 |
| D-004 | verified bytes only → playback | Proposed | 악성 피어/오염 출력 방지 |
| D-005 | 영상 fingerprint + confidence 기반 alignment | Proposed | 길이·해상도 일치만으로 부정확 |
| D-006 | 오디오 한 소스 고정, 비디오 구간만 전환 | Proposed | A/V sync·잡음 회피 |
| D-007 | 서로 다른 코덱은 우선 HLS/fMP4 공통 프로필 변환 | Proposed | 브라우저 파이프라인 연속성 |
| D-008 | 배포는 Linux Docker, M5 가속은 native macOS | Proposed | VideoToolbox 접근성 별개 |
| D-009 | 변환 실패나 불확실한 정렬은 single-source fallback | Proposed | 안정성 > 이론적 peer 수 |

## 검토해야 할 반례

1. 여러 Infohash가 같은 영상 제목을 가졌지만 서로 다른 컷/로고/검열 버전일 수 있다.
2. v1 메타데이터의 조각 경계와 v2의 file root 개념은 다르다. 바이트를 물리적으로 재사용해도 타깃 토렌트의 검증은 생략할 수 없다.
3. 피어가 많아도 마지막에 필요한 조각이 없는 '희귀 구간'에서는 재생이 멈춘다.
4. 새로운 소스의 fingerprint를 얻기 위해 필요한 seek 근처 데이터가 아직 없다면 switch 비용이 커진다.
5. 재인코딩 비용이 내려받는 이득보다 높으면 전환하지 않는 것이 최적이다.
6. 디코더 H/W 지원 여부와 encode 품질·동시 처리량은 기기/OS/코덱/프로필별로 실측해야 한다.

## 참고 문서(개발 중 API 확인)

- libtorrent Torrent Handle / piece deadline: https://libtorrent.org/reference-Torrent_Handle.html
- libtorrent sequential-mode caveat: https://libtorrent.org/reference-Core.html
- librqbit library documentation: https://docs.rs/librqbit/latest/librqbit/
- BitTorrent v2 BEP52: https://www.bittorrent.org/beps/bep_0052.html
- Apple VideoToolbox: https://developer.apple.com/documentation/videotoolbox
- HLS RFC 8216: https://www.rfc-editor.org/rfc/rfc8216

의존성 버전 및 지원 API는 Phase 02/09 실제 빌드에서 다시 검증한다.
