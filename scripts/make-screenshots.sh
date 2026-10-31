#!/usr/bin/env bash
# Takes the app-store screenshots (packaging/screenshots/*.png) from a
# generated demo library: the real UI, offscreen, 1280×800, Breeze Light.
# Usage: scripts/make-screenshots.sh [build-dir]   (default: build)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build="${1:-$root/build}"
demo="$build/demo-library"
out="$root/packaging/screenshots"

cmake --build "$build" --target make_screenshots
[[ -d "$demo" ]] || "$root/scripts/make-demo-library.sh" "$demo"
mkdir -p "$out"
AUDIUM_DEMO_LIBRARY="$demo" AUDIUM_SCREENSHOT_DIR="$out" QT_QPA_PLATFORM=offscreen \
    "$build/bin/make_screenshots" 2>&1 | grep -E '^(PASS|FAIL|SKIP|Totals)|FAIL!|Loc:'
ls -1 "$out"
