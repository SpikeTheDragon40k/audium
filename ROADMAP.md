# Audium — Roadmap

All builds run locally; GitHub hosts only the code and the Releases (see ARCHITECTURE §3). Every milestone ends with the following steps:
1. `scripts/check.sh`, which runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, the CMake build and `ctest`.
2. Update the docs.
3. Record what was done and what's still open in this file.
4. Commit and push.

## Current status

- **Done:** environment check and design docs; M0, M1 and M2 on Linux (2026-10-29); M3 on Linux (2026-10-30, reviewed); **M4 on Linux** (2026-10-30, reviewed).
- **Next:** Milestone 5 (insights and portability).
- **Remote:** private GitHub repo `SpikeTheDragon40k/audium`, used for code and Releases only (no GitHub Actions).

## Resume here (updated 2026-10-30)

State:
- Branch `main`. M0–M4 are committed and pushed (M4 reviewed and tried by the maintainer on 2026-10-30).
- `scripts/check.sh` passes.

Next: Milestone 5 (Insights and portability):
1. Stats queries in `audium-storage` from `sessions`: listened today, this week, this month and all time; current streak (consecutive days with listening); books finished; per-book totals; daily totals for a chart. Unit tests on synthetic sessions, including day boundaries and local time zones.
2. `StatsModel` (bridge) and a Stats page (tiles plus a simple bar chart in QML Shapes/Canvas).
3. JSON export: versioned schema (`schema_version`); progress, bookmarks, sessions, overrides, settings, collections. Books are identified by fingerprint (normalized title + author + total duration + partial hashes).
4. JSON import: match by fingerprint, preview conflicts (newer progress, duplicate bookmarks), then apply in one transaction. Round-trip test.
5. Settings → back up the database now (`VACUUM INTO`, with a file dialog).

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

### M3 status (2026-10-30)
Done and verified locally with `scripts/check.sh`. `test_session` passed 12 more runs, 4 in parallel.

What's in it:
- **`audium-core`:** pure state machines and functions with unit tests:
  - `progress` (save schedule),
  - `listening` (sessions),
  - `sleep` (timer, fade, extension),
  - `position::resume_point` (resume near track boundaries, missing tracks).
- **`audium-storage`:**
  - `playback` (progress, finished/restart, per-book speed, sessions, settings);
  - the `Writer` thread;
  - the Continue Listening, Recently Added and Finished filters;
  - progress in `BookSummary`;
  - `book_summary()`, so the details page no longer lists every book.
