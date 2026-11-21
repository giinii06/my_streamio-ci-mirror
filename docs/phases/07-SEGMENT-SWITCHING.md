# Phase 07 — 시간 구간별 소스 선택·트랜스코딩·연속 재생

**의존:** 06. **목표:** 다른 코덱의 영상을 타임라인으로 정렬한 뒤, 재생 구간 단위로 빠른 소스에서 데이터를 받아 **유효한 하나의 출력 스트림**으로 전달한다.

## 원칙

- A의 H.264 바이트 절반과 B의 HEVC 바이트 절반을 BitTorrent piece로 합치지 않는다.
- 서로 다른 인코딩의 video decoder output(frame) 또는 정상 segment 단위에서 합성한다.
- 여러 스웜의 피어를 합치는 것이 아니라 **각 스웜이 제공하는 별개 영상을 동기화해 필요한 시청 구간을 선택**한다.
- 스케줄러가 alternates를 모두 받더라도 전환 비용이 이득보다 크면 primary만 송출한다.

## OutputProfile(초기 가설)

- Baseline web playback: H.264/AVC SDR 1080p, 일정한 output timebase, AAC stereo, fMP4/HLS; 비디오 bitrate/품질은 실측으로 결정.
- HDR 및 4K는 추가 output profile, 색 메타데이터·톤매핑·A/V sink 지원 점검 전까지 비목표.
- 오디오는 원칙적으로 PrimarySource track 유지. 다른 소스로 오디오까지 스위치하는 고급 기능은 별도 실험/검증 게이트.
- source codec, resolution, B-frame GOP, VFR이 달라지면 독립 decode → color/scale/fps normalize → encode; 호환이 명확한 경우에만 remux-only 경로.

## SegmentPlanner 결정

~~~text
for each canonical segment [t, t+duration]:
  candidates = aligned_sources_covering(t, t+duration)
  for candidate in candidates:
    if !metadata_verified || !alignment_verified: reject
    estimate download_finish_ms for required keyframe+preroll window
    estimate decode+normalize+encode_ms (per device/profile)
    estimate switching_discontinuity_cost
    score = deadline_margin - processing_cost - expected_risk
  choose highest score if expected win > switch hysteresis
  else choose existing primary
  request bytes and wait for verification
  render immutable segment before publishing manifest entry
~~~

- 초기 segment duration 2~4초(측정 후 결정), prefetch ahead 15~45초.
- source 전환 빈도 상한(hysteresis/min hold), early cancel, deadline miss 통계, 파이프라인 동시성 상한.
- partial download/transform 중단과 seek는 generation fencing. stale completion으로 manifest가 바뀌지 않아야 한다.
- segment 경계의 PTS/DTS monotonically increasing, IDR/keyframe alignment, audio timestamp continuity, sample count, encoder delay, edit lists 검증.
- 영상 source switch에서 프레임 순서가 이어져도 **영상 표현/색/HDR 효과 변화**가 생길 수 있음. 시각적 안정성 metric/샘플 검사가 필요.
- HLS discontinuity가 필요한 변경은 올바르게 명시하고 player 지원을 시험. 무분별한 EXT-X-DISCONTINUITY로 은폐하지 않는다.
- segment publish: temp render → ffprobe validate → atomic promote → append playlist. 이미 공개한 segment는 덮어쓰지 않는다.

## 실패 복구

| 실패 | 필수 정책 |
|---|---|
| 새 source 다운로드 timeout | 이전 source 또는 준비된 standby로 폴백 |
| 정렬 불확실 / 다른 컷 | source 전환 금지 |
| VideoToolbox/FFmpeg 실패 | CPU 경로가 deadline 맞으면 재시도, 아니면 기존 소스 |
| target timestamp 누락 | manifest에 깨진 세그먼트 대신 buffering/status |
| source crash/process kill | watchdog 종료 및 transient queue 회수 |
| seek 중 늦은 렌더 완료 | generation 비교로 폐기 |

## 검증: 반드시 실제 A/B 교체를 확인

1. A=H.264, B=HEVC의 서로 다른 인코딩을 동일한 합성 영화로 만들고 2초 단위 segment 변환.
2. 의도적으로 A의 중간 구간 공급을 끊으면 B에서 그 재생 시간대가 완성되는지 추적.
3. 출력 전 구간 ffprobe로 timestamps/codec/profile/fps/size/closed GOP/AV sync 확인.
4. 비디오 frame hash/perceptual similarity로 원본 canonical frame들과 비교(압축 손실 허용).
5. A/V sync p95 target: 80ms 이내를 실험 목표로 설정(사용자 체감·codec 별 검토 후 조정). frame gap/duplicate 검출.
6. swap vs no swap의 stall_seconds 개선이 encode CPU 비용과 불안정성보다 큰지 측정.

## 완료 게이트

- end-to-end 1080p 재생에서 실제 다른 인코딩으로부터 alternate segment가 선택되고, 표준 player가 중단 없이 재생.
- 연결된 비디오·오디오가 찢어지거나 영상 구간이 중복/생략되지 않음(ground truth).
- deadline 실패 때의 폴백 경로가 통과. 미검증 source의 혼합은 테스트에서 차단.
