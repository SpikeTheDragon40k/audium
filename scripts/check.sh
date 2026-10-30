#!/usr/bin/env bash
# Local quality gate, run at the end of every milestone (no GitHub CI; see ROADMAP.md).
# Usage: scripts/check.sh [build-dir]   (default: build)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build="${1:-$root/build}"
cd "$root"

step() { printf '\n==> %s\n' "$*"; }

step "cargo fmt --check"
cargo fmt --all --check

step "cargo clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "fixtures"
scripts/make-fixtures.sh >/dev/null

step "cargo test"
cargo test --workspace --locked

step "cmake configure"
cmake -S . -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo

step "cmake build"
cmake --build "$build"

step "install (staged) and desktop metadata"
DESTDIR="$build/stage" cmake --install "$build" >/dev/null
desktop-file-validate "$root/app/io.github.SpikeTheDragon40k.Audium.desktop"
appstreamcli validate --no-net "$root/app/io.github.SpikeTheDragon40k.Audium.metainfo.xml"

step "ctest"
ctest --test-dir "$build" --output-on-failure

printf '\nAll checks passed.\n'
