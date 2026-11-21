# Phase 11 — 입력·네트워크·미디어 파이프라인 보안

**의존:** 01 이후 전 기간. **목표:** 토렌트 메타데이터, peer, tracker 및 ffmpeg가 신뢰되지 않는 입력임을 전제로 공격면 제한.

## 위협 모델

- 악의적 magnet/트래커: 내부 주소 요청 유도(SSRF), 자격증명 유출, 통제되지 않은 URL, 무한 announce.
- 악의적 .torrent: 엄청난 파일 수, 중복 파일 경로, ../ path traversal, unicode normalization 충돌, 압축되지 않은 metadata bomb.
- 악의적 피어: piece hash mismatch, 재전송 폭주, 프로토콜 메시지 길이/타임아웃 악용, socket/FD 포화.
- 미디어 파일: 잘못된 MP4/MKV index, FFmpeg 디코더 취약점, 영구 프로브 대기, mem/CPU 폭주.
- 로컬 API: CSRF, DNS rebinding, 외부 네트워크 bind, 토큰 노출, path injection, 재생 세션 간 경합.
- 운영: 컨테이너로 비밀/호스트 전체 폴더 마운트, 불필요한 privileged socket, 잘못된 cache permissions.

## 필수 통제

1. URL allowlist 또는 공개 tracker 사용 정책을 명시하고 사설/loopback/IP literal 접근은 기본 차단한다. 사용자가 신뢰한 로컬 tracker를 쓰는 경우에만 별도 opt-in.
2. metadata size, torrent file count, file size, path length, magnet query count, HTTP request length, concurrent session 수를 제한.
3. torrent path를 절대 OS 경로로 사용하지 않고 내부 file index/안전한 저장 디렉터리로 매핑; 경로 escape 및 symlink 검증.
4. 검증되지 않은 bitfield, metadata, media bytes는 플레이어와 verified cache에 전달하지 않는다.
5. 외부 소스 FFmpeg는 고정된 인수 목록으로 subprocess 실행; shell string interpolation 금지. timeout/메모리/CPU/파일·프로세스 cap.
6. 기본 API 127.0.0.1 바인딩, origin/Host 검증, 로컬 인증 토큰, 세션별 capability URL. 원격 접속 시 별도 인증·TLS 설계/감사 전 배포 금지.
7. 비공개 tracker/peer IP/log에는 privacy redaction. 중앙 분석/telemetry는 기본 opt-in.
8. 데이터 저장: cache encryption은 threat model 결정 후; 적어도 owner-only permissions 및 데이터 삭제/기록 보유 정책 구현.
9. dependency audit, lockfile pin, FFmpeg 업데이트 정책, CI container image digest pin 검토.
10. automated tests에서 fuzz/rapid cancellation/race/size overflow, LRU eviction 중 read, stale generation publish를 포함.
11. NAT port mapping opt-in, UPnP/PCP/방화벽 변경은 사용자에게 가시화, 종료 시 소유한 mapping만 정리.
12. 법적 사용: 사용자가 권리를 가진 파일에 한정한 테스트/배포 안내, 임의의 DRM/지역 제한 우회 기능 미구현.

## 보안 수용 케이스

- 공격성 path/metadata가 앱 외부 경로에 파일을 쓰지 못함.
- 외부 웹사이트에서 loopback API 호출이 제한됨.
- FFmpeg 입력 timeout/악성 코덱 파일에서 daemon 전체 비정상 종료 없음.
- 어떤 실패에도 플레이어가 검증되지 않은 byte를 읽지 못함.
- 비공개 tracker 인증 정보가 일반 로그/bug report에 기록되지 않음.

## 완료 게이트

위협 모델/테스트 결과를 CI 보고서에 남기고, Critical/High 미해결 결함이 없을 때 릴리스 후보로 진행.
