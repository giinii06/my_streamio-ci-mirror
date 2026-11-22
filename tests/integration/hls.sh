#!/bin/sh
# ALCHEMY CI PROBE - deliberately fails if this HLS job is actually invoked.
echo 'ALCHEMY_REQUIRED_HLS_JOB_PROBE_EXECUTED' >&2
exit 93
# Verifies a real 9-second HLS output that uses HEVC video for seconds 3..6
# and H.264 for the other slices, with primary-source audio throughout.
set -eu
command -v ffmpeg >/dev/null 2>&1 || { echo "missing ffmpeg" >&2; exit 1; }
command -v ffprobe >/dev/null 2>&1 || { echo "missing ffprobe" >&2; exit 1; }
sh tests/fixtures/generate.sh
# Idempotent test output. Never delete anything outside the generated test tree.
rm -rf tests/fixtures/generated/hls-programs/integration-switch
cargo run --bin hls_fixture -- tests/fixtures/generated
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

# Run the opt-in *real network* test in the EXISTING GitHub media-HLS job.
# A passing offline HLS composition test alone is not proof of torrent bytes.
# The explicit --list guard ensures an accidentally filtered-out test is not
# misreported as success.
cargo test --locked -p my-streamio-daemon --features network-fixture \
  --bin my-streamio-daemon -- --list |
  grep -F 'network_e2e::two_real_swarms_deliver_piece_verified_http_seek_bytes: test'
cargo test --locked -p my-streamio-daemon --features network-fixture \
  --bin my-streamio-daemon -- \
  network_e2e::two_real_swarms_deliver_piece_verified_http_seek_bytes --nocapture
echo "Real two-swarm BitTorrent peer-to-verified-HTTP E2E test passed"
