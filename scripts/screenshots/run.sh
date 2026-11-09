#!/usr/bin/env bash
# Rebuilds docs/screenshots from a generated demo library: real meshes, real index, the real UI with a mocked backend.
# Needs cargo, python3, node and a Chromium for Playwright (set CHROME=/path/to/chrome, or `npx playwright install chromium`).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HERE="$ROOT/scripts/screenshots"
WORK="$ROOT/target/screenshots"
PORT="${PORT:-5179}" # not 5173: that is the port of `npm run dev`
CLI="$ROOT/target/release/hoarddeck"

rm -rf "$WORK" && mkdir -p "$WORK"
cargo build --release -p hoarddeck-cli
python3 "$HERE/make-demo-library.py" "$WORK/library"
"$CLI" --index "$WORK/index" add "3D Print" "$WORK/library" >/dev/null
"$CLI" --index "$WORK/index" scan 1 >/dev/null

(cd "$HERE" && npm install --no-audit --no-fund --silent)
(cd "$ROOT/ui" && npx vite --port "$PORT" --strictPort >"$WORK/vite.log" 2>&1) &
VITE=$!
trap '{ kill $VITE; pkill -f "vite --port $PORT"; wait $VITE; } >/dev/null 2>&1 || true' EXIT
for _ in $(seq 1 60); do curl -s -o /dev/null "http://localhost:$PORT/" && break; sleep 0.5; done

export PORT
# Covers are rendered by the app's own 3D viewer, then picked up by a second scan.
node "$HERE/screenshots.mjs" covers "$WORK/index/index.sqlite" "$WORK/library" "$CLI" "$WORK"
"$CLI" --index "$WORK/index" scan 1 >/dev/null
python3 "$HERE/seed-demo-metadata.py" "$WORK/index/index.sqlite"
node "$HERE/screenshots.mjs" shots "$WORK/index/index.sqlite" "$WORK/library" "$CLI" "$ROOT/docs/screenshots"
echo "screenshots written to docs/screenshots"
