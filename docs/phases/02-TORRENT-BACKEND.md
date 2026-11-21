# Phase 02 — TorrentBackend 기술 선택과 바이트 무결성

**의존:** 01. **목표:** Rust 앱이 실제 BitTorrent 데이터에 접근할 수 있는 가장 작은 안정적 어댑터를 선택한다. libtorrent C++ 래퍼와 librqbit 중 하나를 PoC 후 채택.

## 기술 선택 스파이크(둘 다 작성, 비교 후 하나만 유지)

### 후보 A: libtorrent-rasterbar 2.x

- libtorrent 세션 및 토렌트 핸들을 C++ shim에서 소유. Rust에는 opaque handle, 명시적인 안전한 byte buffer 복사, bounded event queue만 노출.
- C ABI 또는 cxx를 조사하되 **BEP52, set_piece_deadline, priorities, peer data, cancel** 지원을 실행으로 확인한다.
- callback에서 Rust 비동기 런타임으로 무제한 재진입하지 않는다. native destructor / cancellation / shutdown 순서 문서화.
- libtorrent에서 'sequential mode' 대신 piece deadline 등 스트리밍용 API 사용을 평가한다.
- 장점 후보: torrent-level 정책 및 rich scheduling. 단점 후보: C++ 빌드, mac/linux 배포와 FFI 메모리 안전.

### 후보 B: librqbit

- 순수 Rust 라이브러리로 workspace 통합, 세션 API/스트리밍 인터페이스/peer progress 정보 조사.
- **필수 API(피스 단위 요청 마감, 검증된 바이트 읽기, 파일 우선순위, piece map, discovery metrics)** 제공 여부를 직접 검증한다.
- 추상화 내부에서 지원하지 않는 기능은 빈 값을 반환/기능을 끈다. fake API나 문서에 없는 symbol을 가정하지 않는다.
- 장점 후보: 빌드·메모리 안전 단순화. 단점 후보: 정교한 piece scheduling/metadata 제약이 있을 수 있음.

## 인터페이스 제안

~~~rust
#[async_trait]
trait TorrentBackend: Send + Sync {
    async fn add_source(&self, magnet: ValidatedMagnet) -> Result<TorrentHandle>;
    async fn metadata(&self, handle: &TorrentHandle) -> Result<TorrentMetadata>;
    async fn read_verified(&self, file: SourceFile, range: ByteRange,
                           deadline: Instant, cancel: CancellationToken)
                           -> Result<VerifiedRead>;
    async fn set_piece_demands(&self, torrent: TorrentId,
                               demands: Vec<PieceDemand>) -> Result<()>;
    async fn peers(&self, torrent: TorrentId) -> Result<PeerStats>;
    async fn remove_source(&self, torrent: TorrentId) -> Result<()>;
}
~~~

인터페이스는 설명용 pseudo-Rust이며 실제 trait는 async trait object safety/ownership 성능을 고려해 검증·조정한다.

## 검증 시나리오

1. 제어된 tracker와 seed 2개, 다운로드 1개를 실행하고 64MiB 합성 파일 SHA-256을 일치시킨다.
2. 같은 byte range를 두 번 요청할 때 캐시 적중과 다운로드 비용 분리.
3. 마지막 토렌트 피스, 파일 경계를 가로지르는 v1 piece, v2-only/hybrid(지원할 경우), 무효 메타데이터.
4. 타임아웃, cancel, peer disconnect, metadata 중단, 디스크 부족, 네트워크 소실 이후 재개.
5. Linux aarch64 + macOS arm64 빌드 실측, native 메모리 누수/FD 누수 계측.
6. 지정 구간이 존재하지 않으면 정확한 실패를 돌려야 하며 partial/unverified bytes를 노출하지 않는다.

## 선택표(테스트 후 기입)

| 지표 | libtorrent 2.x | librqbit |
|---|---|---|
| 검증된 랜덤 구간 읽기 | TBD | TBD |
| 데드라인 기반 피스 요청 | TBD | TBD |
| V1/V2/hybrid 지원(요청 테스트별) | TBD | TBD |
| peer 가용성 관측 | TBD | TBD |
| CI Docker ARM64 빌드·시간 | TBD | TBD |
| macOS ARM64 빌드·시간 | TBD | TBD |
| cancel/recovery 메모리 안정성 | TBD | TBD |

## 성공 게이트

- 채택한 구현체에서 다운로드한 **검증 바이트**를 range API로 반환하는 통합 테스트가 성공한다.
- 성공 결과와 미지원 기능을 표로 남기고 ADR D-002를 Accepted로 변경한다.
- 미통과 시 Phase 03~05 확장 금지, feature를 축소하거나 backend를 전환한다.
