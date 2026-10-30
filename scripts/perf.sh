#!/usr/bin/env bash
# Performance pass (ROADMAP M7): generates a 1000-book library once (kept in
# the build directory), then measures scanning, the library views, search
# and memory with the real app code. Prints the "PERF" lines.
# Usage: scripts/perf.sh [build-dir] [book-count]   (default: build, 1000)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build="${1:-$root/build}"
count="${2:-1000}"
library="$build/perf-library"

cmake --build "$build" --target test_perf
if [[ ! -f "$library/.count-$count" ]]; then
    "$root/scripts/make-big-library.sh" "$library" "$count"
    touch "$library/.count-$count"
fi
AUDIUM_PERF_LIBRARY="$library" QT_QPA_PLATFORM=offscreen "$build/bin/test_perf" 2>&1 \
    | grep -E '^(QINFO.*PERF|PASS|FAIL|Totals)|FAIL!|Loc:' | sed 's/^QINFO *: *TestPerf::[a-zA-Z]*() *//'
