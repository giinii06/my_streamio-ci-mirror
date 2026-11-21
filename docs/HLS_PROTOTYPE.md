# Offline HLS source-switching integration

This is a development proof-of-concept for Phase 07, NOT live torrent source switching.

## Input preparation

Run `sh tests/integration/hls.sh` from the repository root after installing Rust, FFmpeg and FFprobe. The command will:

1. Generate 20-second **synthetic** H.264 and HEVC test video files.
2. Compute the whole-file SHA-256 of each source file.
3. Use a controlled, known-identical timeline: primary H.264 from 0–3 seconds, alternate HEVC from 3–6 seconds, primary H.264 from 6–9 seconds.
4. Use *primary-source audio for the entire 9-second program*, through one FFmpeg filtergraph/encoder instance.
5. Output `tests/fixtures/generated/hls-programs/integration-switch/playlist.m3u8` and MPEG-TS H.264/AAC segments.
6. Check the output with FFprobe and attempt full HLS decode. Packet corruption and non-monotonic PTS errors are failure conditions.

The renderer's input SHA-256 verification is **not proof** that the input came from a BitTorrent swarm. The caller must first verify torrent pieces with librqbit (or another backend), then stage immutable source inputs.

## Why a single encoder

Testing separately encoded MPEG-TS video/audio segments with a naive playlist yielded overlapping AAC timestamps and FFmpeg warnings at boundaries. Concatenating decoded video segments into one normalizing FFmpeg pipeline and encoding a single continuous primary audio track avoided those warnings in a 9-second synthetic command-line test; production claims require identical execution and end-to-end tests of the Rust implementation.

## Remaining for live use

- Use a bounded streaming buffer of verified torrent data rather than only complete local source files.
- Build timeline alignment evidence from real decoded frames and avoid alternate edits.
- Integrate the segment planner with actual per-swarm peer speed and source readiness.
- Generate live HLS playlists from a buffer with generation-fenced manifests.
- Benchmark VideoToolbox on native Mac M5 and Linux CPU fallback.
- Test video frame gaps, audio delay, HDR/SDR and interruptions under real network conditions.
