# Phase 08 — 재생 API, 사용자 인터페이스 및 디버깅

**의존:** 04(single source), 07(multi-encode). **목표:** 사용자가 '어떤 토렌트가 재생되고 있는지', '왜 전환했는지', '왜 버퍼링 중인지'를 볼 수 있는 로컬 플레이어.

## 제품 표면

- 기본 UI: localhost 웹 페이지(Chrome/Safari에서 테스트). 앱 창/GPUI는 UX 및 native packaging 이유가 명확할 때 추후 결정.
- 사용자 워크플로: magnet 또는 .torrent 수동 추가 → 파일 선택 → 여러 파일을 동일 ContentGroup으로 연결 → source 검증 상태 → Play.
- 화면: playable source, codec/resolution/audio, found/connected/useful peer 구분, 유효 goodput, ready buffer seconds, 현재 active source, segment switch count, speed/risk reason, cache 사용량.
- 조작: pause/resume, seek, 강제 primary-only, 자동 multi-swarm 모드, 실패 정보, 디버그 번들 export(민감정보 제거).
- 플레이어 세션마다 output profile 선택: Auto / compatibility(AVC SDR) / passthrough(where supported). 실제 codec support는 브라우저 capability 검사 결과로 표시.
- 공개할 수 없는 저작권 콘텐츠 카탈로그/검색 인덱스, 우회 기능은 MVP에 포함하지 않는다.

## Local API

docs/DATA_CONTRACTS.md의 REST/SSE 계약을 기준으로 구현. 상태 변화는 source_discovered, metadata_ready, peer_health, buffer_changed, segment_ready, source_switched, stall_started/ended, playback_failed로 나눈다.

- POST /contents: 최대 소스 수/매그넷 크기 제한, path traversal 방지, 원본 magnet 비노출.
- POST /playbacks: primary source 검증 전에는 명확한 202 buffering/409 conflict 계약.
- GET /manifest.m3u8: 재생용 token/세션 권한 검사. 세그먼트에 오래된 세션 토큰 허용 범위를 최소화.
- GET /events: 재접속 cursor, 과도한 이벤트 rate limiting, session 삭제 시 종료.
- /metrics/summary: localhost loopback 제한 및 접속 제어. 상세 peer IP는 기본적으로 미노출.

## 재생 UX 실패 설계

- Loading metadata / Finding peers / Buffering / Playing / Waiting on alternate / Unavailable / Source mismatch를 별도 표시.
- source 전환이 없는 건 '오류'가 아니다. 안전하게 primary를 유지한 이유(동일 구간 부재, 재인코딩 부하 등)를 표시한다.
- 영상이 곧 끝나는 상황에서는 alternate warm-up이 더 비싸면 탐색을 축소한다.
- seek 시 이전 segment가 UI에 갱신되지 않도록 event generation 체크.
- 사용자 수동 source 선택이 여러 번 겹치면 마지막 명령만 우선한다.

## UI 수용 테스트

1. 1·3·8소스 입력/삭제, 잘못된 magnet, 중복 infohash.
2. pause/seek(앞/뒤/처음/마지막), UI reload, daemon restart.
3. 0 peer, tracker timeout, file checksum failure, transcoding failure, source mismatch.
4. Source switch 과정에서 화면·오디오가 어긋나지 않고 debug event에 이유가 나타남.
5. Safari/Chrome/macOS 브라우저의 HLS 지원 차이 확인; 미지원 브라우저는 가능한 HLS player fallback을 명확히.
6. 키보드 접근성, text-only 상태 로그, dark mode, 재생 실패 메시지 명확성.

## 완료 게이트

- 사용자가 단일 페이지에서 소스 추가 → 재생 → 상태 진단 → seek → 종료까지 가능.
- 시스템은 발견 피어 수와 실제 전송 기여 피어 수를 절대 같은 지표로 표시하지 않는다.
- UI에 암묵적 성공(실패를 숨김)이 없고 모든 상태 변화의 이유 코드를 확인할 수 있다.
