# Audium — Roadmap

All builds run locally; GitHub hosts only the code and the Releases (see ARCHITECTURE §3). Every milestone ends with the following steps:
1. `scripts/check.sh`, which runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, the CMake build and `ctest`.
2. Update the docs.
3. Record what was done and what's still open in this file.
4. Commit and push.

## Current status

- **Done:** environment check and design docs; M0, M1 and M2 on Linux (2026-10-29); M3 on Linux (2026-10-30, reviewed); **M4 on Linux** (2026-10-30, reviewed); **M5 on Linux** (2026-10-30, reviewed); **M6 on Linux** (2026-10-30, reviewed); **M7, release on Linux** (2026-10-30, reviewed: the maintainer installed the package and it works).
- **On hold:** Milestone 8 (Windows), at the maintainer's request.
- **Next:** release 1.0.0 (Arch package). Then 1.1.0: Flatpak and the Flathub submission.
- **Remote:** private GitHub repo `SpikeTheDragon40k/audium`, used for code and Releases only (no GitHub Actions).

## Resume here (updated 2026-10-30)

State:
- Branch `main`. M0–M7 are committed and pushed. M7 was committed at the maintainer's request before their hands-on review. Windows was moved out of M7 into its own milestone, M8, which is **on hold**.
- `scripts/check.sh` passes. No schema change in M7 (one new setting, `theme`).
- **Arch package: builds.** The first `makepkg` failed at the final link (undefined references to the Rust bridge): makepkg's default `lto` option adds `-flto` to the C++ compiled inside the bridge's build script. Fixed with `options=('!lto')` in the PKGBUILD. Checked: `makepkg` from a tarball of the working tree (`--nodeps`) builds `audium-0.1.0-1-x86_64.pkg.tar.zst` (3.8 MB: binary, themes, icon, `.desktop`, metainfo; licence GPL-3.0-only); the packaged binary has no missing libraries and starts (offscreen, throwaway data dir). The maintainer installed it with `pacman -U` and reports it works flawlessly on their desktop.
- **Licence:** GPL-3.0-only, chosen by the maintainer (2026-10-30): `LICENSE`, README, Cargo manifests, AppStream file, PKGBUILD and `KAboutData`.
- **App ID renamed** to `io.github.SpikeTheDragon40k.Audium` (was `org.audium.Audium`), before 1.0.0, because Flathub only accepts IDs on a domain or code host the developer controls. It names the desktop file, icon, AppStream id, Wayland app id, MPRIS `DesktopEntry` and tray item. Unchanged: the data directories (`~/.local/share/audium`, from the component name `audium`), the MPRIS bus name `org.mpris.MediaPlayer2.audium`, and the internal QML module URIs (`org.audium.app`, `org.audium.bridge`). A launcher pinned to the old desktop file has to be pinned again after updating.

