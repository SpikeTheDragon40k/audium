# Audium — Roadmap

All builds run locally; GitHub hosts only the code and the Releases (see ARCHITECTURE §3). Every milestone ends with the following steps:
1. `scripts/check.sh`, which runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, the CMake build and `ctest`.
2. Update the docs.
3. Record what was done and what's still open in this file.
4. Commit and push.

## Current status

- **Done:** environment check and design docs; M0 and M1 on Linux; **M2 on Linux** (all 2026-10-29).
- **Next:** Milestone 3 (persistence).
- **Remote:** private GitHub repo `SpikeTheDragon40k/audium`, used for code and Releases only (no GitHub Actions).

## Resume here (written 2026-10-29, end of session)

State:
- Branch `main`. M0, M1 and M2 are committed locally; `scripts/check.sh` passes.
- **GitHub (`origin/main`) still has the old pre-rebase history** (tip `3fe2956`, whose first commit has a Claude trailer). The local history was rewritten to remove it. Publish with
  `git push --force-with-lease=main:3fe2956 origin main`
  (the maintainer runs this; the agent may not force-push).
- M2 has not yet been tried on a real library by the maintainer.

Before starting M3:
1. `scripts/check.sh` (rebuilds and runs every test; the first build takes a few minutes).
2. Optionally run `build/bin/audium` on the real library and note grouping problems as M4 input.

M3 plan (Persistence, MVP):
1. **`PlaybackSession` (Rust, `crates/audium-bridge/src/playback_session.rs`):** loads a book by id: tracks and chapters from storage, resume position from `progress`, speed from `book_settings`. It drives the C++ `PlaybackController` through signal/slot wiring in `main.cpp` and replaces `app/src/libraryplayback.*`.
2. **Progress saving:** `progress` (track_id + offset_ms, book_position_ms) is saved debounced every 5 s, and on pause, track change, seek and quit. Add storage functions plus tests for resume near track boundaries.
3. **Sessions:** `sessions` rows (start/end, positions, listened_ms) per play stretch; feeds M5 stats.
4. **End of book:** `endOfBook` → `progress.finished_at`; playing a finished book again starts from 0.
5. **Per-book speed:** `book_settings.speed`, applied on load and saved on change.
6. **Settings:** skip intervals in the `settings` table (replace the 15 s / 30 s constants in `Main.qml`).
7. **Sleep timer:** a pure state machine in `audium-core` (minutes or end of chapter, 10 s fade, extend triggers as in ARCHITECTURE §5), ticked by a QTimer; fades through the controller's volume.
8. **Library rows:** Continue Listening, Recently Added, Finished (new `BookFilter`s).
9. **Shortcuts:** Ctrl+F placeholder (search itself is M4).
10. **Writer thread:** decide whether progress writes need one (see ARCHITECTURE §9 deviation).

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
- `app/tests/test_bridge.cpp`. It checks the items below. *Correction (found in M1):* the "fail on any warning" check was installed only for `initTestCase()`, because `QTest::failOnWarning` lasts for one test function. It now runs before every test.
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

### M1 status (2026-10-29)
Done and verified locally with `scripts/check.sh`. `test_playback` also passed 20 parallel runs (4 at a time) with no failures.
- `PlaybackController` (`app/src/playback/playbackcontroller.{h,cpp}`):
  - abstract, with no mpv types;
  - owns the book-level logic: track offsets, chapters (explicit → embedded → one per track), skip, and previous-chapter with a 3 s grace period;
  - rate is clamped to 0.5–3.0×, volume to 0–100.
- `MpvPlaybackBackend`:
  - `vo=null`, `af=scaletempo2`, `keep-open`, gapless playlist;
  - the wakeup callback posts a queued call, and events are drained on the Qt thread;
  - paths stay `QByteArray` and must be absolute;
  - per-track errors are mapped through mpv playlist entry ids.
- QML:
  - `Main.qml` takes the controller as a required property;
  - Library placeholder page with a temporary "Open Files…" action;
  - a persistent Now Playing page (seek slider, transport, speed, volume, chapter list, error banner);
  - a mini-player footer;
  - shortcuts: Space, ←/→ (15 s / 30 s), `[` / `]` (±0.1×).
- `app/tests/test_playback.cpp` (17 tests, real libmpv, `ao=null`):
  - loading, play/pause, seeking within a track and across tracks;
  - chapter navigation, including the grace period;
  - resume at a start position;
  - seamless multi-file transition;
  - rate and volume clamping;
  - end of book and replay;
  - corrupt, missing and relative paths;
  - a failed track being skipped;
  - Unicode paths;
  - stop and reload;
  - skipping with unknown track durations.
- `test_bridge` now also loads a book, opens Now Playing and plays, with no QML warnings allowed.
- ctest generates fixtures itself (`make_fixtures` is a CTest fixture-setup step).

