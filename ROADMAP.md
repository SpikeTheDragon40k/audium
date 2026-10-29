# Audium — Roadmap

All builds run locally; GitHub hosts only the code and the Releases (see ARCHITECTURE §3). Every milestone ends with the following steps:
1. `scripts/check.sh`, which runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, the CMake build and `ctest`.
2. Update the docs.
3. Record what was done and what's still open in this file.
4. Commit and push.

## Current status

- **Done:** environment check and design docs (2026-10-29); **M0 on Linux** (2026-10-29).
- **Next:** Milestone 1 (real playback).
- **Remote:** private GitHub repo `SpikeTheDragon40k/audium`, used for code and Releases only (no GitHub Actions).

## M0 — Foundation
- Cargo workspace with the 5 crates; `rust-toolchain.toml` (1.99.1).
- Top-level build: CMake + ECM + Corrosion; `cxx_qt_import_crate` for `audium-bridge`.
- A Kirigami `ApplicationWindow` showing one Rust QObject property in QML. This validates CXX-Qt with Qt 6.12 (risk R1).
- `scripts/make-fixtures.sh` generating:
  - sine/noise audio;
  - an m4b with chapters (via ffmetadata);
  - an mp3 with ID3 CHAP;
  - multi-disc folders.
- `scripts/check.sh`, the local quality gate (replaces CLAUDE.md's Linux/Windows CI items).
- `.gitignore` and a README skeleton.
- Exit criteria: the app launches on Plasma Wayland, the bridge round-trips a value, and a Qt test covers it.

### M0 status (2026-10-29)
Done and verified locally with `scripts/check.sh` (all steps pass):
- Cargo workspace (5 crates, edition 2024), `rust-toolchain.toml` 1.99.1, committed `Cargo.lock`.
- CMake + ECM + Corrosion + vendored cxx-qt-cmake 0.10.0.
- `AppInfo` QML singleton in Rust (`version`, `appId`, `echo()`), shown in a Kirigami window.
- `app/tests/test_bridge.cpp`. It fails on *any* Qt/QML warning and checks:
  - C++ → Rust round trip with Unicode text;
  - Rust properties read from C++;
  - QML access to the singleton;
  - `Main.qml` loads (offscreen) with its page present.
- `scripts/make-fixtures.sh` generates 7 layouts: m4b with chapters + cover, mp3 with ID3 CHAP, a folder book needing natural sort with sidecars, multi-disc CD1/CD2, a loose author folder, Unicode names without tags, and corrupt files. The chapters in the m4b and mp3 were checked with ffprobe.
- `scripts/check.sh` is the local gate.

Verified only partially:
- The app launched on Plasma 6 Wayland and ran for 6 s without errors (stopped by timeout). Nobody has looked at the window yet.

Open:
- Windows build via Craft: not attempted (no Windows machine in this session).
- The empty crates (`metadata`, `library`, `storage`) only contain doc comments until M2.
- No app icon yet. The window uses the theme icon `audiobook`, falling back to `media-playback-start`.

## M1 — Real playback
- `PlaybackController` (abstract), `MpvPlaybackBackend`.
- A temporary "open file(s)" dev action; a minimal Now Playing page and the mini-player.
- Seek, speed (scaletempo2, 0.5–3.0×), volume, error surfacing.
- Integration tests against real mpv using the fixtures:
  - the playback state machine;
  - playlist transitions;
  - seeking across tracks;
  - corrupt files.

## M2 — Library
- Spike to verify chapter and metadata readers (ARCHITECTURE §6).
- Storage: migrations v1, backup before migrating, the DB writer thread.
- Folder picker with multiple roots.
- Scanner: grouping heuristics, confidence scores, reconciliation, cancellation, progress reporting.
- Cover thumbnail cache.
- `BookListModel`, grid and list views, Book Details.
- Needs Review queue (list only; merge/split comes in M4).
- Tests:
  - grouping layouts and natural sort;
  - the metadata fallback chain;
  - migrations;
  - Unicode, long and non-UTF-8 paths;
  - missing and corrupt files;
  - reconciliation after a move or rename.

## M3 — Persistence (MVP)
- `PlaybackSession`:
  - debounced saves (every 5 s, and on pause, track change and quit);
  - resume;
  - end of book → marked finished.
- Per-book speed; configurable skip intervals.
- Sleep timer:
  - by minutes or end of chapter, with a 10 s fade;
  - extend triggers as listed in ARCHITECTURE §5.
- Library rows: Continue Listening, Recently Added, Finished.
- Shortcuts: Space, ←/→, `[` / `]`, and a Ctrl+F placeholder.
- Tests: resume near track boundaries, sleep-timer logic, session recording.

## M4 — Organization
- Search (FTS) and sorting.
- Author, series and collection pages; chapter browser.
- Bookmarks with notes (`B` key).
- Override editor; merge and split from Needs Review.
- Bookshelf view with a toggle against the grid.

## M5 — Insights and portability
- Stats page:
  - time listened today, this week, this month and all time;
  - current streak and books finished;
  - per-book totals;
  - a simple chart (QML Canvas/Shapes).
- JSON export/import:
  - versioned schema;
  - fingerprint matching;
  - conflict preview before applying;
  - a round-trip test.
- Settings → database backup.

## M6 — Desktop integration
- Linux: MPRIS2 via QtDBus (also handles media keys); KNotifications.
- Tray: KStatusNotifierItem, closing to the tray while playing (configurable).
- Windows: SMTC.

## M7 — Release
- Linux packaging: PKGBUILD, `.desktop` file, icon, AppStream metainfo.
- Windows: installer (Craft NSIS), built locally.
- `scripts/release.sh`: build artifacts locally and run `gh release create`.
- Dracula and Nord theme tokens.
- Performance pass on a 1000-book synthetic library: scan time, list scrolling, memory.
- Docs and known limitations.