- **Bridge:** `PlaybackSession` (replaces M2's C++ `LibraryPlayback`); progress and finished properties on `BookDetails`; progress roles (`positionMs`, `progress`, `finished`) and filters 2–4 on `BookListModel`.
- **C++:** `SessionWiring`; `PlaybackController::setVolumeFactor`.
- **UI:**
  - Library has a Home tab (Continue Listening, Recently Added, Finished rows) and an All Books tab (grid/list);
  - covers show progress bars and a finished mark;
  - Book Details shows Play / Resume (with the time left) / Play Again and a progress bar;
  - a sleep-timer button (5–90 min, end of chapter, off; shows the time left; a tooltip during the fade) in the mini-player and Now Playing;
  - Now Playing shows the book's cover and title;
  - Settings has Playback skip intervals (persisted);
  - a "Needs review" banner replaces the toolbar action;
  - Ctrl+F shows a "not available yet" note.
- **Shortcuts:** Space, ←/→ (now the configured intervals), `[` / `]`, Ctrl+F placeholder. (`B` for bookmarks is M4.)

Tests:
- 101 Rust tests.
- Qt: 18 playback, 11 session, 5 UI test functions.
- `test_session` uses real libmpv and checks the database directly:
  - progress saved on pause and resume after a restart;
  - saved immediately on a track change, and on quit;
  - resuming at a track end goes to the next track;
  - speed remembered per book across runs;
  - end of book marks it finished, nothing undoes that, and replay starts at 0 and clears "finished";
  - listening sessions are recorded;
  - the sleep timer fades and pauses without changing the user's volume;
  - activity, including a key event through the wiring's filter, extends the fade;
  - end-of-chapter sleep pauses at the chapter end;
  - "Open Files" detaches the book without touching its progress;
  - skip intervals persist.

Bugs found by the maintainer on a real library (2026-10-30) and fixed:
- **Home and All Books were blank and could not be clicked.** The views were in a `Loader` inside a `ScrollView`, which gives the `Loader` no size. The tests only checked that objects existed. Each view now fills the page and scrolls itself. `test_bridge` now shows the window, checks the views' sizes, and clicks a tile with a real mouse event in Home and All Books, expecting that book's details page. Verified: with the old layout the test fails (0×0 view).
- **A book split into 12 numbered `.m4b` parts showed as 12 books.** Containers sharing an album tag are now one book (see ARCHITECTURE "Scanning"). Covered by a unit test of that exact layout and a new `multi-part-m4b` fixture in the end-to-end scan test. A read-only scan of the maintainer's `~/Music` into a throwaway database now gives 2 books (12 parts in order, 12 chapters, cover, confidence 100).
- **Existing libraries keep a wrong grouping after an update**, since unchanged folders are never regrouped. Fixed with the rules version: the next scan of an existing library re-reads everything once.
- **…and the first attempt at that did nothing.** A scripted text edit silently matched nothing, so the scan saved "rules version 2" without re-reading anything. The maintainer still saw 1 + 12 books. Fixed (now version 3, so already-affected libraries are re-read), and the version is only saved once every folder was really re-read. Regrouping now keeps the book with progress. There's a new end-to-end test (old one-book-per-part state, progress on part 2, normal scan leaves it, forced scan merges into part 2's book with progress intact). Verified on a copy of the maintainer's database: 13 → 2 books.
- **Process fix:** my scripted edits now fail when a pattern doesn't match exactly once. Checking afterwards showed the M2 and M3 sections of ARCHITECTURE.md had also never been written; they are now (§10–12).

Bugs found and fixed during M3:
- **Resume at the start of track N played track 1.** mpv started the first playlist entry before `playlist-play-index` took effect. This was an M1 bug, hidden because M1 only tested start offsets above 0. The backend now sets `playlist-start` and ignores stray loads; `test_playback` has a regression test.
- **Hidden toolbar actions (`visible: false` at creation)** caused the same Kirigami incubation warnings as `AlwaysHide`. The view toggles are now disabled instead, and Needs Review became a banner. A new empty-library UI test catches this; it was checked by temporarily adding a hidden action, which made it fail.
- `StandardKey.Find` needs `sequences:`, not `sequence:`.

Verified by hand:
- The real app, with isolated data and cache folders, a fixtures copy as its library, saved progress and a finished book, started without warnings on Wayland and offscreen.

Verified by the maintainer (2026-10-30), on their real library:
- Audible playback through PipeWire works.
- Grouping is correct after the fixes below (2 books, including a book split into 12 m4b parts).
- The features work.

Open from that test:
- **The UI doesn't scale well when the window is resized.** It's functional, but the layouts don't adapt well. Moved to M7 (release polish), see there.

Open / known limitations:
- **Sleep timer:** only for library books.
- **Activity outside Audium:** activity in other applications can't extend the fade on Wayland (by design). Media keys come with M6.
- **Untranslated text:** Rust status strings are still not translatable.
- **Default speed:** new books start at 1.0×; there's no global default-speed setting yet.

## M4 — Organization
- Search (FTS) and sorting.
- Author, series and collection pages; chapter browser.
- Bookmarks with notes (`B` key).
- Override editor; merge and split from Needs Review.
- Bookshelf view with a toggle against the grid.

### M4 status (2026-10-30)
Done and verified locally with `scripts/check.sh` (all steps pass).

