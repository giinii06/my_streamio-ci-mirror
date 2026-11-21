#!/bin/sh
# Runs against a built my-streamio-daemon; only calls its local HTTP API.
set -eu

command -v curl >/dev/null 2>&1 || { echo "curl is required" >&2; exit 1; }
binary="${1:-./target/debug/my-streamio-daemon}"
test -x "$binary" || { echo "build the daemon first" >&2; exit 1; }
workdir="$(mktemp -d)"
pid=""
cleanup() {
  if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  rm -rf "$workdir"
}
trap cleanup EXIT INT TERM
export STREAMIO_PORT=30311
export STREAMIO_DATA_DIR="$workdir/data"
"$binary" >"$workdir/daemon.log" 2>&1 &
pid=$!

ready=0
i=0
while [ "$i" -lt 40 ]; do
  if curl --max-time 1 --fail --silent "http://127.0.0.1:$STREAMIO_PORT/healthz" > "$workdir/health.json"; then
    ready=1
    break
  fi
  i=$((i+1))
  sleep 0.5
done
if [ "$ready" -ne 1 ]; then
  echo "daemon startup failed" >&2
  tail -n 80 "$workdir/daemon.log" >&2
  exit 1
fi

grep -q '"status":"ok"' "$workdir/health.json"
curl --max-time 5 --fail --silent "http://127.0.0.1:$STREAMIO_PORT/" | grep -q 'My Streamio'
curl --max-time 5 --fail --silent "http://127.0.0.1:$STREAMIO_PORT/" | grep -q 'Single-source playback'
curl --max-time 5 --fail --silent "http://127.0.0.1:$STREAMIO_PORT/api/v1/sources" | grep -q '"torrents"'
bad_host="$(curl --max-time 5 --silent -o /dev/null -w '%{http_code}' \
  -H 'Host: attacker.example' "http://127.0.0.1:$STREAMIO_PORT/healthz")"
test "$bad_host" = "403"
bad_source="$(curl --max-time 5 --silent -o /dev/null -w '%{http_code}' \
  -H 'Content-Type: application/json' -d '{"magnet":"not-a-magnet"}' \
  "http://127.0.0.1:$STREAMIO_PORT/api/v1/sources")"
test "$bad_source" = "422"
echo "Local HTTP API smoke passed"
