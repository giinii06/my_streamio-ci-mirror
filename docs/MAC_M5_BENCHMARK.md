# Native Apple M5 media acceleration benchmark

Run on the target **Mac M5, 24GB**, not in Oracle Linux ARM64 CI or a macOS Docker VM.

```bash
bash scripts/benchmark_m5.sh
```

Requires `ffmpeg` and `ffprobe` installed on macOS. The script synthesizes a short 1920×1080/24fps video and encodes it twice: once using CPU libx264 and once using `h264_videotoolbox` if the installed FFmpeg advertises the encoder. The output directory includes source video, FFprobe stream metadata, time/CPU information (FFmpeg benchmark output), logs, and a summary. No third-party movie files are used.

## Interpreting results

- `encoder_succeeded` means the VideoToolbox **encoding path completed**; it is not proof of optimal hardware utilization, energy savings, or compatibility with every H.264/HEVC/HDR profile.
- Compare output frame count, duration, bitrate, CPU time and wall time before claiming an improvement.
- Longer tests should include 1080p 24/30/60fps, several simultaneous decodes, alternate-source switching, battery/thermal and 4K HDR as separate dimensions.
- The encoder can fail or use a software path depending on driver and FFmpeg options; the script reports failure instead of pretending acceleration happened.
- The Linux GitLab Runner can syntax-check the script but **cannot execute native macOS VideoToolbox**.

Results must be attached to a Mac-specific CI job or test report to satisfy Phase 09; the file's existence alone does not close that gate.