What's in it:
- **Search:** FTS5, prefix matching, accents folded, across title, author, narrator and series. Ctrl+F opens Books and focuses the field.
- **Sorting:** title, author (then series order), recently played, recently added.
- **Browse:** Authors and Series tabs (cards with cover and count) open a book page per group; series list in reading order. Author, narrator and series names on the details page are links.
- **Collections:** create, rename and delete in the Collections tab; add and remove from the details page (menu with checkmarks, "New Collection…"); the book's collections are shown as chips.
- **Bookmarks:** `B` adds one at the current position (with a notification). The details page lists them with time, chapter, title and note; click to play from there; edit or delete.
- **Chapter browser:** clicking a chapter on the details page plays from it (or seeks, if the book is already playing).
- **Metadata editor:** title, subtitle, authors, narrators, series, number, year, genre, description. Stored as overrides, never written to files; an emptied field returns to the tags; the book is then re-read by itself.
- **Needs Review:** select books and merge them; mark as reviewed. Splitting is on the details page (check the files that belong to a new book).
- **Merge and split stay** across rescans (`grouping_locked`), including merges across folders.
- **Bookshelf view:** a third Books view, with covers on shelves grouped by series (plus "Not in a series"), author or collection, and a progress bar under each cover.

Tests:
- Rust:
  - search (prefix, accents, narrator, series, all words, operators as text), index kept in sync on rescan and removal, migration 2 indexing existing books;
  - sorting and scopes;
  - collections, bookmarks, overrides, dismissal;
  - merge (tracks, bookmarks, progress, collections move) and split (bookmarks follow);
  - the scanner keeping merged (across folders) and split books, a new file beside a locked book, overrides via a one-book re-read.
- Qt:
  - session bookmarks (position, track, edit, delete) and `playBookAt` (load at a position, or seek);
  - the UI test now also clicks covers in the Books grid and the bookshelf, searches and checks the grid narrows, opens author and series cards, the Collections tab and the metadata editor.

Bugs found and fixed during M4:
- **Settings could not be closed** (found by the maintainer). It opened as a column beside the library, and in a wide window Kirigami gives such a column no back or close button. The metadata editor and split page had the same problem when left without saving. All three now open as layers over the window, which have a back button, and Escape closes a layer too. `test_bridge` opens Settings and closes it by clicking the back button like a user (after the slide-in finishes), then with Escape, and does the same for the editor. Also checked under the Breeze desktop style.
- **Views didn't update after a model reload.** All three Rust list models returned their `count` property from `rowCount()`, but updated it after `endResetModel()`. Views showed the old number of items after a search, scan or progress change. Found by the new search UI test; now there's an explicit check of the row count seen at `modelReset`.
- **Test realism:** the UI test now moves the mouse before clicking. Without that, a tooltip from the previous cover (still "hovered", because the test never moved the pointer) swallowed the first click, which never happens with a real mouse.

Verified by hand:
- Migration to schema 2 on a *copy* of the maintainer's database: backup written, search finds their books, progress and sessions intact, no warnings.

Not verified:
- **Interactive use by a person:** editing, merging, splitting, collections, bookmarks, the bookshelf look.
- **Layout at different window sizes:** see M7's responsive pass.

Open / known limitations:
- **A new untagged file beside a locked (merged) book** takes its title from the folder name, because the folder looks like it holds a single book.
- **A merged book's metadata comes from the majority of its files' tags.** Edit it if that's wrong.
- **Splitting doesn't move progress:** the new book starts without progress, and the original keeps it, resuming by book position if its saved track moved away.
- **Narrators have no tab:** narrator pages are reached via the details page links.

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
- **Responsive layout pass** (maintainer's feedback, M3): the UI doesn't scale well when the window is resized. Screenshot every page offscreen at narrow, normal and wide sizes, fix the layouts (Home rows and tiles, grid cells, Now Playing, Book Details, the library and details columns side by side), and add the screenshots or size checks to `test_bridge`.
- Performance pass on a 1000-book synthetic library: scan time, list scrolling, memory.
- Docs and known limitations.
