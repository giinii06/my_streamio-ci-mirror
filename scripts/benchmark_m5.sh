#!/usr/bin/env bash
# Native macOS-only benchmark. Never claims VideoToolbox worked without
# checking the encoder's exit code, output stream and measured timings.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This benchmark requires native macOS (not Linux Docker)" >&2
  exit 2
fi

for executable in ffmpeg ffprobe sysctl sw_vers; do
  command -v "$executable" >/dev/null || {
    echo "Required tool is missing: $executable" >&2
    exit 2
  }
done

duration="${STREAMIO_BENCH_DURATION:-12}"
if [[ ! "$duration" =~ ^[0-9]+$ ]] || (( duration < 3 || duration > 60 )); then
  echo "STREAMIO_BENCH_DURATION must be an integer 3..60" >&2
  exit 2
fi

out="${1:-$(mktemp -d -t my-streamio-m5-bench)}"
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
echo "Benchmark directory: $out"

{
  echo "Platform: $(uname -sm)"
  echo "Hardware: $(sysctl -n hw.model 2>/dev/null || echo unknown)"
  echo "RAM bytes: $(sysctl -n hw.memsize 2>/dev/null || echo unknown)"
  echo "macOS: $(sw_vers -productVersion)"
  echo "FFmpeg: $(ffmpeg -version | head -n 1)"
} >"$out/platform.txt"

echo "Generating synthetic 1080p/$duration-second source"
ffmpeg -hide_banner -loglevel error -nostdin -y \
  -f lavfi -i "testsrc2=size=1920x1080:rate=24:duration=$duration" \
  -pix_fmt yuv420p -c:v libx264 -preset ultrafast -g 48 \
  -movflags +faststart "$out/source.mp4"

echo "CPU H.264 encode benchmark"
ffmpeg -hide_banner -benchmark -loglevel info -nostdin -y \
  -i "$out/source.mp4" -an -vf "fps=24,format=yuv420p" \
  -c:v libx264 -preset veryfast -b:v 6M -maxrate 6M -bufsize 12M \
  -g 48 -pix_fmt yuv420p "$out/cpu.mp4" >"$out/cpu.log" 2>&1

hw_status="unavailable"
if ffmpeg -hide_banner -encoders 2>/dev/null | grep -q 'h264_videotoolbox'; then
  echo "VideoToolbox H.264 encoder discovered; testing real encode"
  if ffmpeg -hide_banner -benchmark -loglevel info -nostdin -y \
    -i "$out/source.mp4" -an -vf "fps=24,format=nv12" \
    -c:v h264_videotoolbox -b:v 6M -g 48 \
    "$out/hardware.mp4" >"$out/hardware.log" 2>&1; then
    hw_status="encoder_succeeded"
  else
    hw_status="encoder_failed"
    echo "VideoToolbox encode failed; do not label this a hardware success" >&2
  fi
else
  echo "No h264_videotoolbox encoder in this FFmpeg build" >&2
fi

ffprobe -v error -show_streams -show_format -of json "$out/cpu.mp4" >"$out/cpu.json"
if [[ "$hw_status" == "encoder_succeeded" ]]; then
  ffprobe -v error -show_streams -show_format -of json "$out/hardware.mp4" >"$out/hardware.json"
fi

{
  echo "VideoToolbox status: $hw_status"
  echo "CPU benchmark:"
  grep -E '^bench:|^frame=' "$out/cpu.log" | tail -n 5 || true
  if [[ "$hw_status" == "encoder_succeeded" ]]; then
    echo "VideoToolbox benchmark:"
    grep -E '^bench:|^frame=' "$out/hardware.log" | tail -n 5 || true
  fi
  echo "Swap: $(sysctl -n vm.swapusage 2>/dev/null || echo unknown)"
} | tee "$out/summary.txt"

echo "Results saved to $out"
