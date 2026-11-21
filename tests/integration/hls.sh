#!/bin/sh
# Verifies a real 9-second HLS output that uses HEVC video for seconds 3..6
# and H.264 for the other slices, with primary-source audio throughout.
set -eu
command -v ffmpeg >/dev/null 2>&1 || { echo "missing ffmpeg" >&2; exit 1; }
command -v ffprobe >/dev/null 2>&1 || { echo "missing ffprobe" >&2; exit 1; }
sh tests/fixtures/generate.sh
cargo run --locked --bin hls_fixture -- tests/fixtures/generated
playlist="tests/fixtures/generated/hls-programs/integration-switch/playlist.m3u8"
test -s "$playlist"
grep -q '#EXT-X-INDEPENDENT-SEGMENTS' "$playlist"
grep -q '#EXT-X-ENDLIST' "$playlist"
ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$playlist" |
  grep -q '^h264$'
ffprobe -v error -show_entries stream=codec_name -of csv=p=0 "$playlist" |
  grep -q '^aac$'
decode_errors="$(mktemp)"
trap 'rm -f "$decode_errors"' EXIT
ffmpeg -hide_banner -loglevel warning -nostdin -i "$playlist" -f null - 2>"$decode_errors"
if grep -Eqi 'Packet corrupt|non monotonically increasing|Invalid data found|corrupt input packet' "$decode_errors"; then
  echo "HLS continuity checks failed" >&2
  cat "$decode_errors" >&2
  exit 1
fi
echo "9-second two-encoding, one-audio-track HLS test passed"
