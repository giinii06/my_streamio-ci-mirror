# Phase 06 — 이종 인코딩 소스의 콘텐츠 타임라인 정렬

**의존:** 04(재생 가능한 기본 소스), 05(동일 파일 재사용은 선택적). **목표:** 코덱/FPS/컨테이너가 다른 둘 이상의 **실질적으로 동일한 컷**에서 동일 장면을 찾고, 합성이 안전한 시간 범위를 계산.

## 핵심 설계

"러닝타임과 해상도가 같다"는 정렬 증거가 아니다. 두 릴리스는 오프닝 로고, 검은 화면, 검열, 엔딩, 방송용 편집, FPS drift가 다를 수 있다. 잘못된 시간 일치는 스토리 점프·A/V 오프셋을 유발한다.

출력 타임라인은 PrimarySource의 **canonical media time**을 기준으로 한다. 다른 소스마다 (source_time → reference_time) piecewise affine mapping과 **검증된 coverage intervals**를 만든다.

## 정렬 처리 6단계

1. **미디어 프로브:** ffprobe로 stream index, codec/profile, width/height, fps(avg/r), timebase, PTS/DTS, duration, audio track, sample rate, color primaries/transfer, HDR metadata를 수집한다.
2. **샘플 획득:** 재생 시작+중반+끝과 seek 주변의 제한된 keyframe 구간을 가져온다. 아직 peer에게 없으면 후보를 Pending으로 남긴다. 전체 파일을 일괄 다운로드하지 않는다.
3. **지문 생성:** 비디오에서 대표 프레임을 소형 grayscale+perceptual hash 등으로 추출하고, 오디오가 호환되면 시간별 spectral landmark로 보조한다. 압축 노이즈·밝기·crop에 강한 다중 특징 필요.
4. **후보 정렬:** coarse offset을 cross correlation / nearest-match에서 추정한 뒤 RANSAC/동적 정렬로 outlier 제거. source_time과 ref_time의 local slope, drift, split/cut 판정.
5. **검증:** 서로 다른 시간대의 independent samples로 편차, coverage, ambiguous repeating sequences(검은 화면/크레딧/반복 영상)를 검증한다.
6. **타임라인 저장:** confidence, window-level residual, valid intervals, candidate exclusions, revision을 versioned manifest로 저장. 새 지문으로 수정되면 revision 증가, 이미 공개한 segment 불변.

## 구간별 조합 허용 조건

- 정렬된 두 구간이 동일 영상 내용임을 검증할 충분한 독립 샘플 존재.
- frame/scene offset residual p95가 임계 이내(초기 실험: 1 video frame 이내를 목표; 별도 benchmark로 조정).
- 동일 구간 안에서 timestamp가 monotonic, 컷/삽입/삭제로 mapping 단절이 없음.
- 서로 다른 censored edit/alternate scene, logo boundary, credits mismatch를 감지하면 그 구간 **전환 금지**.
- 소스의 중요한 audio mix/언어가 달라도 오디오는 primary에 고정하고 비디오 일치 여부 독립 검증.
- fingerprint가 충분치 않으면 Pending/LowConfidence; '완벽히 동기화됨' 가정 불가.

## 데이터 구조 예시

~~~rust
struct AlignmentWindow {
    source_start_ms: i64,
    source_end_ms: i64,
    ref_start_ms: i64,
    ref_end_ms: i64,
    affine_slope: f64,
    affine_offset_ms: f64,
    p95_error_ms: f32,
    sample_count: u32,
    ambiguous: bool,
    approved_for_switch: bool,
}
struct AlignmentEvidence {
    ref_source: SourceId,
    alternate_source: SourceId,
    windows: Vec<AlignmentWindow>,
    revision: u64,
}
~~~

## 테스트 데이터 세트

- 같은 컷: H.264 23.976fps vs HEVC 24fps; 로고 +350ms; 오디오 시작 +120ms.
- 30fps vs 24fps로 프레임 중복/제거. SDR 색보정 vs HDR 톤 차이. 자막 burn-in. 검은 장면 20초.
- 편집 컷 2초 삽입 및 5초 삭제, 엔딩 잘림, fade 변경, 다른 aspect ratio/crop.
- 가짜 높은 유사도: 반복 배경/동일 장면 다른 시간. 반드시 false positive를 측정.

## 완료 게이트

- ground truth와 추정 매핑 오차 분포를 fixture마다 수치로 기록.
- 정렬 검증에 실패한 alternate source의 전환 허용 구간은 0개.
- 여러 anchor에서 드리프트를 계측하고 단일 offset 가정이 틀린 케이스를 정확히 분리.
- fingerprinting 계산량(초/재생 시간, RAM, 디스크, 피어 전송 비용)을 측정.
