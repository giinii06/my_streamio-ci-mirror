#!/bin/sh
set -eu

# Generate synthetic videos with no third-party movie content.
command -v ffmpeg >/dev/null 2>&1 || { echo "ffmpeg is required" >&2; exit 1; }
command -v ffprobe >/dev/null 2>&1 || { echo "ffprobe is required" >&2; exit 1; }

out="${1:-tests/fixtures/generated}"
mkdir -p "$out"
ffmpeg -hide_banner -loglevel error -nostdin -y \
  -f lavfi -i "testsrc2=size=640x360:rate=24:duration=20" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=20" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -g 48 \
  -c:a aac -shortest -movflags +faststart "$out/reference.mp4"
ffmpeg -hide_banner -loglevel error -nostdin -y \
  -i "$out/reference.mp4" -c:v libx265 -preset ultrafast \
  -x265-params "log-level=error" -c:a copy "$out/alternate.mp4"
ffprobe -v error -show_streams -of json "$out/reference.mp4" > "$out/reference.json"
ffprobe -v error -show_streams -of json "$out/alternate.mp4" > "$out/alternate.json"
printf 'Generated %s/reference.mp4 and %s/alternate.mp4\n' "$out" "$out"