Bugs found and fixed during M1:
- After `stop()`, a late `idle-active` notification put the next `load()` into Error. Errors are now detected only from `END_FILE` of entries in the current playlist.
- With unknown track durations ("Open Files"), skipping jumped to the last track. A track of unknown length is now open-ended for seeking, and a late-known length clamps the seek to 0.5 s before that track's end.

Not verified:
- **Audible output.** Tests use mpv's null audio output. Real playback through PipeWire on Plasma hasn't been heard yet. The app launches without warnings, but someone needs to listen to it.

Open / known limitations:
- With "Open Files", durations of tracks that haven't been loaded are unknown, so the total duration and the seek bar grow as tracks load. The library (M2) supplies durations up front.
- Skip intervals are hard-coded (15 s back, 30 s forward) until settings exist (M3).
- `MpvPlaybackBackend` sets `LC_NUMERIC=C` process-wide, which libmpv requires.

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

### M2 status (2026-10-29)
Done and verified locally with `scripts/check.sh` (all steps pass).

Metadata spike:

| Need | Library | Fixture result |
|---|---|---|
| Tags, duration, cover | lofty 0.25.5 | ✅ |
| MP3 chapters (ID3 CHAP) | id3 1.17.2 | ✅ |
| M4B chapters (Nero list and QuickTime track) | mp4ameta 0.13.0 | ✅ |
| ffprobe | — | not needed for these formats, not used |

What's in it:
- **`audium-core`:** typed ids, `BookUid` (v4), natural sort, text cleanup and keys, track/book position conversion.
- **`audium-metadata`:** probing (tags, duration, chapters); cover extraction with magic-byte checks; sidecars (`metadata.json` in Audiobookshelf style, `desc.txt`, `reader.txt`, `cover.*`/`folder.*`); folder and filename heuristics; the resolution chain with provenance for each field.
- **`audium-storage`:**
  - schema v1 covering every table in CLAUDE.md §9;
  - backup with `VACUUM INTO` before each migration; a failed migration rolls back and leaves the file unchanged; a newer schema is refused;
  - WAL and foreign keys;
  - lossless path BLOBs;
  - single-transaction apply; books with user data are never deleted.
- **`audium-library`:** discovery, grouping heuristics, confidence and review reasons, partial hash, incremental scans with move detection, missing and unmounted handling, cancellation, parallel reading (up to 4 threads).
- **Bridge:** `LibraryController`, `BookListModel` (a Rust `QAbstractListModel`), `BookDetails`, and the C++ `LibraryPlayback` glue.
- **QML:**
  - library grid/list with covers and an empty state;
  - folder picker;
  - scan progress with cancel;
  - Book Details (cover, metadata, review reasons, chapters, files with missing markers, Play);
  - Needs Review list;
  - Settings (folders add/remove, rescan, re-read everything, about).

Tests:
- 82 Rust tests:
  - grouping layouts: m4b/m4a, folder books, disc folders with and without tags, author folders split by album and by filename, numbered names, unreadable files;
  - natural sort, the metadata chain level by level, sidecars (including malformed and oversized ones), migrations (backup, rollback, newer schema);
  - reconciliation: move, rename, missing files, unmounted or emptied root;
  - symlink loops, non-UTF-8 and >1000-byte paths, cancellation;
  - a check that scanning leaves every file byte-identical.
- `test_bridge` scans the fixtures through `LibraryController`, opens every page, and plays a library book; any Qt/QML warning fails it.

Verified by hand:
- The real app, with an isolated `XDG_DATA_HOME`/`XDG_CACHE_HOME` and a copy of the fixtures as its library, ran its startup scan (8 books, 1 to review, about 5 ms) without warnings. The database is at `$XDG_DATA_HOME/audium/audium.db`; covers are only in `$XDG_CACHE_HOME/audium/covers/`.

Not verified:
- **A real library.** Nobody has scanned your library or looked at the UI with real covers yet. Scan speed and grouping quality on 1000+ messy books are unknown until M7's synthetic test, or until you try it.

Bugs found and fixed during M2:
- Untagged files named `Chapitre 01 – début`, `Chapitre 02 – fin` were split into one book per file. The numbering check now looks after the names' common prefix.
- `DisplayHint.AlwaysHide` actions caused Kirigami incubation warnings at startup.

Open / known limitations:
- **Needs Review:** only lists books and explains why. Merge, split, dismiss and editing come in M4, as does honoring `grouping_locked`.
- **Sidecar edits:** a changed `desc.txt` or `metadata.json` in an otherwise unchanged folder is only re-read by "Re-read Everything".
- **Untranslated status text:** scan status and error strings come from Rust and aren't translatable yet.
- **Covers:** full-size cache copies, no thumbnails.
- **Unmounted-root guard:** a library folder emptied on purpose is treated as "unmounted". Removing the folder in Settings is the way to clear it.
- **Windows:** paths with unpaired UTF-16 surrogates lose those characters when handed to mpv (UTF-8). Windows is still unbuilt.

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