- **Reset Statistics** (maintainer's request, 2026-10-30): Settings → Your Data → "Reset Statistics…" with a confirmation dialog deletes the listening history (`sessions`); progress, bookmarks and finished books stay. Tested in `test_bridge` (clicked like a user, then the database checked) and in the storage tests.

Next: release 1.0.0 (see "1.0.0 release" below).

How the work goes (keep doing this):
- One milestone at a time. At the end: `scripts/check.sh`, update ROADMAP and ARCHITECTURE, then **stop for the maintainer's review**. Commit (no AI trailer) and push only when they say so.
- The maintainer tries every milestone on their real library (`~/Music`). Real-use bugs found so far were UI-level (blank views, pages that couldn't be closed). For every UI change, test what a person does: click with the mouse in `test_bridge` (move first, wait for animations), and check sizes, not just that objects exist.
- Scripted file edits go through the checked helper: every pattern must match exactly once. Unchecked edits once silently did nothing.
- `AUDIUM_SCREENSHOT_DIR=<dir> QT_QPA_PLATFORM=offscreen build/bin/test_layout` saves every page at three window sizes and in both themes, to look at layouts. `test_layout` itself fails on text that can't be read (ARCHITECTURE §13a).
- `scripts/perf.sh` repeats the performance measurements on a generated 1000-book library.

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

### M5 status (2026-10-30)
Done and verified locally with `scripts/check.sh` (all steps pass).

What's in it:
- **Statistics page** (library toolbar → Statistics; a layer with a back button):
  - tiles for today, this week, this month, all time, current streak (best streak in its tooltip), and books finished (total and this month);
  - a bar chart of listening per day for the last 30 days, with Monday labels, a scale guide and a tooltip per bar;
  - "Most listened" books, which open the book.
- **Settings → Your Data:**
  - **Export…:** JSON with progress, finished state, bookmarks, listening history, edits, speeds, collections and settings.
  - **Import…:** a preview dialog with what will change, what is kept because yours is newer, and which books aren't in the library. Apply, or cancel without changes.
  - **Back Up Database…:** a complete copy of the library database.

Tests:
- **Rust:**
  - calendar arithmetic (leap years, week and month starts; a round trip over ±2,000 years);
  - period totals, streaks (including "not played yet today" and across month boundaries);
  - SQL splitting of sessions at local midnight, which passes under UTC, New York, Kolkata (+5:30), Auckland (+13) and Rome;
  - per-book totals and finished dates;
  - export/import round trip between two "computers" with different paths: hash matching, a title/author/length match, a missing book, newer local progress kept, the plan applied exactly, a re-import changing nothing;
  - refusal of foreign, newer and damaged files.
- **Qt:**
  - the stats page shows a session, its chart is laid out, and a "most listened" row opens the book;
  - export → preview → apply through `LibraryController` (re-importing your own export changes nothing; a plan applies once; a damaged file gives an error);
  - backup, including replacing an existing backup.
- **Looked at:** `AUDIUM_SCREENSHOT_DIR=<dir>` makes `test_bridge` save screenshots; the stats page was checked that way.

Not verified:
- **Stats with months of real listening history:** only seen with a test session.
- **An export from one real library imported into another:** only tested with synthetic libraries.

Bugs found by the maintainer during M5 review, fixed:
- **A book's details panel could not be closed.** Book details, a group's books, Needs Review and Now Playing open as columns beside the library, and in a wide window Kirigami's back button is disabled for columns that are all on screen. Each of these pages now has a Close (×) action, and Escape closes the rightmost column when no layer is open. `test_bridge` opens a book from the grid and closes it by clicking × like a user, then with Escape, and does the same for Now Playing and the author and series pages. The test's clicks now also wait for column and layer animations to stop, as a person would.

Open / known limitations:
- **A week starts on Monday**, whatever the locale.
- **Sessions longer than 24 hours** are split only at the first midnight.
- **A finished book counts once:** if you finish it again, the finished date is updated rather than counting twice.
- **"Books finished" is all-time and this month;** there's no per-week figure.

## M6 — Desktop integration
- Linux: MPRIS2 via QtDBus (also handles media keys); KNotifications.
- Tray: KStatusNotifierItem, closing to the tray while playing (configurable).
- Windows: SMTC.

### M6 status (2026-10-30)
Done and verified locally with `scripts/check.sh` (all steps pass). Tested by the maintainer on Plasma 6 (Wayland): media keys, media widget, tray, close to tray and notifications work.

What's in it (details in ARCHITECTURE §13):
- **MPRIS2:** Plasma's media controls, media keys and the lock screen control Audium. The "track" is the chapter.
- **Tray icon:** the current book and chapter, Play/Pause, Show/Hide, Quit; middle click plays or pauses.
- **Close to tray:** closing the window while a book plays keeps it playing in the tray (Settings → Playback, on by default).
- **Notifications:** "Book finished" and "Sleep timer".
- **Windows SMTC: deferred** to the Windows bring-up in M7. It can be neither compiled nor tested here; nothing uncompiled was added.

Tests:
- `test_integration` (new, on a private D-Bus):
  - MPRIS driven over the bus from a second connection: Identity and DesktopEntry; PlaybackStatus and PropertiesChanged; metadata (chapter title, book, author, chapter length, cover); Pause, PlayPause, Next, Previous with grace; SetPosition (and a stale id ignored); Seek plus the `Seeked` signal; Rate and Volume writes;
  - a second Audium getting its own name;
  - Play/Pause extending the sleep fade;
  - the tray's text and actions;
  - both notifications;
  - closing to the tray (hide while playing, close while paused, close when the setting is off).
- `test_playback`: seeking after playing and pausing sticks; "next chapter" works repeatedly while paused; playback resumes from the seek target.
- `test_session`: playing a finished book from a chapter works normally (progress saved, no longer finished, even if paused at once); "play from here" on a paused book plays.

Verified on the real desktop (the app offscreen, with a throwaway library, on the maintainer's session bus):
- Plasma sees the MPRIS service (Identity "Audium", DesktopEntry `org.audium.Audium`).
- Plasma's StatusNotifierWatcher lists Audium's tray item.

Not verified:
- **Physical media keys and the Plasma media widget by hand.**
- **A notification appearing on screen:** the tests check they are sent, not displayed.
- **The tray menu by hand.**

Bugs found and fixed during M6 (several were in earlier milestones' code):
- **Paused chapter navigation** (since M1): after a seek while paused, the reported position fell back by mpv's audio buffer, so "next chapter" while paused showed the previous chapter and repeated the same jump.
- **Crash on quit** (new code): KStatusNotifierItem owns its menu, and the tray also deleted it.
- **Playing a finished book from a chapter or bookmark** (since M4): it only seeked, so progress wasn't saved and the sleep timer didn't run. Restarting a finished book now also counts at once, even if paused immediately.
- **"Play from here" on a paused book** (since M4) only seeked; now it plays.
- **MPRIS "Seeked"** was first derived from position jumps and missed small seeks. It now comes from an explicit controller signal.
- **Test infrastructure:** the private test bus activated `xdg-desktop-portal`, which held ctest's output open. Also, a shell `grep` wrapper (ugrep) hung in a pipeline during development; scripts and checks now call `/usr/bin/grep` and use `timeout`.

## M7 — Release (Linux)
- **Responsive layout pass** (maintainer's feedback, M3): the UI doesn't scale well when the window is resized. Screenshot every page offscreen at narrow, normal and wide sizes, fix the layouts, and add size checks to the tests.
- Dracula and Nord theme tokens.
- Performance pass on a 1000-book synthetic library: scan time, list scrolling, memory.
- Linux packaging: PKGBUILD, `.desktop` file, icon, AppStream metainfo; `scripts/release.sh` builds the artifacts locally and runs `gh release create`.
- Docs and known limitations.
- (Windows moved to M8.)

### M7 status (2026-10-30)
Done and verified locally with `scripts/check.sh` (all steps pass); the Arch package builds (see "Resume here"). Details in ARCHITECTURE §13a.

**Window sizes:**
- The library keeps most of the window; a page opened beside it (details, a group's books, Needs Review, Now Playing) takes about 40% (it used to be the reverse: a 360-pixel library and a huge details column).
- Grids fit as many columns as the width allows. Tiles size themselves from the font, so titles and authors are never cut off, and covers line up.
- In a narrow window the library tabs become icons, the search field gets its own row, and buttons wrap. Settings, Statistics and the editors stay readable and centred in a wide window.
- The window's minimum height went from 20 to 25 grid units (below that only the header and player were visible).

**Themes:** Settings → Appearance → Colours: System (Breeze, follows the desktop), Dracula or Nord. Saved, and applied before the window appears.

**Performance** (1000 books; numbers in ARCHITECTURE §13a): first scan about 1 s, unchanged rescan 0.1 s, grid and list scroll at 7 ms per frame, search under 0.1 s, 200 MiB with the window open. The bookshelf was the one slow view: each shelf loaded the whole library twice before its own books. Fixed; it is still the slowest view (26 ms average, 70 ms worst per scroll step), listed as a known limitation.

**Packaging:** `.desktop` file, AppStream metainfo, an original app icon (also used for the window and the tray), install rules, `packaging/linux/PKGBUILD`, `scripts/release.sh`. The PKGBUILD needs `options=('!lto')`: with makepkg's LTO the final link fails.

**Docs:** README rewritten for users (features, keys, data locations, installing, known limitations); ARCHITECTURE §13a; §6 corrected: the `ffprobe` chapter fallback it described was never built.

Tests:
- `test_layout` (new): every page at the minimum, default and 1920×1080 size, plus the default size in Dracula and Nord. It fails on any warning and on unreadable text (cut off, a tab not shown, text outside its button or tile). It checks that a theme's colours reach the window and that System restores the desktop's.
- `test_session`: the theme setting is saved and loaded; an unknown value falls back to System.
- `test_perf` (not in ctest; `scripts/perf.sh`): scan, rescan, views, scrolling, search, memory, with loose limits.
- UI test helpers moved to `app/tests/uitest.h`. Tests now add listening data through the `sqlite3` tool in a separate process: Qt's own SQLite in the same process broke the app's file locks ("disk I/O error").

Bugs found and fixed:
- **Binding-loop warnings on Now Playing in a narrow window** (since M1): the persistent page was created as a child of the window instead of the page stack.
- **Bookshelf** loaded the whole library twice per shelf (since M4).
- **List rows and grid tiles** were partly under the scroll bar (since M2).

Not verified:
- **Everything on the real desktop by hand:** resizing, the themes, the icon in the launcher and tray, installing the package with pacman.
- **The theme switch back to a light desktop:** tested here only against the dark Breeze colours of this machine.

## M8 — Windows (on hold)
On hold at the maintainer's request (2026-10-30). Nothing for Windows has been built or tested; the list below is the plan for when it resumes.

Needs a Windows 11 x64 machine; nothing here can be built or tested on Linux.
- Build with KDE Craft (Qt 6, KF6, mpv, Rust via Corrosion); fix what doesn't compile. Check CXX-Qt on MSVC (risk R1) and long, non-UTF-8-safe paths (`\\?\` prefix, a test).
- Windows media controls (SMTC) as another class in `app/src/integration/`, beside MPRIS.
- Tray: KStatusNotifierItem falls back to `QSystemTrayIcon`; check it.
- Themes: check that `Theme` finds `share/audium/themes` in Craft's layout.
- Installer (Craft NSIS), built locally, and a Windows step in `scripts/release.sh`.

## 1.0.0 release
- Version 1.0.0 in `CMakeLists.txt`, the Cargo workspace, the PKGBUILD and an AppStream `<release>` entry.
- Commit, tag `v1.0.0`, then the maintainer runs `scripts/release.sh 1.0.0` (checks, source tarball, Arch package, checksums; asks before pushing the tag and creating the GitHub release).

## 1.1.0 — Flatpak and Flathub
Decided by the maintainer: the Flathub submission comes in 1.1.0, not 1.0.0.
- Tools: `flatpak-builder` (`sudo pacman -S flatpak-builder`), the KDE SDK and runtime, and the Rust SDK extension from Flathub.
- Manifest `io.github.SpikeTheDragon40k.Audium.yml` on the KDE runtime (Qt 6, KF6). mpv and the libraries it needs that the runtime lacks are built as modules; Corrosion too. Rust crates vendored for an offline build with Flathub's `flatpak-cargo-generator`.
- Permissions: Wayland/X11, PulseAudio, GPU; the library read-only (`--filesystem=…:ro`; Flathub reviewers may ask why); MPRIS bus name, the tray (`org.kde.StatusNotifierWatcher`), notifications.
- Flathub requirements: a public repository, screenshots at public URLs in the AppStream file, passing `flatpak-builder-lint`. Then a pull request to `flathub/flathub`; a volunteer reviews it (days to weeks).
